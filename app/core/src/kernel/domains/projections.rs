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
    decode_action_results, ACTION_RESULTS_SCHEMA_ID, PROFILE_SCHEMA_ID, RELAY_DIAGNOSTICS_SCHEMA_ID,
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
pub(crate) fn dispatch_typed_frame(
    state: &mut AppState,
    frame_bytes: &[u8],
    now: u64,
) -> Vec<Effect> {
    // Decode the frame envelope and full typed-projection sidecar together.
    // A frame that fails to decode (wrong file identifier, truncated, etc.)
    // is silently dropped — D6. We also catch FlatBuffers panics (e.g. on
    // slices shorter than the minimum FlatBuffers header size) so that garbage
    // bytes from tests or adversarial frames cannot abort the actor thread.
    let frame_copy = frame_bytes.to_vec(); // must be 'static for catch_unwind
    let decode_result = std::panic::catch_unwind(move || {
        nmp_core::decode_snapshot_envelope(&frame_copy).and_then(|envelope| {
            nmp_core::decode_snapshot_typed_projections(&frame_copy)
                .map(|projections| (envelope, projections))
        })
    });
    let (envelope, projections) = match decode_result {
        Ok(Ok(decoded)) => decoded,
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

            // ── Phase 3D arm: "claimed_profiles" — REMOVED (ADR-0063 Lane H) ──
            // NMP deleted the bulk `"claimed_profiles"` typed sidecar; visited
            // profile resolution is now served by the per-key `refs.profile`
            // row-delta projection below.

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
            // projection is registered at boot by `nmp_nip23::register`.
            // D6: decode errors are silent no-ops.
            // D1: raw fields only — no "Untitled" fallback, no "min read" label,
            // no hashtag formatting in the stored rows.
            super::articles::ARTICLES_SCHEMA_ID => {
                super::articles::apply_articles(state, &proj.payload);
            }

            // ── Phase 4D arm: NMP "nmp.nip50.search" (N50S sidecar) ───────────
            // Decode NMP's typed N50S search-results sidecar (registered by
            // `NmpApp::open_search`, driven from `search::run_effect_run_search`
            // on each `AppAction::RunSearch` dispatch) and store raw SearchHitRow
            // items in AppState::search_results. Bounded by NMP's projection
            // max_hits cap (default 200 — Non-Negotiable #7). D6: decode errors
            // are silent no-ops. D1: raw fields only — no "X results" labels.
            // SEARCH_SCHEMA_ID == nmp_nip50::SEARCH_RESULTS_SCHEMA_ID.
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
                    let mut new_effects = blossom::apply_action_results(state, &model, now);
                    effects.append(&mut new_effects);
                }
                Err(e) => {
                    tracing::trace!(
                        error = %e,
                        "dispatch_typed_frame: action_results decode failed — no-op (D6)"
                    );
                }
            },

            // ── #1653 arm: "hl.bookmark_sets" ────────────────────────────────
            // Decode the hl-owned serde-JSON sets snapshot (registered by
            // `bookmark_sets::register_set_projections`) and store raw
            // BookmarkSetRow items in AppState::all_bookmark_sets /
            // all_curation_sets (unfiltered — projection helpers in
            // `bookmark_sets` do identity-based filtering at snapshot time).
            // D6: decode errors are silent no-ops. D1: raw fields only.
            super::bookmark_sets::BOOKMARK_SETS_SCHEMA_ID => {
                super::bookmark_sets::apply_bookmark_sets(state, &proj.payload);
            }

            // ── #1653 arm: "hl.web_bookmarks" ────────────────────────────────
            // Decode the hl-owned serde-JSON web-bookmarks snapshot and store
            // raw WebBookmarkRow items in AppState::web_bookmarks.
            // D6: decode errors are silent no-ops.
            super::bookmark_sets::WEB_BOOKMARKS_SCHEMA_ID => {
                super::bookmark_sets::apply_web_bookmarks(state, &proj.payload);
            }

            // ── Phase 7 arm: "refs.profile" ──────────────────────────────────
            // ADR-0063 Lane H: visited-profile cards arrive as per-key row
            // deltas. `RefProfileStore` needs the frame identity to reject
            // stale deltas across session/epoch changes.
            nmp_core::refs::REFS_PROFILE_KEY => {
                profiles::apply_refs_profile(
                    state,
                    &proj.payload,
                    envelope.session_id,
                    envelope.snapshot_epoch,
                );
            }

            // ── Phase 7 arm: "refs.event" ─────────────────────────────────────
            "refs.event" => {
                super::entities::apply_refs_event(state, &proj.payload);
            }

            // ── Default: unknown schema_id — silent no-op (D6) ────────────────
            // Framework projections that hl has not opted into (e.g.
            // action_stages, bunker_handshake) arrive here. This is the
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
    use nmp_core::refs::{encode_ref_row_delta_batch, RefRow, RefRowDeltaBatch};
    use nmp_core::typed_projections::{encode_profile, ProfileCardModel};

    fn make_state() -> AppState {
        AppState::default()
    }

    fn make_profile_card(pubkey: &str, display_name: &str) -> ProfileCardModel {
        ProfileCardModel {
            pubkey: pubkey.to_string(),
            display_name: Some(display_name.to_string()),
            picture_url: Some(format!("https://example.com/{display_name}.png")),
            ..Default::default()
        }
    }

    fn refs_profile_sidecar(baseline: bool, rows: Vec<RefRow>) -> Vec<u8> {
        encode_ref_row_delta_batch(&RefRowDeltaBatch {
            namespace: "profile".to_string(),
            baseline,
            rows,
        })
    }

    fn refs_profile_frame(session_id: u64, snapshot_epoch: u64, payload: Vec<u8>) -> Vec<u8> {
        nmp_core::encode_snapshot_frame(
            &nmp_core::SnapshotEnvelope {
                session_id,
                snapshot_epoch,
                ..Default::default()
            },
            &[nmp_core::TypedProjectionData {
                key: nmp_core::refs::REFS_PROFILE_KEY.to_string(),
                schema_id: nmp_core::refs::REFS_PROFILE_KEY.to_string(),
                schema_version: 1,
                file_identifier: "NRRD".to_string(),
                payload,
                ..Default::default()
            }],
        )
    }

    // 3A-T1: decode_dispatch_handles_unknown_schema_id_gracefully (D6)
    //
    // A zero-length slice is not a valid update frame — decode fails and returns
    // an empty effects list without panicking or mutating state (D6).
    #[test]
    fn decode_dispatch_handles_unknown_schema_id_gracefully() {
        let mut state = make_state();
        let effects = dispatch_typed_frame(&mut state, &[], 0);
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
        let effects = dispatch_typed_frame(&mut state, garbage, 0);
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
            let _effects: Vec<Effect> = dispatch_typed_frame(&mut state, &[], 0);
        })
        .join();
        assert!(
            result.is_ok(),
            "dispatch_typed_frame must be callable from any thread"
        );
    }

    #[test]
    fn refs_profile_frame_updates_and_clears_claimed_profiles() {
        let mut state = make_state();
        let pk = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

        let baseline = refs_profile_frame(
            42,
            7,
            refs_profile_sidecar(
                true,
                vec![RefRow::changed(
                    pk,
                    1,
                    encode_profile(&make_profile_card(pk, "Alice")),
                )],
            ),
        );

        let effects = dispatch_typed_frame(&mut state, &baseline, 0);
        assert!(effects.is_empty());
        assert_eq!(
            state
                .claimed_profiles
                .get(pk)
                .and_then(|card| card.display_name.as_deref()),
            Some("Alice")
        );

        let update = refs_profile_frame(
            42,
            7,
            refs_profile_sidecar(
                false,
                vec![RefRow::changed(
                    pk,
                    2,
                    encode_profile(&make_profile_card(pk, "Alice v2")),
                )],
            ),
        );
        dispatch_typed_frame(&mut state, &update, 0);
        assert_eq!(
            state
                .claimed_profiles
                .get(pk)
                .and_then(|card| card.display_name.as_deref()),
            Some("Alice v2")
        );

        let clear = refs_profile_frame(
            42,
            7,
            refs_profile_sidecar(false, vec![RefRow::cleared(pk, 3)]),
        );
        dispatch_typed_frame(&mut state, &clear, 0);
        assert!(
            !state.claimed_profiles.contains_key(pk),
            "refs.profile clear must remove the profile snapshot row"
        );
    }
}
