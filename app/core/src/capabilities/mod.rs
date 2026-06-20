//! Capability bridge types — Phase 1: Keychain; Phase 5H: Audio; Phase 5K: Share-extension.
//!
//! Native shells execute raw OS capabilities and report results back via
//! `HighlighterApp::provide_capability_result`. Rust decides what the result
//! means and what happens next (D7). No business logic lives in native.

pub mod keychain;

// ── Phase 5K additions (append-only) ─────────────────────────────────────────
pub mod share;

// ── Phase 5H additions (append-only) ─────────────────────────────────────────
pub mod audio;

// ── Phase 5D additions (append-only) ─────────────────────────────────────────
pub mod ocr;

pub use audio::{AudioOp, AudioResult};
pub use keychain::{KeychainOp, KeychainResult};
pub use ocr::{OcrLine, OcrOp, OcrRect, OcrResult, OcrWord};
pub use share::{RawSharePayload, ShareOp, ShareResult};

/// A request from the Rust kernel to the native shell to execute an OS
/// capability. Emitted via `HighlighterObserver::on_capability_request`.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum CapabilityRequest {
    /// Keychain load/clear request.
    Keychain(KeychainOp),

    // ── Phase 5K additions (append-only) ─────────────────────────────────────
    /// Share-extension App Group read/write request.
    Share(ShareOp),

    // ── Phase 5H additions (append-only) ─────────────────────────────────────
    /// Audio player transport request (load/play/pause/seek/stop/waveform).
    Audio(AudioOp),

    // ── Phase 5D additions (append-only) ─────────────────────────────────────
    /// Vision text-recognition request. Native runs VNRecognizeTextRequest,
    /// returns raw line observations (text + bbox + words + confidence).
    /// Large images stay on disk (image_handle = data_dir temp path, D5).
    Ocr(OcrOp),
}

/// The native shell's response to a prior `CapabilityRequest`. Delivered via
/// `HighlighterApp::provide_capability_result`.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum CapabilityResult {
    /// Response to a `CapabilityRequest::Keychain`.
    Keychain(KeychainResult),

    // ── Phase 5K additions (append-only) ─────────────────────────────────────
    /// Response to a `CapabilityRequest::Share`.
    Share(ShareResult),

    // ── Phase 5H additions (append-only) ─────────────────────────────────────
    /// Response to a `CapabilityRequest::Audio` — progress, loaded, peaks, or error.
    Audio(AudioResult),

    // ── Phase 5D additions (append-only) ─────────────────────────────────────
    /// Response to a `CapabilityRequest::Ocr`.
    Ocr(OcrResult),
}
