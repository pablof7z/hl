//! Discussions domain — NIP-29 kind:11 discussion projection + write actions
//! (Phase 7 discussions).
//!
//! ## Responsibilities
//!
//! * **READ** — per-room `DiscussionObserver` wraps `GroupEventsProjection`
//!   (nmp-nip29, pinned d16aea60) as a `KernelEventObserver`. On each raw
//!   Nostr event the observer:
//!   (a) delegates ingest to the underlying `GroupEventsProjection`,
//!   (b) filters to kind==11 events that carry a `["t","discussion"]` tag, and
//!   (c) rebuilds a bounded `Vec<DiscussionRow>` (newest-first, cap 64) from the
//!   current `GroupEventsSnapshot`, then sends
//!   `KernelEvent::RoomDiscussionsUpdated { group_id, rows }` into the actor
//!   channel.
//!
//!   The actor's `reduce_event` arm stores the rows in `AppState::room_discussions`
//!   keyed by `group_id`.
//!
//!   This is the Family-B integration pattern (observer to actor-channel path), the
//!   same as comments.rs and reactions.rs. A fresh `GroupEventsProjection` is
//!   registered per watched room.
//!
//! * **WRITE** — `hl.discussion.post` envelope -> `reduce_action_post_discussion` ->
//!   `Effect::PublishDiscussionEvent { json }` -> `run_effect_publish_discussion`
//!   calls `ActorCommand::PublishRawEvent` with kind:11, fire-and-forget (D6).
//!   nmp fills `id`/`sig`/`pubkey`/`created_at` before broadcasting. The kernel
//!   is the sole kind:11 writer for ported screens.
//!
//! ## Tag shape (kind:11 event, mirroring live `discussions.rs`)
//!
//! ```text
//! ["h",      group_id        ]  NIP-29 h-tag routing
//! ["t",      "discussion"    ]  discussion marker (for is_discussion filter)
//! ["title",  title           ]  discussion title (non-empty required)
//! ["r",      attachment_url  ]  optional URL attachment
//! ```
//!
//! `body` is the event `content` field.
//!
//! ## Wire registration
//!
//! `register_discussion_observer(nmp_ref, group_id, tx)` is called per watched
//! room when `ViewId::RoomDiscussions { group_id }` opens.
//!
//! ## Lifecycle
//!
//! * `ViewId::RoomDiscussions` open -> register `DiscussionObserver`.
//! * Logout -> `clear_on_logout` removes all entries from `AppState::room_discussions`.
//! * The observer buffer is bounded (cap 64) so memory is bounded.
//!
//! ## D-rules satisfied
//!
//! * D1 -- `DiscussionRow` carries raw protocol fields only.
//! * D6 -- malformed events -> silent no-op. Empty `group_id` or `title` -> no effects.
//! * D8 -- `on_kernel_event` has no blocking awaits.
//! * D9 -- `created_at` comes from the protocol event; kernel never stamps time.

use std::sync::{Arc, Mutex};

use nmp_core::substrate::{ObservedProjection, ObservedProjectionRegistrar};
use nmp_core::ObservedProjectionSink;
use nmp_ffi::NmpApp;
use nmp_planner::InterestShape;
use tokio::sync::mpsc;

use crate::kernel::action::{KernelEvent, PostDiscussionPayload};
use crate::kernel::actor::Cmd;
use crate::kernel::app::AppState;
use crate::kernel::snapshot::{ArtifactPreviewRow, DiscussionRow, RoomDiscussionsSnapshot};

// Constants

/// NIP-29 kind:11 -- discussion thread event.
const KIND_DISCUSSION: u32 = 11;

/// `["t", "discussion"]` marker tag value.
const DISCUSSION_MARKER_TAG: &str = "discussion";

/// Maximum rows retained per room in `AppState::room_discussions`.
pub(crate) const ROOM_DISCUSSIONS_CAP: usize = 64;

// READ side: ObservedProjectionSink wrapper

/// Observer wrapper for a single NIP-29 room. Ingests raw Nostr events into
/// `GroupEventsProjection` and produces `KernelEvent::RoomDiscussionsUpdated`
/// for each accepted kind:11+discussion event.
///
/// Family-B integration pattern: same structure as `CommentObserver`.
///
/// D6: non-kind:11 or kind:11 without `["t","discussion"]` -> silent return.
/// D8: `on_kernel_event` is synchronous; channel send is non-blocking.
struct DiscussionObserver {
    group_id: String,
    tx: mpsc::UnboundedSender<Cmd>,
    events: Mutex<Vec<GroupEventRow>>,
}

impl ObservedProjectionSink for DiscussionObserver {
    fn on_kernel_event(&self, event: &nmp_core::substrate::KernelEvent) {
        if event.kind != KIND_DISCUSSION {
            return;
        }
        if !has_discussion_marker(&event.tags) {
            return;
        }
        if h_tag_value(&event.tags) != Some(self.group_id.as_str()) {
            return;
        }

        let rows = {
            let Ok(mut events) = self.events.lock() else {
                return;
            };
            events.push(GroupEventRow::from_kernel_event(event));
            events.sort_by(|a, b| {
                b.created_at
                    .cmp(&a.created_at)
                    .then_with(|| b.id.cmp(&a.id))
            });
            events.dedup_by(|a, b| a.id == b.id);
            events.truncate(ROOM_DISCUSSIONS_CAP);
            build_discussion_rows(events.as_slice())
        };

        let _ = self
            .tx
            .send(Cmd::Event(KernelEvent::RoomDiscussionsUpdated {
                group_id: self.group_id.clone(),
                rows,
            }));
    }
}

#[derive(Clone, Debug)]
struct GroupEventRow {
    id: String,
    pubkey: String,
    content: String,
    created_at: u64,
    kind: u32,
    tags: Vec<Vec<String>>,
    #[allow(dead_code)]
    relay_provenance: Vec<String>,
}

impl GroupEventRow {
    fn from_kernel_event(event: &nmp_core::substrate::KernelEvent) -> Self {
        Self {
            id: event.id.clone(),
            pubkey: event.author.clone(),
            content: event.content.clone(),
            created_at: event.created_at,
            kind: event.kind,
            tags: event.tags.clone(),
            relay_provenance: event.relay_provenance.clone(),
        }
    }
}

// Filtering helpers

/// Returns `true` when `tags` contains a `["t", "discussion"]` entry.
fn has_discussion_marker(tags: &[Vec<String>]) -> bool {
    tags.iter()
        .any(|t| t.len() >= 2 && t[0] == "t" && t[1] == DISCUSSION_MARKER_TAG)
}

fn h_tag_value(tags: &[Vec<String>]) -> Option<&str> {
    tags.iter()
        .find(|t| t.len() >= 2 && t[0] == "h" && !t[1].is_empty())
        .map(|t| t[1].as_str())
}

/// Extract the `["title", value]` tag value, or empty string if absent.
///
/// D1: no fallback string -- Swift owns display fallbacks.
fn extract_title(tags: &[Vec<String>]) -> String {
    tags.iter()
        .find(|t| t.len() >= 2 && t[0] == "title")
        .map(|t| t[1].clone())
        .unwrap_or_default()
}

/// Extract the first URL attachment (`["r", url]` tag), or `None` if absent.
///
/// Only non-empty values qualify. D1: returns raw URL string, no validation.
fn extract_attachment_url(tags: &[Vec<String>]) -> Option<String> {
    tags.iter()
        .find(|t| t.len() >= 2 && t[0] == "r" && !t[1].is_empty())
        .map(|t| t[1].clone())
}

/// Extract the canonical artifact coordinate from `a`/`e`/`i` reference tags.
///
/// Priority: `a` → `e` → `i`. Returns `"<tag>:<value>"` canonical form (matching
/// the coordinate scheme used in `AppState::artifact_previews`) or `None` when no
/// recognized reference tag with a non-empty value is present.
///
/// D6: non-empty guard prevents blank coordinate strings from entering the map.
fn extract_artifact_coordinate(tags: &[Vec<String>]) -> Option<String> {
    for prefix in ["a", "e", "i"] {
        if let Some(v) = tags
            .iter()
            .find(|t| t.len() >= 2 && t[0] == prefix && !t[1].is_empty())
            .map(|t| t[1].clone())
        {
            return Some(format!("{prefix}:{v}"));
        }
    }
    None
}

// Snapshot builder

/// Build a bounded `Vec<DiscussionRow>` from a slice of `GroupEventRow`s.
///
/// Filters to kind==11 events with the `["t","discussion"]` marker, maps each
/// to a `DiscussionRow`, sorts newest-first by `created_at`, and caps at
/// `ROOM_DISCUSSIONS_CAP` (64).
///
/// D1: all fields are raw protocol data. D6: missing tags -> defaults (no panic).
fn build_discussion_rows(events: &[GroupEventRow]) -> Vec<DiscussionRow> {
    let mut rows: Vec<DiscussionRow> = events
        .iter()
        .filter(|e| e.kind == KIND_DISCUSSION && has_discussion_marker(&e.tags))
        .map(|e| DiscussionRow {
            event_id: e.id.clone(),
            author_pubkey: e.pubkey.clone(),
            title: extract_title(&e.tags),
            body: e.content.clone(),
            attachment_url: extract_attachment_url(&e.tags),
            artifact_coordinate: extract_artifact_coordinate(&e.tags),
            created_at: e.created_at,
        })
        .collect();

    // Newest-first (descending created_at).
    rows.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    rows.truncate(ROOM_DISCUSSIONS_CAP);
    rows
}

// State event handler (called from reduce_event in actor.rs)

/// Apply `KernelEvent::RoomDiscussionsUpdated` to `AppState::room_discussions`.
///
/// D1: stores raw protocol data only. D6: always succeeds (no Result).
pub(crate) fn reduce_event_room_discussions_updated(
    state: &mut AppState,
    group_id: String,
    rows: Vec<DiscussionRow>,
) -> Vec<crate::kernel::effect::Effect> {
    state.room_discussions.insert(group_id, rows);
    vec![]
}

// Logout handler

/// Clear all per-room discussion rows from `AppState::room_discussions` on logout.
///
/// Discussions are identity-scoped. On logout they must not surface under a new
/// account. Called from `auth::reduce_action_logout`.
pub(crate) fn clear_on_logout(state: &mut AppState) {
    state.room_discussions.clear();
}

// WRITE side: reduce_action helper

/// Handle the `hl.discussion.post` envelope action.
///
/// Validates the payload (D6 guards), builds a kind:11 event template with the
/// correct tag shape, serialises via `serde_json::json!` (never `format!`), and
/// emits `Effect::PublishDiscussionEvent { json }`.
///
/// Tag shape:
/// ```text
/// ["h",     group_id        ]   NIP-29 routing
/// ["t",     "discussion"    ]   discussion marker
/// ["title", title           ]   discussion title
/// ["r",     attachment_url  ]   optional, only when non-empty
/// ```
///
/// D6 guards:
/// - Empty `group_id` (trimmed) -> return `vec![]` (no-op, no publish).
/// - Empty `title` (trimmed) -> return `vec![]` (title is required).
pub(crate) fn reduce_action_post_discussion(
    payload: PostDiscussionPayload,
) -> Vec<crate::kernel::effect::Effect> {
    if payload.group_id.trim().is_empty() {
        return vec![];
    }
    if payload.title.trim().is_empty() {
        return vec![];
    }

    let mut tags: Vec<Vec<String>> = vec![
        vec!["h".to_string(), payload.group_id.clone()],
        vec!["t".to_string(), DISCUSSION_MARKER_TAG.to_string()],
        vec!["title".to_string(), payload.title.clone()],
    ];

    if let Some(url) = &payload.attachment_url {
        if !url.trim().is_empty() {
            tags.push(vec!["r".to_string(), url.clone()]);
        }
    }

    let template = serde_json::json!({
        "kind": KIND_DISCUSSION,
        "content": payload.body,
        "tags": tags,
    });

    let json = match serde_json::to_string(&template) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "discussions::reduce_action_post_discussion: serde error (D6)");
            return vec![];
        }
    };

    vec![crate::kernel::effect::Effect::PublishDiscussionEvent { json }]
}

// Effect runner

/// Execute `Effect::PublishDiscussionEvent` -- sends `ActorCommand::PublishRawEvent`
/// with the kind:11 event template via `nmp_ref.actor_sender()`.
///
/// nmp fills `id`, `sig`, `pubkey`, and `created_at` before broadcasting (D7, D9).
/// Target is `PublishTarget::Auto` (NIP-65 outbox, D3). Fire-and-forget (D6).
///
/// No-op when `nmp` is `None` (test mode).
pub(crate) fn run_effect_publish_discussion(
    json: String,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
) {
    let Some(handle) = nmp else {
        tracing::debug!("PublishDiscussionEvent: no live NmpApp (test mode)");
        return;
    };
    let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };

    #[derive(serde::Deserialize)]
    struct EventTemplate {
        kind: u32,
        content: String,
        tags: Vec<Vec<String>>,
    }

    let Ok(template) = serde_json::from_str::<EventTemplate>(&json) else {
        tracing::warn!("PublishDiscussionEvent: failed to deserialize event template (D6)");
        return;
    };

    let _ = nmp_ref
        .actor_sender()
        .send(nmp_core::actor::ActorCommand::Publish(
            nmp_core::actor::PublishCommand::RawEvent {
                kind: template.kind,
                content: template.content,
                tags: template.tags,
                target: nmp_core::publish::PublishTarget::Auto,
                signer_pubkey: None,
                correlation_id: None,
            },
        ));
}

// Snapshot computation

/// Compute a `RoomDiscussionsSnapshot` for the given `group_id`.
///
/// Reads `AppState::room_discussions` for the most recent bounded row slice.
/// Returns an empty snapshot when no data is present yet -- never panics (D6).
pub(crate) fn compute_room_discussions_snapshot(
    state: &AppState,
    group_id: &str,
) -> RoomDiscussionsSnapshot {
    let rows = state
        .room_discussions
        .get(group_id)
        .cloned()
        .unwrap_or_default();

    // Resolve thin previews for the artifact coordinates the rows reference, so
    // Swift can render a rich discussion attachment chip (title/image/author)
    // instead of a bare URL. Deduped, in first-seen row order. Seeded by
    // `ensure_room_artifact_previews` on `RoomDiscussionsUpdated`; a missing
    // entry simply omits that chip's rich data (Swift falls back to the URL).
    let artifact_previews: Vec<ArtifactPreviewRow> = {
        let mut seen = std::collections::HashSet::new();
        rows.iter()
            .filter_map(|r| r.artifact_coordinate.as_ref())
            .filter(|coord| seen.insert((*coord).clone()))
            .filter_map(|coord| state.artifact_previews.get(coord).cloned())
            .collect()
    };

    RoomDiscussionsSnapshot {
        group_id: group_id.to_string(),
        rows,
        artifact_previews,
    }
}

// Projection registration

/// Wire a fresh `DiscussionObserver` (wrapping a new `GroupEventsProjection` for
/// `group_id`) as a `KernelEventObserver` against `nmp_ref`.
///
/// Called from the actor when `ViewId::RoomDiscussions { group_id }` opens.
///
/// D6: if `register_live_event_tap` returns id `0` (slot full), the observer is
/// silently dropped and room discussions will not update for this room.
pub(crate) fn register_discussion_observer(
    nmp_ref: &NmpApp,
    group_id: String,
    tx: mpsc::UnboundedSender<Cmd>,
) {
    let consumer_id = format!("hl.discussions.{group_id}");
    let mut shape = InterestShape::default();
    shape.kinds.insert(KIND_DISCUSSION);
    shape
        .tags
        .entry("h".to_string())
        .or_default()
        .insert(group_id.clone());
    shape
        .tags
        .entry("t".to_string())
        .or_default()
        .insert(DISCUSSION_MARKER_TAG.to_string());
    let observer = Arc::new(DiscussionObserver {
        group_id,
        tx,
        events: Mutex::new(Vec::new()),
    });

    let observer_id = nmp_ref.open_observed_projection(ObservedProjection::from_shape(
        observer as Arc<dyn ObservedProjectionSink>,
        consumer_id,
        1,
        shape,
        ROOM_DISCUSSIONS_CAP,
    ));
    if observer_id.0 == 0 {
        tracing::warn!(
            "discussions::register_discussion_observer: event-observer registration failed (D6)"
        );
    }
}

// Unit tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::{AppAction, KernelEvent};
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::effect::Effect;

    fn make_state() -> AppState {
        AppState::default()
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    fn make_kind11_row(
        id: &str,
        pubkey: &str,
        title: &str,
        body: &str,
        group_id: &str,
        created_at: u64,
        attachment_url: Option<&str>,
    ) -> GroupEventRow {
        let mut tags = vec![
            vec!["h".to_string(), group_id.to_string()],
            vec!["t".to_string(), "discussion".to_string()],
            vec!["title".to_string(), title.to_string()],
        ];
        if let Some(url) = attachment_url {
            tags.push(vec!["r".to_string(), url.to_string()]);
        }
        GroupEventRow {
            id: id.to_string(),
            pubkey: pubkey.to_string(),
            content: body.to_string(),
            created_at,
            kind: 11,
            tags,
            relay_provenance: vec![],
        }
    }

    const GROUP_ID: &str = "test_group_1";
    const PUBKEY_A: &str = "aaaa000000000000000000000000000000000000000000000000000000000001";

    /// `discussions_filter_kind11_from_group_events`
    ///
    /// Only kind:11 events with `["t","discussion"]` should appear.
    /// Other kinds and kind:11 without the marker must be dropped.
    #[test]
    fn discussions_filter_kind11_from_group_events() {
        let clock = ManualClock::new(1_000_000);
        let mut state = make_state();

        let good_row = make_kind11_row("evt_good", PUBKEY_A, "First", "body", GROUP_ID, 1000, None);
        let rows = build_discussion_rows(&[good_row]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].event_id, "evt_good");

        // kind:7 must not appear.
        let kind7_row = GroupEventRow {
            id: "evt_kind7".to_string(),
            pubkey: PUBKEY_A.to_string(),
            content: "+".to_string(),
            created_at: 1001,
            kind: 7,
            tags: vec![vec!["h".to_string(), GROUP_ID.to_string()]],
            relay_provenance: vec![],
        };
        let rows = build_discussion_rows(&[kind7_row]);
        assert!(rows.is_empty(), "kind:7 must be filtered out");

        // kind:11 without discussion marker must not appear.
        let kind11_no_marker = GroupEventRow {
            id: "evt_no_marker".to_string(),
            pubkey: PUBKEY_A.to_string(),
            content: "body".to_string(),
            created_at: 1002,
            kind: 11,
            tags: vec![
                vec!["h".to_string(), GROUP_ID.to_string()],
                vec!["t".to_string(), "other_type".to_string()],
            ],
            relay_provenance: vec![],
        };
        let rows = build_discussion_rows(&[kind11_no_marker]);
        assert!(
            rows.is_empty(),
            "kind:11 without discussion marker must be filtered out"
        );

        // Inject via actor channel to confirm state integration.
        let effects = step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::RoomDiscussionsUpdated {
                group_id: GROUP_ID.to_string(),
                rows: vec![DiscussionRow {
                    event_id: "evt_good".to_string(),
                    author_pubkey: PUBKEY_A.to_string(),
                    title: "First".to_string(),
                    body: "body".to_string(),
                    attachment_url: None,
                    artifact_coordinate: None,
                    created_at: 1000,
                }],
            }),
        );
        assert!(effects.is_empty());
        let stored_rows = &state.room_discussions[GROUP_ID];
        assert_eq!(stored_rows.len(), 1);
        assert_eq!(stored_rows[0].event_id, "evt_good");
    }

    /// `room_discussions_snapshot_resolves_artifact_chip_previews`
    ///
    /// A discussion referencing an artifact (via `a`/`e`/`i`) must surface its
    /// resolved thin preview on the snapshot so Swift renders a rich attachment
    /// chip (title/image/author) instead of a bare URL — the discussion-chip #1
    /// gap. Coordinates with no seeded preview contribute no chip (no fabricated
    /// empties); coordinates are deduped.
    #[test]
    fn room_discussions_snapshot_resolves_artifact_chip_previews() {
        let mut state = make_state();
        let addr = "30023:author:essay";
        let coord = format!("a:{addr}");

        // Two discussions referencing the SAME artifact + one referencing an
        // unseeded artifact (must not produce a chip).
        let referencing = |id: &str, created_at: u64, address: &str| GroupEventRow {
            id: id.to_string(),
            pubkey: PUBKEY_A.to_string(),
            content: "look at this".to_string(),
            created_at,
            kind: 11,
            tags: vec![
                vec!["h".to_string(), GROUP_ID.to_string()],
                vec!["t".to_string(), "discussion".to_string()],
                vec!["title".to_string(), "A discussion".to_string()],
                vec!["a".to_string(), address.to_string()],
            ],
            relay_provenance: vec![],
        };
        let rows = build_discussion_rows(&[
            referencing("disc-1", 1000, addr),
            referencing("disc-2", 1001, addr),
            referencing("disc-3", 1002, "30023:author:unseeded"),
        ]);
        // build_discussion_rows sorts newest-first; assert order-independently
        // that both the seeded coord and the unseeded coord were extracted.
        assert!(rows
            .iter()
            .any(|r| r.artifact_coordinate.as_deref() == Some(coord.as_str())));
        assert!(rows
            .iter()
            .any(|r| r.artifact_coordinate.as_deref() == Some("a:30023:author:unseeded")));
        state.room_discussions.insert(GROUP_ID.to_string(), rows);

        // Seed a resolved preview for only the first coordinate.
        state.artifact_previews.insert(
            coord.clone(),
            ArtifactPreviewRow {
                coordinate: coord.clone(),
                title: Some("Essay".to_string()),
                image_url: Some("https://img.example/x.jpg".to_string()),
                author_pubkey: Some("author".to_string()),
                summary: None,
                kind: crate::kernel::snapshot::ArtifactPreviewKind::Article,
                pending: false,
                display_url: None,
            },
        );

        let snap = compute_room_discussions_snapshot(&state, GROUP_ID);
        assert_eq!(
            snap.artifact_previews.len(),
            1,
            "deduped + only the seeded coordinate yields a chip"
        );
        assert_eq!(snap.artifact_previews[0].coordinate, coord);
        assert_eq!(snap.artifact_previews[0].title.as_deref(), Some("Essay"));
    }

    /// `discussion_attachment_extracted_from_tags`
    ///
    /// The `["r", url]` tag should be extracted into `attachment_url`.
    /// Events without an `r` tag should have `attachment_url == None`.
    #[test]
    fn discussion_attachment_extracted_from_tags() {
        let with_attachment = make_kind11_row(
            "evt_attach",
            PUBKEY_A,
            "Has attachment",
            "body",
            GROUP_ID,
            2000,
            Some("https://example.com/image.jpg"),
        );
        let rows = build_discussion_rows(&[with_attachment]);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].attachment_url.as_deref(),
            Some("https://example.com/image.jpg")
        );

        let without_attachment = make_kind11_row(
            "evt_no_attach",
            PUBKEY_A,
            "No attachment",
            "body",
            GROUP_ID,
            1999,
            None,
        );
        let rows = build_discussion_rows(&[without_attachment]);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].attachment_url.is_none());

        // Empty r-tag value should yield None.
        let empty_r_row = GroupEventRow {
            id: "evt_empty_r".to_string(),
            pubkey: PUBKEY_A.to_string(),
            content: "body".to_string(),
            created_at: 1998,
            kind: 11,
            tags: vec![
                vec!["h".to_string(), GROUP_ID.to_string()],
                vec!["t".to_string(), "discussion".to_string()],
                vec!["r".to_string(), "".to_string()],
            ],
            relay_provenance: vec![],
        };
        let rows = build_discussion_rows(&[empty_r_row]);
        assert_eq!(rows.len(), 1);
        assert!(
            rows[0].attachment_url.is_none(),
            "empty r-tag value must yield None"
        );
    }

    /// `post_discussion_publishes_kind11_raw`
    ///
    /// `hl.discussion.post` envelope with valid group_id + title should produce
    /// a single `Effect::PublishDiscussionEvent` whose JSON is a kind:11 template.
    #[test]
    fn post_discussion_publishes_kind11_raw() {
        use serde_json::Value;

        let clock = ManualClock::new(1_000_000);
        let mut state = make_state();

        let payload_json = serde_json::to_string(&serde_json::json!({
            "group_id": GROUP_ID,
            "title": "Test discussion title",
            "body": "Discussion body text.",
            "attachment_url": "https://example.com/page",
        }))
        .unwrap();

        let effects = step(
            &mut state,
            &clock,
            Cmd::ActionEnvelope(crate::kernel::action::AppActionEnvelope {
                namespace: "hl.discussion.post".to_string(),
                json: payload_json,
            }),
        );

        assert_eq!(effects.len(), 1);
        let Effect::PublishDiscussionEvent { json } = &effects[0] else {
            panic!("expected PublishDiscussionEvent, got {:?}", effects[0]);
        };

        let parsed: Value = serde_json::from_str(json).expect("effect JSON must be valid");
        assert_eq!(parsed["kind"], 11, "kind must be 11");
        assert_eq!(parsed["content"].as_str().unwrap(), "Discussion body text.");

        let tags = parsed["tags"].as_array().unwrap();
        let has_h = tags
            .iter()
            .any(|t| t[0].as_str() == Some("h") && t[1].as_str() == Some(GROUP_ID));
        let has_t = tags
            .iter()
            .any(|t| t[0].as_str() == Some("t") && t[1].as_str() == Some("discussion"));
        let has_title = tags.iter().any(|t| {
            t[0].as_str() == Some("title") && t[1].as_str() == Some("Test discussion title")
        });
        let has_r = tags.iter().any(|t| {
            t[0].as_str() == Some("r") && t[1].as_str() == Some("https://example.com/page")
        });
        assert!(has_h, "must have h-tag with group_id");
        assert!(has_t, "must have t-tag 'discussion'");
        assert!(has_title, "must have title tag");
        assert!(has_r, "must have r-tag with attachment_url");
    }

    /// `discussions_snapshot_bounded_64`
    ///
    /// Injecting more than 64 rows must result in exactly 64 rows, newest-first.
    #[test]
    fn discussions_snapshot_bounded_64() {
        let clock = ManualClock::new(2_000_000);
        let mut state = make_state();

        let rows: Vec<GroupEventRow> = (1u64..=70)
            .map(|i| {
                make_kind11_row(
                    &format!("evt_{i:04}"),
                    PUBKEY_A,
                    &format!("Title {i}"),
                    "body",
                    GROUP_ID,
                    i,
                    None,
                )
            })
            .collect();

        let discussion_rows = build_discussion_rows(&rows);
        assert_eq!(discussion_rows.len(), 64, "must be bounded at 64");
        assert_eq!(discussion_rows[0].created_at, 70, "newest first");
        assert_eq!(discussion_rows[63].created_at, 7, "oldest kept");

        let effects = step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::RoomDiscussionsUpdated {
                group_id: GROUP_ID.to_string(),
                rows: discussion_rows.clone(),
            }),
        );
        assert!(effects.is_empty());
        assert_eq!(state.room_discussions[GROUP_ID].len(), 64);
        assert_eq!(state.room_discussions[GROUP_ID][0].created_at, 70);
    }

    /// `cleared_on_logout`
    ///
    /// After logout, `AppState::room_discussions` must be empty.
    #[test]
    fn cleared_on_logout() {
        let clock = ManualClock::new(3_000_000);
        let mut state = make_state();

        let _ = step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::RoomDiscussionsUpdated {
                group_id: GROUP_ID.to_string(),
                rows: vec![DiscussionRow {
                    event_id: "evt_pre_logout".to_string(),
                    author_pubkey: PUBKEY_A.to_string(),
                    title: "Pre-logout discussion".to_string(),
                    body: "body".to_string(),
                    attachment_url: None,
                    artifact_coordinate: None,
                    created_at: 1000,
                }],
            }),
        );
        assert!(
            !state.room_discussions.is_empty(),
            "row must be present before logout"
        );

        let _ = step(&mut state, &clock, Cmd::Action(AppAction::Logout));

        assert!(
            state.room_discussions.is_empty(),
            "room_discussions must be cleared after logout"
        );
    }

    /// `malformed_no_op`
    ///
    /// Malformed `hl.discussion.post` payloads must produce zero effects (D6).
    #[test]
    fn malformed_no_op() {
        let clock = ManualClock::new(4_000_000);
        let mut state = make_state();

        // Empty group_id
        let effects = step(
            &mut state,
            &clock,
            Cmd::ActionEnvelope(crate::kernel::action::AppActionEnvelope {
                namespace: "hl.discussion.post".to_string(),
                json: serde_json::to_string(&serde_json::json!({
                    "group_id": "",
                    "title": "Some title",
                    "body": "body",
                    "attachment_url": null,
                }))
                .unwrap(),
            }),
        );
        assert!(effects.is_empty(), "empty group_id must produce no effects");

        // Whitespace-only title
        let effects = step(
            &mut state,
            &clock,
            Cmd::ActionEnvelope(crate::kernel::action::AppActionEnvelope {
                namespace: "hl.discussion.post".to_string(),
                json: serde_json::to_string(&serde_json::json!({
                    "group_id": GROUP_ID,
                    "title": "   ",
                    "body": "body",
                    "attachment_url": null,
                }))
                .unwrap(),
            }),
        );
        assert!(
            effects.is_empty(),
            "whitespace-only title must produce no effects"
        );

        // Invalid JSON
        let effects = step(
            &mut state,
            &clock,
            Cmd::ActionEnvelope(crate::kernel::action::AppActionEnvelope {
                namespace: "hl.discussion.post".to_string(),
                json: "not valid json {{".to_string(),
            }),
        );
        assert!(
            effects.is_empty(),
            "invalid JSON payload must produce no effects"
        );
    }
}
