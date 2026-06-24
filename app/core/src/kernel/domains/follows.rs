//! Follows domain — NIP-02 follow-list projection (slice 3C).
//!
//! ## Responsibilities
//!
//! * **READ** — decode `"nmp.nip02.follow_list"` typed-sidecar frames into
//!   `AppState::follows` (raw hex pubkeys). Called from
//!   `projections::dispatch_typed_frame` when the `schema_id` arm matches.
//!
//! * **WRITE** — `AppAction::Follow{pubkey}` / `Unfollow{pubkey}` → reducer
//!   emits `Effect::DispatchFollowAction` → effect runner calls
//!   `nmp_app_dispatch_action("nmp.follow"|"nmp.unfollow", {"pubkey":...})`.
//!   Fire-and-forget (D6, Non-Negotiable #3): the updated follow list arrives
//!   back via the NMP update callback as a `FollowListUpdated` event.
//!
//! * **QUERY** — `AppState::is_following(pubkey)` (defined on `AppState` in
//!   `app.rs`; reads `AppState::follows`) is the single query point.
//!
//! ## NMP follow/unfollow seam
//!
//! Follow and unfollow dispatch goes through `nmp_app_dispatch_action` with the
//! `"nmp.follow"` / `"nmp.unfollow"` namespaces exposed by
//! `nmp_nip02::FollowModule` / `UnfollowModule`. These are registered at app
//! boot via `nmp_nip02::register_actions(&mut builder)` (in `start_nmp_app`).
//! The wire shape is `{"pubkey":"<64-char hex>"}` (`nmp_nip02::PubkeyAction`).
//!
//! ## Projection wiring
//!
//! `register_follow_list_projection(nmp_ref, active_pubkey)` (defined here)
//! wires the `FollowListProjection` event observer + typed snapshot projection
//! against the live `NmpApp`. It follows the Chirp pattern in
//! `apps/chirp/nmp-app-chirp/src/ffi/register.rs::nmp_app_chirp_register_follow_list`.
//! Call it at boot (after `nmp_app_start`) and re-call on `IdentityChanged(Some)`
//! — the projection accumulates all observed authors but only surfaces the
//! active pubkey's follow list.
//!
//! ## Threading
//!
//! `apply_follow_list` runs on the **actor thread** (inside
//! `projections::dispatch_typed_frame`, called from `reduce_event`). It is
//! synchronous and non-blocking (FlatBuffers decode only, no I/O). D6: decode
//! errors leave `AppState::follows` unchanged.

use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::{Arc, Mutex};

use nmp_ffi::NmpApp;
use nmp_nip02::projection::FollowListProjection;
use nmp_nip02::wire::typed_fb::decode_follow_list;

use crate::kernel::app::AppState;

// Re-export so callers (`projections.rs` dispatch arm) can match the schema
// id without importing nmp_nip02 directly.
pub(crate) use nmp_nip02::FOLLOW_LIST_SCHEMA_ID as SCHEMA_ID;

// ─── nmp-ffi C ABI declarations ─────────────────────────────────────────────

// `nmp_app_dispatch_action` is #[no_mangle] extern "C" in nmp-ffi/src/action.rs.
// We declare it here so the follows effect runner can call it directly.
#[allow(improper_ctypes)]
extern "C" {
    fn nmp_app_dispatch_action(
        app: *mut NmpApp,
        namespace: *const c_char,
        action_json: *const c_char,
    ) -> *mut c_char;
}

// `nmp_free_string` is the canonical free path for all C strings returned by
// nmp-ffi (they are allocated via `CString::into_raw` in the Rust allocator;
// calling host `free()` would use a different allocator — UB). Re-exported in
// `nmp_ffi::nmp_free_string` but we use the direct C ABI here to avoid a
// separate Rust function call layer.
use nmp_ffi::nmp_free_string;

// ─── READ side: projection frame apply ──────────────────────────────────────

/// Apply a decoded `"nmp.nip02.follow_list"` FlatBuffers payload to `state`.
///
/// Called from `projections::dispatch_typed_frame` when `schema_id ==
/// "nmp.nip02.follow_list"`. Updates `AppState::follows` with raw hex pubkeys.
/// D6: any decode error leaves `AppState::follows` unchanged (silent no-op).
///
/// Must be non-blocking — runs on the actor thread (FlatBuffers decode only).
pub(crate) fn apply_follow_list(state: &mut AppState, payload: &[u8]) {
    match decode_follow_list(payload) {
        Ok(snapshot) => {
            state.follows = snapshot.follows.into_iter().map(|e| e.pubkey).collect();
        }
        Err(e) => {
            tracing::trace!(
                error = %e,
                "follows::apply_follow_list: decode error — AppState::follows unchanged (D6)"
            );
        }
    }
}

// ─── WRITE side: reduce_action helpers ──────────────────────────────────────

/// Handle `AppAction::Follow { pubkey }` — emit `Effect::DispatchFollowAction`.
///
/// The reducer does NOT speculatively update `AppState::follows` (optimistic
/// update) — the authoritative update arrives via the projection frame
/// (`FollowListUpdated`) after NMP re-publishes kind:3. This keeps the state
/// machine consistent with the actual on-chain follow set (D6).
pub(crate) fn reduce_action_follow(pubkey: String) -> Vec<crate::kernel::effect::Effect> {
    vec![crate::kernel::effect::Effect::DispatchFollowAction {
        follow: true,
        pubkey,
    }]
}

/// Handle `AppAction::Unfollow { pubkey }` — emit `Effect::DispatchFollowAction`.
/// Symmetric with `reduce_action_follow`.
pub(crate) fn reduce_action_unfollow(pubkey: String) -> Vec<crate::kernel::effect::Effect> {
    vec![crate::kernel::effect::Effect::DispatchFollowAction {
        follow: false,
        pubkey,
    }]
}

// ─── Effect runner ───────────────────────────────────────────────────────────

/// Execute `Effect::DispatchFollowAction` — calls `nmp_app_dispatch_action`
/// with `"nmp.follow"` or `"nmp.unfollow"` and `{"pubkey":"<hex>"}`.
///
/// Fire-and-forget (D6): the return value (`{correlation_id}` JSON string) is
/// freed and discarded. The updated follow list arrives back as a
/// `FollowListUpdated` projection event via the NMP update callback.
///
/// No-op if `nmp` is `None` (test mode — tests inject `FollowListUpdated`
/// directly to drive the reducer).
pub(crate) fn run_effect_dispatch_follow_action(
    follow: bool,
    pubkey: String,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
) {
    let Some(handle) = nmp else { return };

    let namespace = if follow { "nmp.follow" } else { "nmp.unfollow" };
    // Serialize the wire shape: {"pubkey":"<hex>"}
    let action_json = format!("{{\"pubkey\":\"{pubkey}\"}}");

    let ns_c = match CString::new(namespace) {
        Ok(s) => s,
        Err(_) => return,
    };
    let json_c = match CString::new(action_json) {
        Ok(s) => s,
        Err(_) => return,
    };

    // SAFETY: handle.ptr is a valid non-null NmpApp pointer kept alive by
    // NmpHandle for the full actor lifetime. ns_c and json_c are valid CStrings
    // alive for the duration of this call. The returned pointer is freed below.
    let result_ptr =
        unsafe { nmp_app_dispatch_action(handle.ptr.as_ptr(), ns_c.as_ptr(), json_c.as_ptr()) };

    // Free the returned correlation-id JSON string. nmp-ffi returns a
    // CString::into_raw pointer; `nmp_free_string` is the canonical free path
    // (same Rust allocator as the allocation). Calling host `free()` would be
    // UB. A null pointer is a no-op (nmp-ffi D6 null-safety contract).
    if !result_ptr.is_null() {
        // nmp_free_string takes ownership of the CString::into_raw pointer and
        // frees it through the same Rust allocator. It handles null gracefully
        // but we guard anyway to be explicit about the non-null path.
        nmp_free_string(result_ptr);
    }
}

// ─── Projection registration ─────────────────────────────────────────────────

/// Wire the `FollowListProjection` event observer + typed snapshot projection
/// against `nmp_ref`. Follows the Chirp pattern in
/// `apps/chirp/nmp-app-chirp/src/ffi/register.rs::nmp_app_chirp_register_follow_list`.
///
/// `active_account_slot` is the live `Arc<Mutex<Option<String>>>` that NMP
/// itself updates on sign-in/switch/logout. Pass `nmp_ref.active_account_handle()`
/// so the projection auto-tracks the active account without manual updates.
/// Using a fresh `Arc::new(Mutex::new(None))` would leave the projection
/// permanently pointed at None, so follows would never populate AppState.
///
/// Must be called once at boot (after `nmp_app_start`). The slot automatically
/// reflects future identity changes because NMP writes through the same Arc.
///
/// The kernel already fetches kind:3 for the active account via the
/// `account_profile_interest` (kind:0 + kind:3 + kind:10002); no separate
/// interest push is needed — events arrive through the standing subscription.
///
/// D6: a null or poisoned observer slot degrades to a silent return without
/// registering the typed projection (so the snapshot never updates but the
/// app does not crash).
pub(crate) fn register_follow_list_projection(
    nmp_ref: &NmpApp,
    active_account_slot: Arc<Mutex<Option<String>>>,
) {
    // FollowListProjection is now a pure read-model over ContactsLookup (NMP
    // ADR-0063 Lane H). It no longer implements KernelEventObserver — the
    // canonical follow state lives in the shared ContactsLookup written by
    // Kind3Parser on every ingest, so no event observation is needed.
    let contacts_lookup = nmp_ref.contacts_lookup();
    let projection = Arc::new(FollowListProjection::new(active_account_slot, contacts_lookup));

    // Register the typed sidecar projection under the canonical key.
    // KEY = "nmp.follow_list"; SCHEMA_ID (in the payload) = "nmp.nip02.follow_list".
    // This mirrors the Chirp wiring exactly (key/schema_id split is deliberate).
    let typed_proj = Arc::clone(&projection);
    nmp_ref.register_typed_snapshot_projection("nmp.follow_list", move || {
        let snapshot = typed_proj.snapshot();
        Some(nmp_core::TypedProjectionData {
            key: "nmp.follow_list".to_string(),
            schema_id: SCHEMA_ID.to_string(),
            schema_version: nmp_nip02::FOLLOW_LIST_SCHEMA_VERSION,
            // FILE_IDENTIFIER is a &[u8;4]; convert to a UTF-8 string as
            // Chirp does (String::from_utf8_lossy — "NF02" is valid ASCII).
            file_identifier: String::from_utf8_lossy(nmp_nip02::FOLLOW_LIST_FILE_IDENTIFIER)
                .into_owned(),
            payload: nmp_nip02::encode_follow_list(&snapshot),
            ..Default::default()
        })
    });
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

    // 3C-T1: follow dispatches DispatchFollowAction{follow:true}
    //
    // AppAction::Follow{pubkey} must produce exactly one Effect::DispatchFollowAction
    // with follow=true and the correct pubkey. Fire-and-forget: dispatch returns Vec<Effect>
    // (models the () contract, Non-Negotiable #3).
    #[test]
    fn follow_dispatches_nmp_follow_action() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let pk = "deadbeef00000000000000000000000000000000000000000000000000000001";
        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::Follow {
                pubkey: pk.to_string(),
            }),
        );
        assert_eq!(effects.len(), 1, "Follow must emit exactly one effect");
        match &effects[0] {
            Effect::DispatchFollowAction { follow, pubkey } => {
                assert!(*follow, "follow=true for Follow action");
                assert_eq!(pubkey, pk, "pubkey threads through verbatim");
            }
            other => panic!("expected DispatchFollowAction, got {:?}", other),
        }
    }

    // 3C-T2: unfollow dispatches DispatchFollowAction{follow:false}
    #[test]
    fn unfollow_dispatches_nmp_unfollow_action() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let pk = "cafebabe00000000000000000000000000000000000000000000000000000002";
        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::Unfollow {
                pubkey: pk.to_string(),
            }),
        );
        assert_eq!(effects.len(), 1, "Unfollow must emit exactly one effect");
        match &effects[0] {
            Effect::DispatchFollowAction { follow, pubkey } => {
                assert!(!follow, "follow=false for Unfollow action");
                assert_eq!(pubkey, pk, "pubkey threads through verbatim");
            }
            other => panic!("expected DispatchFollowAction, got {:?}", other),
        }
    }

    // 3C-T3: follow_list_frame_updates_follow_set
    //
    // Injecting KernelEvent::FollowListUpdated with a list of pubkeys must update
    // AppState::follows and make is_following return true for listed pubkeys.
    #[test]
    fn follow_list_frame_updates_follow_set() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let pk1 = "aabbcc0000000000000000000000000000000000000000000000000000000001";
        let pk2 = "aabbcc0000000000000000000000000000000000000000000000000000000002";

        assert!(state.follows.is_empty(), "follows must start empty");
        assert!(!state.is_following(pk1), "is_following false before update");

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::FollowListUpdated(vec![
                pk1.to_string(),
                pk2.to_string(),
            ])),
        );

        assert_eq!(state.follows.len(), 2, "both pubkeys stored");
        assert!(
            state.is_following(pk1),
            "is_following true after FollowListUpdated"
        );
        assert!(
            state.is_following(pk2),
            "is_following true for second pubkey"
        );
        assert!(
            !state.is_following("0000000000000000000000000000000000000000000000000000000000000000"),
            "is_following false for unlisted pubkey"
        );
    }

    // 3C-T4: is_following_reflects_projection
    //
    // is_following is derived purely from the projection; never speculatively
    // updated by Follow/Unfollow actions.
    #[test]
    fn is_following_reflects_projection_not_action() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let pk = "1111110000000000000000000000000000000000000000000000000000000001";

        // Dispatch Follow — this must NOT speculatively set is_following.
        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::Follow {
                pubkey: pk.to_string(),
            }),
        );
        assert!(
            !state.is_following(pk),
            "is_following must remain false until FollowListUpdated arrives"
        );

        // Simulate the projection frame arriving.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::FollowListUpdated(vec![pk.to_string()])),
        );
        assert!(
            state.is_following(pk),
            "is_following true after projection update"
        );
    }

    // 3C-T5: follow dispatch returns unit (fire-and-forget, Non-Negotiable #3)
    #[test]
    fn follow_dispatch_returns_unit() {
        let mut state = make_state();
        let clock = ManualClock::new(0);
        // The return type Vec<Effect> models the () contract.
        let _effects: Vec<Effect> = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::Follow {
                pubkey: "aaaa000000000000000000000000000000000000000000000000000000000001".into(),
            }),
        );
        // No panic, no Result — fire-and-forget contract satisfied.
    }

    // 3C-T6: malformed follow_list frame (apply_follow_list) is a silent no-op (D6)
    //
    // apply_follow_list is called by projections::dispatch_typed_frame with the
    // raw FlatBuffers payload. Garbage bytes must not panic or corrupt state.
    #[test]
    fn malformed_follow_list_payload_is_noop() {
        let mut state = make_state();
        // Seed with an existing entry to confirm it is left unchanged.
        state.follows = vec!["existing_pubkey".to_string()];

        apply_follow_list(&mut state, b"NOT A VALID FLATBUFFER");

        assert_eq!(
            state.follows,
            vec!["existing_pubkey"],
            "malformed payload must leave AppState::follows unchanged (D6)"
        );
    }

    // 3C-T7: follows cleared on Logout
    //
    // AppAction::Logout must wipe AppState::follows so stale contacts from the
    // previous account don't survive into the next session.
    #[test]
    fn follows_cleared_on_logout() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let pk = "deadbeef00000000000000000000000000000000000000000000000000000001";

        // Seed follows and a present session.
        state.follows = vec![pk.to_string()];
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("somepubkey".into()))),
        );

        // Logout — follows must be cleared.
        step(&mut state, &clock, Cmd::Action(AppAction::Logout));

        assert!(
            state.follows.is_empty(),
            "follows must be empty after Logout"
        );
        assert!(
            !state.is_following(pk),
            "is_following must return false after Logout"
        );
    }

    // 3C-T8: follows cleared on IdentityChanged(None)
    //
    // NMP fires IdentityChanged(None) on account removal. AppState::follows
    // must be wiped so stale contacts don't outlive the removed account.
    #[test]
    fn follows_cleared_on_identity_changed_none() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let pk = "deadbeef00000000000000000000000000000000000000000000000000000002";

        // Seed follows and a present session.
        state.follows = vec![pk.to_string()];
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("anotherpubkey".into()))),
        );

        // Account removed — follows must be cleared.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(None)),
        );

        assert!(
            state.follows.is_empty(),
            "follows must be empty after IdentityChanged(None)"
        );
        assert!(
            !state.is_following(pk),
            "is_following must return false after account removal"
        );
    }

    // 3C-T9: empty follow list clears the follow set
    //
    // A valid follow-list frame with zero entries must empty AppState::follows.
    // This is the correct behaviour when the user unfollows everyone.
    #[test]
    fn empty_follow_list_clears_follow_set() {
        let mut state = make_state();
        let clock = ManualClock::default();

        // Seed with an existing entry.
        state.follows = vec!["some_pubkey".to_string()];

        // Inject an empty FollowListUpdated.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::FollowListUpdated(vec![])),
        );

        assert!(
            state.follows.is_empty(),
            "empty follow-list update must clear the set"
        );
        assert!(
            !state.is_following("some_pubkey"),
            "previously-followed pubkey must no longer be tracked"
        );
    }
}
