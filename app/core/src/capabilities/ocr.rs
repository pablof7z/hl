//! OCR capability types — Phase 5D.
//!
//! The iOS Vision framework is a native resource; running `VNRecognizeTextRequest`
//! is a capability (the kernel cannot invoke Vision directly). All business logic
//! — markdown reconstruction, reading-order normalisation, header/footer stripping,
//! selectable-word projection — lives in the Rust domain
//! (`kernel/domains/ocr.rs`). Native only executes the raw Vision request (D7).
//!
//! Large images stay on disk: `image_handle` is a `data_dir` temp-file path so
//! the capability boundary carries a path string rather than raw bytes (D5).
//!
//! ## Types
//!
//! `OcrLine`, `OcrWord`, and `OcrRect` are the canonical types defined in the live
//! bespoke lane (`crate::ocr`). They are re-exported here so all kernel-domain code
//! can import them from one place (`crate::capabilities::ocr`) without duplicating
//! the `uniffi::Record` definitions (which would create FFI symbol conflicts).

/// Re-export the canonical OCR geometry and observation types from the live lane.
/// The kernel domain's `reconstruct_markdown` and `selectable_words` work with
/// these types end-to-end; native Vision bridge returns them via `OcrResult::Lines`.
pub use crate::ocr::{OcrLine, OcrRect, OcrWord};

/// What the kernel is asking the native OCR bridge to do.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum OcrOp {
    /// Run `VNRecognizeTextRequest` on the image at `image_handle`.
    ///
    /// `image_handle` is a `data_dir` temp-file path. Large images stay on
    /// disk; the capability boundary carries a path, not raw bytes (D5).
    /// The native bridge loads the image, runs Vision, and returns the raw
    /// `VNRecognizedTextObservation` data as `OcrResult::Lines`.
    RecognizeText {
        /// Temp-file path within `data_dir` (opaque — D3: kernel never
        /// constructs file paths beyond what it receives here).
        image_handle: String,
    },
}

/// Raw result from the native OCR capability bridge, reported via
/// `provide_capability_result`. Errors are data (D6).
#[derive(Debug, Clone, uniffi::Enum)]
pub enum OcrResult {
    /// `RecognizeText` completed. Contains the raw `VNRecognizedTextObservation`
    /// data as line observations (possibly empty when the image has no text).
    /// The kernel reconstructs markdown and projects selectable words from these.
    Lines(Vec<OcrLine>),
    /// A native Vision error occurred (e.g. image load failure, OS permission
    /// denied). Errors are data (D6) — the kernel surfaces them as typed state
    /// and never panics on bad input.
    Error(String),
}
