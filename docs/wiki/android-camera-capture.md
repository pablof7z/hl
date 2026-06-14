---
title: Android Camera Capture
slug: android-camera-capture
topic: nmp-app
summary: The OCR capture flow includes book-picker recents (primed by RequestBookPickerRecents), manual ISBN entry with normalizeIsbn validation and RequestIsbnPreview l
tags:
  - capture
volatility: warm
confidence: medium
created: 2026-06-13
updated: 2026-06-14
verified: 2026-06-13
compiled-from: conversation
sources:
  - session:847487cd-e15b-4222-85ee-4a5a2b6f590b
---

# Android Camera Capture

## Capture Panel

The OCR capture flow includes book-picker recents (primed by RequestBookPickerRecents), manual ISBN entry with normalizeIsbn validation and RequestIsbnPreview lookup, and an ISBN barcode scanner (ML Kit BarcodeScanning accepting EAN-13/EAN-8/ISBN).

The camera capture path uses CameraX with DEFAULT_BACK_CAMERA and falls back to DEFAULT_FRONT_CAMERA if back-camera binding fails; Preview and ImageCapture are bound together in a single bindToLifecycle call. The CAMERA permission is declared in AndroidManifest.xml and requested at runtime via rememberLauncherForActivityResult(RequestPermission) before opening the camera viewfinder.

Captured photos are processed by ML Kit TextRecognition to extract OCR lines, which are then reconstructed into markdown via OcrStructureReconstructor (a port of iOS's OCRStructureReconstructor), producing both markdown and a flattened alt-text for Blossom uploads.

The CapturePanel orchestrates three phases (Idle, Camera, Review). In the Camera phase, UploadCapturePhoto is dispatched immediately on shutter. In the Review phase, the user selects a quote and publishes via PublishCaptureHighlight with the full HighlightDraft, including a BlossomUpload whose alt text is derived from the OCR output.

CapturePageReview uses a plain Column (not verticalScroll) because it is hosted inside MainScaffold's LazyColumn, which already provides scrolling; nesting verticalScroll inside the LazyColumn item previously caused a crash.

<!-- citations: [^84748-150] [^84748-151] [^84748-152] [^84748-138] [^84748-139] [^84748-140] [^84748-141] [^84748-142] [^84748-149] [^84748-167] [^84748-181] [^84748-193] [^84748-206] -->
