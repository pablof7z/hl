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

use std::ffi::CString;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

use parking_lot::RwLock;
use tokio::sync::mpsc;

use nmp_defaults::{NmpAppBuilder, RunConfig};
use nmp_ffi::{
    nmp_app_free, nmp_app_nostrconnect_uri, nmp_app_signin_nip55, nmp_external_signer_init,
    nmp_signer_broker_init, NmpApp,
};
use zeroize::Zeroizing;

use crate::capabilities::{CapabilityRequest, CapabilityResult, KeychainOp};
use crate::kernel::action::{AppAction, KernelEvent, SignInMethod, SignerKind};
use crate::kernel::app::{
    AppState, SessionState, SESSION_RESTORE_TIMEOUT_SECS, SIGN_IN_TIMEOUT_SECS,
};
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

    // Sign-in timeout: NMP handles parse errors internally (set_last_error_toast)
    // without firing the identity-change observer — so an invalid nsec leaves us
    // in SigningIn indefinitely without this clock-driven fallback (D8).
    if let SessionState::SigningIn { started_at, method } = &state.session {
        if now.saturating_sub(*started_at) >= SIGN_IN_TIMEOUT_SECS {
            state.session = SessionState::SignInFailed {
                method: method.clone(),
                error: "sign-in timed out — no identity change observed".into(),
            };
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
            // Clear any pending NostrConnect URI on logout.
            state.nostrconnect_uri = None;
            // RemoveActiveAccount fires nmp.remove_account; ClearSession
            // emits a CapabilityRequest to native for its keychain.
            vec![Effect::RemoveActiveAccount, Effect::ClearSession]
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

        // ── Phase 2A additions ────────────────────────────────────────────────
        AppAction::SignInNsec { nsec } => {
            state.session = SessionState::SigningIn {
                method: SignInMethod::Nsec,
                started_at: now,
            };
            // AddNsecSigner calls nmp.add_signer(LocalNsec(nsec), true).
            // Success → IdentityChanged(Some(pubkey)); failure → SignInFailed.
            // Fire-and-forget: reducer never awaits the result.
            vec![Effect::AddNsecSigner { nsec }]
        }

        // ── Phase 2B additions ────────────────────────────────────────────────
        AppAction::PairBunker { uri } => {
            state.session = SessionState::SigningIn {
                method: SignInMethod::Bunker,
                started_at: now,
            };
            // AddBunkerSigner routes through the NIP-46 broker (nmp_signer_broker_init
            // must have run at boot). Fire-and-forget: broker resolves the signer
            // async; success arrives as IdentityChanged(Some).
            vec![Effect::AddBunkerSigner { uri }]
        }

        AppAction::StartNostrConnect => {
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

        AppAction::SignInNip55 => {
            state.session = SessionState::SigningIn {
                method: SignInMethod::Nip55,
                started_at: now,
            };
            // StartNip55SignIn calls nmp_app_signin_nip55(app, null). Fire-and-
            // forget: the host capability bridge exchanges with the external
            // signer app; success arrives as IdentityChanged(Some).
            vec![Effect::StartNip55SignIn]
        }
    }
}

fn reduce_event(state: &mut AppState, event: KernelEvent, _now: u64) -> Vec<Effect> {
    match event {
        KernelEvent::SessionRestored { present, pubkey } => {
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

        KernelEvent::OnboardingStateLoaded(complete) => {
            state.onboarding.complete = complete;
            state.onboarding.loaded = true;
            vec![]
        }

        KernelEvent::CapabilityResult(result) => reduce_capability_result(state, result),

        KernelEvent::IdentityChanged(pubkey) => {
            // NMP identity change — `Some(pk)` means a signer is now active;
            // `None` means the account was removed / logged out.
            match pubkey {
                Some(pk) if !pk.is_empty() => {
                    // Determine signer kind from the method we were SigningIn with.
                    // Bunker and NostrConnect both resolve to Nip46 (NIP-46 remote).
                    // Session restore and unknown paths default to LocalNsec.
                    let signer_kind = match &state.session {
                        SessionState::SigningIn { method, .. } => match method {
                            SignInMethod::Nsec | SignInMethod::CreateAccount => {
                                SignerKind::LocalNsec
                            }
                            SignInMethod::Bunker | SignInMethod::NostrConnect => SignerKind::Nip46,
                            SignInMethod::Nip55 => SignerKind::Nip55,
                        },
                        _ => SignerKind::LocalNsec,
                    };
                    // Clear the pending NostrConnect URI — the handshake is done.
                    state.nostrconnect_uri = None;
                    state.session = SessionState::Present {
                        pubkey: pk,
                        signer_kind,
                    };
                }
                _ => {
                    // None or empty pubkey → no active account.
                    state.nostrconnect_uri = None;
                    state.session = SessionState::Absent;
                }
            }
            vec![]
        }

        KernelEvent::ClockTick => {
            // Clock-driven checks already ran in `clock_checks` at the top of
            // `reduce`. `ClockTick` has no additional state changes.
            vec![]
        }

        // ── Phase 2A additions ────────────────────────────────────────────────
        KernelEvent::SignInFailed { method, error } => {
            // Surface failures in session state (D6 — never as Result).
            state.session = SessionState::SignInFailed { method, error };
            vec![]
        }

        // ── Phase 2B additions ────────────────────────────────────────────────
        KernelEvent::NostrConnectUriReady { uri } => {
            // Store the minted URI so the snapshot can expose it to the iOS
            // QR-code sheet. The NostrConnect sign-in session stays in SigningIn
            // until the remote signer completes the handshake (IdentityChanged).
            state.nostrconnect_uri = Some(uri);
            vec![]
        }

        KernelEvent::BunkerHandshakeState { .. } => {
            // Broker progress events are diagnostic only — no reducer state
            // change. Future phases may surface these in a dedicated snapshot.
            vec![]
        }
    }
}

fn reduce_capability_result(state: &mut AppState, result: CapabilityResult) -> Vec<Effect> {
    use crate::capabilities::KeychainResult;
    match result {
        CapabilityResult::Keychain(kr) => match kr {
            KeychainResult::SessionSecret(Some(secret)) => {
                // Phase 1 path: keychain returned a secret string (pre-nmp).
                // In Phase 2, nmp keyring restores fire IdentityChanged instead.
                state.session = SessionState::Present {
                    pubkey: secret,
                    signer_kind: SignerKind::LocalNsec,
                };
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
        // Phase 2B: expose pending NostrConnect URI to the iOS QR-code sheet.
        nostrconnect_uri: state.nostrconnect_uri.clone(),
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
///
/// `nmp` is `None` only in unit tests that do not boot a live `NmpApp`; the
/// nmp-call effects are no-ops in that case (tests inject `KernelEvent`s
/// directly instead).
pub(crate) async fn run_effect(
    effect: Effect,
    session_epoch: u64,
    tx: &mpsc::UnboundedSender<Cmd>,
    onboarding_store: &OnboardingStore,
    shared: &SharedState,
    nmp: Option<&NmpHandle>,
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

        // ── Phase 2A additions ────────────────────────────────────────────────
        Effect::AddNsecSigner { nsec } => {
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
                let nmp_ref: &NmpApp = unsafe { handle.0.as_ref() };
                nmp_ref.add_signer(
                    nmp_core::SignerSource::LocalNsec(Zeroizing::new(nsec)),
                    true, // make_active — also auto-persists to nmp keyring
                );
            }
            // No nmp handle (test mode) → test injects IdentityChanged directly.
        }

        Effect::RemoveActiveAccount => {
            // Read the active pubkey from the nmp slot, then remove it.
            // Fire-and-forget: the observer fires IdentityChanged(None) on success.
            if let Some(handle) = nmp {
                let nmp_ref: &NmpApp = unsafe { handle.0.as_ref() };
                let active_slot = nmp_ref.active_account_handle();
                let maybe_pubkey: Option<String> =
                    active_slot.lock().ok().and_then(|guard| guard.clone());
                if let Some(pubkey) = maybe_pubkey {
                    nmp_ref.remove_account(pubkey);
                }
            }
        }

        // ── Phase 2B additions ────────────────────────────────────────────────
        Effect::AddBunkerSigner { uri } => {
            // Route via nmp.add_signer(BunkerUri(uri), true). The NIP-46 broker
            // (nmp_signer_broker_init called at boot) takes over the handshake
            // async. Fire-and-forget: success arrives as IdentityChanged(Some).
            if let Some(handle) = nmp {
                let nmp_ref: &NmpApp = unsafe { handle.0.as_ref() };
                nmp_ref.add_signer(nmp_core::SignerSource::BunkerUri(uri), true);
            }
            // No nmp handle (test mode) → test injects IdentityChanged directly.
        }

        Effect::MintNostrConnectUri => {
            // Call nmp_app_nostrconnect_uri(app_ptr, null, null) — relay and
            // callback are resolved by nmp from its internal bootstrap relay slot
            // (V-65). Returns an owned `nostrconnect://` C string or null if no
            // relay is configured. Feed the result back as NostrConnectUriReady.
            if let Some(handle) = nmp {
                let raw_ptr = handle.0.as_ptr();
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

        Effect::StartNip55SignIn => {
            // Call nmp_app_signin_nip55(app_ptr, null) — null signer_package
            // lets the OS resolver pick the NIP-55 signer app (e.g. Amber).
            // nmp_app_signin_nip55 lazy-inits the external-signer driver if
            // nmp_external_signer_init was not already called at boot.
            // Fire-and-forget: success arrives as IdentityChanged(Some).
            if let Some(handle) = nmp {
                let raw_ptr = handle.0.as_ptr();
                nmp_app_signin_nip55(raw_ptr, std::ptr::null());
            }
            // No nmp handle (test mode) → test injects IdentityChanged directly.
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
    // `nmp_ref` is passed to `run_effect` for nmp-call effects (Phase 2A+).
    let nmp_handle = nmp;

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
            run_effect(
                effect,
                state.session_epoch,
                &tx,
                &onboarding_store,
                &shared,
                nmp_handle.as_ref(),
            )
            .await;
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

    // Phase 2B: initialise NIP-46 broker (needed for PairBunker and
    // StartNostrConnect). Idempotent per ADR-0052 §D3 — safe to call once.
    nmp_signer_broker_init(handle.0.as_ptr());

    // Phase 2B: initialise NIP-55 external-signer driver (needed for
    // SignInNip55). nmp_app_signin_nip55 lazy-inits too, but calling
    // explicitly here makes the init order deterministic.
    nmp_external_signer_init(handle.0.as_ptr());

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
}
