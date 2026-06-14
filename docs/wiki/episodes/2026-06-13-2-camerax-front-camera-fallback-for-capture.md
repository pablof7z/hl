---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: superseded
subjects:
  - camerax-binding
  - ocr-capture
  - android-camera
supersedes: []
related_claims: []
source_lines:
  - 2982-3092
captured_at: 2026-06-13T17:52:11Z
---

# Episode: CameraX front-camera fallback for capture pipeline

## Prior State

CameraCapture bound DEFAULT_BACK_CAMERA only, with a single catch that logged the error and set an errorMessage but never retried — leaving both Preview and ImageCapture unbound

## Trigger

Emulator validation threw IllegalArgumentException: 'No available camera can be found' because the emulator only exposed a front camera; the barcode scanner (which bound Preview+ImageAnalysis together) worked while the capture path failed at takePicture

## Decision

Add front-camera fallback: if DEFAULT_BACK_CAMERA bind fails, unbindAll and retry with DEFAULT_FRONT_CAMERA, binding both Preview and ImageCapture in the same call

## Consequences

- Camera capture initializes on front-camera-only devices/emulators
- Barcode scanner already worked because its binding succeeded independently
- Full shutter→OCR→review pipeline still needs physical-device validation for real-world OCR quality

## Open Tail

- Emulator UI degradation blocked end-to-end validation of the shutter→OCR path; real-device test remains needed

## Evidence

- transcript lines 2982-3092

