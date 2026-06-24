//! Route domain — tab selection, sheet management, onboarding state, and
//! snapshot projection arms of the kernel.
//!
//! Covers: SelectRootTab / PresentSheet / DismissSheet / CompleteOnboarding
//!         (actions); OnboardingStateLoaded (event); LoadOnboardingFlag (effect);
//!         and the project_app_root / project_root_shell snapshot helpers.

use nostr_ndb::nostr::nips::nip19::ToBech32;
use nostr_ndb::nostr::PublicKey;

use crate::onboarding::OnboardingStore;

use crate::kernel::app::{AppState, SessionState};
use crate::kernel::domains::relay_diagnostics::project_relay_diagnostics;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{
    AppRootSnapshot, KernelNetworkSettingsSnapshot, RelayDiagnosticsViewSnapshot,
    RootShellSnapshot, RouteKind, ToastSnapshot, ViewSnapshot,
};
use crate::kernel::view::ViewId;

// ─── Reducer (action) ────────────────────────────────────────────────────────

pub(crate) fn reduce_action_select_root_tab(state: &mut AppState, tab: u8) -> Vec<Effect> {
    state.route.root_tab = tab;
    vec![]
}

pub(crate) fn reduce_action_present_sheet(state: &mut AppState, sheet_id: String) -> Vec<Effect> {
    state.route.sheet_id = Some(sheet_id);
    vec![]
}

pub(crate) fn reduce_action_dismiss_sheet(state: &mut AppState) -> Vec<Effect> {
    state.route.sheet_id = None;
    vec![]
}

pub(crate) fn reduce_action_complete_onboarding(state: &mut AppState) -> Vec<Effect> {
    state.onboarding.complete = true;
    // OnboardingStore::set_complete is called as part of the
    // LoadOnboardingFlag effect's write path; here we update in-memory
    // state. The durable write is a side effect handled by the actor
    // after the reduce pass when it detects the flag changed.
    vec![]
}

// ─── Reducer (event) ─────────────────────────────────────────────────────────

pub(crate) fn reduce_event_onboarding_state_loaded(
    state: &mut AppState,
    complete: bool,
) -> Vec<Effect> {
    state.onboarding.complete = complete;
    state.onboarding.loaded = true;
    vec![]
}

// ─── Clock checks ────────────────────────────────────────────────────────────

/// Toast auto-dismiss check — clears the toast when its dismiss deadline has
/// passed. Called on every reduce pass via `clock_checks`.
pub(crate) fn clock_check_toast_dismiss(state: &mut AppState, now: u64) {
    if let Some(toast) = &state.chrome.toast {
        if now >= toast.dismiss_at_unix {
            state.chrome.toast = None;
        }
    }
}

// ─── Effect runner ───────────────────────────────────────────────────────────

pub(crate) async fn run_effect_load_onboarding_flag(
    onboarding_store: &OnboardingStore,
    tx: &tokio::sync::mpsc::UnboundedSender<crate::kernel::actor::Cmd>,
) {
    use crate::kernel::action::KernelEvent;
    use crate::kernel::actor::Cmd;
    let complete = onboarding_store.is_complete();
    let _ = tx.send(Cmd::Event(KernelEvent::OnboardingStateLoaded(complete)));
}

// ─── Snapshot projection ─────────────────────────────────────────────────────

/// Recompute the snapshot for one open view from `AppState`.
pub(crate) fn project_snapshot(
    state: &AppState,
    id: &ViewId,
    clock_now: u64,
) -> Option<ViewSnapshot> {
    match id {
        ViewId::AppRoot => Some(ViewSnapshot::AppRoot(project_app_root(state))),
        ViewId::RootShell => Some(ViewSnapshot::RootShell(project_root_shell(
            state, clock_now,
        ))),

        // ── Phase 2E additions ────────────────────────────────────────────────
        // Both network-settings and relay-diagnostics views are fed from the same
        // relay_diagnostics sidecar state. The view is only emitted when the view
        // is open AND at least one frame has been received (D5 / Non-Negotiable #7).
        ViewId::NetworkSettings => {
            let snap = project_relay_diagnostics(state)?;
            Some(ViewSnapshot::NetworkSettings(
                KernelNetworkSettingsSnapshot {
                    relays: snap.relays,
                },
            ))
        }
        ViewId::RelayDiagnostics => {
            let snap = project_relay_diagnostics(state)?;
            Some(ViewSnapshot::RelayDiagnostics(
                RelayDiagnosticsViewSnapshot {
                    relays: snap.relays,
                },
            ))
        }

        // ── Phase 3B additions (append-only) ─────────────────────────────────
        // ViewId::Communities is handled upstream in actor::project_snapshot
        // before this function is called. This arm is unreachable in practice
        // but required for exhaustive match coverage.
        ViewId::Communities => None,

        // ── Phase 3E additions (append-only) ─────────────────────────────────
        // ViewId::RoomExplorer is handled upstream in actor::project_snapshot
        // before this function is called. This arm is unreachable in practice
        // but required for exhaustive match coverage.
        ViewId::RoomExplorer => None,

        // ── Phase 3D additions (append-only) ─────────────────────────────────
        // ViewId::Profile is handled upstream in actor::project_snapshot.
        // This arm is unreachable in practice but required for exhaustive match.
        ViewId::Profile { .. } => None,

        // ── Phase 3F additions (append-only) ─────────────────────────────────
        // ViewId::RoomHome is handled upstream in actor::project_snapshot.
        // This arm is unreachable in practice but required for exhaustive match.
        ViewId::RoomHome { .. } => None,

        // ── Phase 4C additions (append-only) ─────────────────────────────────
        // ViewId::Bookmarks is handled upstream in actor::project_snapshot.
        // This arm is unreachable in practice but required for exhaustive match.
        ViewId::Bookmarks => None,

        // ── Phase 4A additions (append-only) ─────────────────────────────────
        // ViewId::ArticleReader is handled upstream in actor::project_snapshot.
        // This arm is unreachable in practice but required for exhaustive match.
        ViewId::ArticleReader { .. } => None,

        // ── Phase 4D additions (append-only) ─────────────────────────────────
        // ViewId::Search is handled upstream in actor::project_snapshot.
        // This arm is unreachable in practice but required for exhaustive match.
        ViewId::Search => None,

        // ── Phase 4G additions (append-only) ─────────────────────────────────
        // ViewId::ArticleFeed is handled upstream in actor::project_snapshot.
        // This arm is unreachable in practice but required for exhaustive match.
        ViewId::ArticleFeed => None,

        // ── Phase 4H additions (append-only) ─────────────────────────────────
        // ViewId::HighlightFeed is handled upstream in actor::project_snapshot.
        // This arm is unreachable in practice but required for exhaustive match.
        ViewId::HighlightFeed => None,

        // ── Phase 4J additions (append-only) ─────────────────────────────────
        // ViewId::HomeFeed is handled upstream in actor::project_snapshot.
        // This arm is unreachable in practice but required for exhaustive match.
        ViewId::HomeFeed => None,

        // ── Phase 5A additions (append-only) ─────────────────────────────────
        // ViewId::WhatsNew is handled upstream in actor::project_snapshot.
        // This arm is unreachable in practice but required for exhaustive match.
        ViewId::WhatsNew => None,

        // ── Phase 5C additions (append-only) ─────────────────────────────────
        // ViewId::BookPicker is handled upstream in actor::project_snapshot.
        // This arm is unreachable in practice but required for exhaustive match.
        ViewId::BookPicker => None,

        // ── Phase 5K additions (append-only) ─────────────────────────────────
        // ViewId::ShareComposer is handled upstream in actor::project_snapshot.
        // This arm is unreachable in practice but required for exhaustive match.
        ViewId::ShareComposer => None,

        // ── #21 share-flow additions (append-only) ───────────────────────────
        // ViewId::SharePublish is handled upstream in actor::project_snapshot.
        // This arm is unreachable in practice but required for exhaustive match.
        ViewId::SharePublish => None,

        // ── Phase 5H additions (append-only) ─────────────────────────────────
        // ViewId::PodcastListening is handled upstream in actor::project_snapshot.
        // This arm is unreachable in practice but required for exhaustive match.
        ViewId::PodcastListening => None,

        // ── Phase 5D additions (append-only) ─────────────────────────────────
        // ViewId::Capture is handled upstream in actor::project_snapshot.
        // This arm is unreachable in practice but required for exhaustive match.
        ViewId::Capture => None,

        // ── Phase 7 additions (append-only) ─────────────────────────────────
        // ViewId::CommentThread is handled upstream in actor::project_snapshot.
        // This arm is unreachable in practice but required for exhaustive match.
        ViewId::CommentThread { .. } => None,

        // ── Phase 7 feedback additions (append-only) ──────────────────────────
        // ViewId::FeedbackThreads and ViewId::FeedbackThread are handled upstream
        // in actor::project_snapshot. These arms are unreachable in practice but
        // required for exhaustive match coverage.
        ViewId::FeedbackThreads => None,
        ViewId::FeedbackThread { .. } => None,
        // ── Phase 7 chat additions (append-only) ─────────────────────────────
        // ViewId::RoomChat is handled upstream in actor::project_snapshot.
        // This arm is unreachable in practice but required for exhaustive match.
        ViewId::RoomChat { .. } => None,
        // ── Phase 7 discussions additions (append-only) ──────────────────────
        // ViewId::RoomDiscussions is handled upstream in actor::project_snapshot.
        // This arm is unreachable in practice but required for exhaustive match.
        ViewId::RoomDiscussions { .. } => None,
    }
}

pub(crate) fn project_app_root(state: &AppState) -> AppRootSnapshot {
    let session_present = matches!(state.session, SessionState::Present { .. });
    let route_kind = if !state.onboarding.complete {
        RouteKind::Onboarding
    } else if !session_present {
        RouteKind::Login
    } else {
        RouteKind::RootShell
    };
    // Phase 7: surface a failed restore / sign-in error so LoginView's inline
    // error has a kernel source. Other states carry no error → None (which is
    // how the field self-clears on a new attempt / success / logout).
    let auth_error = match &state.session {
        SessionState::RestoreFailed { error } => Some(error.clone()),
        SessionState::SignInFailed { error, .. } => Some(error.clone()),
        _ => None,
    };
    // Phase 7 Part C: expose the active pubkey so Swift can build CurrentUser
    // after kernel sign-in without a bespoke lane call.
    let (active_pubkey_hex, active_pubkey_npub) = match &state.session {
        SessionState::Present { pubkey, .. } => {
            let hex = pubkey.clone();
            let npub = PublicKey::from_hex(&hex)
                .ok()
                .and_then(|pk| pk.to_bech32().ok());
            (Some(hex), npub)
        }
        _ => (None, None),
    };
    AppRootSnapshot {
        route_kind,
        session_present,
        onboarding_complete: state.onboarding.complete,
        // Phase 2B: expose pending NostrConnect URI to the iOS QR-code sheet.
        nostrconnect_uri: state.nostrconnect_uri.clone(),
        auth_error,
        active_pubkey_hex,
        active_pubkey_npub,
    }
}

pub(crate) fn project_root_shell(state: &AppState, _clock_now: u64) -> RootShellSnapshot {
    let toast = state.chrome.toast.as_ref().map(|t| ToastSnapshot {
        message: t.message.clone(),
        dismiss_at_unix: t.dismiss_at_unix,
    });
    RootShellSnapshot {
        selected_tab: state.route.root_tab,
        tab_count: 5, // Feed / Discover / Capture / Notifications / Settings
        toast,
        sheet_id: state.route.sheet_id.clone(),
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::{AppAction, KernelEvent, RootTab};
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::app::{AppState, ToastState};
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::effect::Effect;
    use crate::kernel::snapshot::{RouteKind, ViewSnapshot};
    use crate::kernel::view::ViewId;

    fn make_state() -> AppState {
        AppState::default()
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    // Gate 5: snapshot shape is bounded.
    #[test]
    fn snapshot_size_is_view_shaped() {
        let mut state = make_state();
        let clock = ManualClock::default();
        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::SelectRootTab {
                tab: RootTab::Discover,
            }),
        );
        let now = clock.now_unix_seconds();
        if let Some(ViewSnapshot::RootShell(s)) = project_snapshot(&state, &ViewId::RootShell, now)
        {
            assert_eq!(s.tab_count, 5);
        } else {
            panic!("expected RootShell snapshot");
        }
    }

    // Phase 7: AppRootSnapshot.auth_error surfaces RestoreFailed/SignInFailed
    // errors and is None for every non-error state (so it self-clears on a new
    // attempt / success / logout — no explicit clear needed).
    #[test]
    fn app_root_auth_error_surfaces_failures_only() {
        use crate::kernel::action::{SignInMethod, SignerKind};
        use crate::kernel::app::SessionState;

        let mut state = make_state();

        state.session = SessionState::Absent;
        assert_eq!(project_app_root(&state).auth_error, None);

        state.session = SessionState::Present {
            pubkey: "deadbeef".to_string(),
            signer_kind: SignerKind::LocalNsec,
        };
        assert_eq!(project_app_root(&state).auth_error, None);

        state.session = SessionState::RestoreFailed {
            error: "keychain locked".to_string(),
        };
        assert_eq!(
            project_app_root(&state).auth_error.as_deref(),
            Some("keychain locked")
        );

        state.session = SessionState::SignInFailed {
            method: SignInMethod::Nsec,
            error: "invalid nsec".to_string(),
        };
        assert_eq!(
            project_app_root(&state).auth_error.as_deref(),
            Some("invalid nsec")
        );
    }

    // Gate 6: coalescing — N changes → final state only.
    #[test]
    fn observer_coalesces_final_state() {
        let mut state = make_state();
        let clock = ManualClock::default();
        for tab in [
            RootTab::Feed,
            RootTab::Discover,
            RootTab::Capture,
            RootTab::Notifications,
            RootTab::Settings,
        ] {
            step(
                &mut state,
                &clock,
                Cmd::Action(AppAction::SelectRootTab { tab }),
            );
        }
        let now = clock.now_unix_seconds();
        if let Some(ViewSnapshot::RootShell(s)) = project_snapshot(&state, &ViewId::RootShell, now)
        {
            assert_eq!(s.selected_tab, RootTab::Settings as u8);
        } else {
            panic!("expected RootShell snapshot");
        }
    }

    // Gate 7: clock drives toast dismiss.
    #[test]
    fn clock_drives_toast_dismiss() {
        use crate::kernel::app::TOAST_DISMISS_SECS;
        let mut state = make_state();
        let clock = ManualClock::new(0);

        state.chrome.toast = Some(ToastState {
            message: "Shared!".into(),
            dismiss_at_unix: TOAST_DISMISS_SECS,
        });

        // t=2 → still present.
        clock.advance(2);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        let now = clock.now_unix_seconds();
        let snap = project_snapshot(&state, &ViewId::RootShell, now).unwrap();
        if let ViewSnapshot::RootShell(s) = snap {
            assert!(s.toast.is_some(), "toast should be present at t=2");
        }

        // t=3 → dismissed.
        clock.advance(1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        let now = clock.now_unix_seconds();
        let snap = project_snapshot(&state, &ViewId::RootShell, now).unwrap();
        if let ViewSnapshot::RootShell(s) = snap {
            assert!(s.toast.is_none(), "toast should be dismissed at t=3");
        }
    }

    // Gate 9: route selection deterministic.
    #[test]
    fn route_selection_deterministic() {
        let clock = ManualClock::default();
        let now = clock.now_unix_seconds();

        // Onboarding incomplete → Onboarding.
        let state = make_state();
        if let Some(ViewSnapshot::AppRoot(s)) = project_snapshot(&state, &ViewId::AppRoot, now) {
            assert_eq!(s.route_kind, RouteKind::Onboarding);
        }

        // Onboarding complete, no session → Login.
        let mut state = make_state();
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::OnboardingStateLoaded(true)),
        );
        if let Some(ViewSnapshot::AppRoot(s)) = project_snapshot(&state, &ViewId::AppRoot, now) {
            assert_eq!(s.route_kind, RouteKind::Login);
        }

        // Session present → RootShell.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::SessionRestored {
                present: true,
                pubkey: Some("pk".into()),
            }),
        );
        if let Some(ViewSnapshot::AppRoot(s)) = project_snapshot(&state, &ViewId::AppRoot, now) {
            assert_eq!(s.route_kind, RouteKind::RootShell);
            assert!(s.session_present);
        }
    }
}
