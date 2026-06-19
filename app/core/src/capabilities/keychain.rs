//! Keychain capability types — Phase 1.
//!
//! Rust decides what operations to perform and what the result means;
//! native (iOS Keychain / Android Keystore) only executes the raw
//! storage request (D7: native capabilities execute, Rust decides).

/// What the kernel is asking the native keychain to do.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum KeychainOp {
    /// Load the persisted session secret (nsec or bunker URI).
    LoadSession,
    /// Delete the persisted session secret (on logout).
    ClearSession,
}

/// Raw result from the native keychain, reported via `provide_capability_result`.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum KeychainResult {
    /// `LoadSession` completed. `Some(secret)` when a secret was found,
    /// `None` when the slot was empty (no prior session).
    SessionSecret(Option<String>),
    /// `ClearSession` completed successfully.
    Cleared,
    /// The native keychain returned an error (OS / access denial).
    /// Errors are data (D6) — the kernel surfaces them as typed state.
    Error(String),
}
