---
type: episode-card
date: 2026-06-14
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: superseded
subjects:
  - camerax-binding
  - capture-pipeline
  - android-camera
supersedes:
  - 2026-06-13-1-camerax-front-camera-fallback-for-capture
  - 2026-06-13-2-camerax-front-camera-fallback-for-devices
related_claims: []
source_lines:
  - 2970-3092
captured_at: 2026-06-14T08:09:37Z
---

# Episode: CameraX front-camera fallback for capture pipeline

## Prior State

Camera binding only attempted DEFAULT_BACK_CAMERA; on devices/emulators with only a front camera, bindToLifecycle threw IllegalArgumentException with no recovery, leaving both Preview and ImageCapture unbound so the shutter also crashed

## Trigger

Validation on emulator produced 'No available camera can be found' crash; the barcode scanner (which bound Preview+ImageAnalysis together) worked, confirming camera hardware was present but the capture path had no fallback

## Decision

Added a front-camera fallback: catch the back-camera bind failure, unbindAll, and retry with DEFAULT_FRONT_CAMERA, binding both Preview and ImageCapture in a single bindToLifecycle call

## Consequences

- Camera capture works on front-camera-only devices (emulators, some tablets)
- Both use cases are always bound together atomically, preventing the shutter-from-unbound-ImageCapture crash
- If both selectors fail, a user-visible error message surfaces instead of a silent crash

## Open Tail

- Physical-device validation of real-book OCR quality (pipeline proven on emulator)

## Evidence

- transcript lines 2970-3092

