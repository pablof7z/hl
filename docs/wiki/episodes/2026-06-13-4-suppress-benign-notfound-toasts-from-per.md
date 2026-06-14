---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - toast-banner
  - core-error-handling
  - hydration-errors
supersedes:
  - 2026-06-13-3-persistent-not-found-toast-from-benign
related_claims: []
source_lines:
  - 1304-1328
  - 1471-1494
captured_at: 2026-06-13T13:20:33Z
---

# Episode: Suppress benign NotFound toasts from per-card hydration paths

## Prior State

CoreError::NotFound's Display ("not found") was surfaced as a global Error toast via set_toast from three per-card hydration paths: handle_web_metadata_resolved, handle_isbn_preview_resolved, and request_profile subscribe failure. Any dead link or missing preview caused a persistent banner. Android's ToastBanner had no auto-expiry; iOS's ShareToastBanner also lacked a timer.

## Trigger

Diagnosis traced the persistent "not found" banner to three set_toast(Error, ...) calls in nmp_app.rs on benign per-card lookup failures that fire automatically for every visible card.

## Decision

Replace three set_toast calls with tracing::debug!() in nmp_app.rs (fixes both Android and iOS at the source). Add a 4-second auto-expire LaunchedEffect on Android's RootScene as a safety net (keyed on toast object, dispatches ClearToast after delay).

## Consequences

- "Not found" banner no longer appears on benign lookup misses on either platform
- Actual errors still surface via remaining set_toast calls
- Each new toast resets the auto-expire timer
- Core-side logging preserved for debugging via tracing::debug!

## Open Tail

*(none)*

## Evidence

- transcript lines 1304-1328
- transcript lines 1471-1494

