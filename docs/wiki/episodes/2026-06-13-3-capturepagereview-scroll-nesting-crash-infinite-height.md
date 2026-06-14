---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: superseded
subjects:
  - capture-page-review
  - compose-layout
  - ocr-capture
supersedes: []
related_claims: []
source_lines:
  - 3150-3222
captured_at: 2026-06-13T18:06:05Z
---

# Episode: CapturePageReview scroll-nesting crash — infinite height constraint

## Prior State

CapturePageReview's root Column had Modifier.verticalScroll(rememberScrollState()), and it was rendered as a single item inside MainScaffold's LazyColumn (ScaffoldRoute.CAPTURE branch), which provides unbounded height to its items.

## Trigger

Runtime crash on transitioning from camera capture to review screen: IllegalStateException: Vertically scrollable component was measured with an infinity maximum height constraints.

## Decision

Removed the inner verticalScroll modifier from CapturePageReview's root Column (and the unused rememberScrollState/verticalScroll imports). The enclosing LazyColumn in MainScaffold already handles all vertical scrolling for the Capture route.

## Consequences

- Review screen (page image + OCR text + quote field + Use/Retake buttons) now renders without crash
- Establishes pattern: screens rendered as LazyColumn items must not have their own verticalScroll — the LazyColumn is the sole scroll container
- All testTags and OCR/quote/publish logic unchanged

## Open Tail

*(none)*

## Evidence

- transcript lines 3150-3222

