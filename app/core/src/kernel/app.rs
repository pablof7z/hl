//! `AppState` — the single app model owned by the kernel actor.
//!
//! Rust is the single writer for all app facts (Non-Negotiable #2).
//! The state is split into sub-models by concern; all live as fields on
//! `AppState` so the reducer can read and write the full picture atomically.

use std::path::PathBuf;

/// Session state machine.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionState {
    /// Not yet attempted to restore a session.
    Unknown,
    /// Restore in progress: waiting for a `CapabilityResult::Keychain`.
    Restoring {
        /// UNIX second when restoration was started (for timeout).
        started_at: u64,
    },
    /// A session secret is in memory; `pubkey` is the decoded hex public key
    /// (Phase 1: carried forward from the capability result string).
    Present { pubkey: String },
    /// No session secret found (user never logged in or has logged out).
    Absent,
    /// Restore attempt failed — diagnostic carried as state (D6).
    RestoreFailed { error: String },
}

/// Durable onboarding completion flag.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OnboardingState {
    /// Whether the user has completed onboarding. Read from `OnboardingStore`
    /// on startup via `Effect::LoadOnboardingFlag`.
    pub complete: bool,
    /// Whether the flag has been loaded yet (distinguishes "false by default"
    /// from "loaded and genuinely false").
    pub loaded: bool,
}

/// Navigation route and sheet stack.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RouteState {
    /// Index of the selected root tab (matches `RootTab` raw values).
    pub root_tab: u8,
    /// ID of the sheet currently presented over the root shell, if any.
    pub sheet_id: Option<String>,
}

/// In-memory toast state.
#[derive(Debug, Clone, PartialEq)]
pub struct ToastState {
    pub message: String,
    /// UNIX second at which the kernel should auto-dismiss (clock-driven, D8).
    pub dismiss_at_unix: u64,
}

/// App chrome (toast, feedback sheet).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChromeState {
    pub toast: Option<ToastState>,
    pub feedback_presented: bool,
}

/// The single app model. Every field is a fact owned exclusively by the
/// Rust actor; no native store writes here (Non-Negotiable #2).
#[derive(Debug, Clone)]
pub struct AppState {
    pub session: SessionState,
    pub onboarding: OnboardingState,
    pub route: RouteState,
    pub chrome: ChromeState,
    /// Monotonic counter bumped on every logout. Effects keyed to the current
    /// epoch are silently dropped when the epoch has advanced (idempotent
    /// cancellation — plan line 157).
    pub session_epoch: u64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            session: SessionState::Unknown,
            onboarding: OnboardingState::default(),
            route: RouteState::default(),
            chrome: ChromeState::default(),
            session_epoch: 0,
        }
    }
}

/// Configuration passed to `HighlighterApp::new`.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AppConfig {
    /// Application-support / documents directory for this app instance.
    /// The kernel will create `<data_dir>/nmp-lane/` for its own storage.
    pub data_dir: String,
}

/// UNIX seconds after dispatch of `RestoreSession` before the kernel
/// transitions to `SessionState::Absent` (no response from native keychain).
pub const SESSION_RESTORE_TIMEOUT_SECS: u64 = 30;

/// Duration in seconds after presentation before the kernel auto-dismisses
/// a chrome toast (clock-driven, no Swift Timer).
pub const TOAST_DISMISS_SECS: u64 = 3;

impl AppState {
    /// Storage sub-directory the new lane's `NmpApp` will use.
    pub fn nmp_storage_path(data_dir: &str) -> PathBuf {
        PathBuf::from(data_dir).join("nmp-lane")
    }
}
