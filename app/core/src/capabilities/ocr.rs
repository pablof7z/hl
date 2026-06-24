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
//! `OcrLine`, `OcrWord`, and `OcrRect` are the canonical OCR observation types
//! that cross the capability boundary. They are DEFINED here in the kernel lane
//! (Phase 7 — Part-C prep: relocated out of the bespoke `crate::ocr` so the
//! kernel no longer depends on the bespoke lane). The native Vision bridge
//! returns them via `OcrResult::Lines`; the kernel domain
//! (`kernel/domains/ocr.rs`) reconstructs markdown and projects selectable
//! words from them.
//!
//! The bespoke `crate::ocr` module re-imports these definitions while it still
//! exists; it owns the geometry `impl OcrRect` helpers it uses internally.

/// Normalized bounding rect for an OCR observation. Coordinates are in Vision's
/// normalized image space (origin bottom-left, `[0, 1]`).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct OcrRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// A single recognized word with its bounding box and confidence.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct OcrWord {
    pub text: String,
    pub bbox: OcrRect,
    pub confidence: f32,
}

/// A recognized line of text with its bounding box, confidence, and the words
/// it contains. The native Vision bridge produces these; the kernel domain
/// reconstructs structure from them.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct OcrLine {
    pub text: String,
    pub bbox: OcrRect,
    pub confidence: f32,
    pub words: Vec<OcrWord>,
}

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
