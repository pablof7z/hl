---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: superseded
subjects:
  - compose-layout
  - capture-review-screen
  - android-capture
supersedes:
  - 2026-06-13-3-capturepagereview-scroll-nesting-crash-infinite-height
related_claims: []
source_lines:
  - 3159-3169
  - 3200-3217
  - 3233-3248
captured_at: 2026-06-13T18:37:41Z
---

# Episode: Nested verticalScroll crash in capture review screen

## Prior State

CapturePageReview's root Column had Modifier.verticalScroll(rememberScrollState()), making it a scrollable component. MainScaffold's CAPTURE branch renders CapturePanel inside a LazyColumn item, which provides infinite height to children. The nested verticalScroll inside an infinite-height parent is the classic Compose crash pattern.

## Trigger

After the camera fallback fix allowed the shutter to fire and OCR to run, the app crashed with `IllegalStateException: Vertically scrollable component was measured with an infinity maximum height constraints` when transitioning to the review screen.

## Decision

Removed the inner Modifier.verticalScroll(rememberScrollState()) from CapturePageReview's root Column. The outer LazyColumn in MainScaffold already provides all vertical scrolling for the Capture route — the review content only needs its padding modifier.

## Consequences

- Review screen renders correctly after camera capture + OCR
- Single scroll container (LazyColumn) owns scrolling for the entire capture route
- All testTags and OCR/quote/publish logic untouched

## Open Tail

*(none)*

## Evidence

- transcript lines 3159-3169
- transcript lines 3200-3217
- transcript lines 3233-3248

