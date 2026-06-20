//! Camera capability types — Phase 5E.
//!
//! The iOS AVFoundation camera and `VNDocumentCameraViewController` (page scan)
//! are native resources; the kernel cannot invoke them directly. All business logic
//! — image routing into the OCR pipeline, ISBN barcode routing into 5C lookup —
//! lives in the Rust domain (`kernel/domains/camera.rs`). Native only executes
//! the raw camera capture or barcode scan (D7).
//!
//! Large images NEVER cross the FFI boundary as bytes: native writes the
//! perspective-flattened JPEG to a `data_dir` temp path and returns the path
//! string as `image_handle`. The kernel passes this path to the 5D OCR capability
//! and (eventually) to `nmp.blossom.upload` (5G). This satisfies bounded-FFI (D5)
//! and the Blossom `file_path` input contract.
//!
//! ## Types
//!
//! `CameraOp` is the request payload; `CameraResult` is the raw native response.
//! `CameraResult::Denied` and `CameraResult::Cancelled` are data (D6: errors are
//! never panics).

/// What the kernel is asking the native camera bridge to do.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum CameraOp {
    /// Capture a perspective-flattened page image using
    /// `VNDocumentCameraViewController` (book / document scan flow).
    ///
    /// Native writes the JPEG to `{data_dir}/camera-<uuid>.jpg` and returns an
    /// opaque `image_handle` (temp-file path). Large images stay on disk; the
    /// capability boundary carries a path, not raw bytes (D5).
    CapturePage,

    /// Scan EAN-13 / ISBN-13 barcodes via `AVCaptureMetadataOutput`.
    ///
    /// Native runs an `AVCaptureSession` targeting `EAN13` and `ISBN13` metadata
    /// types. The first decoded barcode string is returned as
    /// `CameraResult::Barcode { raw_string }` without normalization — the kernel
    /// normalizes and routes to the 5C ISBN lookup (D7: Rust owns logic).
    ScanBarcode,
}

/// Raw result from the native camera capability bridge, reported via
/// `provide_capability_result`. Errors and cancellations are data (D6).
#[derive(Debug, Clone, uniffi::Enum)]
pub enum CameraResult {
    /// `CapturePage` completed. `image_handle` is the `data_dir` temp-file path
    /// of the written JPEG. `width` and `height` are the pixel dimensions of the
    /// written file (needed by the capture screen before the OCR result arrives).
    ///
    /// The kernel routes `image_handle` into the 5D OCR flow by dispatching
    /// `CapabilityRequest::Ocr(OcrOp::RecognizeText { image_handle })`.
    PageImage {
        /// Opaque temp-file path within `data_dir`.
        image_handle: String,
        /// Pixel width of the written JPEG.
        width: u32,
        /// Pixel height of the written JPEG.
        height: u32,
    },

    /// `ScanBarcode` completed. `raw_string` is the raw barcode value as decoded
    /// by `AVCaptureMetadataOutput` (e.g. `"9780134685991"` for an EAN-13 barcode).
    /// The kernel normalizes the value and routes it to the 5C ISBN lookup.
    Barcode {
        /// Raw barcode string as decoded by AVFoundation. Not yet normalized.
        raw_string: String,
    },

    /// Camera permission was denied or restricted by the OS. Surfaces as state
    /// (D6: errors are data — the capture screen shows a permission prompt).
    Denied,

    /// The user cancelled the camera session without capturing. No-op (D6).
    Cancelled,

    /// A native AVFoundation / Vision error occurred. Surfaces as state (D6).
    Error(String),
}
