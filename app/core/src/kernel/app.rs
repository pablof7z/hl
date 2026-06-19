//! `AppState` — the single app model owned by the kernel actor.
//!
//! Rust is the single writer for all app facts (Non-Negotiable #2).
//! The state is split into sub-models by concern; all live as fields on
//! `AppState` so the reducer can read and write the full picture atomically.

use std::path::PathBuf;

use crate::kernel::action::{SignInMethod, SignerKind};

/// Session state machine.
///
/// Append-only: new variants at the bottom keep rebases mechanical.
/// Signer policy lives here in Rust; native never mutates session facts.
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
    /// and `signer_kind` records which backend is active.
    Present {
        pubkey: String,
        signer_kind: SignerKind,
    },
    /// No session secret found (user never logged in or has logged out).
    Absent,
    /// Restore attempt failed — diagnostic carried as state (D6).
    RestoreFailed { error: String },

    // ── Phase 2A additions (append-only) ─────────────────────────────────────
    /// `add_signer` / `add_bunker` call dispatched; waiting for the
    /// identity-change observer to fire. The clock-driven timeout (30 s) will
    /// transition to `SignInFailed` if the observer never fires.
    SigningIn {
        method: SignInMethod,
        /// UNIX second when sign-in was started (for timeout, future phases).
        started_at: u64,
    },
    /// Sign-in attempt failed — method and error carried as state (D6).
    /// Never returned across the dispatch boundary as a `Result`.
    SignInFailed { method: SignInMethod, error: String },
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

    // ── Phase 2B additions ────────────────────────────────────────────────────
    /// The most recently minted `nostrconnect://` URI, if any. Cleared when
    /// sign-in completes (IdentityChanged fires) or on Logout. Bounded: one
    /// String ≤ 512 bytes per NIP-46 spec. Exposed via `AppRootSnapshot` so
    /// the iOS QR-code sheet can render it without polling.
    pub nostrconnect_uri: Option<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            session: SessionState::Unknown,
            onboarding: OnboardingState::default(),
            route: RouteState::default(),
            chrome: ChromeState::default(),
            session_epoch: 0,
            nostrconnect_uri: None,
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

/// UNIX seconds after dispatch of a sign-in action before the kernel
/// transitions to `SessionState::SignInFailed`.
///
/// NMP handles parse errors internally (`set_last_error_toast`) without firing
/// the identity-change observer. This clock-driven timeout ensures an invalid
/// nsec — or any other case where the observer never fires — surfaces in
/// `SessionState` rather than leaving the UI stuck in `SigningIn` forever (D6).
pub const SIGN_IN_TIMEOUT_SECS: u64 = 30;

impl AppState {
    /// Storage sub-directory the new lane's `NmpApp` will use.
    pub fn nmp_storage_path(data_dir: &str) -> PathBuf {
        PathBuf::from(data_dir).join("nmp-lane")
    }
}
