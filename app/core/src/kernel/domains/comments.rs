//! Comments domain — NIP-22 comment thread projection + write actions (Phase 7).
//!
//! ## Responsibilities
//!
//! * **READ** — register a `CommentObserver` wrapper around `CommentThreadProjection`
//!   (nmp-nip22) as a `KernelEventObserver`. On each kind:1111 event the observer:
//!   (a) delegates ingest to the underlying `CommentThreadProjection`,
//!   (b) extracts the `root_tag_value` via `nmp_nip22::try_from_kernel_event`, and
//!   (c) calls `projection.snapshot_for(root_tag_value)` and sends
//!   `KernelEvent::CommentThreadUpdated { root_tag_value, snapshot }` back into the
//!   actor channel. The actor's `reduce_event` arm stores the snapshot in
//!   `AppState::comment_threads` keyed by `root_tag_value`.
//!
//!   This is the Family-B integration pattern (observer→actor-channel path), the
//!   same as reactions.rs. No typed-snapshot / FlatBuffers registration is used
//!   because `CommentThreadProjection` has no FlatBuffers encoding and
//!   `snapshot_for` is per-root with no public entry iterator.
//!
//! * **WRITE** — `hl.comment.post` envelope → `reduce_action_post_comment` →
//!   `Effect::DispatchCommentAction { json }` → `run_effect_dispatch_comment_action`
//!   calls `nmp_app_dispatch_action("nmp.nip22.post_comment", json)` fire-and-forget.
//!
//! ## Wire registration
//!
//! `register_comment_projection(nmp_ref, tx)` is called ONCE at boot from
//! `start_nmp_app` (after `nmp_app_start`). It creates a FRESH
//! `Arc<CommentThreadProjection>` (separate from the one in `nmp-defaults`
//! `register_defaults` — double observation is harmless, same pattern as bookmarks).
//! The `nmp.nip22.post_comment` action is NOT re-registered here (nmp-defaults
//! already wired it). The new projection observer is solely for the hl snapshot path.
//!
//! ## Threading
//!
//! `CommentObserver::on_kernel_event` is called from the NMP event-dispatch thread.
//! It:
//! 1. Calls `self.projection.on_kernel_event(event)` (ingest — brief Mutex acquire).
//! 2. Calls `nmp_nip22::try_from_kernel_event(event)` to extract `root_tag_value`.
//! 3. Calls `self.projection.snapshot_for(&root_tag_value)` (brief Mutex acquire).
//! 4. Sends `Cmd::Event(KernelEvent::CommentThreadUpdated{…})` — non-blocking.
//!
//! D8 compliant: no blocking awaits. D6: if `try_from_kernel_event` returns None
//! (not a valid comment), the observer returns silently.
//!
//! ## D-rules satisfied
//!
//! * D1 — `CommentRecordRow` carries raw protocol fields only (no formatted
//!   timestamps, no byline strings). `is_top_level` is a pure boolean derived
//!   from `parent_tag_value == root_tag_value`. Swift owns all display formatting.
//! * D6 — `try_from_kernel_event` returning None (malformed event) → silent no-op.
//!   Empty `root_tag_value` or empty `content` in `PostCommentPayload` → no effects.
//! * Non-Negotiable #3 — `reduce_action_post_comment` returns `Vec<Effect>` (never
//!   `Result`); fire-and-forget.

use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::Arc;

use nmp_core::KernelEventObserver;
use nmp_ffi::NmpApp;
use nmp_nip22::{CommentThreadProjection, CommentThreadSnapshot, KIND_COMMENT};
use tokio::sync::mpsc;

use crate::kernel::action::{KernelEvent, PostCommentPayload};
use crate::kernel::actor::Cmd;
use crate::kernel::app::AppState;
use crate::kernel::snapshot::{CommentRecordRow, CommentThreadKernelSnapshot};

// ─── nmp-ffi C ABI declarations ─────────────────────────────────────────────

// `nmp_app_dispatch_action` is #[no_mangle] extern "C" in nmp-ffi/src/action.rs.
// Declared here so the comments effect runner can call it directly without
// importing the full nmp-ffi action surface.
#[allow(improper_ctypes)] // NmpApp is opaque; the pointer is safe — nmp-ffi uses the same ABI.
extern "C" {
    fn nmp_app_dispatch_action(
        app: *mut NmpApp,
        namespace: *const c_char,
        action_json: *const c_char,
    ) -> *mut c_char;
}

use nmp_ffi::nmp_free_string;

// ─── READ side: KernelEventObserver wrapper ──────────────────────────────────

/// Observer wrapper that ingests NMP `KernelEvent`s (raw Nostr events) into
/// the `CommentThreadProjection` and sends `KernelEvent::CommentThreadUpdated`
/// back into the actor channel for each affected `root_tag_value`.
///
/// This is the Family-B integration pattern (same as `ReactionObserver` in
/// reactions.rs). On each kind:1111 event the observer:
/// 1. Delegates ingest to the underlying `CommentThreadProjection`.
/// 2. Extracts `root_tag_value` via `nmp_nip22::try_from_kernel_event`.
/// 3. Calls `projection.snapshot_for(root_tag_value)` and sends
///    `KernelEvent::CommentThreadUpdated` into the actor channel.
///
/// D6: malformed kind:1111 events (no valid root scope tag) → silent no-op.
struct CommentObserver {
    projection: Arc<CommentThreadProjection>,
    tx: mpsc::UnboundedSender<Cmd>,
}

impl KernelEventObserver for CommentObserver {
    fn on_kernel_event(&self, event: &nmp_core::substrate::KernelEvent) {
        if event.kind != KIND_COMMENT {
            return;
        }
        // Delegate ingest to the projection first (updates internal entries map).
        self.projection.on_kernel_event(event);

        // D6: if the event cannot be decoded as a NIP-22 comment, return silently.
        let record = match nmp_nip22::try_from_kernel_event(event) {
            Some(r) => r,
            None => return,
        };

        let root_tag_value = record.root_tag_value.clone();

        // Snapshot the updated thread for this root and push to actor.
        let snapshot = self.projection.snapshot_for(&root_tag_value);
        let _ = self.tx.send(Cmd::Event(KernelEvent::CommentThreadUpdated {
            root_tag_value,
            snapshot,
        }));
    }
}

// ─── State event handler (called from reduce_event in actor.rs) ──────────────

/// Apply a `KernelEvent::CommentThreadUpdated` to `AppState::comment_threads`.
///
/// Stores the full `CommentThreadSnapshot` keyed by `root_tag_value`. The
/// snapshot may then be projected to `CommentThreadKernelSnapshot` by
/// `compute_comment_thread_snapshot` when a `ViewId::CommentThread` is open.
///
/// D1: stores raw projection data only — no formatted strings, no counts.
pub(crate) fn reduce_event_comment_thread_updated(
    state: &mut AppState,
    root_tag_value: String,
    snapshot: CommentThreadSnapshot,
) -> Vec<crate::kernel::effect::Effect> {
    state.comment_threads.insert(root_tag_value, snapshot);
    vec![]
}

// ─── WRITE side: reduce_action helper ────────────────────────────────────────

/// Handle the `hl.comment.post` envelope action.
///
/// Serialises the NIP-22 `PostCommentAction` payload via `serde_json` and emits
/// `Effect::DispatchCommentAction` with namespace `"nmp.nip22.post_comment"`.
///
/// D6 guards:
/// - Empty `root_tag_value` (trimmed) → return `vec![]` (no-op).
/// - Empty `content` (trimmed) → return `vec![]` (no-op).
///
/// The kernel does NOT speculatively update `AppState::comment_threads`
/// (optimistic UI lives in Swift — D1). The authoritative thread arrives back
/// via `KernelEvent::CommentThreadUpdated` once the `CommentObserver` fires.
pub(crate) fn reduce_action_post_comment(
    payload: PostCommentPayload,
) -> Vec<crate::kernel::effect::Effect> {
    // D6: empty anchor or content → no-op.
    if payload.root_tag_value.trim().is_empty() || payload.content.trim().is_empty() {
        return vec![];
    }

    // Build serde wire shape matching `nmp_nip22::PostCommentAction`.
    // Use serde_json::json! (never format!) for safe serialisation.
    let mut json_map = serde_json::json!({
        "root_tag_name": payload.root_tag_name,
        "root_tag_value": payload.root_tag_value,
        "root_kind": payload.root_kind,
        "content": payload.content,
    });

    if let Some(parent_id) = payload.parent_event_id {
        json_map["parent_event_id"] = serde_json::Value::String(parent_id);
    }
    if let Some(root_author) = payload.root_author_pubkey {
        json_map["root_author_pubkey"] = serde_json::Value::String(root_author);
    }
    if let Some(parent_author) = payload.parent_author_pubkey {
        json_map["parent_author_pubkey"] = serde_json::Value::String(parent_author);
    }

    let json = match serde_json::to_string(&json_map) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "comments::reduce_action_post_comment: serde error — no effect emitted");
            return vec![];
        }
    };

    vec![crate::kernel::effect::Effect::DispatchCommentAction { json }]
}

// ─── Effect runner ───────────────────────────────────────────────────────────

/// Execute `Effect::DispatchCommentAction` — calls `nmp_app_dispatch_action`
/// with namespace `"nmp.nip22.post_comment"` and the serialised JSON payload.
///
/// Fire-and-forget (D6, Non-Negotiable #3): the returned correlation_id JSON
/// string is freed and discarded. The authoritative comment thread arrives back
/// via `KernelEvent::CommentThreadUpdated` from the `CommentObserver` on the
/// next kind:1111 event.
///
/// No-op if `nmp` is `None` (test mode — tests inject `CommentThreadUpdated`
/// directly to drive the reducer).
pub(crate) fn run_effect_dispatch_comment_action(
    json: String,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
) {
    let Some(handle) = nmp else { return };

    let namespace = "nmp.nip22.post_comment";
    let ns_c = match CString::new(namespace) {
        Ok(s) => s,
        Err(_) => return,
    };
    let json_c = match CString::new(json) {
        Ok(s) => s,
        Err(_) => return,
    };

    // SAFETY: handle.ptr is a valid non-null NmpApp pointer kept alive by
    // NmpHandle for the full actor lifetime. ns_c and json_c are valid
    // CStrings alive for the duration of this call. The returned pointer is
    // freed below via nmp_free_string (same Rust allocator as the allocation).
    let result_ptr =
        unsafe { nmp_app_dispatch_action(handle.ptr.as_ptr(), ns_c.as_ptr(), json_c.as_ptr()) };

    // Free the returned correlation-id JSON string (same Rust allocator path).
    if !result_ptr.is_null() {
        nmp_free_string(result_ptr);
    }
}

// ─── Snapshot computation ─────────────────────────────────────────────────────

/// Compute a `CommentThreadKernelSnapshot` for the given `root_tag_value`.
///
/// Reads `AppState::comment_threads` to find the latest projection snapshot,
/// then converts `CommentRecord` rows to `CommentRecordRow` (flat raw list).
///
/// D1: `CommentRecordRow` carries raw protocol data only — no formatted strings,
/// no tree nesting in the snapshot. Swift builds the display tree from
/// `parent_tag_value` relationships. `comment_count` = `records.len() as u32`.
///
/// Returns an empty snapshot (zero records) when no data is present yet —
/// never panics (D6).
pub(crate) fn compute_comment_thread_snapshot(
    state: &AppState,
    root_tag_value: &str,
) -> CommentThreadKernelSnapshot {
    let records = state
        .comment_threads
        .get(root_tag_value)
        .map(|s| {
            s.records
                .iter()
                .map(|r| CommentRecordRow {
                    event_id: r.event_id.clone(),
                    author_pubkey: r.author_pubkey.clone(),
                    body: r.body.clone(),
                    root_tag_name: r.root_tag_name.clone(),
                    root_tag_value: r.root_tag_value.clone(),
                    root_kind: r.root_kind.clone(),
                    parent_tag_name: r.parent_tag_name.clone(),
                    parent_tag_value: r.parent_tag_value.clone(),
                    parent_kind: r.parent_kind.clone(),
                    created_at: r.created_at,
                    is_top_level: r.is_top_level(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let comment_count = records.len() as u32;

    CommentThreadKernelSnapshot {
        root_tag_value: root_tag_value.to_string(),
        records,
        comment_count,
    }
}

// ─── Projection registration ─────────────────────────────────────────────────

/// Wire a fresh `CommentObserver` (wrapping a new `CommentThreadProjection`) as
/// a `KernelEventObserver` against `nmp_ref`.
///
/// This creates a SECOND `CommentThreadProjection` (separate from the one
/// registered by `nmp-defaults::register_defaults`). Double-observation is
/// harmless — both projections read the same kind:1111 events. The write action
/// (`nmp.nip22.post_comment`) is NOT re-registered here; nmp-defaults already
/// wired it via `register_defaults`. This registration is purely for the hl
/// snapshot path (`KernelEvent::CommentThreadUpdated` → `AppState::comment_threads`).
///
/// Called ONCE at boot from `start_nmp_app` (after `nmp_app_start`). No re-
/// registration on `IdentityChanged(Some)` is needed — comments are keyed by
/// `root_tag_value` (content address), not by account identity.
///
/// D6: if `register_event_observer` returns id `0` (slot full), the observer is
/// silently dropped and comment threads will not update (logged as a warning).
pub(crate) fn register_comment_projection(nmp_ref: &NmpApp, tx: mpsc::UnboundedSender<Cmd>) {
    let projection = Arc::new(CommentThreadProjection::new());
    let observer = Arc::new(CommentObserver { projection, tx });

    let observer_id = nmp_ref.register_event_observer(observer as Arc<dyn KernelEventObserver>);
    if observer_id.0 == 0 {
        // Observer slot is full or poisoned — comment threads will not update.
        tracing::warn!(
            "comments::register_comment_projection: event-observer registration failed (D6)"
        );
    }
    // No typed-snapshot registration needed: CommentObserver posts
    // KernelEvent::CommentThreadUpdated directly into the actor channel.
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

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

    /// Build a minimal `CommentThreadSnapshot` for test injection.
    fn make_snapshot(
        root_tag_value: &str,
        records: Vec<nmp_nip22::CommentRecord>,
    ) -> CommentThreadSnapshot {
        let tree = nmp_nip22::build_thread(&records, root_tag_value);
        CommentThreadSnapshot {
            root_tag_value: root_tag_value.to_string(),
            records,
            tree,
        }
    }

    fn make_record(event_id: &str, root_val: &str, parent_val: &str) -> nmp_nip22::CommentRecord {
        nmp_nip22::CommentRecord {
            event_id: event_id.to_string(),
            author_pubkey: "aaaa000000000000000000000000000000000000000000000000000000000001"
                .to_string(),
            body: "test body".to_string(),
            root_tag_name: "E".to_string(),
            root_tag_value: root_val.to_string(),
            root_kind: "1".to_string(),
            parent_tag_name: "e".to_string(),
            parent_tag_value: parent_val.to_string(),
            parent_kind: "1".to_string(),
            created_at: 1_000_000,
        }
    }

    // 7-T1: comment_thread_from_projection_for_root
    //
    // Injecting KernelEvent::CommentThreadUpdated with records must upsert
    // AppState::comment_threads keyed by root_tag_value. Snapshot must contain
    // the correct records.
    #[test]
    fn comment_thread_from_projection_for_root() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let root = "deadbeef00000000000000000000000000000000000000000000000000000001";

        assert!(
            state.comment_threads.is_empty(),
            "comment_threads must start empty"
        );

        let record = make_record(
            "cccc000000000000000000000000000000000000000000000000000000000001",
            root,
            root, // top-level: parent == root
        );
        let snapshot = make_snapshot(root, vec![record.clone()]);

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::CommentThreadUpdated {
                root_tag_value: root.to_string(),
                snapshot,
            }),
        );

        let stored = state
            .comment_threads
            .get(root)
            .expect("comment thread must be present after update");
        assert_eq!(stored.records.len(), 1, "one record stored");
        assert_eq!(
            stored.records[0].event_id, record.event_id,
            "event_id matches"
        );
        assert_eq!(stored.root_tag_value, root, "root_tag_value matches");
    }

    // 7-T2: post_comment_dispatches_nip22_with_correct_payload
    //
    // hl.comment.post must produce exactly one Effect::DispatchCommentAction
    // with a serde-valid JSON payload containing all PostCommentAction fields.
    #[test]
    fn post_comment_dispatches_nip22_with_correct_payload() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let root = "deadbeef00000000000000000000000000000000000000000000000000000001";

        let payload = serde_json::json!({
            "root_tag_name": "E",
            "root_tag_value": root,
            "root_kind": 1u32,
            "content": "hello world",
        });
        let envelope = crate::kernel::action::AppActionEnvelope {
            namespace: "hl.comment.post".to_string(),
            json: serde_json::to_string(&payload).unwrap(),
        };

        let effects = step(&mut state, &clock, Cmd::ActionEnvelope(envelope));

        assert_eq!(effects.len(), 1, "must emit exactly one effect");
        match &effects[0] {
            Effect::DispatchCommentAction { json } => {
                let parsed: serde_json::Value =
                    serde_json::from_str(json).expect("payload must be valid JSON");
                assert_eq!(parsed["root_tag_name"].as_str().unwrap(), "E");
                assert_eq!(parsed["root_tag_value"].as_str().unwrap(), root);
                assert_eq!(parsed["root_kind"].as_u64().unwrap(), 1);
                assert_eq!(parsed["content"].as_str().unwrap(), "hello world");
            }
            _ => panic!("expected DispatchCommentAction"),
        }
    }

    // 7-T3: comment_count_from_thread_snapshot
    //
    // After CommentThreadUpdated, compute_comment_thread_snapshot returns correct
    // comment_count.
    #[test]
    fn comment_count_from_thread_snapshot() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let root = "deadbeef00000000000000000000000000000000000000000000000000000002";

        let r1 = make_record(
            "aaaa000000000000000000000000000000000000000000000000000000000001",
            root,
            root,
        );
        let r2 = make_record(
            "bbbb000000000000000000000000000000000000000000000000000000000002",
            root,
            root,
        );
        let snapshot = make_snapshot(root, vec![r1, r2]);

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::CommentThreadUpdated {
                root_tag_value: root.to_string(),
                snapshot,
            }),
        );

        let kernel_snapshot = compute_comment_thread_snapshot(&state, root);
        assert_eq!(kernel_snapshot.comment_count, 2, "comment_count must be 2");
        assert_eq!(kernel_snapshot.records.len(), 2, "two records in flat list");
    }

    // 7-T4: reply_sets_correct_parent_scope
    //
    // Posting with parent_event_id=Some(...) includes it in the JSON payload.
    #[test]
    fn reply_sets_correct_parent_scope() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let root = "deadbeef00000000000000000000000000000000000000000000000000000001";
        let parent_id = "1111000000000000000000000000000000000000000000000000000000000001";

        let payload = serde_json::json!({
            "root_tag_name": "E",
            "root_tag_value": root,
            "root_kind": 1u32,
            "parent_event_id": parent_id,
            "content": "a reply",
        });
        let envelope = crate::kernel::action::AppActionEnvelope {
            namespace: "hl.comment.post".to_string(),
            json: serde_json::to_string(&payload).unwrap(),
        };

        let effects = step(&mut state, &clock, Cmd::ActionEnvelope(envelope));

        assert_eq!(effects.len(), 1);
        match &effects[0] {
            Effect::DispatchCommentAction { json } => {
                let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
                assert_eq!(
                    parsed["parent_event_id"].as_str().unwrap(),
                    parent_id,
                    "parent_event_id must be in payload"
                );
            }
            _ => panic!("expected DispatchCommentAction"),
        }
    }

    // 7-T5: comment_snapshot_raw_no_formatting
    //
    // CommentRecordRow must carry only raw protocol data (no formatted strings).
    #[test]
    fn comment_snapshot_raw_no_formatting() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let root = "deadbeef00000000000000000000000000000000000000000000000000000003";
        let author = "aaaa000000000000000000000000000000000000000000000000000000000001";

        let record = nmp_nip22::CommentRecord {
            event_id: "cccc000000000000000000000000000000000000000000000000000000000003"
                .to_string(),
            author_pubkey: author.to_string(),
            body: "raw comment body".to_string(),
            root_tag_name: "E".to_string(),
            root_tag_value: root.to_string(),
            root_kind: "30023".to_string(),
            parent_tag_name: "e".to_string(),
            parent_tag_value: root.to_string(),
            parent_kind: "30023".to_string(),
            created_at: 1_700_000_000,
        };
        let snapshot = make_snapshot(root, vec![record]);

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::CommentThreadUpdated {
                root_tag_value: root.to_string(),
                snapshot,
            }),
        );

        let kernel_snapshot = compute_comment_thread_snapshot(&state, root);
        assert_eq!(kernel_snapshot.records.len(), 1);
        let row = &kernel_snapshot.records[0];

        // D1: check raw fields are present without formatting
        assert_eq!(row.author_pubkey, author, "author_pubkey must be raw hex");
        assert_eq!(row.body, "raw comment body", "body must be raw");
        assert_eq!(
            row.created_at, 1_700_000_000,
            "created_at must be raw unix seconds"
        );

        // Check no formatted strings are present
        let debug_str = format!("{:?}", row);
        assert!(
            !debug_str.contains(" ago"),
            "CommentRecordRow must not contain formatted relative time"
        );
        assert!(
            !debug_str.contains("hours"),
            "CommentRecordRow must not contain formatted time labels"
        );
    }

    // 7-T6: malformed_no_anchor_no_op
    //
    // Posting with empty root_tag_value returns empty effects (D6).
    #[test]
    fn malformed_no_anchor_no_op() {
        let mut state = make_state();
        let clock = ManualClock::default();

        let payload = serde_json::json!({
            "root_tag_name": "E",
            "root_tag_value": "",  // empty — must be no-op
            "root_kind": 1u32,
            "content": "some comment",
        });
        let envelope = crate::kernel::action::AppActionEnvelope {
            namespace: "hl.comment.post".to_string(),
            json: serde_json::to_string(&payload).unwrap(),
        };

        let effects = step(&mut state, &clock, Cmd::ActionEnvelope(envelope));

        assert!(
            effects.is_empty(),
            "empty root_tag_value must produce no effects (D6)"
        );
    }

    // 7-T7: top_level_comment_is_top_level_true
    //
    // A record with parent_tag_value == root_tag_value must have is_top_level=true
    // in the CommentRecordRow.
    #[test]
    fn top_level_comment_is_top_level_true() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let root = "deadbeef00000000000000000000000000000000000000000000000000000004";

        // Top-level: parent_tag_value == root_tag_value
        let top_level_record = make_record(
            "aaaa000000000000000000000000000000000000000000000000000000000001",
            root,
            root, // parent == root → top-level
        );
        // Reply: parent_tag_value != root_tag_value
        let reply_record = nmp_nip22::CommentRecord {
            event_id: "bbbb000000000000000000000000000000000000000000000000000000000002"
                .to_string(),
            author_pubkey: "aaaa000000000000000000000000000000000000000000000000000000000001"
                .to_string(),
            body: "reply".to_string(),
            root_tag_name: "E".to_string(),
            root_tag_value: root.to_string(),
            root_kind: "1".to_string(),
            parent_tag_name: "e".to_string(),
            // parent != root → this is a reply
            parent_tag_value: "aaaa000000000000000000000000000000000000000000000000000000000001"
                .to_string(),
            parent_kind: "1111".to_string(),
            created_at: 1_000_001,
        };

        let snapshot = make_snapshot(root, vec![top_level_record, reply_record]);

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::CommentThreadUpdated {
                root_tag_value: root.to_string(),
                snapshot,
            }),
        );

        let kernel_snapshot = compute_comment_thread_snapshot(&state, root);
        let top = kernel_snapshot
            .records
            .iter()
            .find(|r| r.parent_tag_value == root)
            .expect("top-level record must be in snapshot");
        assert!(
            top.is_top_level,
            "top-level comment must have is_top_level=true"
        );

        let reply = kernel_snapshot
            .records
            .iter()
            .find(|r| r.parent_tag_value != root)
            .expect("reply record must be in snapshot");
        assert!(
            !reply.is_top_level,
            "reply comment must have is_top_level=false"
        );
    }

    // 7-T8: dispatch_comment_returns_unit
    //
    // Fire-and-forget contract (returns Vec<Effect>, no panic).
    #[test]
    fn dispatch_comment_returns_unit() {
        let mut state = make_state();
        let clock = ManualClock::new(0);
        let root = "deadbeef00000000000000000000000000000000000000000000000000000005";

        let payload = serde_json::json!({
            "root_tag_name": "E",
            "root_tag_value": root,
            "root_kind": 1u32,
            "content": "fire and forget comment",
        });
        let envelope = crate::kernel::action::AppActionEnvelope {
            namespace: "hl.comment.post".to_string(),
            json: serde_json::to_string(&payload).unwrap(),
        };

        let _effects: Vec<Effect> = step(&mut state, &clock, Cmd::ActionEnvelope(envelope));
        // No panic, no Result — fire-and-forget contract satisfied.
    }

    // 7-T9: observer_sends_comment_thread_updated_on_kind1111
    //
    // CommentObserver::on_kernel_event must send KernelEvent::CommentThreadUpdated
    // into the channel when a kind:1111 event with a root E tag is ingested.
    #[test]
    fn observer_sends_comment_thread_updated_on_kind1111() {
        let (tx, mut rx) = mpsc::unbounded_channel::<Cmd>();
        let projection = Arc::new(CommentThreadProjection::new());
        let observer = CommentObserver { projection, tx };

        let root = "deadbeef00000000000000000000000000000000000000000000000000000006";
        let event = nmp_core::substrate::KernelEvent {
            id: "cccc000000000000000000000000000000000000000000000000000000000006".to_string(),
            author: "aaaa000000000000000000000000000000000000000000000000000000000001".to_string(),
            kind: KIND_COMMENT,
            created_at: 1_000_000,
            tags: vec![
                vec!["E".to_string(), root.to_string()],
                vec!["K".to_string(), "1".to_string()],
                vec!["e".to_string(), root.to_string()],
                vec!["k".to_string(), "1".to_string()],
            ],
            content: "test comment".to_string(),
            relay_provenance: vec![],
        };

        observer.on_kernel_event(&event);

        let cmd = rx.try_recv().expect("CommentThreadUpdated must be sent");
        match cmd {
            Cmd::Event(KernelEvent::CommentThreadUpdated {
                root_tag_value,
                snapshot,
            }) => {
                assert_eq!(root_tag_value, root, "root_tag_value must match E tag");
                assert_eq!(snapshot.records.len(), 1, "one record in snapshot");
                assert_eq!(snapshot.records[0].body, "test comment");
            }
            _ => panic!("expected CommentThreadUpdated from channel"),
        }
    }

    // 7-T10: observer_noop_on_missing_root_tag (D6)
    //
    // A kind:1111 event without a valid root scope tag is malformed per NIP-22.
    // The observer must not panic or send a command — silent no-op (D6).
    #[test]
    fn observer_noop_on_missing_root_tag() {
        let (tx, mut rx) = mpsc::unbounded_channel::<Cmd>();
        let projection = Arc::new(CommentThreadProjection::new());
        let observer = CommentObserver { projection, tx };

        let event = nmp_core::substrate::KernelEvent {
            id: "cccc000000000000000000000000000000000000000000000000000000000007".to_string(),
            author: "aaaa000000000000000000000000000000000000000000000000000000000001".to_string(),
            kind: KIND_COMMENT,
            created_at: 1_000_000,
            tags: vec![], // No root scope tag — malformed
            content: "orphaned comment".to_string(),
            relay_provenance: vec![],
        };

        observer.on_kernel_event(&event);

        assert!(
            rx.try_recv().is_err(),
            "malformed kind:1111 (no root tag) must not send any command (D6)"
        );
    }
}
