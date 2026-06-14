---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - toast-banner
  - core-error-emitter
  - hydration-errors
supersedes: []
related_claims: []
source_lines:
  - 1304-1328
  - 1471-1488
captured_at: 2026-06-13T13:15:31Z
---

# Episode: Persistent 'not found' toast from benign lookup failures

## Prior State

CoreError::NotFound's Display is 'not found'. Three benign per-card hydration paths (handle_web_metadata_resolved, handle_isbn_preview_resolved, request_profile subscribe failure) surfaced it as a global Error toast via set_toast, which appeared persistently because Android's ToastBanner had no auto-expiry.

## Trigger

Diagnosis found the exact emitter lines in nmp_app.rs (8969-8978, 8898-8910, 9536-9544). Any dead link or missing preview from an auto-fired per-card lookup popped the banner on Highlights, Rooms, and Room detail. iOS's ShareToastBanner also lacks a timer, confirming the emitter is shared/core-side.

## Decision

Primary: replaced set_toast(Error, …) with tracing::debug!(…) in all three benign paths in nmp_app.rs (fixes both platforms). Secondary: added a 4-second auto-expire LaunchedEffect keyed on the toast object in RootScene.kt that dispatches ClearToast, as an Android safety net.

## Consequences

- Persistent 'not found' banner eliminated on both platforms
- Only genuine errors surface as toasts
- Each new distinct toast resets the 4-second auto-expire timer

## Open Tail

*(none)*

## Evidence

- transcript lines 1304-1328
- transcript lines 1471-1488

