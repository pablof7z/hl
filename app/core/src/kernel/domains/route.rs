//! Route domain — tab selection, sheet management, onboarding state, and
//! snapshot projection arms of the kernel.
//!
//! Covers: SelectRootTab / PresentSheet / DismissSheet / CompleteOnboarding
//!         (actions); OnboardingStateLoaded (event); LoadOnboardingFlag (effect);
//!         and the project_app_root / project_root_shell snapshot helpers.

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
    AppRootSnapshot {
        route_kind,
        session_present,
        onboarding_complete: state.onboarding.complete,
        // Phase 2B: expose pending NostrConnect URI to the iOS QR-code sheet.
        nostrconnect_uri: state.nostrconnect_uri.clone(),
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
