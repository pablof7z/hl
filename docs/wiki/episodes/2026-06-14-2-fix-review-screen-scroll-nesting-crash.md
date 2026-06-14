---
type: episode-card
date: 2026-06-14
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: superseded
subjects:
  - capture-page-review
  - compose-layout
  - android-ui
supersedes:
  - 2026-06-14-2-remove-nested-verticalscroll-in-capturepagereview
related_claims: []
source_lines:
  - 3150-3248
captured_at: 2026-06-14T08:39:07Z
---

# Episode: Fix review-screen scroll nesting crash

## Prior State

CapturePageReview's root Column had Modifier.verticalScroll(rememberScrollState()), nested as a direct descendant of MainScaffold's LazyColumn item — the classic 'vertically scrollable component measured with infinity maximum height constraints' crash

## Trigger

After camera capture + OCR succeeded, the app crashed with IllegalStateException on the review screen; logcat confirmed the infinite-height Compose layout error

## Decision

Removed the inner verticalScroll modifier and rememberScrollState import from CapturePageReview's root Column; the outer LazyColumn in MainScaffold already handles all vertical scrolling for the Capture route

## Consequences

- Review screen renders without crash after OCR capture
- Only one vertical scroll container per screen — no nesting
- All review-screen content (page image, OCR text, quote field, action buttons) renders in a single LazyColumn item

## Open Tail

*(none)*

## Evidence

- transcript lines 3150-3248

