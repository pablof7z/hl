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
//! `register_reaction_projection(nmp_ref, active_account, tx)` is called ONCE at
//! boot (after `nmp_app_start`) from `start_nmp_app` using
//! `nmp_ref.active_account_handle()` so the observer auto-tracks the active
//! account. No re-registration on `IdentityChanged(Some)` is needed — the
//! `ReactionObserver` reads the live `Arc<Mutex<Option<String>>>` on every event
//! and calls `projection.set_viewer_pubkey(current)` so `viewer_reacted` is
//! always current. `Effect::WireReactionProjection` does not exist.
//!
//! ## Threading
//!
//! `ReactionObserver::on_kernel_event` is called from the NMP event-dispatch
//! thread (same thread as `FollowListProjection::on_kernel_event`). It:
//! 1. Acquires a Mutex lock on the `reaction_id_to_target` secondary map.
//! 2. Acquires a Mutex lock on the `ReactionProjection` internal entries (inside
//!    the nmp-nip25 projection). Locks are brief and never held simultaneously.
//! 3. Sends `Cmd::Event(KernelEvent::ReactionStateUpdated)` into the
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

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use nmp_core::dispatch_envelope::{encode_dispatch_envelope, DISPATCH_ENVELOPE_SCHEMA_VERSION};
use nmp_core::substrate::ActionPayload;
use nmp_core::KernelEventObserver;
use nmp_ffi::{nmp_app_dispatch_action_bytes, nmp_free_string, NmpApp};
use nmp_nip25::{
    ReactAction, ReactionProjection, UnreactAction, KIND_REACTION, KIND_REACTION_DELETE,
};
use tokio::sync::mpsc;

use crate::kernel::action::KernelEvent;
use crate::kernel::actor::Cmd;
use crate::kernel::app::AppState;

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
///
/// ## Bug-fix: kind:5 wrong-key fix
///
/// For kind:5 deletion events, the `["e", ...]` tags contain the
/// *reaction_event_id* being deleted, NOT the original target_event_id.
/// `reaction_id_to_target` maps `reaction_event_id → target_event_id` so the
/// observer can look up which target's count changed before the projection
/// removes the entry from its `entries` map.
///
/// ## Bug-fix: single registration / viewer auto-tracking
///
/// `active_account` is the live `Arc<Mutex<Option<String>>>` from
/// `nmp_ref.active_account_handle()`. The observer reads it on every event and
/// calls `projection.set_viewer_pubkey(current)` so `viewer_reacted` always
/// reflects the signed-in account without re-registering the observer on every
/// `IdentityChanged(Some)`.
struct ReactionObserver {
    projection: Arc<ReactionProjection>,
    /// Live active-account slot from `nmp_ref.active_account_handle()`.
    /// Updated by NMP on sign-in / switch / logout without re-registration.
    active_account: Arc<Mutex<Option<String>>>,
    /// Secondary index: reaction_event_id → target_event_id.
    /// Populated on every kind:7 ingest; consulted on kind:5 to find the
    /// target whose count must be refreshed after the deletion.
    reaction_id_to_target: Mutex<HashMap<String, String>>,
    tx: mpsc::UnboundedSender<Cmd>,
}

impl KernelEventObserver for ReactionObserver {
    fn on_kernel_event(&self, event: &nmp_core::substrate::KernelEvent) {
        match event.kind {
            KIND_REACTION => self.handle_kind7(event),
            KIND_REACTION_DELETE => self.handle_kind5(event),
            _ => {}
        }
    }
}

impl ReactionObserver {
    /// Handle kind:7 — a new reaction.
    ///
    /// 1. Record `reaction_event_id → target_event_id` in the secondary map.
    /// 2. Update `viewer_pubkey` from the live active-account slot.
    /// 3. Delegate ingest to `ReactionProjection`.
    /// 4. Snapshot the affected target and send `ReactionStateUpdated`.
    fn handle_kind7(&self, event: &nmp_core::substrate::KernelEvent) {
        // D6: kind:7 without an "e" tag is malformed — silent no-op.
        let target_event_id = match first_tag_value(&event.tags, "e") {
            Some(id) => id,
            None => return,
        };

        // Record the mapping before ingest so kind:5 deletions can resolve
        // targets. This is a brief Mutex acquire (HashMap insert).
        if let Ok(mut map) = self.reaction_id_to_target.lock() {
            map.insert(event.id.clone(), target_event_id.clone());
        }

        // Sync viewer_pubkey from the live active-account Arc before
        // snapshotting so viewer_reacted is always current (Bug 2 fix).
        let current_viewer = self.active_account.lock().ok().and_then(|g| g.clone());
        self.projection.set_viewer_pubkey(current_viewer);

        // Delegate ingest to the projection (updates internal entries map).
        self.projection.on_kernel_event(event);

        // Snapshot the updated state for this target and push to actor.
        self.push_snapshot(&target_event_id);
    }

    /// Handle kind:5 — deletion of one or more reaction events.
    ///
    /// kind:5 `["e", ...]` tags contain *reaction_event_ids* (not target ids).
    /// Look up each in `reaction_id_to_target` BEFORE delegating to the
    /// projection (which removes them from its entries map), then snapshot each
    /// unique affected target and send `ReactionStateUpdated`.
    fn handle_kind5(&self, event: &nmp_core::substrate::KernelEvent) {
        // Collect deleted reaction_event_ids from all ["e", ...] tags.
        let deleted_reaction_ids: Vec<String> = event
            .tags
            .iter()
            .filter_map(|tag| {
                if tag.first().is_some_and(|name| name == "e") {
                    tag.get(1).filter(|v| !v.is_empty()).cloned()
                } else {
                    None
                }
            })
            .collect();

        if deleted_reaction_ids.is_empty() {
            return; // D6: malformed kind:5 with no "e" tags — silent no-op.
        }

        // Resolve the affected target_event_ids BEFORE the projection removes
        // the entries (we need the mapping while it still exists). Collect into
        // a deduped set so we don't snapshot the same target twice.
        let mut affected_targets: Vec<String> = Vec::new();
        if let Ok(map) = self.reaction_id_to_target.lock() {
            for reaction_id in &deleted_reaction_ids {
                if let Some(target_id) = map.get(reaction_id) {
                    if !affected_targets.contains(target_id) {
                        affected_targets.push(target_id.clone());
                    }
                }
            }
        }

        // Sync viewer before snapshotting.
        let current_viewer = self.active_account.lock().ok().and_then(|g| g.clone());
        self.projection.set_viewer_pubkey(current_viewer);

        // Let the projection remove the entries from its internal map.
        self.projection.on_kernel_event(event);

        // Snapshot each affected target and push updated counts.
        for target_id in &affected_targets {
            self.push_snapshot(target_id);
        }
    }

    /// Snapshot `target_event_id` from `projection` and send
    /// `KernelEvent::ReactionStateUpdated` into the actor channel.
    ///
    /// D1: raw count + viewer_reacted bool only.
    /// D6: send errors (actor shut down) are silently dropped.
    fn push_snapshot(&self, target_event_id: &str) {
        let snapshot = self.projection.snapshot_for(target_event_id);
        let count = snapshot.reactions.len() as u32;
        let viewer_reacted = snapshot.viewer_reaction.is_some();
        // Carry the viewer's own reaction event id into the actor so toggle can
        // unreact later. Kernel-internal — never surfaced across FFI (D1).
        let viewer_reaction_event_id = snapshot
            .viewer_reaction
            .as_ref()
            .map(|v| v.reaction_event_id.clone());
        let _ = self.tx.send(Cmd::Event(KernelEvent::ReactionStateUpdated {
            target_event_id: target_event_id.to_string(),
            count,
            viewer_reacted,
            viewer_reaction_event_id,
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
    state.viewer_reaction_ids.clear();
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

/// Handle `hl.reaction.toggle { target_event_id, target_author_pubkey? }`.
///
/// Like-or-unlike decided from the kernel's own viewer-reaction tracking:
/// - If the active viewer already has a kind:7 `+` on `target_event_id`
///   (`AppState::reaction_state[target].viewer_reacted` true and we hold the
///   reaction event id in `AppState::viewer_reaction_ids`), emit the UNREACT
///   effect (same path as `hl.reaction.unreact`) using that stored id — which
///   NEVER crosses FFI.
/// - Otherwise emit the REACT effect (publish `+` on the target).
///
/// Fire-and-forget (Non-Negotiable #3). The authoritative count/viewer_reacted
/// correction arrives back via `KernelEvent::ReactionStateUpdated`.
pub(crate) fn reduce_action_toggle_reaction(
    state: &AppState,
    target_event_id: String,
    target_author_pubkey: Option<String>,
) -> Vec<crate::kernel::effect::Effect> {
    let already_reacted = state
        .reaction_state
        .get(&target_event_id)
        .is_some_and(|row| row.viewer_reacted);

    if already_reacted {
        if let Some(reaction_event_id) = state.viewer_reaction_ids.get(&target_event_id) {
            return reduce_action_unreact(reaction_event_id.clone());
        }
        // viewer_reacted is true but we somehow lack the id — fall through to
        // react is wrong (would double-like); D6: no-op rather than misfire.
        return vec![];
    }

    reduce_action_react(target_event_id, "+".to_string(), target_author_pubkey)
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

    // Deserialise the pre-built JSON back to the typed struct, then encode as
    // FlatBuffers for the bytes doorway (ADR-0064 / Cut-B).
    let payload_bytes = match namespace.as_str() {
        "nmp.nip25.react" => match serde_json::from_str::<ReactAction>(&json) {
            Ok(action) => action.encode(),
            Err(e) => {
                tracing::warn!(error = %e, "reactions: failed to deserialise ReactAction");
                return;
            }
        },
        "nmp.nip25.unreact" => match serde_json::from_str::<UnreactAction>(&json) {
            Ok(action) => action.encode(),
            Err(e) => {
                tracing::warn!(error = %e, "reactions: failed to deserialise UnreactAction");
                return;
            }
        },
        other => {
            tracing::warn!(namespace = other, "reactions: unknown namespace — no-op");
            return;
        }
    };

    let correlation_id = uuid::Uuid::new_v4().to_string();
    let envelope = encode_dispatch_envelope(
        &correlation_id,
        &namespace,
        DISPATCH_ENVELOPE_SCHEMA_VERSION,
        &payload_bytes,
    );

    let result_ptr =
        nmp_app_dispatch_action_bytes(handle.ptr.as_ptr(), envelope.as_ptr(), envelope.len());

    if !result_ptr.is_null() {
        nmp_free_string(result_ptr);
    }
}

// ─── Projection registration ─────────────────────────────────────────────────

/// Wire the `ReactionObserver` (wrapping `ReactionProjection`) as a
/// `KernelEventObserver` against `nmp_ref`.
///
/// On each ingested kind:7 or kind:5 event the observer:
/// 1. Reads the current viewer from `active_account` and calls
///    `projection.set_viewer_pubkey(current)` so `viewer_reacted` is always current.
/// 2. Records `reaction_event_id → target_event_id` (kind:7) or resolves the
///    target before deletion (kind:5) using the `reaction_id_to_target` map.
/// 3. Delegates ingest to the `ReactionProjection`.
/// 4. Calls `projection.snapshot_for(target_event_id)` and sends
///    `KernelEvent::ReactionStateUpdated` into the actor channel `tx`.
///
/// ## When to call
///
/// Called **ONCE** at boot from `start_nmp_app` (after `nmp_app_start`) with
/// `nmp_ref.active_account_handle()`. The observer auto-tracks account
/// switches via the live `Arc<Mutex<Option<String>>>` — no re-registration on
/// `IdentityChanged(Some)` is needed or desired (re-calling would STACK
/// observers, causing memory leaks and duplicate events).
///
/// D6: if `register_live_event_tap` returns id `0` (slot full), the observer
/// is silently dropped and reaction state will not update (logged as a warning).
pub(crate) fn register_reaction_projection(
    nmp_ref: &NmpApp,
    active_account: Arc<Mutex<Option<String>>>,
    tx: mpsc::UnboundedSender<Cmd>,
) {
    let projection = Arc::new(ReactionProjection::new(None));
    let observer = Arc::new(ReactionObserver {
        projection,
        active_account,
        reaction_id_to_target: Mutex::new(HashMap::new()),
        tx,
    });

    let observer_id = nmp_ref.register_live_event_tap(observer as Arc<dyn KernelEventObserver>);
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
                viewer_reaction_event_id: None,
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
                viewer_reaction_event_id: None,
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

        let active_account = Arc::new(Mutex::new(Some(viewer.to_string())));
        let projection = Arc::new(ReactionProjection::new(None));
        let observer = ReactionObserver {
            projection,
            active_account,
            reaction_id_to_target: Mutex::new(HashMap::new()),
            tx,
        };

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
                ..
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
        let observer = ReactionObserver {
            projection,
            active_account: Arc::new(Mutex::new(None)),
            reaction_id_to_target: Mutex::new(HashMap::new()),
            tx,
        };

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
                viewer_reaction_event_id: None,
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

    // 4B-T12: unreact_updates_target_count_not_reaction_event_key
    //
    // Bug 1 regression test: when a kind:5 deletion event arrives, the
    // ReactionObserver must snapshot the ORIGINAL target's count (which goes
    // from 1→0), NOT the reaction_event_id key (which was never in
    // AppState::reaction_state and would emit a spurious zeroed row there).
    //
    // Sequence:
    //   1. Ingest a kind:7 reaction (reaction_id → target).
    //   2. Ingest a kind:5 delete referencing reaction_id.
    //   3. The channel must carry two ReactionStateUpdated events:
    //      - First for the kind:7 (count=1, target_id key).
    //      - Second for the kind:5 (count=0, target_id key — NOT reaction_id key).
    #[test]
    fn unreact_updates_target_count_not_reaction_event_key() {
        let (tx, mut rx) = mpsc::unbounded_channel::<Cmd>();
        let viewer = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let target_id = "bbbb000000000000000000000000000000000000000000000000000000000002";
        let reaction_id = "cccc000000000000000000000000000000000000000000000000000000000003";

        let active_account = Arc::new(Mutex::new(Some(viewer.to_string())));
        let projection = Arc::new(ReactionProjection::new(None));
        let observer = ReactionObserver {
            projection,
            active_account,
            reaction_id_to_target: Mutex::new(HashMap::new()),
            tx,
        };

        // Step 1: ingest a kind:7 reaction.
        let kind7 = nmp_core::substrate::KernelEvent {
            id: reaction_id.to_string(),
            author: viewer.to_string(),
            kind: KIND_REACTION,
            created_at: 1_000_000,
            tags: vec![vec!["e".to_string(), target_id.to_string()]],
            content: "+".to_string(),
            relay_provenance: vec![],
        };
        observer.on_kernel_event(&kind7);

        // Consume the kind:7 update (count=1 for target).
        let cmd7 = rx
            .try_recv()
            .expect("kind:7 must send ReactionStateUpdated");
        match cmd7 {
            Cmd::Event(KernelEvent::ReactionStateUpdated {
                ref target_event_id,
                count,
                ..
            }) => {
                assert_eq!(
                    target_event_id, target_id,
                    "kind:7 update must key on target_id"
                );
                assert_eq!(count, 1, "count must be 1 after kind:7 ingest");
            }
            _ => panic!("expected ReactionStateUpdated for kind:7"),
        }

        // Step 2: ingest a kind:5 deletion referencing reaction_id (not target_id).
        let kind5 = nmp_core::substrate::KernelEvent {
            id: "dddd000000000000000000000000000000000000000000000000000000000004".to_string(),
            author: viewer.to_string(),
            kind: KIND_REACTION_DELETE,
            created_at: 1_000_001,
            tags: vec![vec!["e".to_string(), reaction_id.to_string()]],
            content: String::new(),
            relay_provenance: vec![],
        };
        observer.on_kernel_event(&kind5);

        // Step 3: the channel must contain a ReactionStateUpdated for TARGET_ID
        // (count=0), not for reaction_id.
        let cmd5 = rx
            .try_recv()
            .expect("kind:5 must send ReactionStateUpdated");
        match cmd5 {
            Cmd::Event(KernelEvent::ReactionStateUpdated {
                target_event_id,
                count,
                ..
            }) => {
                assert_eq!(
                    target_event_id, target_id,
                    "kind:5 update must key on the original target_id, not the reaction_event_id"
                );
                assert_eq!(count, 0, "count must be 0 after the reaction is deleted");
            }
            _ => panic!("expected ReactionStateUpdated for kind:5"),
        }

        // No spurious updates should be buffered.
        assert!(
            rx.try_recv().is_err(),
            "no extra updates must be sent (only one target was affected)"
        );
    }

    // 4B-T13: reaction_observer_viewer_tracks_active_account
    //
    // Bug 2 regression test: the ReactionObserver must read the current viewer
    // from the live active_account Arc on EACH event. If the Arc changes
    // (account switch), viewer_reacted in the next snapshot must reflect the
    // NEW account — without re-registering the observer.
    //
    // Sequence:
    //   1. Observer created with viewer=A in active_account.
    //   2. Ingest a kind:7 reaction from viewer A → viewer_reacted=true.
    //   3. Simulate account switch: update active_account to viewer=B.
    //   4. Ingest a second kind:7 reaction from viewer A for the same target.
    //   5. The second update must have viewer_reacted=false (B didn't react).
    #[test]
    fn reaction_observer_viewer_tracks_active_account() {
        let (tx, mut rx) = mpsc::unbounded_channel::<Cmd>();
        let viewer_a = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let viewer_b = "bbbb000000000000000000000000000000000000000000000000000000000002";
        let target_id = "cccc000000000000000000000000000000000000000000000000000000000003";
        let reaction_id1 = "dddd000000000000000000000000000000000000000000000000000000000004";
        let reaction_id2 = "eeee000000000000000000000000000000000000000000000000000000000005";

        // Start with viewer_a active.
        let active_account: Arc<Mutex<Option<String>>> =
            Arc::new(Mutex::new(Some(viewer_a.to_string())));
        let projection = Arc::new(ReactionProjection::new(None));
        let observer = ReactionObserver {
            projection,
            active_account: Arc::clone(&active_account),
            reaction_id_to_target: Mutex::new(HashMap::new()),
            tx,
        };

        // Step 2: ingest a kind:7 from viewer_a.
        let event1 = nmp_core::substrate::KernelEvent {
            id: reaction_id1.to_string(),
            author: viewer_a.to_string(),
            kind: KIND_REACTION,
            created_at: 1_000_000,
            tags: vec![vec!["e".to_string(), target_id.to_string()]],
            content: "+".to_string(),
            relay_provenance: vec![],
        };
        observer.on_kernel_event(&event1);

        let cmd1 = rx.try_recv().expect("first kind:7 must send update");
        match cmd1 {
            Cmd::Event(KernelEvent::ReactionStateUpdated { viewer_reacted, .. }) => {
                assert!(
                    viewer_reacted,
                    "viewer_reacted must be true when active account (viewer_a) has reacted"
                );
            }
            _ => panic!("expected ReactionStateUpdated"),
        }

        // Step 3: simulate account switch — update the Arc in-place (no observer re-registration).
        if let Ok(mut guard) = active_account.lock() {
            *guard = Some(viewer_b.to_string());
        }

        // Step 4: ingest a second kind:7 from viewer_a.
        let event2 = nmp_core::substrate::KernelEvent {
            id: reaction_id2.to_string(),
            author: viewer_a.to_string(),
            kind: KIND_REACTION,
            created_at: 1_000_001,
            tags: vec![vec!["e".to_string(), target_id.to_string()]],
            content: "+".to_string(),
            relay_provenance: vec![],
        };
        observer.on_kernel_event(&event2);

        // Step 5: viewer_reacted must now be false because active account is viewer_b.
        let cmd2 = rx.try_recv().expect("second kind:7 must send update");
        match cmd2 {
            Cmd::Event(KernelEvent::ReactionStateUpdated {
                viewer_reacted,
                count,
                ..
            }) => {
                assert_eq!(count, 2, "two reactions ingested — count must be 2");
                assert!(
                    !viewer_reacted,
                    "viewer_reacted must be false after account switch to viewer_b (who has not reacted)"
                );
            }
            _ => panic!("expected ReactionStateUpdated"),
        }
    }

    // hl.reaction.toggle: first toggle on a not-yet-reacted target → REACT;
    // a second toggle (after the viewer's reaction id is known) → UNREACT with
    // that exact reaction_event_id. The id never crosses FFI — it lives only in
    // AppState::viewer_reaction_ids.
    #[test]
    fn toggle_reacts_then_unreacts_same_target() {
        let target = "tttt000000000000000000000000000000000000000000000000000000000001";
        let reaction_id = "7777000000000000000000000000000000000000000000000000000000000001";

        // State 1: viewer has NOT reacted → toggle emits a react ("+").
        let mut state = make_state();
        let effects = reduce_action_toggle_reaction(&state, target.to_string(), None);
        assert_eq!(effects.len(), 1, "toggle must emit exactly one effect");
        match &effects[0] {
            Effect::DispatchReactAction { namespace, json } => {
                assert_eq!(namespace, "nmp.nip25.react", "first toggle → react");
                let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
                assert_eq!(parsed["target_event_id"].as_str().unwrap(), target);
                assert_eq!(parsed["reaction"].as_str().unwrap(), "+");
            }
            _ => panic!("expected DispatchReactAction (react)"),
        }

        // State 2: the reaction landed — reaction_state.viewer_reacted = true and
        // viewer_reaction_ids holds the kind:7 id (as the observer would record).
        state.reaction_state.insert(
            target.to_string(),
            crate::kernel::snapshot::ReactionRow {
                target_event_id: target.to_string(),
                count: 1,
                viewer_reacted: true,
            },
        );
        state
            .viewer_reaction_ids
            .insert(target.to_string(), reaction_id.to_string());

        let effects = reduce_action_toggle_reaction(&state, target.to_string(), None);
        assert_eq!(
            effects.len(),
            1,
            "second toggle must emit exactly one effect"
        );
        match &effects[0] {
            Effect::DispatchReactAction { namespace, json } => {
                assert_eq!(namespace, "nmp.nip25.unreact", "second toggle → unreact");
                let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
                assert_eq!(
                    parsed["reaction_event_id"].as_str().unwrap(),
                    reaction_id,
                    "unreact must use the stored reaction_event_id"
                );
            }
            _ => panic!("expected DispatchReactAction (unreact)"),
        }
    }

    // D6: viewer_reacted true but no stored reaction id → toggle is a no-op
    // (never double-likes).
    #[test]
    fn toggle_noop_when_reacted_but_id_missing() {
        let target = "tttt000000000000000000000000000000000000000000000000000000000002";
        let mut state = make_state();
        state.reaction_state.insert(
            target.to_string(),
            crate::kernel::snapshot::ReactionRow {
                target_event_id: target.to_string(),
                count: 1,
                viewer_reacted: true,
            },
        );
        let effects = reduce_action_toggle_reaction(&state, target.to_string(), None);
        assert!(effects.is_empty(), "no id → no-op, never a double-like");
    }
}
