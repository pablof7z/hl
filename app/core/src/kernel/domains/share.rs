//! Share-extension intake domain — Phase 5K.
//!
//! ## Design
//!
//! The iOS share extension writes a raw pending-shares JSON file into the App
//! Group container and opens a `highlighter://process-share` URL. The main app
//! dispatches `AppAction::DrainShareQueue` which triggers a
//! `CapabilityRequest::Share(ShareOp::DrainQueue)`. Native reads the file,
//! returns `ShareResult::Pending(items)` via `provide_capability_result`, then
//! deletes the file.
//!
//! The kernel owns all business logic:
//!   - **Dedupe** by `(group_id, url)` pair — same share queued twice is a no-op.
//!   - **Drain partition** — succeeded/failed/requeue logic (ported from
//!     `src/share_extension.rs::share_queue_drain_projection`).
//!   - **Share-target projections** — article/artifact/highlight preview rows
//!     (ported from `src/share_targets.rs`).
//!   - **Share-URL construction** — article naddr and highlight nevent URLs via
//!     injected `SharePolicy` host config (D3: no hardcoded `highlighter.com`
//!     literals in kernel logic).
//!   - **Community handoff snapshot** — JSON the share extension reads at
//!     launch time (ported `communities_snapshot_json`).
//!   - **Publish** — in-group shares via the existing `AppAction::ShareToRoom`
//!     path (`nmp.nip29.share_event_in_group`); kernel is the sole writer on
//!     ported screens.
//!
//! ## Device-local vs nostr
//!
//! The share queue itself is device-local (App Group + transient `AppState`).
//! The dedupe set is transient in-memory (cleared per session). The published
//! kind:11 in-group share event is the only nostr fact produced here — and it
//! reuses the existing Phase-4E `ShareToRoom` path (kernel sole writer, no new
//! publish lane).
//!
//! ## Live-lane coexistence (Non-Negotiable #6)
//!
//! The live `HighlighterCore` lane (`src/share_extension.rs`,
//! `src/share_targets.rs`, `src/share_links.rs`) is UNTOUCHED. This domain
//! duplicates the logic in the kernel lane only; both coexist until the iOS
//! cutover (Phase 6).

use std::collections::HashSet;

use serde::Serialize;

use crate::capabilities::{CapabilityRequest, ShareOp};
use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;

// ─── Share policy (D3: injected, not hardcoded) ───────────────────────────────

/// Share-URL host configuration injected at kernel boot.
///
/// Mirrors `room_policy.invite_link_base` — product hosts are policy data, not
/// literals in kernel logic (D3). The kernel supplies the bech32 entity string;
/// native opens the composed URL.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // wired into KernelPolicy at iOS cutover (Phase 6)
pub struct SharePolicy {
    /// Base URL for article share links (e.g. `"https://highlighter.com/a/"`).
    pub article_share_base_url: String,
    /// Base URL for highlight share links
    /// (e.g. `"https://beta.highlighter.com/highlight/"`).
    pub highlight_share_base_url: String,
}

// ─── Per-item queue state ─────────────────────────────────────────────────────

/// A pending share item in the in-kernel queue.
///
/// Ported from `ShareQueueItem` in the bespoke lane (`src/share_extension.rs:13`).
/// All fields are raw strings (D1: no formatted / decoded values across the
/// kernel boundary; Swift formats display text from these raw fields).
#[derive(Debug, Clone, PartialEq)]
pub struct ShareQueueItem {
    /// Stable item identifier (used for dedupe and retry tracking).
    pub id: String,
    /// NIP-29 local group id to share into.
    pub group_id: String,
    /// URL or text content to share.
    pub url: String,
    /// Optional user note.
    pub note: String,
    /// UNIX second timestamp when the share was queued.
    pub created_at_unix_seconds: f64,
}

/// Result of one publish attempt for a share item.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // used in drain_partition and tests; full use at iOS cutover (Phase 6)
pub struct ShareAttempt {
    pub item: ShareQueueItem,
    pub succeeded: bool,
}

// ─── AppState shard ───────────────────────────────────────────────────────────

/// Transient in-kernel share-queue state.
///
/// This is device-local (NOT a nostr fact). It is cleared when the session
/// ends; the App Group file is the durable handoff store — native drains it
/// on each `AppAction::DrainShareQueue`.
#[derive(Debug, Clone, Default)]
pub struct ShareQueueState {
    /// Items currently in the queue awaiting publish.
    pub pending: Vec<ShareQueueItem>,
    /// Dedupe set: `(group_id, url)` pairs seen this session. Prevents
    /// double-publishing the same URL into the same group.
    pub seen: HashSet<(String, String)>,
    /// Toast message to surface after a successful drain (e.g. "Shared to Readers").
    ///
    /// Written by the Phase 6 iOS cutover when it wires the full drain-publish cycle
    /// through `drain_partition` (which produces the toast string). In Phase 5K
    /// (Rust-only) this field is always `None` in live code — only
    /// `drain_partition` tests exercise the toast logic. The live writer is:
    ///   `state.share_queue.drain_toast = drain_partition(&attempts, &communities).toast;`
    ///   (called after each batch of `AppAction::ShareToRoom` outcomes returns).
    pub drain_toast: Option<String>,
}

// ─── Snapshot ─────────────────────────────────────────────────────────────────

/// Raw share-composer snapshot.
///
/// All fields are raw (D1): Swift formats display labels, community names, etc.
/// The `community_rows` are the same rows from `AppState::communities` projected
/// down to the fields the share composer needs.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ShareComposerRow {
    /// NIP-29 local group id.
    pub group_id: String,
    /// Host relay URL.
    pub host_relay_url: String,
    /// Community display name, if known.
    pub name: Option<String>,
    /// Community picture URL, if known.
    pub picture: Option<String>,
}

/// Snapshot for `ViewId::ShareComposer` — the share-extension intake screen.
///
/// Raw fields only (D1). The kernel emits this after draining the App Group
/// queue so the iOS composer can present the pending share item alongside the
/// community picker.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ShareComposerSnapshot {
    /// The pending share item currently being composed, if any.
    pub pending_url: String,
    pub pending_note: String,
    pub pending_group_id: String,
    /// Available target communities for the picker (raw rows from joined groups).
    pub communities: Vec<ShareComposerRow>,
    /// Non-empty when a drain just completed with at least one success.
    pub toast: Option<String>,
}

// ─── Drain-partition projection ───────────────────────────────────────────────

/// Outcome of partitioning a batch of share attempts.
///
/// Ported from `share_queue_drain_projection` in the bespoke lane
/// (`src/share_extension.rs:59`). All fields are raw (D1).
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // fully used at iOS cutover (Phase 6); tested now
pub struct DrainPartition {
    /// Items that failed and should be re-queued for retry.
    pub requeue: Vec<ShareQueueItem>,
    /// Number of items that published successfully this drain cycle.
    pub success_count: u64,
    /// Toast message to surface (None when success_count == 0).
    pub toast: Option<String>,
}

/// Partition a batch of share attempts into succeeded / failed / requeue.
///
/// Ported from `src/share_extension.rs::share_queue_drain_projection`.
/// Pure function, no I/O.
#[allow(dead_code)] // called at iOS cutover (Phase 6); tested now
pub fn drain_partition(
    attempts: &[ShareAttempt],
    communities: &[crate::kernel::snapshot::CommunityRow],
) -> DrainPartition {
    let mut requeue = Vec::new();
    let mut success_count = 0u64;
    let mut last_success_community: Option<String> = None;

    for attempt in attempts {
        if attempt.succeeded {
            success_count += 1;
            last_success_community = Some(community_label(&attempt.item.group_id, communities));
        } else {
            requeue.push(attempt.item.clone());
        }
    }

    let toast = if success_count == 0 {
        None
    } else if success_count == 1 {
        Some(format!(
            "Shared to {}",
            last_success_community.unwrap_or_else(|| "community".into())
        ))
    } else {
        Some(format!("Shared {success_count} items"))
    };

    DrainPartition {
        requeue,
        success_count,
        toast,
    }
}

#[allow(dead_code)] // called by drain_partition; both used at iOS cutover
fn community_label(
    group_id: &str,
    communities: &[crate::kernel::snapshot::CommunityRow],
) -> String {
    communities
        .iter()
        .find(|c| c.group_id == group_id)
        .and_then(|c| c.name.clone())
        .unwrap_or_else(|| group_id.to_string())
}

// ─── Communities handoff snapshot ─────────────────────────────────────────────

/// Minimal community summary serialised into the App Group handoff file.
///
/// Ported from `SharedCommunitySummary` + `communities_snapshot_json` in the
/// bespoke lane (`src/share_extension.rs:41-57`). The share extension reads
/// this JSON to populate the community picker without loading the full core.
#[derive(Serialize)]
struct HandoffCommunitySummary {
    id: String,
    name: String,
    picture: String,
}

/// Build the JSON bytes for the communities handoff file (`joined-communities-v1.json`).
///
/// Ported from `src/share_extension.rs::communities_snapshot_json`. Returns
/// `b"[]"` on serialization error (defensive, should never fail for valid UTF-8
/// community names).
pub fn communities_snapshot_json(communities: &[crate::kernel::snapshot::CommunityRow]) -> Vec<u8> {
    let rows: Vec<HandoffCommunitySummary> = communities
        .iter()
        .map(|c| HandoffCommunitySummary {
            id: c.group_id.clone(),
            name: c.name.clone().unwrap_or_default(),
            picture: c.picture.clone().unwrap_or_default(),
        })
        .collect();
    serde_json::to_vec(&rows).unwrap_or_else(|_| b"[]".to_vec())
}

// ─── Share-URL construction ───────────────────────────────────────────────────

/// Build an article share URL from an addressable coordinate.
///
/// Ported from `src/share_links.rs::article_share_url`. Uses the injected
/// `SharePolicy::article_share_base_url` instead of a hardcoded literal (D3).
/// Returns `None` when the address is malformed or the policy host is empty.
#[allow(dead_code)] // used when iOS cutover wires the share composer (Phase 6)
pub fn article_share_url(address: &str, _relay_hint: &str, policy: &SharePolicy) -> Option<String> {
    if policy.article_share_base_url.is_empty() {
        return None;
    }
    // Delegate to the existing bespoke library fn which owns the encoding logic.
    // We re-use the stateless encoding from the live lane — no duplicate logic.
    // (The bespoke file is UNTOUCHED; we call it as a pure library here.)
    crate::share_links::article_share_url(address.to_string())
        .ok()
        .map(|url| {
            // Replace the hardcoded base with the injected policy base so the
            // kernel never hard-codes a product URL (D3).
            let bespoke_base = "https://highlighter.com/a/";
            if let Some(tail) = url.strip_prefix(bespoke_base) {
                format!("{}{}", policy.article_share_base_url, tail)
            } else {
                url
            }
        })
}

/// Build a highlight share URL from an event-id hex.
///
/// Ported from `src/share_links.rs::highlight_share_url`. Uses the injected
/// `SharePolicy::highlight_share_base_url` instead of a hardcoded literal (D3).
#[allow(dead_code)] // used when iOS cutover wires the share composer (Phase 6)
pub fn highlight_share_url(
    event_id_hex: &str,
    author_pubkey_hex: Option<&str>,
    policy: &SharePolicy,
) -> Option<String> {
    if policy.highlight_share_base_url.is_empty() {
        return None;
    }
    crate::share_links::highlight_share_url(
        event_id_hex.to_string(),
        author_pubkey_hex.map(str::to_string),
    )
    .ok()
    .map(|url| {
        let bespoke_base = "https://beta.highlighter.com/highlight/";
        if let Some(tail) = url.strip_prefix(bespoke_base) {
            format!("{}{}", policy.highlight_share_base_url, tail)
        } else {
            url
        }
    })
}

// ─── Reducer action arms ──────────────────────────────────────────────────────

/// Emit the capability request to drain the App Group share queue.
///
/// Called from `actor.rs::reduce_action` on `AppAction::DrainShareQueue`.
/// No state mutation — the drain response arrives via
/// `KernelEvent::CapabilityResult(CapabilityResult::Share(ShareResult::Pending(_)))`.
pub fn reduce_action_drain_share_queue() -> Vec<Effect> {
    vec![Effect::EmitCapabilityRequest(CapabilityRequest::Share(
        ShareOp::DrainQueue,
    ))]
}

/// Process incoming raw share payloads from the App Group drain.
///
/// Called from `reduce_event_capability_result` in `domains/session.rs` (or
/// directly from `actor.rs::reduce_event`) on
/// `CapabilityResult::Share(ShareResult::Pending(payloads))`.
///
/// - Deduplicates by `(group_id, url)`.
/// - Appends non-duplicate items to `AppState::share_queue.pending`.
/// - Returns an effect to write the updated communities snapshot if the queue
///   was empty before (first drain in the session) and communities are present.
pub fn reduce_event_share_queue_drained(
    state: &mut AppState,
    payloads: Vec<crate::capabilities::share::RawSharePayload>,
) -> Vec<Effect> {
    let mut effects = Vec::new();
    let mut newly_added: Vec<ShareQueueItem> = Vec::new();
    let queue = &mut state.share_queue;

    for payload in payloads {
        let key = (payload.group_id.clone(), payload.url.clone());
        if queue.seen.contains(&key) {
            // Dedupe: skip items we have already queued or processed this session.
            continue;
        }
        queue.seen.insert(key);
        let item = ShareQueueItem {
            id: payload.id,
            group_id: payload.group_id,
            url: payload.url,
            note: payload.note,
            created_at_unix_seconds: payload.created_at_unix_seconds,
        };
        queue.pending.push(item.clone());
        newly_added.push(item);
    }

    // #21: publish the newly-drained items as kind:11 artifact shares. This is
    // the cutover behavior — the iOS Share Extension's entire purpose is to
    // publish on drain (the bespoke `ShareQueueProcessor.drain` published each
    // item via `publishShareQueueItem`). The kernel is now the sole writer.
    //
    // `pending` is left intact (the ShareComposer snapshot still reads it); the
    // dedupe `seen` set prevents the same `(group_id, url)` from being published
    // twice across drains.
    effects.extend(publish_queue_items(state, &newly_added));

    // Write the communities handoff snapshot so the share extension can
    // populate its picker. We do this on every successful drain so that if
    // the community list changed since the last write, the extension always
    // has fresh data. No-op when communities are empty (extension shows an
    // empty picker — not an error).
    let json_bytes = communities_snapshot_json(&state.communities);
    effects.push(Effect::EmitCapabilityRequest(CapabilityRequest::Share(
        ShareOp::WriteCommunitiesSnapshot { json_bytes },
    )));

    effects
}

/// Process the result of a `WriteCommunitiesSnapshot` capability call.
///
/// Called on `CapabilityResult::Share(ShareResult::CommunitiesWritten)`.
/// Nothing to do in the kernel — write is fire-and-forget; this path is a
/// no-op in the reducer (D6: errors from native are surfaced via `ShareResult::Error`
/// when they matter; CommunitiesWritten is acknowledgement-only).
pub fn reduce_event_communities_written() -> Vec<Effect> {
    vec![]
}

/// Handle a share-capability error result.
///
/// Called on `CapabilityResult::Share(ShareResult::Error(msg))`. Errors are
/// data (D6): currently logged / surfaced in debug builds; no user-visible toast
/// for a failed drain (the share extension will retry on next launch because the
/// file was not deleted on error). Future phases may surface a toast here.
pub fn reduce_event_share_capability_error(_error: String) -> Vec<Effect> {
    // Defensive no-op — error is silently absorbed. The App Group file was not
    // deleted (native only deletes on successful read), so the next drain will
    // retry. No toast: the user initiated the drain implicitly via the share
    // extension, and the next foreground launch retries automatically.
    vec![]
}

/// Update the communities handoff snapshot when the joined-groups list changes.
///
/// Called from the `KernelEvent::JoinedGroupsUpdated` arm in `actor.rs` so
/// that the App Group file always reflects the current community list, even
/// when no share is in flight. Emits `WriteCommunitiesSnapshot` only when at
/// least one community is known.
pub fn on_communities_updated(
    communities: &[crate::kernel::snapshot::CommunityRow],
) -> Vec<Effect> {
    if communities.is_empty() {
        return vec![];
    }
    let json_bytes = communities_snapshot_json(communities);
    vec![Effect::EmitCapabilityRequest(CapabilityRequest::Share(
        ShareOp::WriteCommunitiesSnapshot { json_bytes },
    ))]
}

// ─── Snapshot projection ──────────────────────────────────────────────────────

/// Project the share-composer snapshot for `ViewId::ShareComposer`.
///
/// Returns `None` when the share queue is empty (view should not be open).
pub fn project_share_composer_snapshot(state: &AppState) -> Option<ShareComposerSnapshot> {
    let item = state.share_queue.pending.first()?;

    let communities: Vec<ShareComposerRow> = state
        .communities
        .iter()
        .map(|c| ShareComposerRow {
            group_id: c.group_id.clone(),
            host_relay_url: c.host_relay_url.clone(),
            name: c.name.clone(),
            picture: c.picture.clone(),
        })
        .collect();

    Some(ShareComposerSnapshot {
        pending_url: item.url.clone(),
        pending_note: item.note.clone(),
        pending_group_id: item.group_id.clone(),
        communities,
        toast: state.share_queue.drain_toast.clone(),
    })
}

// ─── Share publish: WRITE port (#21) ──────────────────────────────────────────
//
// The kernel is the sole writer for the bespoke share publishes:
//   - SHARE-TO-ROOM artifact   → kind:11 standalone artifact share (ex `artifacts::publish`)
//   - SHARE-TO-ROOM highlight   → kind:16 generic repost      (ex `highlights::share_to_community`)
//   - SHARE-QUEUE drain item   → kind:11 artifact share       (ex `client::publish_share_queue_item`)
//   - ROOM invite mint          → kind:9009 create-invite      (ex `groups::create_invite_codes`)
//
// The three raw event publishes (artifact / repost / drain) go through
// `Effect::PublishShareEvent` → the VALIDATED `nmp.publish` `PublishRaw` ingress
// (`nmp_app_dispatch_action`), host-pinned to the group's host relay
// (`PublishTarget::Explicit`, D3 fail-closed). The invite mint goes through the
// TYPED `nmp.nip29.create_invite` action via
// `Effect::DispatchCreateInviteWithCorrelation`. Both ingresses are validated by
// the pinned nmp (reserved-kind rejection, fail-closed routing) and thread a
// correlation id so the publish verdict drives `SharePublishState` → Done/Error.
//
// The pure tag builders below reproduce the bespoke event templates
// FIELD-COMPLETELY; the parity tests at the bottom assert each kernel template
// equals the bespoke `EventBuilder` output on the same fixture.

use crate::kernel::models::ArtifactPreview;

/// Kind constants — single source mirrors the bespoke lane.
pub(crate) const KIND_ARTIFACT_SHARE: u32 = 11;
pub(crate) const KIND_GENERIC_REPOST: u32 = 16;
pub(crate) const KIND_HIGHLIGHT: u32 = 9802;

/// Reserved replaceable-list kinds that `PublishAction::PublishRaw` rejects on
/// the pinned nmp (`publish/policy.rs`): kind:0 (profile metadata), kind:3
/// (contacts), kind:10003 (NIP-51 global bookmarks). Each has a dedicated typed
/// builder / per-NIP action module that applies protocol-specific validation +
/// read-modify-write merging; a raw publish would bypass that processing.
///
/// The share publishes route through the SAME validated `nmp.publish` ingress
/// (`run_effect_publish_share_event` → `nmp_app_dispatch_action("nmp.publish",
/// PublishRaw{..})`), so the live nmp enforces this rejection. We mirror the set
/// here as a fail-closed kernel guard so a future builder emitting a reserved
/// kind is rejected at the kernel ingress too — never silently signed (and the
/// reducer is unit-testable without a live NmpApp).
fn is_reserved_publish_kind(kind: u32) -> bool {
    matches!(kind, 0 | 3 | 10003)
}

/// Phase of an in-flight share publish (D6: errors are state, not exceptions).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum SharePublishPhase {
    /// No publish in flight.
    #[default]
    Idle,
    /// A publish was dispatched and is awaiting the action-result verdict.
    Publishing,
    /// The most recent publish succeeded.
    Done,
    /// The most recent publish failed; `message` is the raw error (D1).
    Error { message: String },
}

/// Transient FSM for the active share-to-room / drain publish.
///
/// Device-local: tracks the correlation id of the in-flight publish so the
/// action-result verdict can be matched, plus the phase the iOS sheet renders.
#[derive(Debug, Clone, Default)]
pub struct SharePublishState {
    pub phase: SharePublishPhase,
    /// Correlation id of the publish awaiting a verdict (matches the
    /// action-result row). `None` when no publish is in flight.
    pub pending_correlation_id: Option<String>,
    /// Invite codes minted by the most recent `mint_invite` action. Raw codes
    /// (D1): Swift composes the share link from `RoomPolicy::invite_link_base`
    /// + group_id + code. Empty until an invite is minted.
    pub last_invite_codes: Vec<String>,
}

/// Snapshot for the iOS share sheet — raw fields only (D1).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct SharePublishSnapshot {
    /// `true` while a publish is awaiting its relay verdict.
    pub publishing: bool,
    /// `true` once the most recent publish succeeded.
    pub did_publish: bool,
    /// Raw publish error when the last attempt failed, else `None`. D1: Swift formats.
    pub error_message: Option<String>,
    /// Raw invite codes minted by the most recent `mint_invite`. Swift composes
    /// the share link (D1/D3: no URL literal in kernel logic). Empty otherwise.
    pub invite_codes: Vec<String>,
}

/// Project the share-publish snapshot for the iOS share sheet.
pub fn project_share_publish_snapshot(state: &AppState) -> SharePublishSnapshot {
    let invite_codes = state.share_publish.last_invite_codes.clone();
    match &state.share_publish.phase {
        SharePublishPhase::Idle => SharePublishSnapshot {
            publishing: false,
            did_publish: false,
            error_message: None,
            invite_codes,
        },
        SharePublishPhase::Publishing => SharePublishSnapshot {
            publishing: true,
            did_publish: false,
            error_message: None,
            invite_codes,
        },
        SharePublishPhase::Done => SharePublishSnapshot {
            publishing: false,
            did_publish: true,
            error_message: None,
            invite_codes,
        },
        SharePublishPhase::Error { message } => SharePublishSnapshot {
            publishing: false,
            did_publish: false,
            error_message: Some(message.clone()),
            invite_codes,
        },
    }
}

// ── Pure tag builders (field-complete ports) ──────────────────────────────────

/// Build the kind:11 artifact-share tag list. FIELD-COMPLETE port of
/// `artifacts::build_share_event` (`app/core/src/artifacts.rs`). Order and
/// conditional emission are identical so the parity test asserts byte-for-byte.
pub(crate) fn build_artifact_share_tags(
    group_id: &str,
    preview: &ArtifactPreview,
) -> Vec<Vec<String>> {
    let mut tags: Vec<Vec<String>> = vec![
        vec!["h".into(), group_id.to_string()],
        vec!["d".into(), preview.id.clone()],
        vec!["title".into(), preview.title.clone()],
        vec!["source".into(), preview.source.clone()],
    ];

    match preview.reference_tag_name.as_str() {
        "i" => {
            if !preview.url.is_empty() {
                tags.push(vec![
                    "i".into(),
                    preview.reference_tag_value.clone(),
                    preview.url.clone(),
                ]);
            } else {
                tags.push(vec!["i".into(), preview.reference_tag_value.clone()]);
            }
            if !preview.reference_kind.is_empty() {
                tags.push(vec!["k".into(), preview.reference_kind.clone()]);
            }

            // Secondary feed-level NIP-73 identifier for podcast episodes so
            // discovery-by-show still works. Skip when the primary reference IS
            // the feed-level tag. Mirrors the bespoke builder exactly.
            let ref_is_item = preview
                .reference_tag_value
                .starts_with("podcast:item:guid:");
            let has_feed_guid = !preview.podcast_guid.is_empty();
            let feed_catalog = format!("podcast:guid:{}", preview.podcast_guid);
            let ref_is_feed = preview.reference_tag_value == feed_catalog;
            if ref_is_item && has_feed_guid && !ref_is_feed {
                tags.push(vec!["i".into(), feed_catalog]);
            }
        }
        other if !other.is_empty() => {
            tags.push(vec![other.to_string(), preview.reference_tag_value.clone()]);
        }
        _ => {}
    }

    if !preview.url.is_empty() {
        tags.push(vec!["r".into(), preview.url.clone()]);
    }
    if !preview.author.is_empty() {
        tags.push(vec!["author".into(), preview.author.clone()]);
    }
    if !preview.image.is_empty() {
        tags.push(vec!["image".into(), preview.image.clone()]);
    }
    if !preview.description.is_empty() {
        tags.push(vec!["summary".into(), preview.description.clone()]);
    }
    if !preview.podcast_guid.is_empty() {
        tags.push(vec!["podcast_guid".into(), preview.podcast_guid.clone()]);
    }
    if !preview.podcast_show_title.is_empty() {
        tags.push(vec![
            "podcast_show_title".into(),
            preview.podcast_show_title.clone(),
        ]);
    }
    if !preview.audio_url.is_empty() {
        tags.push(vec!["audio".into(), preview.audio_url.clone()]);
    }
    if !preview.audio_preview_url.is_empty() {
        tags.push(vec![
            "audio_preview".into(),
            preview.audio_preview_url.clone(),
        ]);
    }
    if !preview.transcript_url.is_empty() {
        tags.push(vec!["transcript".into(), preview.transcript_url.clone()]);
    }
    if !preview.feed_url.is_empty() {
        tags.push(vec!["feed".into(), preview.feed_url.clone()]);
    }
    if !preview.published_at.is_empty() {
        tags.push(vec!["published_at".into(), preview.published_at.clone()]);
    }
    if let Some(d) = preview.duration_seconds {
        if d >= 0 {
            tags.push(vec!["duration".into(), d.to_string()]);
        }
    }

    tags
}

/// Build the kind:16 highlight-repost tag list. FIELD-COMPLETE port of
/// `highlights::build_repost_event`. The `e` tag preserves the relay hint as
/// the third element — the field the nmp `repost_in_group` action drops.
pub(crate) fn build_highlight_repost_tags(
    highlight_event_id: &str,
    highlight_author_pubkey: &str,
    target_group_id: &str,
    relay_hint: &str,
) -> Vec<Vec<String>> {
    vec![
        vec![
            "e".into(),
            highlight_event_id.to_string(),
            relay_hint.to_string(),
        ],
        vec!["k".into(), KIND_HIGHLIGHT.to_string()],
        vec!["p".into(), highlight_author_pubkey.to_string()],
        vec!["h".into(), target_group_id.to_string()],
    ]
}

/// Invite-code alphabet — avoids look-alike glyphs (0/O, 1/I/l). Mirrors
/// `INVITE_CODE_ALPHABET` in the bespoke `groups.rs` so codes mint identically.
const INVITE_CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789";

/// Generate one random `length`-char invite code. Port of
/// `groups::generate_invite_code`. Code generation is device-local (not a nostr
/// fact) but the kernel owns it so the relay never sees a code minted by two
/// different code paths.
pub(crate) fn generate_invite_code(length: usize) -> String {
    let n = INVITE_CODE_ALPHABET.len() as u32;
    let mut code = String::with_capacity(length);
    while code.len() < length {
        for byte in uuid::Uuid::new_v4().as_bytes() {
            if code.len() == length {
                break;
            }
            code.push(INVITE_CODE_ALPHABET[(*byte as u32 % n) as usize] as char);
        }
    }
    code
}

/// Serialise a host-pinned publish template `{ kind, content, tags, host_relay_url }`.
pub(crate) fn share_template_json(
    kind: u32,
    content: &str,
    tags: Vec<Vec<String>>,
    host_relay_url: &str,
) -> String {
    serde_json::json!({
        "kind": kind,
        "content": content,
        "tags": tags,
        "host_relay_url": host_relay_url,
    })
    .to_string()
}

/// Build one host-pinned `PublishShareEvent` effect, applying the two fail-closed
/// invariants shared by every share publish:
///
///   1. **D3 host-relay pin must FAIL CLOSED.** An in-group (NIP-29) event MUST
///      land on the group's host relay. An empty/missing `host_relay_url` is a
///      state/routing bug — NOT a request to widen to `Auto`/outbox routing
///      (that would publish a group event to the wrong relays). Empty host →
///      `Error` (D6), no publish.
///   2. **Reserved-kind rejection.** A reserved replaceable-list kind (0/3/10003)
///      is rejected here (mirrors the validated `nmp.publish` PublishRaw ingress)
///      so a future builder emitting one is never silently signed.
///
/// On success: mints a correlation id, sets the FSM to `Publishing`, returns the
/// single `PublishShareEvent` effect. On either guard failure: sets the FSM to
/// `Error` and returns no effect. The verdict (Done/Error) arrives via
/// `apply_action_result_row` → `reduce_event_share_publish_action_result`.
fn emit_share_publish(
    state: &mut AppState,
    kind: u32,
    content: &str,
    tags: Vec<Vec<String>>,
    host_relay_url: &str,
) -> Vec<Effect> {
    if host_relay_url.trim().is_empty() {
        // D3 fail-closed: never widen a group write to Auto/outbox routing.
        state.share_publish.phase = SharePublishPhase::Error {
            message: "host relay required for in-group share (no outbox fallback)".into(),
        };
        return vec![];
    }
    if is_reserved_publish_kind(kind) {
        state.share_publish.phase = SharePublishPhase::Error {
            message: format!("kind:{kind} is reserved and may not be published raw"),
        };
        return vec![];
    }

    let json = share_template_json(kind, content, tags, host_relay_url);
    let correlation_id = crate::kernel::domains::capture_draft::new_correlation_id();
    state.share_publish.phase = SharePublishPhase::Publishing;
    state.share_publish.pending_correlation_id = Some(correlation_id.clone());

    vec![Effect::PublishShareEvent {
        json,
        correlation_id,
    }]
}

fn host_relay_for_group(state: &AppState, group_id: &str) -> String {
    state
        .communities
        .iter()
        .find(|c| c.group_id == group_id)
        .map(|c| c.host_relay_url.trim().to_string())
        .unwrap_or_default()
}

// ── Reducers ──────────────────────────────────────────────────────────────────

/// Reduce `hl.share.artifact_to_room` — publish a kind:11 artifact share into a
/// room. Sets the FSM to `Publishing` and emits one `PublishShareEvent`. The
/// verdict arrives via `KernelEvent::SharePublishActionResult`. Mints its own
/// correlation id (same pattern as `capture_draft`).
pub(crate) fn reduce_action_artifact_to_room(
    state: &mut AppState,
    group_id: String,
    preview: ArtifactPreview,
    note: String,
) -> Vec<Effect> {
    if group_id.trim().is_empty() {
        state.share_publish.phase = SharePublishPhase::Error {
            message: "group_id must not be empty".into(),
        };
        return vec![];
    }
    let host_relay_url = host_relay_for_group(state, &group_id);
    let tags = build_artifact_share_tags(&group_id, &preview);
    let content = note.trim().to_string();
    emit_share_publish(state, KIND_ARTIFACT_SHARE, &content, tags, &host_relay_url)
}

/// Reduce `hl.share.highlight_to_room` — publish a kind:16 highlight repost into
/// a room.
pub(crate) fn reduce_action_highlight_to_room(
    state: &mut AppState,
    target_group_id: String,
    highlight_event_id: String,
    highlight_author_pubkey: String,
    relay_hint: String,
) -> Vec<Effect> {
    if target_group_id.trim().is_empty() {
        state.share_publish.phase = SharePublishPhase::Error {
            message: "target_group_id must not be empty".into(),
        };
        return vec![];
    }
    let host_relay_url = host_relay_for_group(state, &target_group_id);
    let tags = build_highlight_repost_tags(
        &highlight_event_id,
        &highlight_author_pubkey,
        &target_group_id,
        &relay_hint,
    );
    emit_share_publish(state, KIND_GENERIC_REPOST, "", tags, &host_relay_url)
}

/// Reduce `hl.share.mint_invite` — generate `count` invite codes and publish
/// kind:9009 create-invite events via the EXISTING kernel `nmp.nip29.create_invite`
/// path (the field-complete nmp `CreateInviteAction`, same path
/// `room_home::reduce_action_create_room_invites` uses — no second kind:9009
/// writer). Code generation (device-local, not a nostr fact) is owned by the
/// kernel so the relay never sees a code minted by two different code paths.
///
/// The generated codes are stored in `share_publish.last_invite_codes` so the
/// iOS `RoomShareCard` can compose the share link from raw group_id + code
/// (D1/D3: the kernel never hard-codes the `highlighter.com/r/.../join/...` URL).
///
/// Replaces the bespoke `groups::create_invite_codes` (code-gen + publish) +
/// `client::get_room_share_link_snapshot`. Fans out to multiple kind:9009 events
/// internally inside `CreateInviteAction` when `count` exceeds the relay cap.
///
/// Finding 3: the FSM is set to `Publishing` and a correlation id is threaded
/// through the create-invite dispatch so its publish verdict drives
/// `SharePublishState` → Done (success) / Error (failure). The mint is NOT marked
/// Done until the kind:9009 publish actually succeeds.
pub(crate) fn reduce_action_mint_invite(
    state: &mut AppState,
    group_id: String,
    count: u32,
) -> Vec<Effect> {
    if group_id.trim().is_empty() {
        state.share_publish.phase = SharePublishPhase::Error {
            message: "group_id must not be empty".into(),
        };
        return vec![];
    }
    let host_relay_url = host_relay_for_group(state, &group_id);
    // D3 fail-closed: a kind:9009 create-invite is a NIP-29 group write. The
    // host relay must resolve from Rust-owned community state; missing state is
    // not a request to widen to Auto/outbox routing. Error (D6), no publish.
    if host_relay_url.trim().is_empty() {
        state.share_publish.phase = SharePublishPhase::Error {
            message: "host relay required for invite mint (no outbox fallback)".into(),
        };
        return vec![];
    }
    let count = count.clamp(1, 100) as usize;
    let codes: Vec<String> = (0..count).map(|_| generate_invite_code(24)).collect();

    // Store raw codes for the snapshot (Swift composes the link). The codes are
    // valid only once relay29 ACCEPTS the kind:9009 — so the FSM stays Publishing
    // until the create-invite publish verdict arrives (finding 3: do NOT mark
    // Done before nmp.nip29.create_invite can fail).
    state.share_publish.last_invite_codes = codes.clone();

    // Reuse the existing field-complete `nmp.nip29.create_invite` path — ONE
    // kind:9009 writer in the kernel — but thread a correlation id so the publish
    // verdict drives the share-mint FSM. The `nmp.nip29.create_invite` payload is
    // identical to the fire-and-forget `reduce_action_create_room_invites`; we
    // build it inline so we can attach the correlation id (the room_home path
    // stays fire-and-forget for `hl.room.create_invites`).
    let json = serde_json::json!({
        "group": {
            "host_relay_url": host_relay_url,
            "local_id": group_id
        },
        "codes": codes
    })
    .to_string();

    let correlation_id = crate::kernel::domains::capture_draft::new_correlation_id();
    state.share_publish.phase = SharePublishPhase::Publishing;
    state.share_publish.pending_correlation_id = Some(correlation_id.clone());

    vec![Effect::DispatchCreateInviteWithCorrelation {
        json,
        correlation_id,
    }]
}

/// Publish the given share-queue `items` as host-pinned kind:11 artifact shares.
///
/// FIELD-COMPLETE port of `client::publish_share_queue_item` (the iOS Share
/// Extension drain): each item's URL is run through the pure `build_preview`
/// (URL normalization + NIP-73 reference resolution — no I/O), then the same
/// `build_artifact_share_tags` path as `artifact_to_room`. The host relay is
/// looked up from `state.communities` by `group_id`.
///
/// Each item gets its own publish effect + correlation id; only the last item's
/// id drives the FSM verdict. Items whose URL fails to build a preview are
/// skipped (D6 — logged, not fatal). Does NOT mutate the queue — the dedupe
/// `seen` set prevents double-publishing across drains.
fn publish_queue_items(state: &mut AppState, items: &[ShareQueueItem]) -> Vec<Effect> {
    if items.is_empty() {
        return vec![];
    }

    // Pre-build (host_relay_url, tags, content) per item; skip items whose URL
    // fails to build a preview (D6 — logged, not fatal).
    let mut prepared: Vec<(String, Vec<Vec<String>>, String)> = Vec::new();
    for item in items {
        let preview = match crate::artifacts::build_preview(&item.url) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    url = %item.url,
                    "publish_queue_items: build_preview failed: {e} — skipping item (D6)"
                );
                continue;
            }
        };
        let host_relay_url = host_relay_for_group(state, &item.group_id);
        // D3 fail-closed: a drained item whose group has no known host relay
        // must NOT publish (never widen a NIP-29 group write to Auto/outbox).
        // `emit_share_publish` sets the FSM to Error and returns no effect.
        if host_relay_url.trim().is_empty() {
            tracing::warn!(
                group_id = %item.group_id,
                "publish_queue_items: no host relay for group — skipping publish (D3 fail-closed)"
            );
        }
        let tags = build_artifact_share_tags(&item.group_id, &preview);
        let content = item.note.trim().to_string();
        prepared.push((host_relay_url, tags, content));
    }

    let mut effects = Vec::new();
    for (host_relay_url, tags, content) in prepared {
        // The last item that produces an effect drives the FSM verdict; an empty
        // host relay yields an Error phase + no effect (D3/D6).
        effects.extend(emit_share_publish(
            state,
            KIND_ARTIFACT_SHARE,
            &content,
            tags,
            &host_relay_url,
        ));
    }

    effects
}

/// Reduce `KernelEvent::SharePublishActionResult` — route a publish verdict to
/// the FSM (D6: errors are state). Ignores rows whose correlation id does not
/// match the in-flight publish (stale / fan-out sibling).
pub(crate) fn reduce_event_share_publish_action_result(
    state: &mut AppState,
    correlation_id: String,
    success: bool,
    error: String,
) -> Vec<Effect> {
    if state.share_publish.pending_correlation_id.as_deref() != Some(correlation_id.as_str()) {
        return vec![];
    }
    state.share_publish.pending_correlation_id = None;
    state.share_publish.phase = if success {
        SharePublishPhase::Done
    } else {
        SharePublishPhase::Error {
            message: if error.trim().is_empty() {
                "publish failed".into()
            } else {
                error
            },
        }
    };
    vec![]
}

/// Reduce `KernelEvent::SharePublishCorrelationMinted` — swap the reducer-minted
/// placeholder in `share_publish.pending_correlation_id` for the real nmp-minted
/// id so `apply_action_result_row` can match the arriving create-invite
/// `action_results` row (#21 finding 3).
///
/// Guarded against a reset race: only swaps when the placeholder is still the
/// in-flight id (a `reset_publish` between dispatch and the minted event cleared
/// it; the verdict then arrives as an unknown-correlation no-op, D6).
pub(crate) fn reduce_event_share_publish_correlation_minted(
    state: &mut AppState,
    placeholder_correlation_id: String,
    nmp_correlation_id: String,
) -> Vec<Effect> {
    if state.share_publish.pending_correlation_id.as_deref()
        == Some(placeholder_correlation_id.as_str())
    {
        state.share_publish.pending_correlation_id = Some(nmp_correlation_id);
    } else {
        tracing::debug!(
            placeholder = %placeholder_correlation_id,
            nmp_cid = %nmp_correlation_id,
            "reduce_event_share_publish_correlation_minted: placeholder not in flight (reset race?) — dropping"
        );
    }
    vec![]
}

/// Reduce `hl.share.reset_publish` — clear a terminal publish state when the
/// iOS sheet is dismissed or re-opened.
pub(crate) fn reduce_action_reset_share_publish(state: &mut AppState) -> Vec<Effect> {
    state.share_publish = SharePublishState::default();
    vec![]
}

// ── Effect runner ─────────────────────────────────────────────────────────────

/// Execute `Effect::PublishShareEvent` — sign-and-publish a host-pinned event
/// through the **VALIDATED** `nmp.publish` ingress (`PublishAction::PublishRaw`
/// via `nmp_app_dispatch_action`), NOT the raw `ActorCommand::PublishRawEvent`
/// door directly (finding 1). Routing through `nmp.publish` means the pinned nmp
/// `PublishModule::start` validation applies to this ingress too: reserved
/// replaceable-list kinds (0/3/10003) are rejected, and an empty `Explicit`
/// relay set is fail-closed (it never silently widens to `Auto`).
///
/// The `host_relay_url` is lowered to `PublishTarget::Explicit` so the event
/// lands on the group's host relay (NIP-29, D3 — fail-closed). The kernel
/// reducer (`emit_share_publish`) already rejects an empty host relay before this
/// runner is reached, so `host_relay_url` is non-empty here.
///
/// nmp mints its own correlation id for a `PublishRaw` dispatch (the event id is
/// not known until it signs). The runner parses it and, if it differs from the
/// reducer placeholder, emits `KernelEvent::SharePublishCorrelationMinted` to
/// swap it into `share_publish.pending_correlation_id` so
/// `apply_action_result_row` can match the publish verdict. A dispatch rejection
/// (`{"error":..}`, e.g. a reserved kind) drives the FSM → Error directly (D6).
///
/// No-op when nmp is `None` (test mode inspects the emitted `Effect` directly).
pub(crate) fn run_effect_publish_share_event(
    json: String,
    correlation_id: String,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
    tx: &tokio::sync::mpsc::UnboundedSender<crate::kernel::actor::Cmd>,
) {
    let Some(handle) = nmp else {
        tracing::debug!("PublishShareEvent: no live NmpApp (test mode) — no-op");
        return;
    };

    #[derive(serde::Deserialize)]
    struct ShareTemplate {
        kind: u32,
        content: String,
        tags: Vec<Vec<String>>,
        host_relay_url: String,
    }

    let template = match serde_json::from_str::<ShareTemplate>(&json) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("PublishShareEvent: failed to deserialize template: {e} — no-op (D6)");
            return;
        }
    };

    // Build the validated `nmp.publish` PublishRaw payload. Host-pin to the
    // group's relay via an Explicit target (D3 fail-closed — empty host already
    // rejected in the reducer). nmp fills id/sig/pubkey/created_at.
    let action_json = serde_json::json!({
        "PublishRaw": {
            "kind": template.kind,
            "tags": template.tags,
            "content": template.content,
            "target": { "Explicit": { "relays": [template.host_relay_url] } },
        }
    })
    .to_string();

    dispatch_share_publish_action(handle, "nmp.publish", action_json, correlation_id, tx);
}

/// Execute `Effect::PublishClipRepostEvent` — same validated `nmp.publish`
/// host-pinned path as `PublishShareEvent`, but routed back to the podcast clip
/// FSM instead of the global share FSM.
pub(crate) fn run_effect_publish_clip_repost_event(
    json: String,
    correlation_id: String,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
    tx: &tokio::sync::mpsc::UnboundedSender<crate::kernel::actor::Cmd>,
) {
    let Some(handle) = nmp else {
        tracing::debug!("PublishClipRepostEvent: no live NmpApp (test mode) — no-op");
        return;
    };

    let Some(action_json) = share_publish_action_json(&json, "PublishClipRepostEvent") else {
        return;
    };
    dispatch_clip_repost_publish_action(handle, "nmp.publish", action_json, correlation_id, tx);
}

/// Execute `Effect::DispatchCreateInviteWithCorrelation` — dispatch the validated
/// `nmp.nip29.create_invite` (kind:9009) action with a correlation id so the
/// create-invite publish verdict drives the share-mint FSM (finding 3). Shares
/// the same dispatch + correlation-swap + reject-to-Error plumbing as the share
/// publish (`dispatch_share_publish_action`). No-op when nmp is `None`.
pub(crate) fn run_effect_dispatch_create_invite_with_correlation(
    json: String,
    correlation_id: String,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
    tx: &tokio::sync::mpsc::UnboundedSender<crate::kernel::actor::Cmd>,
) {
    let Some(handle) = nmp else {
        tracing::debug!("DispatchCreateInviteWithCorrelation: no live NmpApp (test mode) — no-op");
        return;
    };
    dispatch_share_publish_action(handle, "nmp.nip29.create_invite", json, correlation_id, tx);
}

/// Shared dispatch plumbing for the correlation-aware share publishes (artifact /
/// repost / drain via `nmp.publish`; invite mint via `nmp.nip29.create_invite`).
///
/// Calls `nmp_app_dispatch_action(namespace, action_json)`, then:
///   - `{"correlation_id":"<id>"}` matching the placeholder → no swap needed.
///   - `{"correlation_id":"<id>"}` differing → emit
///     `SharePublishCorrelationMinted` so the FSM tracks the real id.
///   - `{"error":".."}` (or null / no correlation_id) → the dispatch was
///     REJECTED (e.g. a reserved kind, fail-closed validation): drive the FSM →
///     Error via `SharePublishActionResult`-equivalent routing (D6). This is the
///     mechanism that surfaces reserved-kind rejection on this ingress.
fn dispatch_share_publish_action(
    handle: &crate::kernel::actor::NmpHandle,
    namespace: &str,
    action_json: String,
    placeholder_correlation_id: String,
    tx: &tokio::sync::mpsc::UnboundedSender<crate::kernel::actor::Cmd>,
) {
    use crate::kernel::action::KernelEvent;
    use crate::kernel::actor::Cmd;

    let reject = |error: String| {
        let _ = tx.send(Cmd::Event(KernelEvent::ShareMintDispatchRejected {
            correlation_id: placeholder_correlation_id.clone(),
            error,
        }));
    };

    let dispatch_result = crate::kernel::domains::dispatch_bytes::dispatch_action_bytes_for(
        &handle.app,
        namespace,
        &action_json,
    );

    match dispatch_result {
        Err(e) => {
            tracing::warn!(
                error = %e,
                "dispatch_share_publish_action: nmp rejected dispatch — FSM → Error"
            );
            reject(e);
        }
        Ok(nmp_id) if nmp_id != placeholder_correlation_id => {
            let _ = tx.send(Cmd::Event(KernelEvent::SharePublishCorrelationMinted {
                placeholder_correlation_id,
                nmp_correlation_id: nmp_id,
            }));
        }
        Ok(_) => {
            // Same id — action_results will match the placeholder directly.
        }
    }
}

fn share_publish_action_json(json: &str, label: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct ShareTemplate {
        kind: u32,
        content: String,
        tags: Vec<Vec<String>>,
        host_relay_url: String,
    }

    let template = match serde_json::from_str::<ShareTemplate>(json) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("{label}: failed to deserialize template: {e} — no-op (D6)");
            return None;
        }
    };

    Some(
        serde_json::json!({
            "PublishRaw": {
                "kind": template.kind,
                "tags": template.tags,
                "content": template.content,
                "target": { "Explicit": { "relays": [template.host_relay_url] } },
            }
        })
        .to_string(),
    )
}

fn dispatch_clip_repost_publish_action(
    handle: &crate::kernel::actor::NmpHandle,
    namespace: &str,
    action_json: String,
    placeholder_correlation_id: String,
    tx: &tokio::sync::mpsc::UnboundedSender<crate::kernel::actor::Cmd>,
) {
    use crate::kernel::action::KernelEvent;
    use crate::kernel::actor::Cmd;

    let reject = |error: String| {
        let _ = tx.send(Cmd::Event(KernelEvent::ClipRepostDispatchRejected {
            correlation_id: placeholder_correlation_id.clone(),
            error,
        }));
    };

    let dispatch_result = crate::kernel::domains::dispatch_bytes::dispatch_action_bytes_for(
        &handle.app,
        namespace,
        &action_json,
    );

    match dispatch_result {
        Err(e) => reject(e),
        Ok(nmp_id) if nmp_id != placeholder_correlation_id => {
            let _ = tx.send(Cmd::Event(KernelEvent::ClipRepostCorrelationMinted {
                placeholder_correlation_id,
                nmp_correlation_id: nmp_id,
            }));
        }
        Ok(_) => {}
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::share::RawSharePayload;
    use crate::kernel::app::AppState;
    use crate::kernel::snapshot::CommunityRow;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn raw_payload(id: &str, group_id: &str, url: &str) -> RawSharePayload {
        RawSharePayload {
            id: id.into(),
            group_id: group_id.into(),
            url: url.into(),
            note: String::new(),
            created_at_unix_seconds: 1_000.0,
        }
    }

    fn community_row(group_id: &str, name: &str) -> CommunityRow {
        CommunityRow {
            group_id: group_id.into(),
            host_relay_url: "wss://relay.example.com".into(),
            name: Some(name.into()),
            picture: None,
            about: None,
            member_count: 0,
            public: true,
            open: true,
            is_admin: false,
        }
    }

    // ── 5K: share_intake_parses_url_payload ──────────────────────────────────

    #[test]
    fn share_intake_parses_url_payload() {
        let mut state = AppState::default();
        let payload = raw_payload("item-1", "group-a", "https://example.com/article");

        reduce_event_share_queue_drained(&mut state, vec![payload]);

        assert_eq!(state.share_queue.pending.len(), 1);
        assert_eq!(
            state.share_queue.pending[0].url,
            "https://example.com/article"
        );
        assert_eq!(state.share_queue.pending[0].group_id, "group-a");
        assert_eq!(state.share_queue.pending[0].id, "item-1");
    }

    // ── 5K: share_intake_dedupes ─────────────────────────────────────────────

    #[test]
    fn share_intake_dedupes() {
        let mut state = AppState::default();
        let p1 = raw_payload("item-1", "group-a", "https://example.com/article");
        let p2 = raw_payload("item-2", "group-a", "https://example.com/article"); // same (group_id, url)
        let p3 = raw_payload("item-3", "group-b", "https://example.com/article"); // different group

        reduce_event_share_queue_drained(&mut state, vec![p1, p2, p3]);

        // item-2 is deduplicated; item-1 and item-3 are distinct
        assert_eq!(state.share_queue.pending.len(), 2);
        assert_eq!(state.share_queue.pending[0].id, "item-1");
        assert_eq!(state.share_queue.pending[1].id, "item-3");

        // A second drain with a duplicate must still be a no-op
        let p4 = raw_payload("item-4", "group-a", "https://example.com/article");
        reduce_event_share_queue_drained(&mut state, vec![p4]);
        assert_eq!(state.share_queue.pending.len(), 2);
    }

    // ── 5K: target_community_selection ───────────────────────────────────────

    #[test]
    fn target_community_selection() {
        let communities = vec![
            community_row("group-a", "Readers"),
            community_row("group-b", "Writers"),
        ];

        let attempts = vec![
            ShareAttempt {
                item: ShareQueueItem {
                    id: "1".into(),
                    group_id: "group-a".into(),
                    url: "https://example.com".into(),
                    note: String::new(),
                    created_at_unix_seconds: 1.0,
                },
                succeeded: true,
            },
            ShareAttempt {
                item: ShareQueueItem {
                    id: "2".into(),
                    group_id: "group-b".into(),
                    url: "https://other.com".into(),
                    note: String::new(),
                    created_at_unix_seconds: 2.0,
                },
                succeeded: false,
            },
        ];

        let partition = drain_partition(&attempts, &communities);

        assert_eq!(partition.success_count, 1);
        assert_eq!(partition.toast.as_deref(), Some("Shared to Readers"));
        assert_eq!(partition.requeue.len(), 1);
        assert_eq!(partition.requeue[0].group_id, "group-b");
    }

    // ── #21 cutover: drain auto-publishes (was 5K device-local-only) ──────────

    #[test]
    fn share_queue_drain_auto_publishes_kind11() {
        // #21 cutover: the iOS Share Extension's purpose is to publish on drain
        // (the bespoke `ShareQueueProcessor.drain` published each item). Draining
        // a valid item now emits exactly one host-pinned PublishShareEvent
        // (kind:11 artifact share) AND the communities-snapshot capability.
        let mut state = AppState::default();
        // Seed the group's host relay so the drain publish is host-pinned (D3
        // fail-closed: a group with no known host relay would NOT publish).
        state.communities = vec![community_row("group-a", "Readers")];
        let payload = raw_payload("item-1", "group-a", "https://example.com");

        let effects = reduce_event_share_queue_drained(&mut state, vec![payload]);

        let publish_count = effects
            .iter()
            .filter(|e| matches!(e, crate::kernel::effect::Effect::PublishShareEvent { .. }))
            .count();
        assert_eq!(
            publish_count, 1,
            "drain must auto-publish one kind:11 share"
        );
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, crate::kernel::effect::Effect::EmitCapabilityRequest(_))),
            "drain still writes the communities handoff snapshot"
        );
        // The pending queue is retained for the ShareComposer snapshot; dedupe
        // prevents a re-publish on the next drain.
        assert_eq!(state.share_queue.pending.len(), 1);
        assert_eq!(state.share_publish.phase, SharePublishPhase::Publishing);

        // A second drain of the same (group_id, url) is deduped → no re-publish.
        let again = reduce_event_share_queue_drained(
            &mut state,
            vec![raw_payload("item-2", "group-a", "https://example.com")],
        );
        let republish = again
            .iter()
            .filter(|e| matches!(e, crate::kernel::effect::Effect::PublishShareEvent { .. }))
            .count();
        assert_eq!(republish, 0, "dedupe guard must bite — no double-publish");
    }

    // ── 5K: malformed_payload_no_op ──────────────────────────────────────────

    #[test]
    fn malformed_payload_no_op() {
        // Payloads with empty id or url should still be accepted (the kernel
        // does not validate the URL — that happens at publish time). Payloads
        // with empty group_id are accepted; the publish action will fail gracefully
        // (D6). The important invariant is that no panic occurs.
        let mut state = AppState::default();
        let empty_url = raw_payload("", "", "");
        // Should not panic; empty items enter the queue (dedupe key is ("",""))
        reduce_event_share_queue_drained(&mut state, vec![empty_url]);
        assert_eq!(state.share_queue.pending.len(), 1);

        // A second empty payload is deduped
        let empty_url2 = raw_payload("", "", "");
        reduce_event_share_queue_drained(&mut state, vec![empty_url2]);
        assert_eq!(state.share_queue.pending.len(), 1);
    }

    // ── 5K: share_composer_snapshot_raw ──────────────────────────────────────

    #[test]
    fn share_composer_snapshot_raw() {
        let mut state = AppState::default();
        state.communities = vec![community_row("group-a", "Readers")];

        // No pending item → no snapshot
        assert!(project_share_composer_snapshot(&state).is_none());

        // After a drain, snapshot is present with raw fields
        let payload = raw_payload("item-1", "group-a", "https://example.com/article");
        reduce_event_share_queue_drained(&mut state, vec![payload]);

        let snapshot = project_share_composer_snapshot(&state).expect("snapshot");
        assert_eq!(snapshot.pending_url, "https://example.com/article");
        assert_eq!(snapshot.pending_group_id, "group-a");
        assert_eq!(snapshot.communities.len(), 1);
        assert_eq!(snapshot.communities[0].group_id, "group-a");
        assert_eq!(snapshot.communities[0].name.as_deref(), Some("Readers"));
        assert!(snapshot.toast.is_none());
    }

    // ── communities_snapshot_json (ported from bespoke lane) ─────────────────

    #[test]
    fn communities_snapshot_json_matches_extension_schema() {
        let communities = vec![community_row("group-a", "Readers")];
        // Override picture via a fresh row so we can test the picture field
        let communities = vec![CommunityRow {
            picture: Some("https://example.com/a.jpg".into()),
            ..communities.into_iter().next().unwrap()
        }];

        let json = communities_snapshot_json(&communities);

        assert_eq!(
            String::from_utf8(json).unwrap(),
            r#"[{"id":"group-a","name":"Readers","picture":"https://example.com/a.jpg"}]"#
        );
    }

    #[test]
    fn communities_snapshot_json_empty_is_array() {
        let json = communities_snapshot_json(&[]);
        assert_eq!(String::from_utf8(json).unwrap(), "[]");
    }

    // ── drain_partition (ported from bespoke lane) ────────────────────────────

    #[test]
    fn drain_partition_requeues_failures_and_names_single_success() {
        let communities = vec![community_row("group-a", "Readers")];
        let attempts = vec![
            ShareAttempt {
                item: ShareQueueItem {
                    id: "ok".into(),
                    group_id: "group-a".into(),
                    url: "https://example.com".into(),
                    note: String::new(),
                    created_at_unix_seconds: 1.0,
                },
                succeeded: true,
            },
            ShareAttempt {
                item: ShareQueueItem {
                    id: "retry".into(),
                    group_id: "group-b".into(),
                    url: "https://other.com".into(),
                    note: String::new(),
                    created_at_unix_seconds: 2.0,
                },
                succeeded: false,
            },
        ];

        let partition = drain_partition(&attempts, &communities);
        assert_eq!(partition.success_count, 1);
        assert_eq!(partition.toast.as_deref(), Some("Shared to Readers"));
        assert_eq!(partition.requeue.len(), 1);
        assert_eq!(partition.requeue[0].id, "retry");
    }

    #[test]
    fn drain_partition_counts_multiple_successes() {
        let attempts = vec![
            ShareAttempt {
                item: ShareQueueItem {
                    id: "one".into(),
                    group_id: "group-a".into(),
                    url: "https://example.com".into(),
                    note: String::new(),
                    created_at_unix_seconds: 1.0,
                },
                succeeded: true,
            },
            ShareAttempt {
                item: ShareQueueItem {
                    id: "two".into(),
                    group_id: "group-b".into(),
                    url: "https://other.com".into(),
                    note: String::new(),
                    created_at_unix_seconds: 2.0,
                },
                succeeded: true,
            },
        ];

        let partition = drain_partition(&attempts, &[]);
        assert_eq!(partition.success_count, 2);
        assert_eq!(partition.toast.as_deref(), Some("Shared 2 items"));
        assert!(partition.requeue.is_empty());
    }

    #[test]
    fn drain_partition_zero_successes_no_toast() {
        let attempts = vec![ShareAttempt {
            item: ShareQueueItem {
                id: "fail".into(),
                group_id: "group-a".into(),
                url: "https://example.com".into(),
                note: String::new(),
                created_at_unix_seconds: 1.0,
            },
            succeeded: false,
        }];

        let partition = drain_partition(&attempts, &[]);
        assert_eq!(partition.success_count, 0);
        assert!(partition.toast.is_none());
        assert_eq!(partition.requeue.len(), 1);
    }

    // ── #21 share-publish parity + reducer tests ─────────────────────────────

    use nostr_sdk::prelude::{EventBuilder, Keys};

    /// Extract a signed event's tags as `Vec<Vec<String>>` for parity comparison.
    fn event_tags(builder: EventBuilder) -> Vec<Vec<String>> {
        let keys = Keys::generate();
        let event = builder.sign_with_keys(&keys).expect("sign");
        event.tags.iter().map(|t| t.as_slice().to_vec()).collect()
    }

    fn full_preview() -> ArtifactPreview {
        // A podcast-episode artifact exercises the richest tag path: i/k +
        // secondary feed-level i + r/author/image/summary + all podcast fields.
        ArtifactPreview {
            id: "artifact-d".into(),
            url: "https://pod.example.com/ep/42".into(),
            title: "Episode 42".into(),
            author: "Jane Host".into(),
            image: "https://pod.example.com/cover.jpg".into(),
            description: "A great episode".into(),
            source: "podcast".into(),
            domain: "pod.example.com".into(),
            catalog_id: String::new(),
            catalog_kind: String::new(),
            podcast_guid: "feed-guid-1".into(),
            podcast_item_guid: "ep-guid-42".into(),
            podcast_show_title: "The Show".into(),
            audio_url: "https://pod.example.com/ep/42.mp3".into(),
            audio_preview_url: "https://pod.example.com/ep/42-preview.mp3".into(),
            transcript_url: "https://pod.example.com/ep/42.vtt".into(),
            feed_url: "https://pod.example.com/feed.xml".into(),
            published_at: "1700000000".into(),
            duration_seconds: Some(3600),
            reference_tag_name: "i".into(),
            reference_tag_value: "podcast:item:guid:ep-guid-42".into(),
            reference_kind: "podcast:item".into(),
            highlight_tag_name: "i".into(),
            highlight_tag_value: "podcast:item:guid:ep-guid-42".into(),
            highlight_reference_key: String::new(),
            chapters: Vec::new(),
        }
    }

    /// GOTCHA #7b: SHARE-TO-ROOM artifact — kernel kind:11 tag builder is
    /// FIELD-COMPLETE vs the bespoke `artifacts::build_share_event` on the SAME
    /// fixture. Asserts kind, content, and ALL tags in order.
    #[test]
    fn parity_artifact_share_matches_bespoke_builder() {
        let preview = full_preview();
        let group_id = "room-x";
        let note = Some("look at this");

        // Bespoke: build → sign → extract.
        let bespoke_builder =
            crate::artifacts::build_share_event(group_id, &preview, note).expect("bespoke builder");
        let bespoke_tags = event_tags(bespoke_builder);

        // Kernel: the pure tag builder + content normalization.
        let kernel_tags = build_artifact_share_tags(group_id, &preview);

        assert_eq!(
            kernel_tags, bespoke_tags,
            "kernel artifact-share tags must equal bespoke build_share_event tags"
        );

        // Verify the kernel produces the full rich tag set (no lossy cut): the
        // primary i, the secondary feed-level i, k, r, author, image, summary,
        // all podcast fields, duration.
        let has = |name: &str| {
            kernel_tags
                .iter()
                .any(|t| t.first().map(String::as_str) == Some(name))
        };
        for name in [
            "h",
            "d",
            "title",
            "source",
            "i",
            "k",
            "r",
            "author",
            "image",
            "summary",
            "podcast_guid",
            "podcast_show_title",
            "audio",
            "audio_preview",
            "transcript",
            "feed",
            "published_at",
            "duration",
        ] {
            assert!(has(name), "kernel artifact share missing `{name}` tag");
        }
        // Secondary feed-level i tag is present (episode discovery-by-show).
        assert!(
            kernel_tags
                .iter()
                .any(|t| t == &vec!["i".to_string(), "podcast:guid:feed-guid-1".to_string()]),
            "kernel artifact share missing secondary feed-level i tag"
        );
    }

    /// Parity for a non-podcast `a`-reference article preview (different branch
    /// of the reference match).
    #[test]
    fn parity_artifact_share_article_reference_branch() {
        let mut preview = full_preview();
        preview.source = "article".into();
        preview.podcast_guid = String::new();
        preview.podcast_item_guid = String::new();
        preview.podcast_show_title = String::new();
        preview.audio_url = String::new();
        preview.audio_preview_url = String::new();
        preview.transcript_url = String::new();
        preview.feed_url = String::new();
        preview.duration_seconds = None;
        preview.reference_tag_name = "a".into();
        preview.reference_tag_value = "30023:author:d-tag".into();
        preview.reference_kind = String::new();

        let bespoke_tags =
            event_tags(crate::artifacts::build_share_event("room-y", &preview, None).unwrap());
        let kernel_tags = build_artifact_share_tags("room-y", &preview);
        assert_eq!(kernel_tags, bespoke_tags);
        // The `a` reference is emitted as a single ["a", value] tag.
        assert!(kernel_tags
            .iter()
            .any(|t| t == &vec!["a".to_string(), "30023:author:d-tag".to_string()]));
    }

    /// GOTCHA #7b: SHARE-TO-ROOM highlight — kernel kind:16 repost tag builder is
    /// FIELD-COMPLETE vs the bespoke `highlights::build_repost_event`, including
    /// the `e`-tag RELAY HINT (the field the nmp `repost_in_group` action drops).
    #[test]
    fn parity_highlight_repost_matches_bespoke_builder() {
        use nostr_sdk::prelude::{EventId, PublicKey};

        let highlight_id = EventId::all_zeros();
        let author = Keys::generate().public_key();
        let author_hex = author.to_hex();
        let group = "room-z";
        let relay_hint = "wss://relay.highlighter.com";

        let bespoke_tags = event_tags(
            crate::highlights::build_repost_event(highlight_id, &author_hex, group, relay_hint)
                .expect("bespoke repost builder"),
        );
        let kernel_tags =
            build_highlight_repost_tags(&highlight_id.to_hex(), &author_hex, group, relay_hint);

        assert_eq!(
            kernel_tags, bespoke_tags,
            "kernel repost tags must equal bespoke build_repost_event tags (incl. relay hint)"
        );
        // The relay hint is the third element of the e tag — prove it survives.
        let e_tag = kernel_tags
            .iter()
            .find(|t| t.first().map(String::as_str) == Some("e"))
            .expect("e tag present");
        assert_eq!(e_tag.get(2).map(String::as_str), Some(relay_hint));
        // k = 9802, p = author, h = group.
        assert!(kernel_tags
            .iter()
            .any(|t| t == &vec!["k".to_string(), "9802".to_string()]));
        assert!(kernel_tags
            .iter()
            .any(|t| t == &vec!["p".to_string(), author_hex.clone()]));
        assert!(kernel_tags
            .iter()
            .any(|t| t == &vec!["h".to_string(), group.to_string()]));
        // PublicKey round-trips (sanity on the fixture).
        assert!(PublicKey::from_hex(&author_hex).is_ok());
    }

    /// GOTCHA #7c: SHARE-QUEUE drain — the kernel drain reducer publishes a
    /// kind:11 artifact share whose tags equal the bespoke per-item publish
    /// (`build_preview` → `build_share_event`) on the SAME queued URL.
    #[test]
    fn parity_drain_queue_publish_matches_bespoke_per_item() {
        let url = "https://example.com/post";
        let group_id = "group-a";

        // Bespoke per-item path: build_preview(url) → build_share_event.
        let preview = crate::artifacts::build_preview(url).expect("preview");
        let bespoke_tags =
            event_tags(crate::artifacts::build_share_event(group_id, &preview, None).unwrap());

        // Kernel auto-drain: seed a community host relay, drain the App Group
        // payload — `reduce_event_share_queue_drained` publishes inline.
        let mut state = AppState::default();
        state.communities = vec![CommunityRow {
            group_id: group_id.into(),
            host_relay_url: "wss://host.example.com".into(),
            name: Some("Readers".into()),
            picture: None,
            about: None,
            member_count: 0,
            public: true,
            open: true,
            is_admin: false,
        }];
        let effects = reduce_event_share_queue_drained(
            &mut state,
            vec![raw_payload("item-1", group_id, url)],
        );

        let publishes: Vec<&Effect> = effects
            .iter()
            .filter(|e| matches!(e, Effect::PublishShareEvent { .. }))
            .collect();
        assert_eq!(publishes.len(), 1, "one publish per drained item");
        let Effect::PublishShareEvent { json, .. } = publishes[0] else {
            panic!("expected PublishShareEvent");
        };

        #[derive(serde::Deserialize)]
        struct T {
            kind: u32,
            tags: Vec<Vec<String>>,
            host_relay_url: String,
        }
        let t: T = serde_json::from_str(json).unwrap();
        assert_eq!(t.kind, KIND_ARTIFACT_SHARE);
        assert_eq!(
            t.tags, bespoke_tags,
            "drain publish tags must equal bespoke per-item tags"
        );
        // Host-pinned to the group's host relay (from communities).
        assert_eq!(t.host_relay_url, "wss://host.example.com");
        assert_eq!(state.share_publish.phase, SharePublishPhase::Publishing);
    }

    /// The artifact-to-room reducer sets the FSM and emits one host-pinned
    /// publish whose template matches the kernel tag builder.
    #[test]
    fn artifact_to_room_reducer_emits_host_pinned_publish() {
        let mut state = AppState::default();
        state.communities = vec![community_row("room-x", "Readers")];
        let preview = full_preview();
        let effects = reduce_action_artifact_to_room(
            &mut state,
            "room-x".into(),
            preview.clone(),
            "  note  ".into(),
        );
        assert_eq!(effects.len(), 1);
        let Effect::PublishShareEvent {
            json,
            correlation_id,
        } = &effects[0]
        else {
            panic!("expected PublishShareEvent");
        };
        assert!(!correlation_id.is_empty());
        assert_eq!(
            state.share_publish.pending_correlation_id.as_deref(),
            Some(correlation_id.as_str())
        );
        assert_eq!(state.share_publish.phase, SharePublishPhase::Publishing);

        #[derive(serde::Deserialize)]
        struct T {
            kind: u32,
            content: String,
            tags: Vec<Vec<String>>,
            host_relay_url: String,
        }
        let t: T = serde_json::from_str(json).unwrap();
        assert_eq!(t.kind, KIND_ARTIFACT_SHARE);
        assert_eq!(t.content, "note", "note is trimmed");
        assert_eq!(t.host_relay_url, "wss://relay.example.com");
        assert_eq!(t.tags, build_artifact_share_tags("room-x", &preview));
    }

    /// Empty group_id is a guard that BITES: no publish effect, FSM → Error (D6).
    #[test]
    fn artifact_to_room_guard_bites_on_empty_group() {
        let mut state = AppState::default();
        let effects =
            reduce_action_artifact_to_room(&mut state, "   ".into(), full_preview(), String::new());
        assert!(effects.is_empty(), "guard must suppress the publish");
        assert!(
            matches!(state.share_publish.phase, SharePublishPhase::Error { .. }),
            "guard must set an error state (D6)"
        );
    }

    /// The action-result verdict drives the FSM → Done on success and Error on
    /// failure (D6). Stale correlation ids are ignored.
    #[test]
    fn share_publish_action_result_drives_fsm() {
        let mut state = AppState::default();
        state.share_publish.phase = SharePublishPhase::Publishing;
        state.share_publish.pending_correlation_id = Some("cid-1".into());

        // Stale id → ignored.
        reduce_event_share_publish_action_result(&mut state, "other".into(), true, String::new());
        assert_eq!(state.share_publish.phase, SharePublishPhase::Publishing);

        // Failure verdict → Error with the raw message.
        reduce_event_share_publish_action_result(
            &mut state,
            "cid-1".into(),
            false,
            "relay rejected".into(),
        );
        assert_eq!(
            state.share_publish.phase,
            SharePublishPhase::Error {
                message: "relay rejected".into()
            }
        );
        assert!(state.share_publish.pending_correlation_id.is_none());
    }

    /// `mint_invite` generates codes, stores them for the snapshot, and reuses
    /// the field-complete `nmp.nip29.create_invite` path (ONE kind:9009 writer —
    /// emits a correlation-aware DispatchCreateInviteWithCorrelation, not a second
    /// PublishShareEvent). Finding 3: the FSM is Publishing (NOT Done) until the
    /// create-invite publish verdict arrives.
    #[test]
    fn mint_invite_generates_codes_and_reuses_create_invite_path() {
        let mut state = AppState::default();
        state.communities = vec![community_row("room-x", "Readers")];
        let effects = reduce_action_mint_invite(&mut state, "room-x".into(), 3);
        assert_eq!(state.share_publish.last_invite_codes.len(), 3);
        for code in &state.share_publish.last_invite_codes {
            assert_eq!(code.len(), 24, "invite codes are 24 chars");
        }
        // Exactly one correlation-aware create_invite dispatch (codes ≤ cap fit
        // in one event).
        assert_eq!(effects.len(), 1);
        let cid = match &effects[0] {
            Effect::DispatchCreateInviteWithCorrelation {
                json,
                correlation_id,
            } => {
                assert!(json.contains("\"codes\""));
                assert!(json.contains("\"local_id\""), "payload pins the group");
                assert!(!correlation_id.is_empty());
                correlation_id.clone()
            }
            other => panic!("expected DispatchCreateInviteWithCorrelation, got {other:?}"),
        };
        // Finding 3: FSM is Publishing, the correlation id is tracked, NOT Done.
        assert_eq!(state.share_publish.phase, SharePublishPhase::Publishing);
        assert_eq!(
            state.share_publish.pending_correlation_id.as_deref(),
            Some(cid.as_str())
        );
        // The codes surface in the snapshot (Swift composes the link — D1/D3).
        let snap = project_share_publish_snapshot(&state);
        assert_eq!(snap.invite_codes.len(), 3);
        assert!(snap.publishing, "mint is publishing, not yet done");
        assert!(
            !snap.did_publish,
            "finding 3: not Done before create_invite settles"
        );
    }

    /// Finding 3: a create-invite FAILURE verdict drives the share-mint FSM →
    /// Error (NOT a premature Done). PRE-FIX this failed because mint set Done
    /// unconditionally and discarded the create-invite correlation id, so the
    /// failure verdict never matched the FSM.
    #[test]
    fn mint_invite_failure_drives_fsm_to_error() {
        let mut state = AppState::default();
        state.communities = vec![community_row("room-x", "Readers")];
        let effects = reduce_action_mint_invite(&mut state, "room-x".into(), 1);
        let Effect::DispatchCreateInviteWithCorrelation { correlation_id, .. } = &effects[0] else {
            panic!("expected DispatchCreateInviteWithCorrelation");
        };
        let cid = correlation_id.clone();
        // Before the verdict: still Publishing (not Done).
        assert_eq!(state.share_publish.phase, SharePublishPhase::Publishing);

        // The create-invite publish FAILS → FSM → Error.
        reduce_event_share_publish_action_result(
            &mut state,
            cid,
            false,
            "relay rejected invite".into(),
        );
        assert_eq!(
            state.share_publish.phase,
            SharePublishPhase::Error {
                message: "relay rejected invite".into()
            }
        );
        let snap = project_share_publish_snapshot(&state);
        assert!(
            !snap.did_publish,
            "must NOT be Done on create-invite failure"
        );
        assert_eq!(snap.error_message.as_deref(), Some("relay rejected invite"));
    }

    /// Success path: a create-invite SUCCESS verdict drives the FSM → Done with
    /// the codes still exposed for the share link.
    #[test]
    fn mint_invite_success_drives_fsm_to_done_with_codes() {
        let mut state = AppState::default();
        state.communities = vec![community_row("room-x", "Readers")];
        let effects = reduce_action_mint_invite(&mut state, "room-x".into(), 2);
        let Effect::DispatchCreateInviteWithCorrelation { correlation_id, .. } = &effects[0] else {
            panic!("expected DispatchCreateInviteWithCorrelation");
        };
        let cid = correlation_id.clone();

        reduce_event_share_publish_action_result(&mut state, cid, true, String::new());
        let snap = project_share_publish_snapshot(&state);
        assert!(snap.did_publish, "success verdict → Done");
        assert_eq!(snap.invite_codes.len(), 2, "codes remain for the link");
    }

    /// Finding 3: the correlation-minted swap re-keys the in-flight publish to the
    /// nmp-minted id so the create-invite verdict (which arrives under the nmp id)
    /// drives the FSM.
    #[test]
    fn mint_invite_correlation_swap_then_verdict() {
        let mut state = AppState::default();
        state.communities = vec![community_row("room-x", "Readers")];
        let effects = reduce_action_mint_invite(&mut state, "room-x".into(), 1);
        let Effect::DispatchCreateInviteWithCorrelation { correlation_id, .. } = &effects[0] else {
            panic!("expected DispatchCreateInviteWithCorrelation");
        };
        let placeholder = correlation_id.clone();

        // nmp minted a different id → swap.
        reduce_event_share_publish_correlation_minted(
            &mut state,
            placeholder.clone(),
            "nmp-real-id".into(),
        );
        assert_eq!(
            state.share_publish.pending_correlation_id.as_deref(),
            Some("nmp-real-id")
        );
        // A verdict under the OLD placeholder is now ignored (stale).
        reduce_event_share_publish_action_result(&mut state, placeholder, false, "stale".into());
        assert_eq!(state.share_publish.phase, SharePublishPhase::Publishing);
        // The verdict under the nmp id drives the FSM.
        reduce_event_share_publish_action_result(
            &mut state,
            "nmp-real-id".into(),
            true,
            String::new(),
        );
        assert!(project_share_publish_snapshot(&state).did_publish);
    }

    /// Finding 2 (fail-closed host relay): a missing Rust-owned host relay for
    /// mint_invite → Error + NO dispatch (never widen a group write to Auto/outbox).
    #[test]
    fn mint_invite_missing_host_relay_fails_closed() {
        let mut state = AppState::default();
        let effects = reduce_action_mint_invite(&mut state, "room-x".into(), 1);
        assert!(
            effects.is_empty(),
            "no publish/dispatch on empty host relay"
        );
        assert!(
            matches!(state.share_publish.phase, SharePublishPhase::Error { .. }),
            "missing host relay → Error (D3 fail-closed)"
        );
        assert!(
            state.share_publish.pending_correlation_id.is_none(),
            "no in-flight publish on fail-closed"
        );
    }

    /// Finding 2 (fail-closed host relay) for artifact_to_room: missing host →
    /// Error + NO publish. PRE-FIX the runner widened an empty host to Auto.
    #[test]
    fn artifact_to_room_missing_host_relay_fails_closed() {
        let mut state = AppState::default();
        let effects = reduce_action_artifact_to_room(
            &mut state,
            "room-x".into(),
            full_preview(),
            String::new(),
        );
        assert!(effects.is_empty(), "no publish on missing host relay (D3)");
        assert!(matches!(
            state.share_publish.phase,
            SharePublishPhase::Error { .. }
        ));
    }

    /// Finding 2 (fail-closed host relay) for highlight_to_room.
    #[test]
    fn highlight_to_room_missing_host_relay_fails_closed() {
        let mut state = AppState::default();
        let effects = reduce_action_highlight_to_room(
            &mut state,
            "room-z".into(),
            "deadbeef".into(),
            "author".into(),
            "wss://hint".into(),
        );
        assert!(effects.is_empty(), "no publish on missing host relay (D3)");
        assert!(matches!(
            state.share_publish.phase,
            SharePublishPhase::Error { .. }
        ));
    }

    /// Finding 2 (fail-closed host relay) for the SHARE-QUEUE drain: a drained
    /// item whose group has NO known host relay must NOT publish (Error, no
    /// effect). PRE-FIX the drain published with an empty host (→ Auto outbox).
    #[test]
    fn drain_empty_host_relay_fails_closed() {
        let mut state = AppState::default();
        // No communities seeded → host_for returns "" for the item's group.
        let effects = reduce_event_share_queue_drained(
            &mut state,
            vec![raw_payload("item-1", "group-a", "https://example.com/post")],
        );
        let publishes = effects
            .iter()
            .filter(|e| matches!(e, Effect::PublishShareEvent { .. }))
            .count();
        assert_eq!(
            publishes, 0,
            "no publish for a group with no host relay (D3)"
        );
        assert!(
            matches!(state.share_publish.phase, SharePublishPhase::Error { .. }),
            "drain with no host relay → Error (D3 fail-closed)"
        );
    }

    /// Finding 1 (validated ingress): a reserved replaceable-list kind (e.g.
    /// kind:0 a future builder might emit) is REJECTED at the kernel share ingress
    /// → Error + NO publish, mirroring the validated `nmp.publish` PublishRaw
    /// rejection. PRE-FIX `emit_share_publish` did not exist and the runner sent
    /// `ActorCommand::PublishRawEvent` directly, bypassing reserved-kind rejection.
    #[test]
    fn reserved_kind_rejected_through_validated_ingress() {
        let mut state = AppState::default();
        // Simulate a (future) builder emitting a reserved kind through the shared
        // validated emit path.
        let effects = emit_share_publish(
            &mut state,
            0, // reserved: kind:0 profile metadata
            "",
            vec![vec!["h".into(), "room-x".into()]],
            "wss://host.example.com",
        );
        assert!(effects.is_empty(), "reserved kind must NOT publish");
        assert!(
            matches!(state.share_publish.phase, SharePublishPhase::Error { .. }),
            "reserved kind → Error (validated ingress rejection)"
        );
        // The other reserved kinds (3 contacts, 10003 bookmarks) are rejected too.
        for k in [3u32, 10003] {
            let mut s = AppState::default();
            let e = emit_share_publish(&mut s, k, "", vec![], "wss://host");
            assert!(e.is_empty(), "kind:{k} must be rejected");
            assert!(matches!(
                s.share_publish.phase,
                SharePublishPhase::Error { .. }
            ));
        }
        // A normal kind:11 share still publishes (guard does not over-fire).
        let mut s = AppState::default();
        let e = emit_share_publish(&mut s, KIND_ARTIFACT_SHARE, "", vec![], "wss://host");
        assert_eq!(e.len(), 1, "kind:11 still publishes");
        assert_eq!(s.share_publish.phase, SharePublishPhase::Publishing);
    }

    /// Snapshot reflects each FSM phase (the iOS sheet renders from this).
    #[test]
    fn share_publish_snapshot_reflects_phase() {
        let mut state = AppState::default();
        assert!(!project_share_publish_snapshot(&state).publishing);

        state.share_publish.phase = SharePublishPhase::Publishing;
        assert!(project_share_publish_snapshot(&state).publishing);

        state.share_publish.phase = SharePublishPhase::Done;
        assert!(project_share_publish_snapshot(&state).did_publish);

        state.share_publish.phase = SharePublishPhase::Error {
            message: "boom".into(),
        };
        assert_eq!(
            project_share_publish_snapshot(&state)
                .error_message
                .as_deref(),
            Some("boom")
        );
    }
}
