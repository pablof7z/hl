---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: active
subjects:
  - android-toast
  - nmp-app-error-handling
supersedes:
  - 2026-06-13-2-not-found-toast-spam-from-benign
related_claims: []
source_lines:
  - 1471-1488
  - 1587-1588
captured_at: 2026-06-13T16:02:40Z
---

# Episode: Eliminate spurious 'not found' toast spam

## Prior State

Three set_toast(Error, ...) calls in the shared Rust core (nmp_app.rs) showed benign 'not found' error toasts on the Android UI whenever an ISBN preview, web metadata lookup, or profile subscription failed — normal expected-not-found conditions presented as user-facing errors.

## Trigger

Found during jank investigation: the toast spam contributed to UI noise and was triggered by the same emit storm that caused jank.

## Decision

Replace all three set_toast(Error, ...) calls in nmp_app.rs with tracing::debug!(...) (affects both Android and iOS since the emitter is shared). On the Android side, add a 4-second auto-expire LaunchedEffect in RootScene.kt that dispatches ClearToast after delay, keyed on the toast object so each distinct toast resets the timer.

## Consequences

- No spurious 'not found' toasts appear during normal feed load
- Real error toasts still surface but auto-dismiss after 4 seconds
- The Rust-side change applies to both platforms (shared core)

## Open Tail

*(none)*

## Evidence

- transcript lines 1471-1488
- transcript lines 1587-1588

