//! Session domain — keychain-backed session restore / clear arms of the kernel.
//!
//! Covers: RestoreSession / RetryRestore (actions); SessionRestored /
//!         CapabilityResult (events); and the RestoreSessionSecret / ClearSession /
//!         EmitCapabilityRequest effect-runner arms.

use tokio::sync::mpsc;

use crate::capabilities::{
    CapabilityRequest, CapabilityResult, KeychainOp, KeychainResult, ShareResult,
};
use crate::kernel::action::KernelEvent;
use crate::kernel::action::SignerKind;
use crate::kernel::actor::{Cmd, SharedState};
use crate::kernel::app::{AppState, SessionState, SESSION_RESTORE_TIMEOUT_SECS};
use crate::kernel::effect::Effect;

// ─── Reducer (action) ────────────────────────────────────────────────────────

pub(crate) fn reduce_action_restore_session(state: &mut AppState, now: u64) -> Vec<Effect> {
    if matches!(state.session, SessionState::Present { .. }) {
        return vec![Effect::LoadOnboardingFlag];
    }
    state.session = SessionState::Restoring { started_at: now };
    // Fire both onboarding load and session restore in parallel.
    vec![Effect::LoadOnboardingFlag, Effect::RestoreSessionSecret]
}

// ─── Reducer (event) ─────────────────────────────────────────────────────────

pub(crate) fn reduce_event_session_restored(
    state: &mut AppState,
    present: bool,
    pubkey: Option<String>,
) -> Vec<Effect> {
    if present {
        state.session = SessionState::Present {
            pubkey: pubkey.unwrap_or_default(),
            signer_kind: SignerKind::LocalNsec,
        };
    } else {
        state.session = SessionState::Absent;
    }
    vec![]
}

pub(crate) fn reduce_event_capability_result(
    state: &mut AppState,
    result: CapabilityResult,
) -> Vec<Effect> {
    match result {
        CapabilityResult::Keychain(kr) => match kr {
            KeychainResult::SessionSecret(Some(secret)) => {
                // Keychain returns a secret, not an identity. Never project it
                // as `active_pubkey`; NMP must install the signer and report
                // the public key through IdentityChanged(Some(pubkey)).
                if secret.starts_with("nsec1") {
                    vec![Effect::AddNsecSigner { nsec: secret }]
                } else if secret.starts_with("bunker://") || secret.starts_with("nostrconnect://") {
                    vec![Effect::AddBunkerSigner { uri: secret }]
                } else {
                    state.session = SessionState::RestoreFailed {
                        error: "stored session secret is not a supported signer URI".into(),
                    };
                    vec![]
                }
            }
            KeychainResult::SessionSecret(None) => {
                state.session = SessionState::Absent;
                vec![]
            }
            KeychainResult::Cleared => {
                // Session already cleared by Logout action; this is the ack.
                vec![]
            }
            KeychainResult::Error(e) => {
                state.session = SessionState::RestoreFailed { error: e };
                vec![]
            }
        },

        // ── Phase 5K additions (append-only) ─────────────────────────────────
        CapabilityResult::Share(sr) => match sr {
            ShareResult::Pending(payloads) => {
                crate::kernel::domains::share::reduce_event_share_queue_drained(state, payloads)
            }
            ShareResult::CommunitiesWritten => {
                crate::kernel::domains::share::reduce_event_communities_written()
            }
            ShareResult::Error(msg) => {
                crate::kernel::domains::share::reduce_event_share_capability_error(msg)
            }
        },

        // ── Phase 5H additions (append-only) ─────────────────────────────────
        CapabilityResult::Audio(ar) => {
            crate::kernel::domains::podcast::reduce_capability_audio(state, ar)
        }

        // ── Phase 5D additions (append-only) ─────────────────────────────────
        CapabilityResult::Ocr(or) => {
            crate::kernel::domains::ocr::reduce_event_ocr_result(state, or)
        }

        // ── Phase 5E additions (append-only) ─────────────────────────────────
        CapabilityResult::Camera(cr) => {
            crate::kernel::domains::camera::reduce_capability_camera(state, cr)
        }
    }
}

// ─── Clock checks ────────────────────────────────────────────────────────────

/// Session restore timeout — transitions Restoring → Absent after
/// SESSION_RESTORE_TIMEOUT_SECS. Called on every reduce pass via `clock_checks`.
///
/// Fires when a capability result never arrives (observer not registered yet,
/// keychain locked, etc.). Allows the UI to show a retry prompt.
pub(crate) fn clock_check_restore_timeout(state: &mut AppState, now: u64) {
    if let SessionState::Restoring { started_at } = &state.session {
        if now.saturating_sub(*started_at) >= SESSION_RESTORE_TIMEOUT_SECS {
            state.session = SessionState::Absent;
        }
    }
}

// ─── Effect runner ───────────────────────────────────────────────────────────

pub(crate) async fn run_effect_restore_session_secret(
    shared: &SharedState,
    tx: &mpsc::UnboundedSender<Cmd>,
) {
    // Ask native for the keychain secret via the observer.
    // The round-trip completes when provide_capability_result is called.
    let observer = shared.observer.read().clone();
    if let Some(obs) = observer {
        obs.on_capability_request(CapabilityRequest::Keychain(KeychainOp::LoadSession));
    } else {
        // No observer registered yet — treat as absent session.
        let _ = tx.send(Cmd::Event(KernelEvent::SessionRestored {
            present: false,
            pubkey: None,
        }));
    }
}

pub(crate) async fn run_effect_clear_session(shared: &SharedState) {
    let observer = shared.observer.read().clone();
    if let Some(obs) = observer {
        obs.on_capability_request(CapabilityRequest::Keychain(KeychainOp::ClearSession));
    }
}

pub(crate) async fn run_effect_emit_capability_request(
    req: CapabilityRequest,
    shared: &SharedState,
) {
    let observer = shared.observer.read().clone();
    if let Some(obs) = observer {
        obs.on_capability_request(req);
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::KeychainResult;
    use crate::kernel::action::AppAction;
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::app::AppState;
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::effect::Effect;

    fn make_state() -> AppState {
        AppState::default()
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    // Gate 2 (session restore): failures surface as state.
    #[test]
    fn dispatch_failure_surfaces_as_state_not_return() {
        let mut state = make_state();
        let clock = ManualClock::default();
        step(&mut state, &clock, Cmd::Action(AppAction::RestoreSession));
        assert!(matches!(state.session, SessionState::Restoring { .. }));

        let err = CapabilityResult::Keychain(KeychainResult::Error("keychain locked".into()));
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::CapabilityResult(err)),
        );
        assert!(matches!(
            state.session,
            SessionState::RestoreFailed { ref error } if error.contains("keychain locked")
        ));
    }

    // Gate 3 (sync): reducer is sync (runs in a non-tokio thread).
    #[test]
    fn reduce_is_sync_pure() {
        let epoch = std::thread::spawn(|| {
            let mut state = make_state();
            let clock = ManualClock::new(0);
            step(&mut state, &clock, Cmd::Action(AppAction::RestoreSession));
            step(
                &mut state,
                &clock,
                Cmd::Event(KernelEvent::OnboardingStateLoaded(true)),
            );
            step(&mut state, &clock, Cmd::Action(AppAction::Logout));
            state.session_epoch
        })
        .join()
        .unwrap();
        assert_eq!(epoch, 1);
    }

    // Gate 8: session restore timeout via clock.
    #[test]
    fn session_restore_timeout_via_clock() {
        use crate::kernel::app::SESSION_RESTORE_TIMEOUT_SECS;
        let mut state = make_state();
        let clock = ManualClock::new(0);

        step(&mut state, &clock, Cmd::Action(AppAction::RestoreSession));
        assert!(matches!(
            state.session,
            SessionState::Restoring { started_at: 0 }
        ));

        clock.advance(SESSION_RESTORE_TIMEOUT_SECS - 1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(state.session, SessionState::Restoring { .. }),
            "still restoring"
        );

        clock.advance(1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(state.session, SessionState::Absent),
            "should be Absent after timeout, got {:?}",
            state.session
        );
    }

    #[test]
    fn restore_session_does_not_overwrite_present_nmp_identity() {
        let mut state = make_state();
        state.session = SessionState::Present {
            pubkey: "active-pubkey".into(),
            signer_kind: SignerKind::LocalNsec,
        };
        let clock = ManualClock::new(0);

        let effects = step(&mut state, &clock, Cmd::Action(AppAction::RestoreSession));

        assert!(matches!(
            state.session,
            SessionState::Present { ref pubkey, .. } if pubkey == "active-pubkey"
        ));
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::LoadOnboardingFlag));
    }

    #[test]
    fn keychain_restore_secret_installs_signer_without_projecting_secret_as_pubkey() {
        let mut state = make_state();
        let clock = ManualClock::new(0);

        step(&mut state, &clock, Cmd::Action(AppAction::RestoreSession));

        let nsec = "nsec1testsecret".to_string();
        let effects = step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::CapabilityResult(CapabilityResult::Keychain(
                KeychainResult::SessionSecret(Some(nsec.clone())),
            ))),
        );

        assert!(
            matches!(state.session, SessionState::Restoring { .. }),
            "restore must wait for NMP IdentityChanged instead of treating the secret as a pubkey"
        );
        assert!(
            effects.iter().any(
                |effect| matches!(effect, Effect::AddNsecSigner { nsec: value } if value == &nsec)
            ),
            "expected keychain nsec to be installed through NMP"
        );
    }

    #[test]
    fn keychain_restore_bunker_uri_installs_bunker_signer_without_projecting_secret() {
        let mut state = make_state();
        let clock = ManualClock::new(0);

        step(&mut state, &clock, Cmd::Action(AppAction::RestoreSession));

        let uri = "bunker://pubkey?relay=wss://relay.example".to_string();
        let effects = step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::CapabilityResult(CapabilityResult::Keychain(
                KeychainResult::SessionSecret(Some(uri.clone())),
            ))),
        );

        assert!(matches!(state.session, SessionState::Restoring { .. }));
        assert!(
            effects.iter().any(
                |effect| matches!(effect, Effect::AddBunkerSigner { uri: value } if value == &uri)
            ),
            "expected keychain bunker URI to be installed through NMP"
        );
    }

    // Gate 11 (subset): logout clears session and epoch.
    #[test]
    fn logout_cancels_view_scoped_effects() {
        let mut state = make_state();
        let clock = ManualClock::default();
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::SessionRestored {
                present: true,
                pubkey: Some("pk".into()),
            }),
        );
        let epoch_before = state.session_epoch;
        step(&mut state, &clock, Cmd::Action(AppAction::Logout));
        assert_eq!(state.session_epoch, epoch_before + 1);
        assert!(matches!(state.session, SessionState::Absent));
    }
}
