---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: superseded
subjects:
  - camerax-binding
  - android-capture
  - camera-fallback
supersedes: []
related_claims: []
source_lines:
  - 2971-2998
  - 3044-3091
  - 3150-3158
captured_at: 2026-06-13T18:37:41Z
---

# Episode: CameraX front-camera fallback for capture pipeline

## Prior State

CameraX capture screen bound DEFAULT_BACK_CAMERA only; the single catch block logged the error and showed a message — no fallback selector. On devices/emulators without a back camera, neither Preview nor ImageCapture was bound, so the shutter's takePicture also threw.

## Trigger

Emulator validation of the OCR capture flow crashed with `IllegalArgumentException: No available camera can be found`. The barcode scanner (which binds Preview + ImageAnalysis together) worked, confirming the camera hardware was available — the capture path just couldn't bind to it.

## Decision

Added a front-camera fallback: catch block now calls unbindAll() and retries with DEFAULT_FRONT_CAMERA, binding both Preview and ImageCapture in the same call. A second catch handles total failure.

## Consequences

- Capture pipeline works on front-camera-only devices and emulators
- Back camera remains the primary selector on dual-camera devices
- Both use cases (Preview + ImageCapture) are always bound together in a single bindToLifecycle call — no lazy/separate binding

## Open Tail

*(none)*

## Evidence

- transcript lines 2971-2998
- transcript lines 3044-3091
- transcript lines 3150-3158

