//! Phase 1 kernel integration tests — use only public API.
//!
//! Pure reducer tests (ManualClock + direct state mutation) live in
//! `src/kernel/actor.rs #[cfg(test)]` where they have access to private items.
//! These integration tests exercise the full actor over the bounded FFI surface.

use std::sync::{Arc, Mutex};

use highlighter_core::capabilities::{CapabilityRequest, CapabilityResult, KeychainResult};
use highlighter_core::kernel::{
    AppAction, AppConfig, AppRootSnapshot, HighlighterObserver, RootShellSnapshot, RootTab, ViewId,
    ViewRoute, ViewSnapshot,
};
use highlighter_core::HighlighterApp;

// ─── Recording observer (implements public trait) ────────────────────────────

#[derive(Debug, Default)]
struct RecordingObserver {
    snapshots: Mutex<Vec<(ViewId, ViewSnapshot)>>,
    capability_requests: Mutex<Vec<CapabilityRequest>>,
}

impl HighlighterObserver for RecordingObserver {
    fn on_snapshot(&self, view_id: ViewId, snapshot: ViewSnapshot) {
        self.snapshots.lock().unwrap().push((view_id, snapshot));
    }
    fn on_capability_request(&self, request: CapabilityRequest) {
        self.capability_requests.lock().unwrap().push(request);
    }
}

impl RecordingObserver {
    fn snapshot_count(&self) -> usize {
        self.snapshots.lock().unwrap().len()
    }
    fn latest_for(&self, id: &ViewId) -> Option<ViewSnapshot> {
        self.snapshots
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|(vid, _)| vid == id)
            .map(|(_, s)| s.clone())
    }
    fn capability_count(&self) -> usize {
        self.capability_requests.lock().unwrap().len()
    }
}

fn make_app() -> (
    Arc<HighlighterApp>,
    Arc<RecordingObserver>,
    tempfile::TempDir,
) {
    let tmp = tempfile::tempdir().unwrap();
    let config = AppConfig {
        data_dir: tmp.path().to_string_lossy().into_owned(),
    };
    let app = HighlighterApp::new(config);
    let obs = Arc::new(RecordingObserver::default());
    app.set_observer(obs.clone());
    (app, obs, tmp)
}

// ─── Dispatch returns unit (gate 1) ─────────────────────────────────────────

#[test]
fn dispatch_returns_unit_type() {
    let (app, _obs, _tmp) = make_app();
    // `dispatch` signature is `fn(&self, AppAction)` — return type is ().
    // The compiler would reject this test if dispatch returned anything else.
    let result: () = app.dispatch(AppAction::Logout);
    let _ = result;
}

// ─── current_snapshot returns None for unopened views (gate 5) ──────────────

#[test]
fn current_snapshot_none_for_unopened_view() {
    let (app, _obs, _tmp) = make_app();
    // No views opened → None.
    assert!(app.current_snapshot(ViewId::AppRoot).is_none());
    assert!(app.current_snapshot(ViewId::RootShell).is_none());
}

// ─── resume / suspend / shutdown idempotent (gate 10) ────────────────────────

#[test]
fn resume_suspend_shutdown_idempotent() {
    let (app, _obs, _tmp) = make_app();
    // Multiple calls — no panic.
    for _ in 0..5 {
        app.resume();
        app.suspend();
    }
    for _ in 0..3 {
        app.shutdown();
    }
    // Still usable after shutdown (commands are silently dropped).
    app.dispatch(AppAction::Logout);
}

// ─── Async integration tests (actor loop) ───────────────────────────────────

/// After open_view + dispatch, current_snapshot reflects the latest state.
#[tokio::test]
async fn current_snapshot_reflects_dispatch() {
    let (app, _obs, _tmp) = make_app();
    app.open_view(ViewId::RootShell, ViewRoute::RootShell);

    app.dispatch(AppAction::SelectRootTab {
        tab: RootTab::Discover,
    });

    // Allow actor to process.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let snap = app.current_snapshot(ViewId::RootShell);
    if let Some(ViewSnapshot::RootShell(s)) = snap {
        assert_eq!(s.selected_tab, RootTab::Discover as u8);
        assert_eq!(s.tab_count, 5);
    } else {
        panic!("expected RootShell snapshot, got {:?}", snap);
    }
}

/// Closed view no longer produces snapshots (gate 4).
#[tokio::test]
async fn closed_view_no_current_snapshot() {
    let (app, _obs, _tmp) = make_app();
    app.open_view(ViewId::AppRoot, ViewRoute::AppRoot);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    app.close_view(ViewId::AppRoot);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // After close, no snapshot.
    assert!(app.current_snapshot(ViewId::AppRoot).is_none());
}

/// Session restore + capability result → session present in snapshot (gate 2).
#[tokio::test]
async fn dispatch_failure_surfaces_as_state_via_actor() {
    let (app, _obs, _tmp) = make_app();
    app.open_view(ViewId::AppRoot, ViewRoute::AppRoot);

    app.dispatch(AppAction::RestoreSession);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Feed an error result.
    app.provide_capability_result(CapabilityResult::Keychain(KeychainResult::Error(
        "keychain locked".into(),
    )));
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // The snapshot should show no session (error surfaced as state, not panic/Result).
    if let Some(ViewSnapshot::AppRoot(snap)) = app.current_snapshot(ViewId::AppRoot) {
        assert!(!snap.session_present, "error should leave session absent");
    } else {
        panic!("expected AppRoot snapshot");
    }
}

/// Successful restore → session present → RootShell route.
#[tokio::test]
async fn session_restore_success_route_selection() {
    let (app, _obs, _tmp) = make_app();
    app.open_view(ViewId::AppRoot, ViewRoute::AppRoot);

    // Manually complete onboarding in the state machine.
    app.dispatch(AppAction::CompleteOnboarding);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Provide a successful session secret.
    app.provide_capability_result(CapabilityResult::Keychain(KeychainResult::SessionSecret(
        Some("my_nsec_key".into()),
    )));
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    if let Some(ViewSnapshot::AppRoot(snap)) = app.current_snapshot(ViewId::AppRoot) {
        assert!(snap.session_present);
        assert!(snap.onboarding_complete);
        // route_kind check: both session + onboarding → RootShell
        use highlighter_core::kernel::RouteKind;
        assert_eq!(snap.route_kind, RouteKind::RootShell);
    } else {
        panic!("expected AppRoot snapshot");
    }
}

/// Logout bumps session epoch and session becomes Absent.
#[tokio::test]
async fn logout_clears_session_via_actor() {
    let (app, obs, _tmp) = make_app();
    app.open_view(ViewId::AppRoot, ViewRoute::AppRoot);

    // Set up session.
    app.dispatch(AppAction::CompleteOnboarding);
    app.provide_capability_result(CapabilityResult::Keychain(KeychainResult::SessionSecret(
        Some("nsec_placeholder".into()),
    )));
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Logout.
    app.dispatch(AppAction::Logout);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    if let Some(ViewSnapshot::AppRoot(snap)) = app.current_snapshot(ViewId::AppRoot) {
        assert!(
            !snap.session_present,
            "session should be absent after logout"
        );
    } else {
        panic!("expected AppRoot snapshot");
    }
}

/// N rapid dispatches → current_snapshot shows final state (not intermediate).
#[tokio::test]
async fn rapid_dispatches_reflect_final_state() {
    let (app, _obs, _tmp) = make_app();
    app.open_view(ViewId::RootShell, ViewRoute::RootShell);

    for tab in [
        RootTab::Feed,
        RootTab::Discover,
        RootTab::Capture,
        RootTab::Notifications,
        RootTab::Settings,
    ] {
        app.dispatch(AppAction::SelectRootTab { tab });
    }

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    if let Some(ViewSnapshot::RootShell(s)) = app.current_snapshot(ViewId::RootShell) {
        assert_eq!(s.selected_tab, RootTab::Settings as u8);
    } else {
        panic!("expected RootShell snapshot");
    }
}
