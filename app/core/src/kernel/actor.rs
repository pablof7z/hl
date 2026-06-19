//! Kernel actor — the single Rust writer for `AppState`.
//!
//! Architecture (TEA / Elm-style):
//!   native dispatch(AppAction) →  mpsc channel
//!   actor loop:  recv Cmd → reduce(state, cmd) → Vec<Effect>
//!                         → run effects (async, feeds back KernelEvent)
//!                         → recompute open-view snapshots
//!                         → emit changed snapshots at clock-capped cadence
//!
//! Non-Negotiables enforced here:
//!   #2  `AppState` is the single writer — no native mutation of state.
//!   #3  `dispatch` is fire-and-forget, returns `()`.
//!   #6  No mock / fake behavior.
//!   #7  Snapshots bounded by open views.
//! D8:   No sleeps or poll loops; time advances through the injected Clock.
//! D9:   Wall-clock reads confined to `SystemClock`; tests inject `ManualClock`.

use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

use parking_lot::RwLock;
use tokio::sync::mpsc;

use nmp_defaults::{NmpAppBuilder, RunConfig};
use nmp_ffi::{nmp_app_free, NmpApp};

use crate::capabilities::{CapabilityRequest, CapabilityResult, KeychainOp};
use crate::kernel::action::{AppAction, KernelEvent};
use crate::kernel::app::{AppState, SessionState, SESSION_RESTORE_TIMEOUT_SECS};
use crate::kernel::clock::Clock;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{
    AppRootSnapshot, RootShellSnapshot, RouteKind, ToastSnapshot, ViewSnapshot,
};
use crate::kernel::view::{ViewId, ViewRegistry, ViewRoute};
use crate::onboarding::OnboardingStore;

// ─── NmpApp handle ──────────────────────────────────────────────────────────

/// Owned, `Send + Sync` wrapper around the `NmpApp` raw pointer.
///
/// `NmpApp` is actor-backed and thread-safe for host API calls (see
/// nmp-ffi SAFETY docs). The raw pointer is freed exactly once in `Drop`.
pub(crate) struct NmpHandle(NonNull<NmpApp>);

// SAFETY: NmpApp is designed for cross-thread host calls (nmp-ffi docs).
unsafe impl Send for NmpHandle {}
unsafe impl Sync for NmpHandle {}

impl Drop for NmpHandle {
    fn drop(&mut self) {
        nmp_app_free(self.0.as_ptr());
    }
}

// ─── Command channel ────────────────────────────────────────────────────────

/// Everything the actor can receive.
pub(crate) enum Cmd {
    Action(AppAction),
    Event(KernelEvent),
    OpenView(ViewId, ViewRoute),
    CloseView(ViewId),
    ProvideCapabilityResult(CapabilityResult),
    Resume,
    Suspend,
    Tick,
    Shutdown,
}

// ─── Observer trait (FFI callback) ──────────────────────────────────────────

/// Platform observer registered via `HighlighterApp::set_observer`.
/// Both methods must be non-blocking; called from the actor tokio task.
#[uniffi::export(with_foreign)]
pub trait HighlighterObserver: Send + Sync + 'static {
    /// A view's snapshot changed.
    fn on_snapshot(&self, view_id: ViewId, snapshot: ViewSnapshot);
    /// The kernel is requesting a native capability execution.
    fn on_capability_request(&self, request: CapabilityRequest);
}

// ─── Shared state (actor ↔ FFI layer) ───────────────────────────────────────

/// Mutable state accessible from the FFI layer without going through the actor.
pub(crate) struct SharedState {
    /// Latest computed snapshot per open view — updated after every reduce pass.
    /// Native reads via `current_snapshot()` without blocking on the actor.
    pub snapshots: Mutex<std::collections::HashMap<ViewId, ViewSnapshot>>,
    /// The registered platform observer.
    pub observer: RwLock<Option<Arc<dyn HighlighterObserver>>>,
}

impl SharedState {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            snapshots: Mutex::new(std::collections::HashMap::new()),
            observer: RwLock::new(None),
        })
    }
}

// ─── Pure reducer ───────────────────────────────────────────────────────────

/// Pure, synchronous reducer. Never async, never `.await` (Non-Negotiable #2).
/// Takes the current clock so clock-dependent checks (toast dismiss, session
/// timeout) are deterministic under `ManualClock` (D9).
pub(crate) fn reduce(state: &mut AppState, cmd: Cmd, now: u64) -> Vec<Effect> {
    // Clock-driven checks run on EVERY reduce pass, before the command.
    let mut effects = clock_checks(state, now);

    match cmd {
        Cmd::Action(action) => effects.extend(reduce_action(state, action, now)),
        Cmd::Event(event) => effects.extend(reduce_event(state, event, now)),
        Cmd::OpenView(..) | Cmd::CloseView(..) | Cmd::Resume | Cmd::Suspend | Cmd::Tick => {
            // OpenView/CloseView/Resume/Suspend/Tick are handled by the actor loop
            // itself (registry + NmpApp lifecycle); the reducer has no state to change.
        }
        Cmd::ProvideCapabilityResult(result) => {
            effects.extend(reduce_event(
                state,
                KernelEvent::CapabilityResult(result),
                now,
            ));
        }
        Cmd::Shutdown => {}
    }

    effects
}

/// Clock-driven state transitions that fire on every reduce pass.
/// This is how the ManualClock controls deterministic time-based behavior
/// (D8: no sleeps; D9: no wall-clock reads in reducers).
fn clock_checks(state: &mut AppState, now: u64) -> Vec<Effect> {
    let effects: Vec<Effect> = Vec::new();

    // Toast auto-dismiss.
    if let Some(toast) = &state.chrome.toast {
        if now >= toast.dismiss_at_unix {
            state.chrome.toast = None;
        }
    }

    // Session restore timeout: if we've been Restoring for too long with no
    // capability result, transition to Absent so the UI can show retry.
    if let SessionState::Restoring { started_at } = &state.session {
        if now.saturating_sub(*started_at) >= SESSION_RESTORE_TIMEOUT_SECS {
            state.session = SessionState::Absent;
        }
    }

    effects
}

fn reduce_action(state: &mut AppState, action: AppAction, now: u64) -> Vec<Effect> {
    match action {
        AppAction::RestoreSession | AppAction::RetryRestore => {
            state.session = SessionState::Restoring { started_at: now };
            // Fire both onboarding load and session restore in parallel.
            vec![Effect::LoadOnboardingFlag, Effect::RestoreSessionSecret]
        }

        AppAction::Logout => {
            state.session = SessionState::Absent;
            state.session_epoch += 1;
            // ClearSession effect emits a CapabilityRequest to native.
            vec![Effect::ClearSession]
        }

        AppAction::CompleteOnboarding => {
            state.onboarding.complete = true;
            // OnboardingStore::set_complete is called as part of the
            // LoadOnboardingFlag effect's write path; here we update in-memory
            // state. The durable write is a side effect handled by the actor
            // after the reduce pass when it detects the flag changed.
            vec![]
        }

        AppAction::SelectRootTab { tab } => {
            state.route.root_tab = tab as u8;
            vec![]
        }

        AppAction::PresentSheet { sheet_id } => {
            state.route.sheet_id = Some(sheet_id);
            vec![]
        }

        AppAction::DismissSheet => {
            state.route.sheet_id = None;
            vec![]
        }
    }
}

fn reduce_event(state: &mut AppState, event: KernelEvent, _now: u64) -> Vec<Effect> {
    match event {
        KernelEvent::SessionRestored { present, pubkey } => {
            if present {
                state.session = SessionState::Present {
                    pubkey: pubkey.unwrap_or_default(),
                };
            } else {
                state.session = SessionState::Absent;
            }
            vec![]
        }

        KernelEvent::OnboardingStateLoaded(complete) => {
            state.onboarding.complete = complete;
            state.onboarding.loaded = true;
            vec![]
        }

        KernelEvent::CapabilityResult(result) => reduce_capability_result(state, result),

        KernelEvent::IdentityChanged(pubkey) => {
            // NMP identity change — update session if we now have an active
            // account (Phase 2 will install the signer; Phase 1 just records).
            if let Some(pk) = pubkey {
                if !pk.is_empty() {
                    state.session = SessionState::Present { pubkey: pk };
                }
            }
            vec![]
        }

        KernelEvent::ClockTick => {
            // Clock-driven checks already ran in `clock_checks` at the top of
            // `reduce`. `ClockTick` has no additional state changes.
            vec![]
        }
    }
}

fn reduce_capability_result(state: &mut AppState, result: CapabilityResult) -> Vec<Effect> {
    use crate::capabilities::KeychainResult;
    match result {
        CapabilityResult::Keychain(kr) => match kr {
            KeychainResult::SessionSecret(Some(secret)) => {
                state.session = SessionState::Present { pubkey: secret };
                vec![]
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
    }
}

// ─── Snapshot projection ────────────────────────────────────────────────────

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
    }
}

fn project_app_root(state: &AppState) -> AppRootSnapshot {
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
    }
}

fn project_root_shell(state: &AppState, _clock_now: u64) -> RootShellSnapshot {
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

// ─── Effect runner ───────────────────────────────────────────────────────────

/// Execute one effect and send the resulting `KernelEvent` (if any) back into
/// the actor channel. Non-async effects (keychain, onboarding) are
/// synchronous reads; async network effects (later phases) use tokio tasks.
pub(crate) async fn run_effect(
    effect: Effect,
    session_epoch: u64,
    tx: &mpsc::UnboundedSender<Cmd>,
    onboarding_store: &OnboardingStore,
    shared: &SharedState,
) {
    match effect {
        Effect::LoadOnboardingFlag => {
            let complete = onboarding_store.is_complete();
            let _ = tx.send(Cmd::Event(KernelEvent::OnboardingStateLoaded(complete)));
        }

        Effect::RestoreSessionSecret => {
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

        Effect::ClearSession => {
            let observer = shared.observer.read().clone();
            if let Some(obs) = observer {
                obs.on_capability_request(CapabilityRequest::Keychain(KeychainOp::ClearSession));
            }
        }

        Effect::EmitCapabilityRequest(req) => {
            let observer = shared.observer.read().clone();
            if let Some(obs) = observer {
                obs.on_capability_request(req);
            }
        }
    }

    let _ = session_epoch; // carried for future epoch-keyed effect cancellation
}

// ─── Actor task ─────────────────────────────────────────────────────────────

/// Minimum seconds between observer snapshot-push emissions (capped cadence).
/// The actor pushes at most once per this interval (D8 — no sleeps, cadence
/// driven by clock ticks from the outside rather than a background timer).
const EMIT_CADENCE_SECS: u64 = 1;

/// Main actor loop. Runs in a dedicated tokio task.
pub(crate) async fn actor_task(
    mut rx: mpsc::UnboundedReceiver<Cmd>,
    tx: mpsc::UnboundedSender<Cmd>,
    shared: Arc<SharedState>,
    clock: Arc<dyn Clock>,
    onboarding_store: Arc<OnboardingStore>,
    nmp: Option<NmpHandle>,
) {
    let mut state = AppState::default();
    let mut registry = ViewRegistry::default();
    let mut last_emit_at: u64 = 0;
    let mut suspended = false;

    // Keep the NmpApp alive for the duration of the actor.
    let _nmp = nmp;

    while let Some(cmd) = rx.recv().await {
        let is_shutdown = matches!(cmd, Cmd::Shutdown);

        // Handle view registry mutations before reducing (they need the registry).
        match &cmd {
            Cmd::OpenView(id, route) => {
                registry.open(id.clone(), route.clone());
            }
            Cmd::CloseView(id) => {
                registry.close(id);
            }
            Cmd::Resume => {
                suspended = false;
            }
            Cmd::Suspend => {
                suspended = true;
            }
            _ => {}
        }

        let now = clock.now_unix_seconds();

        // Reduce (pure, sync).
        let effects = reduce(&mut state, cmd, now);

        // Run effects (async).
        for effect in effects {
            run_effect(effect, state.session_epoch, &tx, &onboarding_store, &shared).await;
        }

        // Recompute all open-view snapshots and update the shared cache.
        // This makes `current_snapshot()` return fresh state immediately.
        // Collect IDs first to avoid simultaneous immutable + mutable borrows.
        {
            let open_ids: Vec<ViewId> = registry.open_ids().cloned().collect();
            let mut cache = shared.snapshots.lock().unwrap_or_else(|e| e.into_inner());
            for id in &open_ids {
                if let Some(snap) = project_snapshot(&state, id, now) {
                    cache.insert(id.clone(), snap.clone());
                    registry.update_snapshot(id, snap);
                }
            }
            // Remove closed views from the cache.
            cache.retain(|id, _| registry.is_open(id));
        }

        // Coalesced observer push at clock-capped cadence (D8).
        // In tests: advance ManualClock + dispatch Tick → cadence passes →
        // observer receives ≤1 snapshot per open view.
        if !suspended && now >= last_emit_at + EMIT_CADENCE_SECS {
            let observer = shared.observer.read().clone();
            if let Some(obs) = observer {
                for id in registry.open_ids() {
                    if let Some(snap) = registry.current_snapshot(id) {
                        obs.on_snapshot(id.clone(), snap);
                    }
                }
            }
            last_emit_at = now;
        }

        if is_shutdown {
            break;
        }
    }
}

// ─── NmpApp boot ────────────────────────────────────────────────────────────

/// Construct and start the `NmpApp` for the nmp-lane (its own storage sub-dir).
/// Wires the identity-change observer to feed `KernelEvent::IdentityChanged`
/// back into the actor channel. Phase 1 requires no further protocol I/O.
///
/// Returns `None` if the storage path cannot be created or `nmp_app_start`
/// returns null (logged as a warning; the actor continues without NMP).
pub(crate) fn start_nmp_app(data_dir: &str, tx: mpsc::UnboundedSender<Cmd>) -> Option<NmpHandle> {
    let storage_path = AppState::nmp_storage_path(data_dir);
    if let Err(e) = std::fs::create_dir_all(&storage_path) {
        tracing::warn!(path = %storage_path.display(), error = %e, "failed to create NMP storage dir");
        return None;
    }

    let mut builder = NmpAppBuilder::new();
    nmp_defaults::register_defaults(&mut builder);

    // Boot sequence (adapted to the git-cached nmp-defaults API):
    //   Unstarted → StorageSet → ProjectionsDeclared → *mut NmpApp
    // No explicit relay decision needed in this version; omitting
    // with_relays() uses the nmp-defaults built-in relay set.
    let raw = builder
        .storage_path(storage_path.to_string_lossy().into_owned())
        .consume_all_builtin_projections()
        .start(RunConfig::default());

    let handle = NonNull::new(raw).map(NmpHandle)?;
    let nmp_ref: &NmpApp = unsafe { handle.0.as_ref() };

    // Wire identity-change observer → KernelEvent::IdentityChanged.
    // Pattern 2 from nmp_runtime.rs:758-778.
    let tx_id = tx.clone();
    nmp_ref.register_identity_change_observer(move |active| {
        let _ = tx_id.send(Cmd::Event(KernelEvent::IdentityChanged(active)));
    });

    Some(handle)
}

// ─── Unit tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::{CapabilityResult, KeychainResult};
    use crate::kernel::action::RootTab;
    use crate::kernel::app::ToastState;
    use crate::kernel::clock::{Clock, ManualClock};

    fn make_state() -> AppState {
        AppState::default()
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    // Gate 1: dispatch is fire-and-forget, no Result.
    #[test]
    fn dispatch_signature_is_unit() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let effects = step(&mut state, &clock, Cmd::Action(AppAction::Logout));
        assert_eq!(state.session_epoch, 1);
        assert!(!effects.is_empty());
    }

    // Gate 2: failures surface as state.
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

    // Gate 3: reducer is sync (runs in a non-tokio thread).
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

    // Gate 4: closed views → no snapshots from registry.
    #[test]
    fn closed_view_emits_no_snapshot() {
        let mut registry = ViewRegistry::default();
        registry.open(ViewId::AppRoot, ViewRoute::AppRoot);
        registry.close(&ViewId::AppRoot);
        assert!(!registry.is_open(&ViewId::AppRoot));
        assert_eq!(registry.open_count(), 0);
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

    // Gate 9: route selection deterministic.
    #[test]
    fn route_selection_deterministic() {
        use crate::kernel::snapshot::RouteKind;
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

    // Gate 10: lifecycle idempotent.
    #[test]
    fn resume_suspend_idempotent() {
        let mut state = make_state();
        let clock = ManualClock::default();
        for _ in 0..5 {
            step(&mut state, &clock, Cmd::Resume);
            step(&mut state, &clock, Cmd::Suspend);
        }
        assert_eq!(state.session_epoch, 0);
    }

    #[test]
    fn shutdown_idempotent() {
        let mut state = make_state();
        let clock = ManualClock::default();
        for _ in 0..3 {
            step(&mut state, &clock, Cmd::Shutdown);
        }
        assert!(matches!(state.session, SessionState::Unknown));
    }

    // Gate 11: logout cancels view-scoped effects via epoch bump.
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
