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
//! `register_follow_list_projection(nmp_ref)` (defined here) delegates to
//! `nmp_nip02::register_follow_state_runtime`, wiring the `"nmp.follow_list"`
//! typed snapshot projection against the live `NmpApp`. It follows the Chirp
//! pattern in `apps/chirp/nmp-app-chirp/src/ffi/register.rs::nmp_app_chirp_register_follow_list`.
//! ADR-0063: the projection is a PURE READ over the shared `ContactsLookup`
//! (no filterless event observer); account changes are tracked internally by the
//! runtime registrar, so a single boot-time call suffices.
//!
//! ## Threading
//!
//! `apply_follow_list` runs on the **actor thread** (inside
//! `projections::dispatch_typed_frame`, called from `reduce_event`). It is
//! synchronous and non-blocking (FlatBuffers decode only, no I/O). D6: decode
//! errors leave `AppState::follows` unchanged.

use nmp_core::dispatch_envelope::{encode_dispatch_envelope, DISPATCH_ENVELOPE_SCHEMA_VERSION};
use nmp_core::substrate::ActionPayload;
use nmp_ffi::{nmp_app_dispatch_action_bytes, nmp_free_string, NmpApp};
use nmp_nip02::wire::typed_fb::decode_follow_list;
use nmp_nip02::{register_follow_state_runtime, PubkeyAction};

use crate::kernel::app::AppState;

// Re-export so callers (`projections.rs` dispatch arm) can match the schema
// id without importing nmp_nip02 directly.
pub(crate) use nmp_nip02::FOLLOW_LIST_SCHEMA_ID as SCHEMA_ID;

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
    let action = PubkeyAction { pubkey };
    let payload_bytes = action.encode();
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let envelope = encode_dispatch_envelope(
        &correlation_id,
        namespace,
        DISPATCH_ENVELOPE_SCHEMA_VERSION,
        &payload_bytes,
    );

    let result_ptr =
        nmp_app_dispatch_action_bytes(handle.ptr.as_ptr(), envelope.as_ptr(), envelope.len());

    // Free the returned correlation-id JSON string. nmp-ffi returns a
    // CString::into_raw pointer; `nmp_free_string` is the canonical free path
    // (same Rust allocator as the allocation). Calling host `free()` is UB.
    // A null pointer is a no-op (nmp-ffi D6 null-safety contract).
    if !result_ptr.is_null() {
        nmp_free_string(result_ptr);
    }
}

// ─── Projection registration ─────────────────────────────────────────────────

/// Wire the NIP-02 follow-list runtime against `nmp_ref`. Delegates to
/// `nmp_nip02::register_follow_state_runtime`, mirroring the Chirp pattern in
/// `apps/chirp/nmp-app-chirp/src/ffi/register.rs::nmp_app_chirp_register_follow_list`.
///
/// ADR-0063: `FollowListProjection` is no longer a filterless event observer; it is
/// a PURE READ over the shared `nmp_core::substrate::ContactsLookup` (written by
/// `nmp_nip01::Kind3Parser` on every kind:3 ingest, including cache-serve and
/// local publishes). `register_follow_state_runtime` sources both the active-
/// account slot (`app.active_pubkey()`) and the canonical contacts lookup
/// (`app.contacts_lookup()` — the SAME `Arc` the `Kind3Parser` writes into) from
/// the live `NmpApp`, then:
///   * registers the `"nmp.follow_list"` typed snapshot projection (schema_id
///     `"nmp.nip02.follow_list"`) so `AppState::follows` keeps updating, and
///   * enqueues a demand-driven `OpenInterest` for `{"kinds":[3],"authors":[<active>]}`
///     (re-opened on each account change) so cache-serve populates the lookup
///     before the first snapshot tick.
///
/// Must be called once at boot (after `nmp_app_start`). Account changes are
/// tracked internally via the identity-change observer NMP installs.
pub(crate) fn register_follow_list_projection(nmp_ref: &NmpApp) {
    // The shared ContactsLookup — the SAME Arc the Kind3Parser writes into via
    // the ingest pipeline. Passed explicitly so the generic runtime registrar
    // depends only on nmp-core traits, not on nmp-ffi (matches Chirp exactly).
    let contacts_lookup = nmp_ref.contacts_lookup();
    register_follow_state_runtime(nmp_ref, contacts_lookup);
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
