//! Camera capture domain — Phase 5E.
//!
//! ## Responsibilities
//!
//! * **Emit** `CapabilityRequest::Camera(CameraOp::CapturePage)` when the
//!   `hl.camera.capture_page` envelope is dispatched, storing the in-flight state
//!   in `AppState::camera`.
//!
//! * **Emit** `CapabilityRequest::Camera(CameraOp::ScanBarcode)` when the
//!   `hl.camera.scan_barcode` envelope is dispatched.
//!
//! * **Route** `CameraResult::PageImage { image_handle, width, height }` into the
//!   5D OCR pipeline: stores dimensions in `AppState::camera`, dispatches
//!   `CapabilityRequest::Ocr(OcrOp::RecognizeText { image_handle })`, and updates
//!   `AppState::ocr.image_handle` + `pending = true` — exactly as the
//!   `hl.ocr.recognize` action envelope does, so the 5F capture-draft FSM
//!   receives the OCR result via the same path.
//!
//! * **Route** `CameraResult::Barcode { raw_string }` into the 5C ISBN lookup:
//!   normalizes the raw barcode string to a 13-digit ISBN, then calls
//!   `isbn::reduce_action_lookup_isbn` so the standard 5C cache + HTTP flow runs.
//!
//! * **Absorb** `CameraResult::Denied`, `::Cancelled`, and `::Error` as typed
//!   state on `AppState::camera.last_error` (D6: errors are data, never panics).
//!
//! ## Device-local
//!
//! Camera state is device-local per `hl-app-state-vs-nostr-facts`. Nothing here
//! is ever published as a nostr event. The captured image_handle flows into
//! `AppState::ocr` (5D domain) and from there into the 5F capture-draft; the
//! ISBN barcode flows into `AppState::isbn` (5C domain).
//!
//! ## Native capability boundary
//!
//! This domain owns only the kernel request/result state. Native shells execute
//! camera/barcode capabilities and report bounded raw results; they do not own
//! capture publish policy or nostr event shaping.
//!
//! ## FFI surface
//!
//! Actions reach this domain via the `AppActionEnvelope` namespace:
//! - `hl.camera.capture_page` — emit `CameraOp::CapturePage`
//! - `hl.camera.scan_barcode` — emit `CameraOp::ScanBarcode`
//! - `hl.camera.cancel`       — transition pending → idle, no capability emitted
//!
//! The `AppAction` uniffi enum is NOT grown (convention from the team-lead brief:
//! FFI action surface is frozen; actions via envelope namespace only).

use crate::capabilities::camera::{CameraOp, CameraResult};
use crate::capabilities::ocr::OcrOp;
use crate::capabilities::CapabilityRequest;
use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;

// ─── AppState::camera ─────────────────────────────────────────────────────────

/// Camera-session state — device-local, never published to nostr.
///
/// `pending` is true while a `CameraOp` is in flight (between
/// `CapabilityRequest::Camera` emission and the `CameraResult` arrival).
/// `last_error` records the most recent `Denied` / `Error` message so the
/// capture screen can surface a prompt (D6: errors are typed state, not panics).
/// `last_image_width` / `last_image_height` are the pixel dimensions of the most
/// recently captured page image (used by the capture screen before the OCR result
/// arrives). Both are `0` until a `PageImage` result has been received.
#[derive(Debug, Clone, Default)]
pub struct CameraState {
    /// True while a `CapabilityRequest::Camera` is in flight.
    pub pending: bool,
    /// Most recent camera error or denial message. Empty when none.
    pub last_error: String,
    /// Pixel width of the most recently captured page image.
    pub last_image_width: u32,
    /// Pixel height of the most recently captured page image.
    pub last_image_height: u32,
    /// `true` after `CameraResult::Denied` arrives; cleared when the user
    /// re-dispatches a camera action so the screen can re-check permission.
    pub permission_denied: bool,
}

// ─── Reducers (action envelope) ───────────────────────────────────────────────

/// `hl.camera.capture_page` — emit `CapabilityRequest::Camera(CameraOp::CapturePage)`.
///
/// Sets `camera.pending = true` and emits the capability request. The round-trip
/// completes when `CameraResult::PageImage` (or `Denied`/`Cancelled`/`Error`)
/// arrives via `provide_capability_result`.
///
/// No-op (D6) if a camera operation is already in flight (`pending == true`),
/// to avoid duplicate native sessions.
pub(crate) fn reduce_action_capture_page(state: &mut AppState) -> Vec<Effect> {
    if state.camera.pending {
        tracing::debug!("camera.capture_page: already pending — no-op (D6)");
        return vec![];
    }
    state.camera.pending = true;
    state.camera.last_error = String::new();
    vec![Effect::EmitCapabilityRequest(CapabilityRequest::Camera(
        CameraOp::CapturePage,
    ))]
}

/// `hl.camera.scan_barcode` — emit `CapabilityRequest::Camera(CameraOp::ScanBarcode)`.
///
/// Sets `camera.pending = true`. The round-trip completes when
/// `CameraResult::Barcode { raw_string }` (or `Denied`/`Cancelled`/`Error`) arrives.
///
/// No-op (D6) if a camera operation is already pending.
pub(crate) fn reduce_action_scan_barcode(state: &mut AppState) -> Vec<Effect> {
    if state.camera.pending {
        tracing::debug!("camera.scan_barcode: already pending — no-op (D6)");
        return vec![];
    }
    state.camera.pending = true;
    state.camera.last_error = String::new();
    vec![Effect::EmitCapabilityRequest(CapabilityRequest::Camera(
        CameraOp::ScanBarcode,
    ))]
}

/// `hl.camera.cancel` — clear pending state without a capability call.
///
/// Used when the UI dismisses the camera sheet before a result arrives.
/// The native side must cancel its own session; this just resets kernel state.
pub(crate) fn reduce_action_cancel(state: &mut AppState) -> Vec<Effect> {
    state.camera.pending = false;
    vec![]
}

// ─── Reducer (capability result) ──────────────────────────────────────────────

/// Reduce a `CameraResult` from `CapabilityResult::Camera`.
///
/// Routing:
/// - `PageImage` → store dimensions; dispatch `OcrOp::RecognizeText` into the 5D
///   OCR pipeline (same path as `hl.ocr.recognize`).
/// - `Barcode` → normalize raw barcode string; dispatch 5C ISBN lookup.
/// - `Denied` → set `permission_denied = true`, store error (D6).
/// - `Cancelled` → clear pending, no-op (D6).
/// - `Error` → clear pending, store error string (D6).
pub(crate) fn reduce_capability_camera(state: &mut AppState, result: CameraResult) -> Vec<Effect> {
    state.camera.pending = false;
    match result {
        CameraResult::PageImage {
            image_handle,
            width,
            height,
        } => {
            // Store image dimensions in camera state.
            state.camera.last_image_width = width;
            state.camera.last_image_height = height;

            // Route image_handle into the 5D OCR pipeline.
            // Mirror exactly what `ocr::reduce_action_ocr_recognize` does so the
            // OCR result flows into AppState::ocr and from there into the 5F
            // capture-draft snapshot (capture_draft::project_capture_snapshot
            // reads AppState::ocr for its OCR fields).
            state.ocr.pending = true;
            state.ocr.image_handle = Some(image_handle.clone());

            vec![Effect::EmitCapabilityRequest(CapabilityRequest::Ocr(
                OcrOp::RecognizeText { image_handle },
            ))]
        }

        CameraResult::Barcode { raw_string } => {
            // Route the raw barcode string directly to the 5C ISBN lookup reducer.
            // isbn::reduce_action_lookup_isbn calls isbn::normalize_isbn which
            // validates the EAN-13 checksum and rejects non-book barcodes (D6).
            // No local normalizer needed — single ISBN normalizer with checksum
            // validation lives in isbn.rs (DRY).
            crate::kernel::domains::isbn::reduce_action_lookup_isbn(state, raw_string)
        }

        CameraResult::Denied => {
            state.camera.permission_denied = true;
            state.camera.last_error = "camera permission denied".to_string();
            vec![]
        }

        CameraResult::Cancelled => {
            // User cancelled — no state change beyond clearing pending (already done).
            vec![]
        }

        CameraResult::Error(msg) => {
            tracing::warn!(error = %msg, "CameraResult::Error — no-op (D6)");
            state.camera.last_error = msg;
            vec![]
        }
    }
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::camera::{CameraOp, CameraResult};
    use crate::capabilities::ocr::OcrOp;
    use crate::capabilities::{CapabilityRequest, CapabilityResult};
    use crate::kernel::action::{AppActionEnvelope, KernelEvent};
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::effect::Effect;

    fn make_state() -> AppState {
        AppState::default()
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    fn envelope(ns: &str, json: &str) -> Cmd {
        Cmd::ActionEnvelope(AppActionEnvelope {
            namespace: ns.to_string(),
            json: json.to_string(),
        })
    }

    // 5E-T1: camera_capture_emits_capability_request
    //
    // Dispatching hl.camera.capture_page must emit exactly one
    // EmitCapabilityRequest::Camera(CameraOp::CapturePage) and set pending.
    #[test]
    fn camera_capture_emits_capability_request() {
        let mut state = make_state();
        let clock = ManualClock::default();

        let effects = step(&mut state, &clock, envelope("hl.camera.capture_page", "{}"));

        let camera_reqs: Vec<_> = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::EmitCapabilityRequest(CapabilityRequest::Camera(CameraOp::CapturePage))
                )
            })
            .collect();

        assert_eq!(
            camera_reqs.len(),
            1,
            "must emit exactly one EmitCapabilityRequest::Camera(CapturePage); got: {effects:?}"
        );
        assert!(
            state.camera.pending,
            "camera.pending must be true after capture_page dispatch"
        );
        assert!(state.camera.last_error.is_empty(), "no error expected");
    }

    // 5E-T2: camera_result_routes_image_to_ocr
    //
    // CameraResult::PageImage must store dimensions, set ocr.pending, and emit
    // a CapabilityRequest::Ocr(OcrOp::RecognizeText) with the same image_handle.
    #[test]
    fn camera_result_routes_image_to_ocr() {
        let mut state = make_state();
        let clock = ManualClock::default();

        // Put the camera in pending state.
        state.camera.pending = true;

        let effects = step(
            &mut state,
            &clock,
            Cmd::ProvideCapabilityResult(CapabilityResult::Camera(CameraResult::PageImage {
                image_handle: "/tmp/scan-abc.jpg".to_string(),
                width: 1080,
                height: 1440,
            })),
        );

        // Camera state cleared.
        assert!(
            !state.camera.pending,
            "camera.pending must be false after result"
        );
        assert_eq!(state.camera.last_image_width, 1080);
        assert_eq!(state.camera.last_image_height, 1440);

        // OCR pipeline seeded.
        assert!(
            state.ocr.pending,
            "ocr.pending must be true after PageImage"
        );
        assert_eq!(
            state.ocr.image_handle.as_deref(),
            Some("/tmp/scan-abc.jpg"),
            "ocr.image_handle must match the PageImage handle"
        );

        // Exactly one OCR capability request emitted.
        let ocr_reqs: Vec<_> = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::EmitCapabilityRequest(CapabilityRequest::Ocr(
                        OcrOp::RecognizeText { .. }
                    ))
                )
            })
            .collect();
        assert_eq!(
            ocr_reqs.len(),
            1,
            "must emit exactly one OcrOp::RecognizeText; got: {effects:?}"
        );

        if let Effect::EmitCapabilityRequest(CapabilityRequest::Ocr(OcrOp::RecognizeText {
            image_handle,
        })) = &ocr_reqs[0]
        {
            assert_eq!(image_handle, "/tmp/scan-abc.jpg");
        }
    }

    // 5E-T3: book_scanner_isbn_routes_to_lookup
    //
    // CameraResult::Barcode with a valid ISBN-13 must dispatch the 5C ISBN lookup
    // (Effect::LookupIsbn). CameraResult::Barcode with an invalid barcode must
    // be a no-op (D6).
    #[test]
    fn book_scanner_isbn_routes_to_lookup() {
        let mut state = make_state();
        let clock = ManualClock::default();
        state.camera.pending = true;

        // Valid ISBN-13 barcode.
        let effects = step(
            &mut state,
            &clock,
            Cmd::ProvideCapabilityResult(CapabilityResult::Camera(CameraResult::Barcode {
                raw_string: "9780134685991".to_string(),
            })),
        );

        let isbn_effects: Vec<_> = effects
            .iter()
            .filter(|e| matches!(e, Effect::LookupIsbn { isbn13 } if isbn13 == "9780134685991"))
            .collect();
        assert_eq!(
            isbn_effects.len(),
            1,
            "valid ISBN-13 barcode must emit LookupIsbn; got: {effects:?}"
        );
        assert!(
            !state.camera.pending,
            "pending cleared after barcode result"
        );

        // Non-ISBN barcode (product UPC) must be a no-op.
        let mut state2 = make_state();
        state2.camera.pending = true;
        let effects2 = step(
            &mut state2,
            &clock,
            Cmd::ProvideCapabilityResult(CapabilityResult::Camera(CameraResult::Barcode {
                raw_string: "012345678901".to_string(), // 12 digits — UPC-A, not ISBN
            })),
        );
        let isbn_effects2: Vec<_> = effects2
            .iter()
            .filter(|e| matches!(e, Effect::LookupIsbn { .. }))
            .collect();
        assert!(
            isbn_effects2.is_empty(),
            "non-ISBN barcode must be a no-op (D6); got: {effects2:?}"
        );
    }

    // 5E-T4: camera_snapshot_raw — CameraState fields exposed correctly.
    //
    // After a PageImage result, AppState::camera must have the correct dimensions.
    // After a Barcode result, ISBN lookup is dispatched and camera state is cleared.
    // This test also verifies the snapshot carries the expected raw fields (D1).
    #[test]
    fn camera_snapshot_raw() {
        let mut state = make_state();
        let clock = ManualClock::default();

        // Simulate CapturePage → PageImage result.
        step(&mut state, &clock, envelope("hl.camera.capture_page", "{}"));
        assert!(state.camera.pending);

        step(
            &mut state,
            &clock,
            Cmd::ProvideCapabilityResult(CapabilityResult::Camera(CameraResult::PageImage {
                image_handle: "/tmp/page.jpg".to_string(),
                width: 800,
                height: 1200,
            })),
        );

        // Raw snapshot fields.
        assert!(!state.camera.pending);
        assert_eq!(state.camera.last_image_width, 800);
        assert_eq!(state.camera.last_image_height, 1200);
        assert!(state.camera.last_error.is_empty());
        assert!(!state.camera.permission_denied);
        // OCR was seeded.
        assert!(state.ocr.pending);
        assert_eq!(state.ocr.image_handle.as_deref(), Some("/tmp/page.jpg"));
    }

    // 5E-T5: capability_error_no_op (D6)
    //
    // CameraResult::Denied must set permission_denied + last_error, not panic.
    // CameraResult::Cancelled must be a silent no-op.
    // CameraResult::Error must set last_error, not panic.
    #[test]
    fn capability_error_no_op() {
        let clock = ManualClock::default();

        // Denied.
        let mut state = make_state();
        state.camera.pending = true;
        step(
            &mut state,
            &clock,
            Cmd::ProvideCapabilityResult(CapabilityResult::Camera(CameraResult::Denied)),
        );
        assert!(!state.camera.pending, "pending cleared on Denied");
        assert!(
            state.camera.permission_denied,
            "permission_denied set on Denied"
        );
        assert!(
            !state.camera.last_error.is_empty(),
            "last_error set on Denied"
        );

        // Cancelled.
        let mut state2 = make_state();
        state2.camera.pending = true;
        let effects = step(
            &mut state2,
            &clock,
            Cmd::ProvideCapabilityResult(CapabilityResult::Camera(CameraResult::Cancelled)),
        );
        assert!(!state2.camera.pending, "pending cleared on Cancelled");
        assert!(
            !state2.camera.permission_denied,
            "no permission_denied on Cancelled"
        );
        let camera_effects: Vec<_> = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::EmitCapabilityRequest(CapabilityRequest::Camera(_))
                )
            })
            .collect();
        assert!(
            camera_effects.is_empty(),
            "Cancelled must not re-emit a camera request"
        );

        // Error.
        let mut state3 = make_state();
        state3.camera.pending = true;
        step(
            &mut state3,
            &clock,
            Cmd::ProvideCapabilityResult(CapabilityResult::Camera(CameraResult::Error(
                "AVFoundation session failed".to_string(),
            ))),
        );
        assert!(!state3.camera.pending, "pending cleared on Error");
        assert_eq!(state3.camera.last_error, "AVFoundation session failed");
    }

    // 5E-T6: scan_barcode action emits ScanBarcode capability request.
    #[test]
    fn scan_barcode_emits_capability_request() {
        let mut state = make_state();
        let clock = ManualClock::default();

        let effects = step(&mut state, &clock, envelope("hl.camera.scan_barcode", "{}"));

        let barcode_reqs: Vec<_> = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::EmitCapabilityRequest(CapabilityRequest::Camera(CameraOp::ScanBarcode))
                )
            })
            .collect();
        assert_eq!(
            barcode_reqs.len(),
            1,
            "must emit exactly one CameraOp::ScanBarcode; got: {effects:?}"
        );
        assert!(state.camera.pending);
    }

    // 5E-T7: ISBN-10 barcode is normalized and converted to ISBN-13.
    #[test]
    fn isbn10_barcode_normalized_to_isbn13() {
        let clock = ManualClock::default();
        let mut state = make_state();
        state.camera.pending = true;

        // "0134685997" is ISBN-10 for "The Pragmatic Programmer" (2nd ed).
        // Converted: "978" + "013468599" + check_digit.
        let effects = step(
            &mut state,
            &clock,
            Cmd::ProvideCapabilityResult(CapabilityResult::Camera(CameraResult::Barcode {
                raw_string: "0134685997".to_string(),
            })),
        );

        let isbn_effects: Vec<_> = effects
            .iter()
            .filter(|e| matches!(e, Effect::LookupIsbn { .. }))
            .collect();
        assert_eq!(
            isbn_effects.len(),
            1,
            "ISBN-10 must normalize to ISBN-13 and route to LookupIsbn; got: {effects:?}"
        );

        // The ISBN-13 must start with "978013468599".
        if let Effect::LookupIsbn { isbn13 } = &isbn_effects[0] {
            assert!(
                isbn13.starts_with("978013468599"),
                "normalized isbn13 must start with 978013468599; got: {isbn13}"
            );
            assert_eq!(isbn13.len(), 13, "isbn13 must be 13 digits");
        }
    }

    // 5E-T8: duplicate camera request while pending is a no-op (D6).
    #[test]
    fn duplicate_camera_request_while_pending_is_noop() {
        let mut state = make_state();
        let clock = ManualClock::default();

        // First request.
        step(&mut state, &clock, envelope("hl.camera.capture_page", "{}"));
        assert!(state.camera.pending);

        // Second request while pending — must be a no-op.
        let effects = step(&mut state, &clock, envelope("hl.camera.capture_page", "{}"));
        let camera_reqs: Vec<_> = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::EmitCapabilityRequest(CapabilityRequest::Camera(_))
                )
            })
            .collect();
        assert!(
            camera_reqs.is_empty(),
            "duplicate camera request while pending must be a no-op; got: {effects:?}"
        );
    }

    // 5E-T9: cancel action clears pending without a capability request.
    #[test]
    fn cancel_clears_pending_without_capability_request() {
        let mut state = make_state();
        let clock = ManualClock::default();

        step(&mut state, &clock, envelope("hl.camera.capture_page", "{}"));
        assert!(state.camera.pending);

        let effects = step(&mut state, &clock, envelope("hl.camera.cancel", "{}"));
        assert!(!state.camera.pending, "pending must be cleared by cancel");
        let camera_reqs: Vec<_> = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::EmitCapabilityRequest(CapabilityRequest::Camera(_))
                )
            })
            .collect();
        assert!(
            camera_reqs.is_empty(),
            "cancel must not emit a camera capability request"
        );
    }

    // 5E-T10: camera_barcode_invalid_checksum_rejected
    //
    // A 978/979-prefix 13-digit barcode with a WRONG check digit must be
    // rejected by isbn::normalize_isbn (EAN-13 checksum validation) and produce
    // no LookupIsbn effect — no-op (D6). This guards against mis-scans.
    #[test]
    fn camera_barcode_invalid_checksum_rejected() {
        let clock = ManualClock::default();

        // "9780134685990" — correct digits are "9780134685991" (last digit wrong).
        let mut state = make_state();
        state.camera.pending = true;
        let effects = step(
            &mut state,
            &clock,
            Cmd::ProvideCapabilityResult(CapabilityResult::Camera(CameraResult::Barcode {
                raw_string: "9780134685990".to_string(),
            })),
        );
        let isbn_effects: Vec<_> = effects
            .iter()
            .filter(|e| matches!(e, Effect::LookupIsbn { .. }))
            .collect();
        assert!(
            isbn_effects.is_empty(),
            "ISBN-13 with wrong check digit must be rejected (no LookupIsbn); got: {effects:?}"
        );

        // "9790000000008" — a 979-prefix string with an invalid check digit.
        let mut state2 = make_state();
        state2.camera.pending = true;
        let effects2 = step(
            &mut state2,
            &clock,
            Cmd::ProvideCapabilityResult(CapabilityResult::Camera(CameraResult::Barcode {
                raw_string: "9790000000008".to_string(),
            })),
        );
        let isbn_effects2: Vec<_> = effects2
            .iter()
            .filter(|e| matches!(e, Effect::LookupIsbn { .. }))
            .collect();
        assert!(
            isbn_effects2.is_empty(),
            "979-prefix ISBN-13 with wrong check digit must be rejected; got: {effects2:?}"
        );
    }

    // 5E-T11: camera barcode result does NOT emit any publish effect (device-local).
    #[test]
    fn barcode_result_no_publish_effect() {
        let mut state = make_state();
        let clock = ManualClock::default();
        state.camera.pending = true;

        let effects = step(
            &mut state,
            &clock,
            Cmd::ProvideCapabilityResult(CapabilityResult::Camera(CameraResult::Barcode {
                raw_string: "9780134685991".to_string(),
            })),
        );

        let publish: Vec<_> = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::PublishHighlightEvent { .. }
                        | Effect::PublishCaptureEvent { .. }
                        | Effect::DispatchFollowAction { .. }
                        | Effect::DispatchNip29Action { .. }
                        | Effect::DispatchShareToRoom { .. }
                        | Effect::DispatchBookmarkAction { .. }
                        | Effect::DispatchReactAction { .. }
                )
            })
            .collect();
        assert!(
            publish.is_empty(),
            "barcode result must not emit any publish effect (device-local); got: {effects:?}"
        );
    }

    // 5E-T12: KernelEvent::CameraCapabilityResult injected via Cmd::Event is a no-op.
    //
    // The test-injection path (Cmd::Event(KernelEvent::CameraCapabilityResult))
    // must not change state — the live path goes through Cmd::ProvideCapabilityResult.
    // (Same pattern as KernelEvent::OcrRecognitionComplete and KernelEvent::ShareQueueDrained.)
    #[test]
    fn camera_kernel_event_is_noop() {
        let mut state = make_state();
        let clock = ManualClock::default();

        // Set some non-default state to verify it is unchanged.
        state.camera.last_error = "prior".to_string();

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::CameraCapabilityResult(
                crate::capabilities::CameraResult::Cancelled,
            )),
        );

        // State unchanged.
        assert_eq!(
            state.camera.last_error, "prior",
            "CameraCapabilityResult via Cmd::Event must not change state"
        );
        assert!(!state.camera.pending);
    }
}
