//! Reactions domain — NIP-25 reaction projection + write actions (slice 4B).
//!
//! ## Responsibilities
//!
//! * **READ** — register a `ReactionObserver` wrapper around `ReactionProjection`
//!   (nmp-nip25) as a `KernelEventObserver`. On each kind:7/kind:5 event the
//!   observer (a) updates the underlying `ReactionProjection` by delegating to
//!   its own `on_kernel_event` impl, then (b) extracts the `["e", target_event_id]`
//!   tag and calls `projection.snapshot_for(target_event_id)` to produce a
//!   `ReactionSnapshot`, and (c) sends `KernelEvent::ReactionStateUpdated` back
//!   into the actor channel. The actor's `reduce_event` arm stores the update in
//!   `AppState::reaction_state` keyed by `target_event_id`.
//!
//!   This is the correct Family-B integration pattern — the observer→actor-channel
//!   path. The typed-snapshot-projection path (Family-A, FlatBuffers schema_id)
//!   is NOT used here because `ReactionProjection` has no FlatBuffers encoding
//!   and `snapshot_for` is per-target with no public entry iterator.
//!
//! * **WRITE** — `AppAction::React{target_event_id, reaction, target_author_pubkey?}`
//!   → reducer emits `Effect::DispatchReactAction{namespace:"nmp.nip25.react", json}`
//!   (serde_json payload) → effect runner calls `nmp_app_dispatch_action` fire-and-forget.
//!   `AppAction::Unreact{reaction_event_id}` → `"nmp.nip25.unreact"`.
//!
//!   The kernel is the **sole kind:7 writer** for ported screens (no live-lane
//!   double-publish for reactions on articles/highlights/artifacts).
//!
//! ## Wire registration
//!
//! `register_reaction_projection(nmp_ref, viewer_pubkey, tx)` is called at boot
//! (after `nmp_app_start`) from `start_nmp_app` and re-called on every
//! `Effect::WireReactionProjection` (emitted on `IdentityChanged(Some)`) so the
//! viewer pubkey tracks the active account.
//!
//! ## Threading
//!
//! `ReactionObserver::on_kernel_event` is called from the NMP event-dispatch
//! thread (same thread as `FollowListProjection::on_kernel_event`). It:
//! 1. Acquires a Mutex lock on the `ReactionProjection` internal entries (inside
//!    the nmp-nip25 projection). Lock is brief (ingest + snapshot of one entry).
//! 2. Sends `Cmd::Event(KernelEvent::ReactionStateUpdated)` into the
//!    `UnboundedSender` — a non-blocking channel send. D8 compliant: no blocking.
//!
//! `AppState::reaction_state` is only mutated by the actor thread (inside
//!  `reduce_event` — Non-Negotiable #2).
//!
//! ## D-rules satisfied
//!
//! * D1 — `ReactionRow` carries raw `count: u32` + `viewer_reacted: bool` only.
//!   No `"X likes"` label, no formatted count string, no optimistic delta.
//!   Swift owns all optimistic UI state (count increment before the projection
//!   fires, toggle animation, accessibility label).
//! * D6 — missing/malformed tags in kind:7 events → silent no-op (no tag with
//!   key `"e"` means no `target_event_id` to snapshot, so no event is sent).
//! * Non-Negotiable #3 — `React`/`Unreact` reduce to `Vec<Effect>` (never
//!   `Result`); fire-and-forget.

use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::Arc;

use nmp_core::KernelEventObserver;
use nmp_ffi::NmpApp;
use nmp_nip25::{ReactionProjection, KIND_REACTION, KIND_REACTION_DELETE};
use tokio::sync::mpsc;

use crate::kernel::action::KernelEvent;
use crate::kernel::actor::Cmd;
use crate::kernel::app::AppState;

// ─── nmp-ffi C ABI declarations ─────────────────────────────────────────────

// `nmp_app_dispatch_action` is #[no_mangle] extern "C" in nmp-ffi/src/action.rs.
// Declared here so the reactions effect runner can call it directly without
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
/// the `ReactionProjection` and then sends `KernelEvent::ReactionStateUpdated`
/// back into the actor channel for each affected `target_event_id`.
///
/// This is the correct Family-B integration pattern: the observer receives
/// raw kind:7/kind:5 events, delegates ingest to the underlying
/// `ReactionProjection`, then immediately snapshots the affected target and
/// sends the result to the actor channel. No typed-snapshot registration is
/// used because `snapshot_for` is per-target and `ReactionProjection` has no
/// public entry iterator at `b4404159`.
struct ReactionObserver {
    projection: Arc<ReactionProjection>,
    tx: mpsc::UnboundedSender<Cmd>,
}

impl KernelEventObserver for ReactionObserver {
    fn on_kernel_event(&self, event: &nmp_core::substrate::KernelEvent) {
        // Only process reaction kinds (7 = react, 5 = kind:5 delete).
        if event.kind != KIND_REACTION && event.kind != KIND_REACTION_DELETE {
            return;
        }

        // Delegate to the ReactionProjection to update its internal state.
        // This call acquires and releases the projection's internal Mutex.
        self.projection.on_kernel_event(event);

        // Extract the target_event_id from the ["e", _] tag.
        // D6: a kind:7 without an "e" tag is malformed — silent no-op.
        let target_event_id = match first_tag_value(&event.tags, "e") {
            Some(id) => id,
            None => return,
        };

        // Snapshot the updated state for this target.
        let snapshot = self.projection.snapshot_for(&target_event_id);

        // Compute derived fields. D1: raw count + viewer_reacted bool only.
        // `snapshot.reactions` is the authoritative list from the projection.
        let count = snapshot.reactions.len() as u32;
        let viewer_reacted = snapshot.viewer_reaction.is_some();

        // Send the updated state to the actor channel. Fire-and-forget (D6):
        // if the actor has shut down the send error is silently dropped.
        let _ = self.tx.send(Cmd::Event(KernelEvent::ReactionStateUpdated {
            target_event_id,
            count,
            viewer_reacted,
        }));
    }
}

/// Extract the first `value` from tags where the first element equals `name`.
/// Returns `None` if no matching tag exists or the value is empty.
fn first_tag_value(tags: &[Vec<String>], name: &str) -> Option<String> {
    tags.iter().find_map(|tag| {
        if tag.first().is_some_and(|n| n == name) {
            tag.get(1).filter(|v| !v.is_empty()).cloned()
        } else {
            None
        }
    })
}

// ─── State clear on identity loss ────────────────────────────────────────────

/// Clear `reaction_state` from `AppState` when the active account is lost
/// (Logout or IdentityChanged(None)).
///
/// Called from `auth::reduce_action_logout` and the `None` arm of
/// `auth::reduce_event_identity_changed` so stale reaction counts from a prior
/// account never surface under a new identity.
pub(crate) fn clear_on_identity_lost(state: &mut AppState) {
    state.reaction_state.clear();
}

// ─── WRITE side: reduce_action helpers ──────────────────────────────────────

/// Handle `AppAction::React{target_event_id, reaction, target_author_pubkey?}`.
///
/// Serialises the NIP-25 `ReactAction` payload via `serde_json` and emits
/// `Effect::DispatchReactAction` with namespace `"nmp.nip25.react"`.
///
/// The kernel does NOT speculatively update `AppState::reaction_state`
/// (optimistic UI lives in Swift — D1). The authoritative count arrives
/// back via `KernelEvent::ReactionStateUpdated` once the
/// `ReactionObserver` fires (D6).
pub(crate) fn reduce_action_react(
    target_event_id: String,
    reaction: String,
    target_author_pubkey: Option<String>,
) -> Vec<crate::kernel::effect::Effect> {
    // Build the serde wire shape matching `nmp_nip25::ReactAction`.
    // Use serde_json::json! (never format!) for safe serialisation.
    let payload = if let Some(author) = target_author_pubkey {
        serde_json::json!({
            "target_event_id": target_event_id,
            "reaction": reaction,
            "target_author_pubkey": author,
        })
    } else {
        serde_json::json!({
            "target_event_id": target_event_id,
            "reaction": reaction,
        })
    };
    let json = match serde_json::to_string(&payload) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "reactions::reduce_action_react: serde error — no effect emitted");
            return vec![];
        }
    };
    vec![crate::kernel::effect::Effect::DispatchReactAction {
        namespace: "nmp.nip25.react".to_string(),
        json,
    }]
}

/// Handle `AppAction::Unreact{reaction_event_id}`.
///
/// Serialises the NIP-25 `UnreactAction` payload via `serde_json` and emits
/// `Effect::DispatchReactAction` with namespace `"nmp.nip25.unreact"`.
/// Fire-and-forget (D6). The reaction count correction arrives via
/// `KernelEvent::ReactionStateUpdated` on the next projection tick.
pub(crate) fn reduce_action_unreact(
    reaction_event_id: String,
) -> Vec<crate::kernel::effect::Effect> {
    // Wire shape matches `nmp_nip25::UnreactAction { reaction_event_id, reason }`.
    // `reason` is `pub reason: String` with `#[serde(default)]` — empty string
    // is the correct zero value (not None; the field is not Option<String>).
    let payload = serde_json::json!({
        "reaction_event_id": reaction_event_id,
        "reason": "",
    });
    let json = match serde_json::to_string(&payload) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "reactions::reduce_action_unreact: serde error — no effect emitted");
            return vec![];
        }
    };
    vec![crate::kernel::effect::Effect::DispatchReactAction {
        namespace: "nmp.nip25.unreact".to_string(),
        json,
    }]
}

// ─── Effect runner ───────────────────────────────────────────────────────────

/// Execute `Effect::DispatchReactAction` — calls `nmp_app_dispatch_action`
/// with the given `namespace` (`"nmp.nip25.react"` or `"nmp.nip25.unreact"`)
/// and the serialised JSON payload.
///
/// Fire-and-forget (D6, Non-Negotiable #3): the returned correlation_id JSON
/// string is freed and discarded. The authoritative reaction state arrives back
/// via `KernelEvent::ReactionStateUpdated` from the `ReactionObserver` on the
/// next kind:7/kind:5 event.
///
/// No-op if `nmp` is `None` (test mode — tests inject `ReactionStateUpdated`
/// directly to drive the reducer).
pub(crate) fn run_effect_dispatch_react_action(
    namespace: String,
    json: String,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
) {
    let Some(handle) = nmp else { return };

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
    // A null pointer is a no-op (nmp-ffi D6 null-safety contract), but guard
    // explicitly to document the non-null path.
    if !result_ptr.is_null() {
        nmp_free_string(result_ptr);
    }
}

// ─── Projection registration ─────────────────────────────────────────────────

/// Wire the `ReactionObserver` (wrapping `ReactionProjection`) as a
/// `KernelEventObserver` against `nmp_ref`.
///
/// On each ingested kind:7 or kind:5 event the observer:
/// 1. Updates the underlying `ReactionProjection` state.
/// 2. Extracts the `["e", target_event_id]` tag.
/// 3. Calls `projection.snapshot_for(target_event_id)` to get the fresh count.
/// 4. Sends `KernelEvent::ReactionStateUpdated` into the actor channel `tx`.
///
/// `viewer_pubkey` is the active account's hex pubkey (or `None` at boot
/// before the first `IdentityChanged(Some)` fires). Pass the current active
/// pubkey from `nmp_ref.active_account_handle()` so `viewer_reacted` reflects
/// the signed-in user.
///
/// ## When to call
///
/// Called once at boot from `start_nmp_app` (after `nmp_app_start`). Also
/// re-called via `Effect::WireReactionProjection` on every
/// `IdentityChanged(Some)` so the `viewer_pubkey` tracks the active account.
/// A fresh `ReactionProjection` is created on each call; prior observations
/// are discarded (consistent with the follows/joined-groups pattern).
///
/// D6: if `register_event_observer` returns id `0` (slot full), the observer
/// is silently dropped and reaction state will not update (logged as a warning).
pub(crate) fn register_reaction_projection(
    nmp_ref: &NmpApp,
    viewer_pubkey: Option<String>,
    tx: mpsc::UnboundedSender<Cmd>,
) {
    let projection = Arc::new(ReactionProjection::new(viewer_pubkey));
    let observer = Arc::new(ReactionObserver { projection, tx });

    let observer_id = nmp_ref.register_event_observer(observer as Arc<dyn KernelEventObserver>);
    if observer_id.0 == 0 {
        // Observer slot is full or poisoned — reaction counts will not update.
        tracing::warn!(
            "reactions::register_reaction_projection: event-observer registration failed (D6)"
        );
    }
    // No typed-snapshot registration needed: the ReactionObserver posts
    // KernelEvent::ReactionStateUpdated directly into the actor channel.
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::{AppAction, KernelEvent};
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::effect::Effect;
    use crate::kernel::snapshot::ReactionRow;

    fn make_state() -> AppState {
        AppState::default()
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    // 4B-T1: reaction_frame_updates_state_raw_count_and_viewer
    //
    // Injecting KernelEvent::ReactionStateUpdated must upsert
    // AppState::reaction_state with raw count + viewer_reacted bool.
    // No formatted strings must appear in the stored row (D1).
    #[test]
    fn reaction_frame_updates_state_raw_count_and_viewer() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let target = "deadbeef00000000000000000000000000000000000000000000000000000001";

        assert!(
            state.reaction_state.is_empty(),
            "reaction_state must start empty"
        );

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::ReactionStateUpdated {
                target_event_id: target.to_string(),
                count: 42,
                viewer_reacted: true,
            }),
        );

        let row = state
            .reaction_state
            .get(target)
            .expect("reaction row must be present after update");
        assert_eq!(row.count, 42, "count must be stored raw (no formatting)");
        assert!(row.viewer_reacted, "viewer_reacted must be true");
        assert_eq!(
            row.target_event_id, target,
            "target_event_id must thread through"
        );
    }

    // 4B-T2: react_dispatches_nip25_react_with_serde_payload
    //
    // AppAction::React must produce exactly one Effect::DispatchReactAction with
    // namespace "nmp.nip25.react" and a serde-valid JSON payload containing
    // target_event_id and reaction fields.
    #[test]
    fn react_dispatches_nip25_react_with_serde_payload() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let target = "cafebabe00000000000000000000000000000000000000000000000000000001";

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::React {
                target_event_id: target.to_string(),
                reaction: "+".to_string(),
                target_author_pubkey: None,
            }),
        );

        assert_eq!(effects.len(), 1, "React must emit exactly one effect");
        match &effects[0] {
            Effect::DispatchReactAction { namespace, json } => {
                assert_eq!(
                    namespace, "nmp.nip25.react",
                    "namespace must match nmp.nip25.react"
                );
                // Validate JSON payload parses and contains required fields.
                let parsed: serde_json::Value =
                    serde_json::from_str(json).expect("payload must be valid JSON");
                assert_eq!(
                    parsed["target_event_id"].as_str().unwrap(),
                    target,
                    "target_event_id must be in payload"
                );
                assert_eq!(
                    parsed["reaction"].as_str().unwrap(),
                    "+",
                    "reaction must be in payload"
                );
                // target_author_pubkey must be absent (omitted, not null) when None.
                assert!(
                    parsed.get("target_author_pubkey").is_none(),
                    "target_author_pubkey must be absent (not null) when not supplied"
                );
            }
            _ => panic!("expected DispatchReactAction"),
        }
    }

    // 4B-T3: react_with_author_includes_target_author_pubkey
    //
    // When target_author_pubkey is supplied it must appear in the JSON payload.
    #[test]
    fn react_with_author_includes_target_author_pubkey() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let target = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let author = "bbbb000000000000000000000000000000000000000000000000000000000002";

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::React {
                target_event_id: target.to_string(),
                reaction: "+".to_string(),
                target_author_pubkey: Some(author.to_string()),
            }),
        );

        assert_eq!(effects.len(), 1);
        match &effects[0] {
            Effect::DispatchReactAction { json, .. } => {
                let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
                assert_eq!(
                    parsed["target_author_pubkey"].as_str().unwrap(),
                    author,
                    "target_author_pubkey must appear in payload when supplied"
                );
            }
            _ => panic!("expected DispatchReactAction"),
        }
    }

    // 4B-T4: unreact_dispatches_nip25_unreact
    //
    // AppAction::Unreact must produce exactly one Effect::DispatchReactAction
    // with namespace "nmp.nip25.unreact" and a serde-valid payload containing
    // reaction_event_id.
    #[test]
    fn unreact_dispatches_nip25_unreact() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let reaction_id = "1111000000000000000000000000000000000000000000000000000000000001";

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::Unreact {
                reaction_event_id: reaction_id.to_string(),
            }),
        );

        assert_eq!(effects.len(), 1, "Unreact must emit exactly one effect");
        match &effects[0] {
            Effect::DispatchReactAction { namespace, json } => {
                assert_eq!(
                    namespace, "nmp.nip25.unreact",
                    "namespace must match nmp.nip25.unreact"
                );
                let parsed: serde_json::Value =
                    serde_json::from_str(json).expect("payload must be valid JSON");
                assert_eq!(
                    parsed["reaction_event_id"].as_str().unwrap(),
                    reaction_id,
                    "reaction_event_id must be in payload"
                );
                // reason must be present as an empty string (UnreactAction.reason: String).
                assert_eq!(
                    parsed["reason"].as_str().unwrap(),
                    "",
                    "reason must be present as empty string"
                );
            }
            _ => panic!("expected DispatchReactAction"),
        }
    }

    // 4B-T5: reaction_state_no_optimistic_in_kernel
    //
    // Dispatching React must NOT speculatively update AppState::reaction_state.
    // The kernel is not the optimistic-update layer — that is Swift's job (D1).
    #[test]
    fn reaction_state_no_optimistic_in_kernel() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let target = "cccc000000000000000000000000000000000000000000000000000000000001";

        // Dispatch React — must NOT update reaction_state.
        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::React {
                target_event_id: target.to_string(),
                reaction: "+".to_string(),
                target_author_pubkey: None,
            }),
        );

        assert!(
            state.reaction_state.get(target).is_none(),
            "React action must not speculatively update reaction_state (D1 — Swift owns optimistic state)"
        );

        // Simulate the authoritative projection frame arriving.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::ReactionStateUpdated {
                target_event_id: target.to_string(),
                count: 1,
                viewer_reacted: true,
            }),
        );

        assert!(
            state.reaction_state.get(target).is_some(),
            "reaction_state populated after ReactionStateUpdated arrives"
        );
    }

    // 4B-T6: reaction_observer_on_kernel_event_sends_state_updated
    //
    // ReactionObserver::on_kernel_event must send KernelEvent::ReactionStateUpdated
    // into the channel when a kind:7 event with an "e" tag is ingested.
    // This validates the observer→channel path that backs real NMP events.
    #[test]
    fn reaction_observer_on_kernel_event_sends_state_updated() {
        let (tx, mut rx) = mpsc::unbounded_channel::<Cmd>();
        let viewer = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let target = "bbbb000000000000000000000000000000000000000000000000000000000002";
        let reaction_id = "cccc000000000000000000000000000000000000000000000000000000000003";

        let projection = Arc::new(ReactionProjection::new(Some(viewer.to_string())));
        let observer = ReactionObserver { projection, tx };

        // Construct a fake kind:7 event with the required tags.
        let event = nmp_core::substrate::KernelEvent {
            id: reaction_id.to_string(),
            author: viewer.to_string(),
            kind: KIND_REACTION,
            created_at: 1_000_000,
            tags: vec![
                vec!["e".to_string(), target.to_string()],
                vec!["p".to_string(), "author_pubkey".to_string()],
            ],
            content: "+".to_string(),
            relay_provenance: vec![],
        };

        observer.on_kernel_event(&event);

        // The observer must have sent a ReactionStateUpdated command.
        let cmd = rx.try_recv().expect("ReactionStateUpdated must be sent");
        match cmd {
            Cmd::Event(KernelEvent::ReactionStateUpdated {
                target_event_id,
                count,
                viewer_reacted,
            }) => {
                assert_eq!(
                    target_event_id, target,
                    "target_event_id must match the e tag"
                );
                assert_eq!(count, 1, "count must be 1 after one reaction ingested");
                assert!(
                    viewer_reacted,
                    "viewer_reacted must be true when viewer is the reactor"
                );
            }
            _ => panic!("expected ReactionStateUpdated from channel"),
        }
    }

    // 4B-T7: reaction_observer_noop_on_missing_e_tag (D6)
    //
    // A kind:7 event without an "e" tag is malformed per NIP-25. The observer
    // must not panic or send a command — silent no-op (D6).
    #[test]
    fn reaction_observer_noop_on_missing_e_tag() {
        let (tx, mut rx) = mpsc::unbounded_channel::<Cmd>();
        let projection = Arc::new(ReactionProjection::new(None));
        let observer = ReactionObserver { projection, tx };

        let event = nmp_core::substrate::KernelEvent {
            id: "aaaa000000000000000000000000000000000000000000000000000000000001".to_string(),
            author: "bbbb000000000000000000000000000000000000000000000000000000000002".to_string(),
            kind: KIND_REACTION,
            created_at: 1_000_000,
            tags: vec![], // No "e" tag — malformed
            content: "+".to_string(),
            relay_provenance: vec![],
        };

        observer.on_kernel_event(&event);

        assert!(
            rx.try_recv().is_err(),
            "malformed kind:7 (no e tag) must not send any command (D6)"
        );
    }

    // 4B-T8: reaction_state_cleared_on_logout
    //
    // AppAction::Logout must wipe AppState::reaction_state so stale reaction
    // counts from the previous account don't survive into the next session.
    #[test]
    fn reaction_state_cleared_on_logout() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let target = "dddd000000000000000000000000000000000000000000000000000000000001";

        // Seed a reaction entry.
        state.reaction_state.insert(
            target.to_string(),
            ReactionRow {
                target_event_id: target.to_string(),
                count: 7,
                viewer_reacted: true,
            },
        );
        // Seed a session so logout has something to clear.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("somepubkey".into()))),
        );

        // Logout — reaction_state must be cleared.
        step(&mut state, &clock, Cmd::Action(AppAction::Logout));

        assert!(
            state.reaction_state.is_empty(),
            "reaction_state must be empty after Logout"
        );
    }

    // 4B-T9: reaction_state_cleared_on_identity_changed_none
    //
    // IdentityChanged(None) must wipe reaction_state so stale counts don't
    // outlive the removed account (symmetric with T8 via the event path).
    #[test]
    fn reaction_state_cleared_on_identity_changed_none() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let target = "eeee000000000000000000000000000000000000000000000000000000000002";

        // Seed a reaction entry and a present session.
        state.reaction_state.insert(
            target.to_string(),
            ReactionRow {
                target_event_id: target.to_string(),
                count: 3,
                viewer_reacted: false,
            },
        );
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("anotherpubkey".into()))),
        );

        // Account removed → reaction_state must be cleared.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(None)),
        );

        assert!(
            state.reaction_state.is_empty(),
            "reaction_state must be empty after IdentityChanged(None)"
        );
    }

    // 4B-T10: dispatch_react_returns_unit (fire-and-forget, Non-Negotiable #3)
    //
    // The return type Vec<Effect> models the fire-and-forget () contract.
    // No Result, no panic.
    #[test]
    fn dispatch_react_returns_unit() {
        let mut state = make_state();
        let clock = ManualClock::new(0);
        let _effects: Vec<Effect> = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::React {
                target_event_id: "ffff000000000000000000000000000000000000000000000000000000000001"
                    .into(),
                reaction: "+".into(),
                target_author_pubkey: None,
            }),
        );
        // No panic, no Result — fire-and-forget contract satisfied.
    }

    // 4B-T11: reaction_state_raw_no_labels
    //
    // ReactionRow must carry only raw count + viewer_reacted (D1). The stored
    // row must not contain formatted strings like "X likes".
    #[test]
    fn reaction_state_raw_no_labels() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let target = "1234000000000000000000000000000000000000000000000000000000000001";

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::ReactionStateUpdated {
                target_event_id: target.to_string(),
                count: 99,
                viewer_reacted: false,
            }),
        );

        let row = state.reaction_state.get(target).unwrap();
        assert_eq!(row.count, 99);
        assert!(!row.viewer_reacted);
        // D1: no label strings in the debug representation.
        let debug_str = format!("{:?}", row);
        assert!(
            !debug_str.contains("likes"),
            "ReactionRow must not contain label strings (D1)"
        );
    }
}
