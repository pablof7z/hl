---
type: episode-card
date: 2026-06-14
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: active
subjects:
  - camera-capture
  - camera-fallback
  - android-camerax
supersedes:
  - 2026-06-14-1-camerax-front-camera-fallback-for-capture
related_claims: []
source_lines:
  - 3050-3092
captured_at: 2026-06-14T08:39:07Z
---

# Episode: Camera binding: add front-camera fallback

## Prior State

CameraX binding used only DEFAULT_BACK_CAMERA with no fallback — if back camera threw (emulators, devices without rear camera), neither Preview nor ImageCapture bound, and the shutter button silently failed

## Trigger

Testing on emulator where DEFAULT_BACK_CAMERA throws; shutter tap produced no image because no use cases were bound

## Decision

Added try/catch: bind DEFAULT_BACK_CAMERA first, and on failure, unbindAll() then retry with DEFAULT_FRONT_CAMERA binding both preview and imageCapture together

## Consequences

- Camera viewfinder and capture now work on devices/emulators with only a front camera
- Both Preview and ImageCapture always bind in a single bindToLifecycle call — no orphaned use cases
- If both selectors fail, a user-visible error message is surfaced

## Open Tail

*(none)*

## Evidence

- transcript lines 3050-3092

