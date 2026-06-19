//! Kernel input types: `AppAction` (from native UI) and `KernelEvent`
//! (from async Rust work or native capability results).
//!
//! Both live here so all inputs to the reducer sit in one place.
//! `KernelEvent` is never exposed across FFI — native dispatches only
//! `AppAction`. The kernel feeds events back to itself internally.

use crate::capabilities::CapabilityResult;

/// The active root tab. Tab index matches the Swift `MainTabView` order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum RootTab {
    Feed = 0,
    Discover = 1,
    Capture = 2,
    Notifications = 3,
    Settings = 4,
}

/// Every user or platform action the kernel understands — Phase 1.
///
/// Dispatch is fire-and-forget (`dispatch(action)` returns `()`; Non-Negotiable #3).
/// Errors never propagate back as `Result` — they surface as typed `ViewSnapshot` state.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum AppAction {
    /// Attempt to restore a prior session from the native keychain.
    RestoreSession,
    /// Retry a failed restore (same effect as `RestoreSession`; separate
    /// variant for UI affordance clarity).
    RetryRestore,
    /// Clear the active session, emit a `ClearSession` capability request,
    /// bump the session epoch to cancel in-flight view-scoped effects.
    Logout,
    /// Mark onboarding as complete in the durable `OnboardingStore`.
    CompleteOnboarding,
    /// Switch the active root tab.
    SelectRootTab { tab: RootTab },
    /// Present a named sheet over the root shell.
    PresentSheet { sheet_id: String },
    /// Dismiss the topmost sheet.
    DismissSheet,
}

/// Internal kernel event — produced by async effects and native capability
/// results, fed back into the actor's command channel. Never crosses FFI.
#[derive(Debug, Clone)]
pub enum KernelEvent {
    /// A session-restore capability round-trip completed.
    /// `present` = a secret was found; `pubkey` = the hex pubkey decoded from it
    /// (Phase 2 — Phase 1 just records presence/absence).
    SessionRestored {
        present: bool,
        pubkey: Option<String>,
    },
    /// The `LoadOnboardingFlag` effect completed; `bool` = `is_complete()`.
    OnboardingStateLoaded(bool),
    /// A native capability result was delivered via `provide_capability_result`.
    CapabilityResult(CapabilityResult),
    /// NMP identity-change observer fired (active account changed or cleared).
    IdentityChanged(Option<String>),
    /// Clock-driven periodic tick — used for toast dismiss, session-restore
    /// timeout, and snapshot coalescing cadence (D8: no wall-clock reads,
    /// no sleeps; time is injected via the `Clock` abstraction, D9).
    ClockTick,
}
