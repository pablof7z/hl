---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - android-toast
  - nmp-app-rs
  - root-scene
supersedes:
  - 2026-06-13-4-suppress-benign-notfound-toasts-from-per
related_claims: []
source_lines:
  - 1320-1328
  - 1471-1488
  - 1587-1588
captured_at: 2026-06-13T15:54:26Z
---

# Episode: "Not found" toast spam from benign lookup failures removed

## Prior State

The app showed a persistent, non-dismissing "not found" error toast banner on the Highlights feed, Rooms list, and Room detail screens. The toast had no auto-expiry — only manual Dismiss — and new failures would reset it, making it effectively permanent.

## Trigger

Diagnosis found three set_toast(Error, ...) calls in nmp_app.rs firing on benign Err arms: handle_isbn_preview_resolved (~line 8903), handle_web_metadata_resolved (~line 8971), and request_profile subscribe failure (~line 9536). These fire automatically per visible card for dead links and missing previews.

## Decision

Replaced all three set_toast calls with tracing::debug!() logging. Added a 4-second auto-expire LaunchedEffect on the Android side (RootScene.kt) as a safety net so any future spurious toast clears itself.

## Consequences

- Toast no longer appears on benign lookup failures on either Android or iOS (the emitter is shared/core-side)
- Debug information still available via tracing logs
- Any future toast will auto-dismiss after 4 seconds, preventing permanent banners

## Open Tail

*(none)*

## Evidence

- transcript lines 1320-1328
- transcript lines 1471-1488
- transcript lines 1587-1588

