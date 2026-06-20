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

use nmp_core::typed_projections::{
    decode_action_results, ACTION_RESULTS_SCHEMA_ID, CLAIMED_PROFILES_SCHEMA_ID, PROFILE_SCHEMA_ID,
    RELAY_DIAGNOSTICS_SCHEMA_ID,
};

use crate::kernel::app::AppState;
use crate::kernel::domains::{blossom, profiles, relay_diagnostics};
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
pub(crate) fn dispatch_typed_frame(state: &mut AppState, frame_bytes: &[u8]) -> Vec<Effect> {
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

    let mut effects: Vec<Effect> = Vec::new();

    for proj in &projections {
        // Extension seam: future slices append arms before the `_` default.
        match proj.schema_id.as_str() {
            // ── Phase 2E arm: "relay_diagnostics" ────────────────────────────
            // Decode the relay-diagnostics typed sidecar and store raw fields
            // in AppState::relay_diagnostics. nmp's pre-formatted *_label /
            // *_display strings are intentionally not projected (D1 raw-data
            // doctrine). Append-only: rebase doctrine requires new arms to be
            // added BELOW this one, never interleaved.
            RELAY_DIAGNOSTICS_SCHEMA_ID => {
                relay_diagnostics::apply(state, &proj.payload);
            }

            // ── Phase 3B: joined-groups projection ───────────────────────────
            // Decodes the `"nmp.nip29.joined_groups"` FlatBuffers payload via
            // `nmp_nip29::decode_joined_groups_snapshot` and maps `JoinedGroup`
            // rows into `CommunityRow` (raw fields only — no formatted labels).
            // `wire_joined_groups` is called at boot and on `IdentityChanged(Some)`
            // via `Effect::WireJoinedGroups` so the projection fires on every tick.
            "nmp.nip29.joined_groups" => {
                super::communities::apply_joined_groups(state, &proj.payload);
            }

            // ── Phase 3C arm: "nmp.nip02.follow_list" ────────────────────────
            // Decode the FlatBuffers follow-list payload and store raw hex
            // pubkeys in AppState::follows. D6: decode errors are silent no-ops.
            super::follows::SCHEMA_ID => {
                super::follows::apply_follow_list(state, &proj.payload);
            }

            // ── Phase 3D arm: "profile" ─────────────────────────────────────
            // Active account's own profile card (built-in Tier-2 projection).
            // Decoded into AppState::own_profile. No ClaimProfile needed — the
            // kernel emits this sidecar automatically for the active account.
            PROFILE_SCHEMA_ID => {
                profiles::apply_own_profile(state, &proj.payload);
            }

            // ── Phase 3D arm: "claimed_profiles" ─────────────────────────────
            // Map of pubkey → ProfileCardModel for all currently claimed profiles
            // (visited via AppAction::ClaimProfile / Effect::ClaimProfile).
            // Decoded into AppState::claimed_profiles (HashMap).
            CLAIMED_PROFILES_SCHEMA_ID => {
                profiles::apply_claimed_profiles(state, &proj.payload);
            }

            // ── Phase 3E arm: "nmp.nip29.discovered_groups" ──────────────────
            // Decode the `"nmp.nip29.discovered_groups"` FlatBuffers payload via
            // `nmp_nip29::decode_discovered_groups_snapshot` and maps rows into
            // `DiscoveredRow` (raw fields only). Stored in
            // `AppState::discovered_groups`.
            super::discovery::DISCOVERED_GROUPS_SCHEMA_ID => {
                super::discovery::apply_discovered_groups(state, &proj.payload);
            }

            // ── Phase 3F arm: "nmp.nip29.group_events" ───────────────────────
            // Decode the `"nmp.nip29.group_events"` FlatBuffers payload via
            // `nmp_nip29::decode_group_events_snapshot` and store raw event rows
            // in `AppState::room_home_events` keyed by `group_id`. Capped at
            // 256 rows per group (ROOM_HOME_EVENTS_CAP). Lane bodies deferred to
            // Phase 4. D6: decode errors are silent no-ops.
            super::room_home::GROUP_EVENTS_SCHEMA_ID => {
                super::room_home::apply_group_events_frame(state, &proj.payload);
            }

            // ── Phase 4C arm: "hl.bookmarks" ─────────────────────────────────
            // Decode the hl-owned serde-JSON bookmark snapshot (registered by
            // `bookmarks::register_bookmark_list_projection`) and store raw
            // BookmarkRow items in AppState::bookmarks. D6: decode errors are
            // silent no-ops. Append-only: new arms go BELOW this one.
            super::bookmarks::BOOKMARK_SCHEMA_ID => {
                super::bookmarks::apply_bookmarks(state, &proj.payload);
            }

            // ── Phase 4A arm: "nmp.nip23.articles" ───────────────────────────
            // Decode the `"nmp.nip23.articles"` FlatBuffers payload via
            // `nmp_content::wire::longform_fb::decode_longform_articles` and
            // store raw `ArticleRow` records in `AppState::articles` keyed by
            // addressable coordinate `kind:author_hex:d_tag`. The longform
            // projection is registered at boot by nmp-defaults (longform: true
            // is the default in NmpDefaults). D6: decode errors are silent no-ops.
            // D1: raw fields only — no "Untitled" fallback, no "min read" label,
            // no hashtag formatting in the stored rows.
            super::articles::ARTICLES_SCHEMA_ID => {
                super::articles::apply_articles(state, &proj.payload);
            }

            // ── Phase 4D arm: "hl.search" ─────────────────────────────────────
            // Decode the hl-owned serde-JSON search snapshot (registered by
            // `search::run_effect_run_search` on each `AppAction::RunSearch`
            // dispatch) and store raw SearchHitRow items in
            // AppState::search_results. Bounded by the projection's max_hits cap
            // (default 200 — Non-Negotiable #7). D6: decode errors are silent
            // no-ops. D1: raw fields only — no "X results" count labels.
            // Append-only: new arms go BELOW this one.
            super::search::SEARCH_SCHEMA_ID => {
                super::search::apply_search_results(state, &proj.payload);
            }

            // ── Phase 5G arm: "action_results" ───────────────────────────────
            // Decode the Tier-2 kernel-owned `"action_results"` typed sidecar
            // (FlatBuffers KARS file identifier). Each row carries a settled
            // action result keyed by `correlation_id`. Route to the blossom
            // domain which matches upload and capture-publish correlation_ids
            // against `AppState::capture_draft` pending ids and applies the
            // appropriate state mutation (upload result + has_upload / FSM
            // transition). Unknown correlation_ids are a silent no-op (D6).
            // Append-only: new arms go BELOW this one.
            ACTION_RESULTS_SCHEMA_ID => match decode_action_results(&proj.payload) {
                Ok(model) => {
                    let mut new_effects = blossom::apply_action_results(state, &model);
                    effects.append(&mut new_effects);
                }
                Err(e) => {
                    tracing::trace!(
                        error = %e,
                        "dispatch_typed_frame: action_results decode failed — no-op (D6)"
                    );
                }
            },

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
