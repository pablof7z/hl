//! Capability bridge types — Phase 1: Keychain only.
//!
//! Native shells execute raw OS capabilities and report results back via
//! `HighlighterApp::provide_capability_result`. Rust decides what the result
//! means and what happens next (D7). No business logic lives in native.

pub mod keychain;

pub use keychain::{KeychainOp, KeychainResult};

/// A request from the Rust kernel to the native shell to execute an OS
/// capability. Emitted via `HighlighterObserver::on_capability_request`.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum CapabilityRequest {
    /// Keychain load/clear request.
    Keychain(KeychainOp),
}

/// The native shell's response to a prior `CapabilityRequest`. Delivered via
/// `HighlighterApp::provide_capability_result`.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum CapabilityResult {
    /// Response to a `CapabilityRequest::Keychain`.
    Keychain(KeychainResult),
}
