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
  - capture-panel
supersedes:
  - 2026-06-13-2-camerax-front-camera-fallback-for-capture
related_claims: []
source_lines:
  - 3044-3091
captured_at: 2026-06-13T18:06:05Z
---

# Episode: CameraX front-camera fallback for devices without back camera

## Prior State

CameraCapture.kt bound only DEFAULT_BACK_CAMERA with a single catch that logged and set an error message — no retry. On devices without a back camera (e.g. emulators with only front camera), neither Preview nor ImageCapture got bound, causing takePicture to also throw IllegalArgumentException.

## Trigger

Emulator validation revealed 'IllegalArgumentException: No available camera can be found' — the shutter's takePicture failed because ImageCapture was never successfully bound to any camera.

## Decision

Added nested try/catch: if DEFAULT_BACK_CAMERA bindToLifecycle throws, unbindAll and retry with DEFAULT_FRONT_CAMERA, binding both preview and imageCapture together. Only if both fail is the error surfaced to UI.

## Consequences

- Camera capture pipeline works on front-camera-only devices (emulators, some tablets)
- Barcode scanner was already working because it bound Preview + ImageAnalysis together, confirming the pattern
- Both use cases (preview + capture) are always bound in a single bindToLifecycle call — never lazily or separately

## Open Tail

*(none)*

## Evidence

- transcript lines 3044-3091

