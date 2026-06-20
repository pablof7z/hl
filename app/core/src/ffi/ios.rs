//! iOS-facing UniFFI export: `HighlighterApp` — the bounded kernel FFI surface.
//!
//! This is the NEW lane introduced in Phase 1. It coexists with `HighlighterCore`
//! (in `client.rs`) — UniFFI supports multiple exported objects in one crate.
//!
//! Exactly 10 methods (spec §step 7):
//!   new / set_observer / dispatch / open_view / close_view /
//!   current_snapshot / resume / suspend / provide_capability_result / shutdown.
//!
//! `dispatch` returns `()` — fire-and-forget, no Result (Non-Negotiable #3 / D6).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::runtime::Runtime;
use tokio::sync::mpsc;

use crate::capabilities::CapabilityResult;
use crate::kernel::action::AppAction;
use crate::kernel::actor::{actor_task, start_nmp_app, Cmd, HighlighterObserver, SharedState};
use crate::kernel::app::{AppConfig, CreateAccountPolicy, KernelPolicy, RoomPolicy, SeedRelay};
use crate::kernel::clock::SystemClock;
use crate::kernel::snapshot::ViewSnapshot;
use crate::kernel::view::{ViewId, ViewRoute};
use crate::onboarding::OnboardingStore;

/// The new-lane kernel object. Swift holds one of these alongside the live
/// `HighlighterCore` during Phase 1; later phases migrate screens one by one.
///
/// Thread safety: all public methods are `&self` — internal mutation is
/// routed through the mpsc channel or shared `Arc<Mutex<...>>` (no `&mut self`
/// needed across the UniFFI boundary).
#[derive(uniffi::Object)]
pub struct HighlighterApp {
    tx: mpsc::UnboundedSender<Cmd>,
    shared: Arc<SharedState>,
    /// Dedicated tokio runtime for the actor task. Stored in a `Mutex<Option<>>`
    /// so we can safely call `shutdown_background()` on Drop even when running
    /// inside another tokio runtime (e.g., in `#[tokio::test]` contexts).
    runtime: Mutex<Option<Runtime>>,
    shutdown_sent: AtomicBool,
}

#[uniffi::export]
impl HighlighterApp {
    /// Construct the kernel and start the actor task.
    ///
    /// May fail only for unrecoverable local init (tokio runtime creation,
    /// storage path creation). Recoverable failures are state.
    #[uniffi::constructor]
    pub fn new(config: AppConfig) -> Arc<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("hl-kernel")
            .build()
            .expect("hl-kernel tokio runtime");

        let (tx, rx) = mpsc::unbounded_channel::<Cmd>();
        let shared = SharedState::new();
        let clock = Arc::new(SystemClock);

        // OnboardingStore reads from the app's data directory.
        let data_dir_path = std::path::PathBuf::from(&config.data_dir);
        let onboarding_store = Arc::new(OnboardingStore::new(&data_dir_path));

        // Boot the NmpApp (Pattern 1 from nmp_runtime.rs:725-860).
        let nmp = start_nmp_app(&config.data_dir, tx.clone());

        // Build the relay/follow policy from relay_policy.json seed defaults.
        // D3: relay URLs live in relay_policy.json, not in kernel logic.
        // Phase 5A: pass data_dir so the whats-new effect runner can read/write
        // the seen-marker file at {data_dir}/whats-new-state-v1.json.
        let policy = Arc::new(build_kernel_policy(&config.data_dir));

        let shared_clone = shared.clone();
        let tx_clone = tx.clone();
        runtime.spawn(actor_task(
            rx,
            tx_clone,
            shared_clone,
            clock,
            onboarding_store,
            nmp,
            policy,
        ));

        Arc::new(Self {
            tx,
            shared,
            runtime: Mutex::new(Some(runtime)),
            shutdown_sent: AtomicBool::new(false),
        })
    }

    /// Register the platform observer for snapshot push and capability requests.
    pub fn set_observer(&self, observer: Arc<dyn HighlighterObserver>) {
        *self.shared.observer.write() = Some(observer);
    }

    /// Fire-and-forget action dispatch. Never returns a Result (Non-Negotiable #3).
    pub fn dispatch(&self, action: AppAction) {
        let _ = self.tx.send(Cmd::Action(action));
    }

    /// Register a bounded projection for a view. Subsequent state changes will
    /// emit snapshots for this view until `close_view` is called.
    pub fn open_view(&self, view_id: ViewId, route: ViewRoute) {
        let _ = self.tx.send(Cmd::OpenView(view_id, route));
    }

    /// Deregister a view's projection. No further snapshots will be emitted
    /// for this `view_id` (Non-Negotiable #7 / D5).
    pub fn close_view(&self, view_id: ViewId) {
        let _ = self.tx.send(Cmd::CloseView(view_id));
    }

    /// Pull the latest computed snapshot for a view without waiting for the
    /// actor (useful for initial render / recovery after background).
    /// Returns `None` if the view is not open.
    pub fn current_snapshot(&self, view_id: ViewId) -> Option<ViewSnapshot> {
        let cache = self
            .shared
            .snapshots
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        cache.get(&view_id).cloned()
    }

    /// Notify the kernel that the app entered the foreground.
    pub fn resume(&self) {
        let _ = self.tx.send(Cmd::Resume);
    }

    /// Notify the kernel that the app entered the background.
    pub fn suspend(&self) {
        let _ = self.tx.send(Cmd::Suspend);
    }

    /// Deliver the native shell's response to a `CapabilityRequest`.
    pub fn provide_capability_result(&self, result: CapabilityResult) {
        let _ = self.tx.send(Cmd::ProvideCapabilityResult(result));
    }

    /// Idempotent shutdown. Safe to call multiple times.
    pub fn shutdown(&self) {
        if self
            .shutdown_sent
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            let _ = self.tx.send(Cmd::Shutdown);
        }
    }

    /// Send a clock tick to the actor — useful for deterministic testing of
    /// clock-driven behavior (toast dismiss, session timeout) without relying
    /// on wall-clock time (D8 / D9).
    pub fn tick(&self) {
        let _ = self.tx.send(Cmd::Tick);
    }
}

// ─── Policy bootstrap ────────────────────────────────────────────────────────

/// Build the `KernelPolicy` injected into `actor_task` at boot.
///
/// Relay URLs come from `relay_policy.json` seed defaults via the public
/// `relays::seed_defaults()` accessor — never from hardcoded literals here
/// (D3: relay policy is data, not kernel logic).
///
/// For `CreateAccount`, the NIP-65 / kind:10002 role follows the
/// `read`/`write` flags in each seed entry:
///   - read && write → "both"
///   - read only     → "read"
///   - write only    → "write"
///   - neither       → skip (pure app-data relay; no NIP-65 entry needed)
///
/// `initial_follows` is empty — ADR-0059 §5: no kind:3 published for a
/// fresh account until the user explicitly chooses follows.
fn build_kernel_policy(data_dir: &str) -> KernelPolicy {
    let seed = crate::relays::seed_defaults();
    let seed_relays: Vec<SeedRelay> = seed
        .into_iter()
        .filter_map(|r| {
            let role = match (r.read, r.write) {
                (true, true) => "both",
                (true, false) => "read",
                (false, true) => "write",
                // Pure rooms/indexer relays have no NIP-65 role; omit from
                // create-account relay list to avoid empty-role entries.
                (false, false) => return None,
            };
            Some(SeedRelay {
                url: r.url,
                role: role.to_string(),
            })
        })
        .collect();

    KernelPolicy {
        create_account: CreateAccountPolicy {
            seed_relays,
            initial_follows: Vec::new(), // ADR-0059 §5: empty → no kind:3
        },
        relay: Default::default(), // Phase 2D: seed_relay_urls populated at runtime
        // Phase 3G: wire discovery relay so RoomExplorer auto-starts on view-open.
        // URL comes from relay_policy.json ("room_explorer_curator") — D3 compliant.
        room: RoomPolicy {
            discovery_relay: crate::relays::room_explorer_curator_relay().to_string(),
            ..Default::default()
        },
        // Phase 5A: data_dir for whats-new seen-marker file I/O.
        data_dir: data_dir.to_string(),
    }
}

impl Drop for HighlighterApp {
    fn drop(&mut self) {
        self.shutdown();
        // Safely shut down the runtime without blocking on running tasks.
        // `shutdown_background()` initiates shutdown and returns immediately,
        // which is safe even when called from within another async context
        // (e.g., `#[tokio::test]` — avoids the "cannot drop runtime in async
        // context" panic).
        if let Ok(mut guard) = self.runtime.lock() {
            if let Some(rt) = guard.take() {
                rt.shutdown_background();
            }
        }
    }
}
