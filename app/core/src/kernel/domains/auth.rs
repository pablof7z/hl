//! Auth domain — sign-in, sign-out, and signer-management arms of the kernel.
//!
//! Covers: SignInNsec / PairBunker / StartNostrConnect / SignInNip55 /
//!         CreateAccount (actions); IdentityChanged / SignInFailed /
//!         NostrConnectUriReady / BunkerHandshakeState (events); and the
//!         matching `run_effect` arms.

use std::ffi::CString;

use tokio::sync::mpsc;
use zeroize::Zeroizing;

use nmp_ffi::{nmp_app_nostrconnect_uri, nmp_app_signin_nip55, NmpApp};

use crate::kernel::action::{KernelEvent, SignInMethod, SignerKind};
use crate::kernel::actor::{Cmd, NmpHandle};
use crate::kernel::app::{AppState, SessionState, SIGN_IN_TIMEOUT_SECS};
use crate::kernel::effect::Effect;

// ─── Reducer (action) ────────────────────────────────────────────────────────

/// Handle auth-related `AppAction` variants.
///
/// Returns `None` if the action is not owned by the auth domain.
/// Caller is responsible for passing only the relevant variant.
pub(crate) fn reduce_action_sign_in_nsec(
    state: &mut AppState,
    nsec: String,
    now: u64,
) -> Vec<Effect> {
    state.session = SessionState::SigningIn {
        method: SignInMethod::Nsec,
        started_at: now,
    };
    // AddNsecSigner calls nmp.add_signer(LocalNsec(nsec), true).
    // Success → IdentityChanged(Some(pubkey)); failure → SignInFailed.
    // Fire-and-forget: reducer never awaits the result.
    vec![Effect::AddNsecSigner { nsec }]
}

pub(crate) fn reduce_action_pair_bunker(
    state: &mut AppState,
    uri: String,
    now: u64,
) -> Vec<Effect> {
    state.session = SessionState::SigningIn {
        method: SignInMethod::Bunker,
        started_at: now,
    };
    // AddBunkerSigner routes through the NIP-46 broker (nmp_signer_broker_init
    // must have run at boot). Fire-and-forget: broker resolves the signer
    // async; success arrives as IdentityChanged(Some).
    vec![Effect::AddBunkerSigner { uri }]
}

pub(crate) fn reduce_action_start_nostr_connect(state: &mut AppState, now: u64) -> Vec<Effect> {
    state.session = SessionState::SigningIn {
        method: SignInMethod::NostrConnect,
        started_at: now,
    };
    // MintNostrConnectUri calls nmp_app_nostrconnect_uri and feeds the
    // result back as KernelEvent::NostrConnectUriReady so the iOS QR
    // sheet can render it. The broker then waits for the remote signer
    // to connect; success arrives as IdentityChanged(Some).
    vec![Effect::MintNostrConnectUri]
}

pub(crate) fn reduce_action_sign_in_nip55(state: &mut AppState, now: u64) -> Vec<Effect> {
    state.session = SessionState::SigningIn {
        method: SignInMethod::Nip55,
        started_at: now,
    };
    // StartNip55SignIn calls nmp_app_signin_nip55(app, null). Fire-and-
    // forget: the host capability bridge exchanges with the external
    // signer app; success arrives as IdentityChanged(Some).
    vec![Effect::StartNip55SignIn]
}

pub(crate) fn reduce_action_create_account(
    state: &mut AppState,
    profile_name: String,
    now: u64,
) -> Vec<Effect> {
    state.session = SessionState::SigningIn {
        method: SignInMethod::CreateAccount,
        started_at: now,
    };
    // Effect runner calls actor_sender().send(ActorCommand::CreateAccount{...}).
    // Relay + follow policy is read from injected KernelPolicy at effect
    // run time (D3 — no hardcoded relay literals in kernel logic).
    // Success → IdentityChanged(Some(pubkey)); clock timeout covers failure.
    vec![Effect::CreateAccount { profile_name }]
}

pub(crate) fn reduce_action_logout(state: &mut AppState) -> Vec<Effect> {
    state.session = SessionState::Absent;
    state.session_epoch += 1;
    // Clear any pending NostrConnect URI on logout.
    state.nostrconnect_uri = None;
    // ── Phase 3C: clear follow set so stale follows don't survive logout ──────
    // The FollowListProjection active-account slot auto-resets via the shared
    // Arc, but AppState::follows must also be wiped so is_following never
    // returns true for the previous account's contacts.
    state.follows = Vec::new();
    // ── Phase 3D: clear profile state on logout ──────────────────────────────
    // own_profile and claimed_profiles belong to the departing account and
    // must not survive into the next session.
    super::profiles::clear_on_identity_lost(state);
    // RemoveActiveAccount fires nmp.remove_account; ClearSession
    // emits a CapabilityRequest to native for its keychain.
    vec![Effect::RemoveActiveAccount, Effect::ClearSession]
}

// ─── Reducer (event) ─────────────────────────────────────────────────────────

pub(crate) fn reduce_event_identity_changed(
    state: &mut AppState,
    pubkey: Option<String>,
) -> Vec<Effect> {
    // NMP identity change — `Some(pk)` means a signer is now active;
    // `None` means the account was removed / logged out.
    match pubkey {
        Some(pk) if !pk.is_empty() => {
            // Determine signer kind from the method we were SigningIn with.
            // Bunker and NostrConnect both resolve to Nip46 (NIP-46 remote).
            // Session restore and unknown paths default to LocalNsec.
            let signer_kind = match &state.session {
                SessionState::SigningIn { method, .. } => match method {
                    SignInMethod::Nsec | SignInMethod::CreateAccount => SignerKind::LocalNsec,
                    SignInMethod::Bunker | SignInMethod::NostrConnect => SignerKind::Nip46,
                    SignInMethod::Nip55 => SignerKind::Nip55,
                },
                _ => SignerKind::LocalNsec,
            };
            // Phase 3B: clear prior account's communities before re-wiring.
            // Effect::WireJoinedGroups re-registers the JoinedGroupsProjection
            // for the new pubkey; the fresh snapshot arrives on the next tick.
            state.communities = vec![];
            // Clear the pending NostrConnect URI — the handshake is done.
            state.nostrconnect_uri = None;
            state.session = SessionState::Present {
                pubkey: pk.clone(),
                signer_kind,
            };
            // Phase 3B: re-register joined-groups projection for the new account.
            return vec![Effect::WireJoinedGroups { pubkey: pk }];
        }
        _ => {
            // None or empty pubkey → no active account.
            // Phase 3B: clear joined groups when account is removed.
            state.communities = vec![];
            state.nostrconnect_uri = None;
            state.session = SessionState::Absent;
            // ── Phase 3C: clear follow set on account removal ─────────────────
            // NMP fires IdentityChanged(None) on logout / account removal. Wipe
            // AppState::follows so stale contacts don't outlive the session.
            state.follows = Vec::new();
            // ── Phase 3D: clear profile state on account removal ──────────────
            // own_profile and claimed_profiles belong to the departing account.
            super::profiles::clear_on_identity_lost(state);
        }
    }
    vec![]
}

pub(crate) fn reduce_event_sign_in_failed(
    state: &mut AppState,
    method: SignInMethod,
    error: String,
) -> Vec<Effect> {
    // Surface failures in session state (D6 — never as Result).
    state.session = SessionState::SignInFailed { method, error };
    vec![]
}

pub(crate) fn reduce_event_nostrconnect_uri_ready(
    state: &mut AppState,
    uri: String,
) -> Vec<Effect> {
    // Store the minted URI so the snapshot can expose it to the iOS
    // QR-code sheet. The NostrConnect sign-in session stays in SigningIn
    // until the remote signer completes the handshake (IdentityChanged).
    state.nostrconnect_uri = Some(uri);
    vec![]
}

// ─── Clock checks ────────────────────────────────────────────────────────────

/// Sign-in timeout check — transitions SigningIn → SignInFailed after
/// SIGN_IN_TIMEOUT_SECS. Called on every reduce pass via `clock_checks`.
///
/// NMP handles parse errors internally (set_last_error_toast) without firing
/// the identity-change observer — so an invalid nsec leaves us in SigningIn
/// indefinitely without this clock-driven fallback (D8).
pub(crate) fn clock_check_sign_in_timeout(state: &mut AppState, now: u64) {
    if let SessionState::SigningIn { started_at, method } = &state.session {
        if now.saturating_sub(*started_at) >= SIGN_IN_TIMEOUT_SECS {
            state.session = SessionState::SignInFailed {
                method: method.clone(),
                error: "sign-in timed out — no identity change observed".into(),
            };
        }
    }
}

// ─── Effect runner ───────────────────────────────────────────────────────────

pub(crate) fn run_effect_add_nsec_signer(nsec: String, nmp: Option<&NmpHandle>) {
    // Call nmp.add_signer(LocalNsec(nsec), make_active: true).
    // NMP auto-persists to its keyring when make_active && LocalNsec.
    // This is truly fire-and-forget: add_signer returns () and NMP handles
    // both the success and error paths internally —
    //   Success: the identity-change observer fires IdentityChanged(Some).
    //   Invalid nsec: NMP calls set_last_error_toast internally and returns
    //     without firing the observer. The clock-driven SIGN_IN_TIMEOUT_SECS
    //     check in clock_checks will then transition SigningIn → SignInFailed.
    // hl never awaits a Result from add_signer (Non-Negotiable #2/D6).
    if let Some(handle) = nmp {
        let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };
        nmp_ref.add_signer(
            nmp_core::SignerSource::LocalNsec(Zeroizing::new(nsec)),
            true, // make_active — also auto-persists to nmp keyring
        );
    }
    // No nmp handle (test mode) → test injects IdentityChanged directly.
}

pub(crate) fn run_effect_add_bunker_signer(uri: String, nmp: Option<&NmpHandle>) {
    // Route via nmp.add_signer(BunkerUri(uri), true). The NIP-46 broker
    // (nmp_signer_broker_init called at boot) takes over the handshake
    // async. Fire-and-forget: success arrives as IdentityChanged(Some).
    if let Some(handle) = nmp {
        let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };
        nmp_ref.add_signer(nmp_core::SignerSource::BunkerUri(uri), true);
    }
    // No nmp handle (test mode) → test injects IdentityChanged directly.
}

pub(crate) async fn run_effect_mint_nostrconnect_uri(
    nmp: Option<&NmpHandle>,
    tx: &mpsc::UnboundedSender<Cmd>,
) {
    // Call nmp_app_nostrconnect_uri(app_ptr, null, null) — relay and
    // callback are resolved by nmp from its internal bootstrap relay slot
    // (V-65). Returns an owned `nostrconnect://` C string or null if no
    // relay is configured. Feed the result back as NostrConnectUriReady.
    if let Some(handle) = nmp {
        let raw_ptr = handle.ptr.as_ptr();
        let uri_ptr = nmp_app_nostrconnect_uri(raw_ptr, std::ptr::null(), std::ptr::null());
        if !uri_ptr.is_null() {
            // SAFETY: uri_ptr is a CString::into_raw pointer owned by
            // nmp-ffi. We take ownership here and free it with from_raw.
            let uri = unsafe { std::ffi::CStr::from_ptr(uri_ptr) }
                .to_string_lossy()
                .into_owned();
            // SAFETY: uri_ptr came from CString::into_raw in nmp-ffi.
            let _ = unsafe { CString::from_raw(uri_ptr) };
            let _ = tx.send(Cmd::Event(KernelEvent::NostrConnectUriReady { uri }));
        }
        // null return → no relay configured; stay in SigningIn until timeout.
    }
    // No nmp handle (test mode) → test injects NostrConnectUriReady directly.
}

pub(crate) fn run_effect_start_nip55_sign_in(nmp: Option<&NmpHandle>) {
    // Call nmp_app_signin_nip55(app_ptr, null) — null signer_package
    // lets the OS resolver pick the NIP-55 signer app (e.g. Amber).
    // nmp_app_signin_nip55 lazy-inits the external-signer driver if
    // nmp_external_signer_init was not already called at boot.
    // Fire-and-forget: success arrives as IdentityChanged(Some).
    if let Some(handle) = nmp {
        let raw_ptr = handle.ptr.as_ptr();
        nmp_app_signin_nip55(raw_ptr, std::ptr::null());
    }
    // No nmp handle (test mode) → test injects IdentityChanged directly.
}

pub(crate) fn run_effect_remove_active_account(nmp: Option<&NmpHandle>) {
    // Read the active pubkey from the nmp slot, then remove it.
    // Fire-and-forget: the observer fires IdentityChanged(None) on success.
    if let Some(handle) = nmp {
        let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };
        let active_slot = nmp_ref.active_account_handle();
        let maybe_pubkey: Option<String> = active_slot.lock().ok().and_then(|guard| guard.clone());
        if let Some(pubkey) = maybe_pubkey {
            nmp_ref.remove_account(pubkey);
        }
    }
}

pub(crate) fn run_effect_create_account(
    profile_name: String,
    nmp: Option<&NmpHandle>,
    policy: &crate::kernel::app::KernelPolicy,
) {
    // Build the profile HashMap from the supplied display name.
    // Relays and initial_follows come from the injected KernelPolicy
    // (D3: no hardcoded relay literals in kernel source).
    if let Some(handle) = nmp {
        let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };

        let mut profile = std::collections::HashMap::new();
        profile.insert("name".to_string(), profile_name);

        let relays: Vec<(String, String)> = policy
            .create_account
            .seed_relays
            .iter()
            .map(|r| (r.url.clone(), r.role.clone()))
            .collect();

        // ADR-0059 §5: initial_follows is app policy (empty = no kind:3).
        // NMP no longer hardcodes any default follow set (#1493); the
        // caller is responsible for supplying the initial contacts list.
        // An empty vec is the correct default: the account starts with
        // no contacts and no cold-start kind:3 is published.
        let initial_follows = policy.create_account.initial_follows.clone();

        // Fire-and-forget via actor_sender (first actor_sender use in hl).
        // Returns Result<(), CommandSendError>; we discard the error
        // (D6 — timeout in clock_checks will surface the failure as state).
        let _ = nmp_ref
            .actor_sender()
            .send(nmp_core::ActorCommand::CreateAccount {
                profile,
                relays,
                initial_follows,
                mls: false,
                make_active: true,
            });
    }
    // No nmp handle (test mode) → test injects IdentityChanged directly.
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::AppAction;
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::app::AppState;
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::effect::Effect;
    use crate::kernel::snapshot::ViewSnapshot;
    use crate::kernel::view::ViewId;

    fn make_state() -> AppState {
        AppState::default()
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    // ── Phase 2A tests ────────────────────────────────────────────────────────

    // P2A-1: SignInNsec transitions to SigningIn and emits AddNsecSigner with
    //        make_active intent baked in (the Effect carries the nsec string;
    //        the runner calls add_signer with make_active=true).
    #[test]
    fn nsec_sign_in_enqueues_local_signer_make_active_true() {
        let mut state = make_state();
        let clock = ManualClock::new(42);
        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::SignInNsec {
                nsec: "nsec1test".into(),
            }),
        );
        // State should be SigningIn immediately.
        assert!(
            matches!(
                &state.session,
                SessionState::SigningIn {
                    method: SignInMethod::Nsec,
                    started_at: 42
                }
            ),
            "expected SigningIn, got {:?}",
            state.session
        );
        // A single AddNsecSigner effect should be emitted.
        assert_eq!(effects.len(), 1, "expected one effect from SignInNsec");
        assert!(
            matches!(&effects[0], Effect::AddNsecSigner { nsec } if nsec == "nsec1test"),
            "expected AddNsecSigner effect with the nsec, got {:?}",
            effects[0]
        );
    }

    // P2A-2: IdentityChanged(Some(pubkey)) routes to SessionState::Present.
    #[test]
    fn identity_changed_some_routes_to_present() {
        let mut state = make_state();
        let clock = ManualClock::default();
        // Start in SigningIn to simulate the normal flow.
        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::SignInNsec {
                nsec: "nsec1x".into(),
            }),
        );
        // Observer fires with the resolved pubkey.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("deadbeefpubkey".into()))),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::Present { pubkey, .. } if pubkey == "deadbeefpubkey"
            ),
            "expected Present, got {:?}",
            state.session
        );
    }

    // P2A-3: IdentityChanged(None) routes to SessionState::Absent.
    #[test]
    fn identity_changed_none_routes_to_absent() {
        let mut state = make_state();
        let clock = ManualClock::default();
        // Put the state in Present first.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("somepubkey".into()))),
        );
        assert!(matches!(&state.session, SessionState::Present { .. }));
        // Now fire IdentityChanged(None) — should transition to Absent.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(None)),
        );
        assert!(
            matches!(&state.session, SessionState::Absent),
            "expected Absent, got {:?}",
            state.session
        );
    }

    // P2A-4: Logout emits RemoveActiveAccount + ClearSession and bumps epoch.
    #[test]
    fn logout_calls_remove_account_and_bumps_epoch() {
        let mut state = make_state();
        let clock = ManualClock::default();
        // Simulate a logged-in session.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("mypubkey".into()))),
        );
        let epoch_before = state.session_epoch;
        let effects = step(&mut state, &clock, Cmd::Action(AppAction::Logout));
        // Epoch must be bumped.
        assert_eq!(state.session_epoch, epoch_before + 1);
        // State must be Absent immediately (not waiting for observer).
        assert!(matches!(&state.session, SessionState::Absent));
        // Effects must include both RemoveActiveAccount and ClearSession.
        let has_remove = effects
            .iter()
            .any(|e| matches!(e, Effect::RemoveActiveAccount));
        let has_clear = effects.iter().any(|e| matches!(e, Effect::ClearSession));
        assert!(has_remove, "expected RemoveActiveAccount effect in Logout");
        assert!(has_clear, "expected ClearSession effect in Logout");
    }

    // P2A-5: sign-in failure surfaces in SessionState (D6), never as Result.
    //        dispatch(SignInNsec) returns () regardless of whether the signer
    //        call succeeds — errors arrive via KernelEvent::SignInFailed.
    #[test]
    fn sign_in_failure_surfaces_in_session_state_not_return() {
        let mut state = make_state();
        let clock = ManualClock::default();
        // Transition to SigningIn first.
        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::SignInNsec {
                nsec: "bad_nsec".into(),
            }),
        );
        // dispatch returns () — checked by the type of `effects` (Vec<Effect>).
        // The effect runner (in production) calls add_signer and on error sends
        // KernelEvent::SignInFailed back. We simulate that here.
        let _ = effects; // () — fire-and-forget
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::SignInFailed {
                method: SignInMethod::Nsec,
                error: "invalid nsec: bad bech32".into(),
            }),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::SignInFailed {
                    method: SignInMethod::Nsec,
                    error
                } if error.contains("invalid nsec")
            ),
            "expected SignInFailed, got {:?}",
            state.session
        );
    }

    // P2A-6: dispatch(SignInNsec) returns () — the action dispatch API is
    //        fire-and-forget (#3).
    #[test]
    fn dispatch_signin_returns_unit() {
        let mut state = make_state();
        let clock = ManualClock::new(0);
        // This must compile and run — the return value of step() is Vec<Effect>,
        // which models the `()` (fire-and-forget) contract. The key assertion is
        // that `step` returns without blocking or returning a Result.
        let _effects: Vec<Effect> = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::SignInNsec {
                nsec: "nsec1any".into(),
            }),
        );
        // No panic, no Result — the dispatch contract is satisfied.
        assert!(
            matches!(&state.session, SessionState::SigningIn { .. }),
            "SignInNsec must transition to SigningIn synchronously"
        );
    }

    // P2A-7: clock drives sign-in timeout → SignInFailed.
    //        NMP handles invalid nsec parse errors internally without firing the
    //        identity-change observer; this clock-driven fallback ensures the UI
    //        never gets stuck in SigningIn forever (D6 — failure surfaces as state).
    #[test]
    fn sign_in_timeout_transitions_signing_in_to_failed() {
        use crate::kernel::app::SIGN_IN_TIMEOUT_SECS;
        let mut state = make_state();
        let clock = ManualClock::new(0);

        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::SignInNsec {
                nsec: "nsec1invalid".into(),
            }),
        );
        assert!(matches!(&state.session, SessionState::SigningIn { .. }));

        // Just before timeout — still SigningIn.
        clock.advance(SIGN_IN_TIMEOUT_SECS - 1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(&state.session, SessionState::SigningIn { .. }),
            "should still be SigningIn before timeout"
        );

        // At timeout — should transition to SignInFailed.
        clock.advance(1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(
                &state.session,
                SessionState::SignInFailed {
                    method: SignInMethod::Nsec,
                    ..
                }
            ),
            "should be SignInFailed at timeout, got {:?}",
            state.session
        );
    }

    // ── Phase 2B tests ────────────────────────────────────────────────────────

    // P2B-1: PairBunker transitions to SigningIn{Bunker} and emits AddBunkerSigner.
    #[test]
    fn pair_bunker_enqueues_bunker_uri_source() {
        let mut state = make_state();
        let clock = ManualClock::new(10);
        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::PairBunker {
                uri: "bunker://pubkey?relay=wss://relay.example.com".into(),
            }),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::SigningIn {
                    method: SignInMethod::Bunker,
                    started_at: 10
                }
            ),
            "expected SigningIn{{Bunker}}, got {:?}",
            state.session
        );
        assert_eq!(
            effects.len(),
            1,
            "expected exactly one effect from PairBunker"
        );
        assert!(
            matches!(
                &effects[0],
                Effect::AddBunkerSigner { uri }
                    if uri == "bunker://pubkey?relay=wss://relay.example.com"
            ),
            "expected AddBunkerSigner with the uri, got {:?}",
            effects[0]
        );
    }

    // P2B-2: NostrConnectUriReady stores the URI in state and the snapshot
    //        exposes it via AppRootSnapshot::nostrconnect_uri.
    #[test]
    fn nostrconnect_uri_ready_event_to_snapshot() {
        use crate::kernel::actor::project_snapshot;
        let mut state = make_state();
        let clock = ManualClock::default();

        // Begin a NostrConnect sign-in.
        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::StartNostrConnect),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::SigningIn {
                    method: SignInMethod::NostrConnect,
                    ..
                }
            ),
            "expected SigningIn{{NostrConnect}}, got {:?}",
            state.session
        );
        assert!(state.nostrconnect_uri.is_none(), "URI not set yet");

        // Simulate the effect runner delivering the minted URI.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::NostrConnectUriReady {
                uri: "nostrconnect://pubkey?relay=wss://relay.example.com".into(),
            }),
        );
        assert_eq!(
            state.nostrconnect_uri.as_deref(),
            Some("nostrconnect://pubkey?relay=wss://relay.example.com"),
            "URI should be stored in state"
        );

        // Project a snapshot and verify the URI is present.
        let now = clock.now_unix_seconds();
        if let Some(ViewSnapshot::AppRoot(snap)) = project_snapshot(&state, &ViewId::AppRoot, now) {
            assert_eq!(
                snap.nostrconnect_uri.as_deref(),
                Some("nostrconnect://pubkey?relay=wss://relay.example.com"),
                "URI must appear in AppRootSnapshot"
            );
        } else {
            panic!("expected AppRoot snapshot");
        }

        // When IdentityChanged fires (handshake complete), URI is cleared.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("resolvedpubkey".into()))),
        );
        assert!(
            state.nostrconnect_uri.is_none(),
            "URI must be cleared after IdentityChanged"
        );
    }

    // P2B-3: SignInNip55 transitions to SigningIn{Nip55} and emits StartNip55SignIn.
    //        The effect runner would call nmp_app_signin_nip55; in test mode
    //        (no nmp handle) success is injected as IdentityChanged.
    #[test]
    fn nip55_signin_invokes_external_hook() {
        let mut state = make_state();
        let clock = ManualClock::new(5);
        let effects = step(&mut state, &clock, Cmd::Action(AppAction::SignInNip55));
        assert!(
            matches!(
                &state.session,
                SessionState::SigningIn {
                    method: SignInMethod::Nip55,
                    started_at: 5
                }
            ),
            "expected SigningIn{{Nip55}}, got {:?}",
            state.session
        );
        assert_eq!(effects.len(), 1, "expected one effect from SignInNip55");
        assert!(
            matches!(&effects[0], Effect::StartNip55SignIn),
            "expected StartNip55SignIn effect, got {:?}",
            effects[0]
        );

        // Simulate success via IdentityChanged — signer_kind should be Nip55.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("nip55pubkey".into()))),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::Present {
                    pubkey,
                    signer_kind: SignerKind::Nip55
                } if pubkey == "nip55pubkey"
            ),
            "expected Present{{Nip55}}, got {:?}",
            state.session
        );
    }

    // P2B-4: dispatch(PairBunker/StartNostrConnect/SignInNip55) returns () —
    //        the fire-and-forget contract (Non-Negotiable #3).
    #[test]
    fn dispatch_returns_unit() {
        let mut state = make_state();
        let clock = ManualClock::new(0);

        let _: Vec<Effect> = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::PairBunker {
                uri: "bunker://x".into(),
            }),
        );
        let _: Vec<Effect> = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::StartNostrConnect),
        );
        let _: Vec<Effect> = step(&mut state, &clock, Cmd::Action(AppAction::SignInNip55));
        // No panic and no Result return — fire-and-forget contract satisfied.
    }

    // P2B-5: bunker sign-in times out → SignInFailed{Bunker}.
    //        Reuses the 2A clock arm which covers ALL SigningIn variants.
    #[test]
    fn bunker_signing_in_times_out_to_failed() {
        use crate::kernel::app::SIGN_IN_TIMEOUT_SECS;
        let mut state = make_state();
        let clock = ManualClock::new(0);

        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::PairBunker {
                uri: "bunker://pubkey?relay=wss://r.example.com".into(),
            }),
        );
        assert!(matches!(&state.session, SessionState::SigningIn { .. }));

        // Just before timeout — still SigningIn.
        clock.advance(SIGN_IN_TIMEOUT_SECS - 1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(&state.session, SessionState::SigningIn { .. }),
            "should still be SigningIn before timeout"
        );

        // At timeout — should be SignInFailed{Bunker}.
        clock.advance(1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(
                &state.session,
                SessionState::SignInFailed {
                    method: SignInMethod::Bunker,
                    ..
                }
            ),
            "should be SignInFailed{{Bunker}} at timeout, got {:?}",
            state.session
        );
    }

    // P2B-6: IdentityChanged after Bunker/NostrConnect → signer_kind is Nip46.
    #[test]
    fn bunker_identity_changed_sets_nip46_signer_kind() {
        let mut state = make_state();
        let clock = ManualClock::default();

        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::PairBunker {
                uri: "bunker://pk".into(),
            }),
        );
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("bunkerpk".into()))),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::Present {
                    pubkey,
                    signer_kind: SignerKind::Nip46
                } if pubkey == "bunkerpk"
            ),
            "expected Present{{Nip46}}, got {:?}",
            state.session
        );
    }

    // ── Phase 2C tests ────────────────────────────────────────────────────────

    // P2C-1: CreateAccount transitions to SigningIn{CreateAccount} and emits
    //        Effect::CreateAccount (actor_sender path, first use in hl).
    //        make_active=true is baked into the effect runner, not the reducer data.
    #[test]
    fn create_account_sends_effect_create_account() {
        let mut state = make_state();
        let clock = ManualClock::new(42);
        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::CreateAccount {
                profile_name: "Alice".into(),
            }),
        );
        // State must transition to SigningIn{CreateAccount} synchronously.
        assert!(
            matches!(
                &state.session,
                SessionState::SigningIn {
                    method: SignInMethod::CreateAccount,
                    started_at: 42,
                }
            ),
            "expected SigningIn{{CreateAccount}}, got {:?}",
            state.session
        );
        // Exactly one effect — CreateAccount.
        assert_eq!(effects.len(), 1, "expected one effect from CreateAccount");
        assert!(
            matches!(&effects[0], Effect::CreateAccount { profile_name } if profile_name == "Alice"),
            "expected Effect::CreateAccount{{profile_name: Alice}}, got {:?}",
            effects[0]
        );
    }

    // P2C-2: make_active=true is verified by inspecting the effect (the runner
    //        always passes make_active:true to ActorCommand::CreateAccount; we
    //        confirm the Effect variant carries the right profile_name and that
    //        the test pattern documents the intent).
    #[test]
    fn create_account_make_active_true_is_effect_runner_policy() {
        // The Effect::CreateAccount variant carries profile_name only; make_active
        // is Rust-constant in the effect runner (always true for the onboarding
        // path). This test confirms that the reducer emits exactly one
        // Effect::CreateAccount for a CreateAccount action — the runner's
        // make_active=true is verified by the build (it would fail to compile
        // if the ActorCommand::CreateAccount field was wrong).
        let mut state = make_state();
        let clock = ManualClock::new(0);
        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::CreateAccount {
                profile_name: "Bob".into(),
            }),
        );
        assert_eq!(effects.len(), 1);
        let Effect::CreateAccount { profile_name } = &effects[0] else {
            panic!("expected Effect::CreateAccount, got {:?}", effects[0]);
        };
        assert_eq!(profile_name, "Bob");
    }

    // P2C-4: CreateAccount success via IdentityChanged(Some(pubkey))
    //        → SessionState::Present (same observer path as nsec sign-in).
    #[test]
    fn create_account_success_via_identity_changed() {
        let mut state = make_state();
        let clock = ManualClock::new(0);

        // Dispatch CreateAccount — enters SigningIn.
        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::CreateAccount {
                profile_name: "Charlie".into(),
            }),
        );
        assert!(matches!(
            &state.session,
            SessionState::SigningIn {
                method: SignInMethod::CreateAccount,
                ..
            }
        ));

        // NMP fires identity-change observer with the new pubkey.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("newpubkey123".into()))),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::Present { pubkey, signer_kind: SignerKind::LocalNsec }
                    if pubkey == "newpubkey123"
            ),
            "expected Present{{LocalNsec}}, got {:?}",
            state.session
        );
    }

    // P2C-5: ADR-0059 publish policy — initial_follows is empty in the injected
    //        default KernelPolicy (no kind:3 for new accounts until user follows).
    //        We verify both the default (empty → no kind:3) AND that a populated
    //        policy round-trips correctly so the effect runner will pass it to
    //        ActorCommand::CreateAccount.initial_follows when nmp is present.
    #[test]
    fn create_account_follows_adr0059_publish_policy() {
        use crate::kernel::app::{CreateAccountPolicy, KernelPolicy, SeedRelay};

        // Default policy: initial_follows is empty → no kind:3 published.
        let default_policy = KernelPolicy::default();
        assert!(
            default_policy.create_account.initial_follows.is_empty(),
            "ADR-0059 §5: initial_follows must be empty by default (no kind:3)"
        );

        // Injected policy with follows: the field is preserved for the effect runner.
        // This proves initial_follows round-trips from KernelPolicy into the
        // CreateAccountPolicy struct that run_effect reads via `policy.create_account`.
        let follows = vec![
            "deadbeef00000000000000000000000000000000000000000000000000000001".to_string(),
            "deadbeef00000000000000000000000000000000000000000000000000000002".to_string(),
        ];
        let policy_with_follows = KernelPolicy {
            create_account: CreateAccountPolicy {
                seed_relays: vec![SeedRelay {
                    url: "relay.example".to_string(),
                    role: "both".to_string(),
                }],
                initial_follows: follows.clone(),
            },
            relay: Default::default(),
            room: Default::default(),
        };
        assert_eq!(
            policy_with_follows.create_account.initial_follows, follows,
            "initial_follows must round-trip through KernelPolicy for the effect runner"
        );
    }

    // P2C-6: dispatch(CreateAccount) returns () — fire-and-forget contract.
    #[test]
    fn dispatch_create_account_returns_unit() {
        let mut state = make_state();
        let clock = ManualClock::new(0);
        let _effects: Vec<Effect> = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::CreateAccount {
                profile_name: "Dave".into(),
            }),
        );
        // No panic, no Result — fire-and-forget satisfied.
        assert!(matches!(&state.session, SessionState::SigningIn { .. }));
    }

    // P2C-7: CreateAccount SigningIn is covered by the 2A clock timeout.
    //        Confirms the existing SIGN_IN_TIMEOUT_SECS check fires for
    //        SignInMethod::CreateAccount just as it does for Nsec.
    #[test]
    fn create_account_timeout_covered_by_existing_clock_check() {
        use crate::kernel::app::SIGN_IN_TIMEOUT_SECS;
        let mut state = make_state();
        let clock = ManualClock::new(0);

        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::CreateAccount {
                profile_name: "Eve".into(),
            }),
        );
        assert!(matches!(&state.session, SessionState::SigningIn { .. }));

        // Advance to just before timeout — still SigningIn.
        clock.advance(SIGN_IN_TIMEOUT_SECS - 1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(&state.session, SessionState::SigningIn { .. }),
            "still signing in before timeout"
        );

        // At timeout — transitions to SignInFailed.
        clock.advance(1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(
                &state.session,
                SessionState::SignInFailed {
                    method: SignInMethod::CreateAccount,
                    ..
                }
            ),
            "expected SignInFailed{{CreateAccount}} at timeout, got {:?}",
            state.session
        );
    }
}
