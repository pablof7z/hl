//! Typed-sidecar dispatch — the bridge between the NMP update-callback frame
//! bytes and the actor's `KernelEvent` channel.
//!
//! ## Extension contract
//!
//! Every future Phase-3 slice (3B communities, 3C follows, 3D profiles,
//! 3E discovery, 2E diagnostics, etc.) adds exactly ONE arm to the
//! `match schema_id` dispatch table in `dispatch_typed_frame` and, where
//! the decoded model needs to be stored in `AppState`, one field in
//! `kernel/app.rs`. The cross-slice serialization point is the `match`
//! arm list — all arms are append-only (rebase doctrine). Each arm calls
//! a decode function from the appropriate `nmp-nip*` crate and returns a
//! `KernelEvent::*Updated` variant that the reducer stores in `AppState`.
//!
//! ## Threading model
//!
//! `dispatch_typed_frame` runs on the **actor thread** (inside `reduce_event`).
//! The callback registered with `nmp_app_set_update_callback` is deliberately
//! thin: it clones the raw bytes and sends `KernelEvent::NmpSnapshotFrame`
//! into the actor channel. Decoding (this module) happens synchronously in
//! the reducer, which is single-threaded and non-async (Non-Negotiable #2).
//!
//! ## D6 — no panics on malformed frames
//!
//! Every decode call is wrapped in a `match` or `let Ok(...)` / `let Some(...)`
//! guard. Unknown `schema_id` values are a silent no-op (logged at trace
//! level). Malformed payloads are a silent no-op. Neither case corrupts
//! `AppState` — the previous value is left unchanged.

use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;

/// Decode one NMP snapshot frame, route each typed-projection sidecar by
/// `schema_id`, and apply updates to `state`. Returns effects to run (none
/// in 3A — projection state is stored directly in `AppState` fields).
///
/// Called from `reduce_event(KernelEvent::NmpSnapshotFrame(bytes))` on the
/// actor thread. Non-async, non-blocking (FlatBuffers decode only).
///
/// ## Extension contract — how future slices add a projection:
///
/// 1. Add a `schema_id` arm to the `match` below (append-only).
/// 2. Call the crate-specific decode fn (e.g. `nmp_nip29::decode_joined_groups_snapshot`).
/// 3. Store the decoded model in the appropriate `AppState` field added by
///    that slice (e.g. `state.projections.joined_groups = Some(decoded)`).
/// 4. Return any effects (e.g. `Effect::DispatchNip29Action` if a wiring
///    helper needs to be called after identity change).
///
/// The cross-slice serialisation points are: the `match schema_id` arm list
/// below, the `AppState` projection fields, and the `project_snapshot` arms
/// in `actor.rs`. All must be append-only (rebase doctrine).
///
/// ## D6 — no panics on malformed frames
///
/// Decode errors are silent no-ops. Unknown `schema_id` values are logged at
/// trace level and skipped. Neither case corrupts `AppState`.
pub(crate) fn dispatch_typed_frame(_state: &mut AppState, frame_bytes: &[u8]) -> Vec<Effect> {
    // Decode the full typed-projection sidecar from the frame.
    // A frame that fails to decode (wrong file identifier, truncated, etc.)
    // is silently dropped — D6. We also catch FlatBuffers panics (e.g. on
    // slices shorter than the minimum FlatBuffers header size) so that garbage
    // bytes from tests or adversarial frames cannot abort the actor thread.
    let frame_copy = frame_bytes.to_vec(); // must be 'static for catch_unwind
    let decode_result =
        std::panic::catch_unwind(move || nmp_core::decode_snapshot_typed_projections(&frame_copy));
    let projections = match decode_result {
        Ok(Ok(p)) => p,
        Ok(Err(_)) | Err(_) => return vec![], // decode error or panic — D6
    };

    let effects: Vec<Effect> = Vec::new();

    for proj in &projections {
        // Extension seam: future slices append arms before the `_` default.
        // Clippy flags this as single-binding while only the default arm exists;
        // the allow is intentional — the match exists for its structure, not the
        // current runtime paths.
        #[allow(clippy::match_single_binding)]
        match proj.schema_id.as_str() {
            // ── Phase 3B arm: "nmp.nip29.joined_groups" ──────────────────────
            // Added by slice 3B.
            // "nmp.nip29.joined_groups" => { ... }

            // ── Phase 3E arm: "nmp.nip29.discovered_groups" ──────────────────
            // Added by slice 3E.
            // "nmp.nip29.discovered_groups" => { ... }

            // ── Phase 3C arm: "nmp.nip02.follow_list" ────────────────────────
            // Added by slice 3C.
            // "nmp.nip02.follow_list" => { ... }

            // ── Phase 3D arm: "profile" ───────────────────────────────────────
            // Active account's own profile card. Added by slice 3D.
            // "profile" => { ... }

            // ── Phase 3D arm: "claimed_profiles" ─────────────────────────────
            // Profiles for visited pubkeys. Added by slice 3D.
            // "claimed_profiles" => { ... }

            // ── Default: unknown schema_id — silent no-op (D6) ────────────────
            // Projections registered by nmp-defaults that hl has not opted into
            // (e.g. action_stages, bunker_handshake) arrive here. This is the
            // expected path; trace-level only so hot paths stay quiet.
            _ => {
                tracing::trace!(
                    schema_id = %proj.schema_id,
                    key = %proj.key,
                    "dispatch_typed_frame: unknown schema_id — skipped (D6)"
                );
            }
        }
    }

    effects
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> AppState {
        AppState::default()
    }

    // 3A-T1: decode_dispatch_handles_unknown_schema_id_gracefully (D6)
    //
    // A zero-length slice is not a valid update frame — decode fails and returns
    // an empty effects list without panicking or mutating state (D6).
    #[test]
    fn decode_dispatch_handles_unknown_schema_id_gracefully() {
        let mut state = make_state();
        let effects = dispatch_typed_frame(&mut state, &[]);
        assert!(
            effects.is_empty(),
            "unknown/empty frame must produce no effects (D6)"
        );
    }

    // 3A-T2: malformed_frame_does_not_panic (D6)
    //
    // Random garbage bytes must not panic.
    #[test]
    fn malformed_frame_does_not_panic() {
        let mut state = make_state();
        let garbage = b"NOT A VALID FLATBUFFER FRAME AT ALL \x00\xFF\xFE";
        let effects = dispatch_typed_frame(&mut state, garbage);
        assert!(
            effects.is_empty(),
            "malformed frame must produce no effects"
        );
    }

    // 3A-T3: dispatch returns Vec<Effect> / reducers stay sync
    //
    // `dispatch_typed_frame` is a pure synchronous function (no async, no
    // blocking I/O). This test runs it from a non-tokio thread to confirm.
    #[test]
    fn dispatch_is_sync_pure() {
        let result = std::thread::spawn(|| {
            let mut state = make_state();
            let _effects: Vec<Effect> = dispatch_typed_frame(&mut state, &[]);
        })
        .join();
        assert!(
            result.is_ok(),
            "dispatch_typed_frame must be callable from any thread"
        );
    }
}
