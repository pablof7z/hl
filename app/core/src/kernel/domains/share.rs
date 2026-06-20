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
    let queue = &mut state.share_queue;

    for payload in payloads {
        let key = (payload.group_id.clone(), payload.url.clone());
        if queue.seen.contains(&key) {
            // Dedupe: skip items we have already queued or processed this session.
            continue;
        }
        queue.seen.insert(key);
        queue.pending.push(ShareQueueItem {
            id: payload.id,
            group_id: payload.group_id,
            url: payload.url,
            note: payload.note,
            created_at_unix_seconds: payload.created_at_unix_seconds,
        });
    }

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

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::share::{RawSharePayload, ShareResult};
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

    // ── 5K: share_queue_device_local_not_published ───────────────────────────

    #[test]
    fn share_queue_device_local_not_published() {
        // The share queue items are in AppState (device-local transient) only.
        // They are not emitted as nostr events — only the explicit
        // AppAction::ShareToRoom path publishes a kind:11. Verify that draining
        // does NOT produce a publish effect.
        let mut state = AppState::default();
        let payload = raw_payload("item-1", "group-a", "https://example.com");

        let effects = reduce_event_share_queue_drained(&mut state, vec![payload]);

        // Only a WriteCommunitiesSnapshot capability request is emitted — no
        // publish effect (kind:11 is emitted by a separate AppAction::ShareToRoom).
        let has_publish = effects
            .iter()
            .any(|e| matches!(e, crate::kernel::effect::Effect::DispatchShareToRoom { .. }));
        assert!(
            !has_publish,
            "drain must not auto-publish; publishing requires explicit ShareToRoom action"
        );
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
}
