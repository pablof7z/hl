---
type: episode-card
date: 2026-06-14
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: superseded
subjects:
  - capture-review-layout
  - compose-scroll-architecture
supersedes:
  - 2026-06-13-2-nested-verticalscroll-crash-in-capture-review
related_claims: []
source_lines:
  - 3150-3250
captured_at: 2026-06-14T08:09:37Z
---

# Episode: Remove nested verticalScroll in CapturePageReview

## Prior State

CapturePageReview's root Column had Modifier.verticalScroll(rememberScrollState()), but it rendered as a LazyColumn item in MainScaffold — a scrollable inside an unbounded-height container, causing IllegalStateException crash at runtime

## Trigger

After camera binding was fixed, shutter→OCR succeeded and ML Kit OCR ran, but the app crashed with 'Vertically scrollable component was measured with an infinity maximum height constraints' on the review screen

## Decision

Removed the inner Modifier.verticalScroll(rememberScrollState()) from CapturePageReview's root Column; the enclosing LazyColumn in MainScaffold already provides vertical scrolling for the CAPTURE route

## Consequences

- Review screen renders correctly: page image thumbnail, OCR text, quote field, Use/Retake buttons all visible
- Single authoritative scroll container per screen eliminates the infinite-height nesting class of bugs
- All testTags and OCR/quote/publish logic preserved unchanged

## Open Tail

*(none)*

## Evidence

- transcript lines 3150-3250

