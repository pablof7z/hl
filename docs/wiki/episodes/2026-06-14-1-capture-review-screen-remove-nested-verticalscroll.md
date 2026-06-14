---
type: episode-card
date: 2026-06-14
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: active
subjects:
  - capture-page-review
  - compose-layout
  - main-scaffold
supersedes:
  - 2026-06-14-2-fix-review-screen-scroll-nesting-crash
related_claims: []
source_lines:
  - 3177-3222
captured_at: 2026-06-14T09:19:44Z
---

# Episode: Capture review screen: remove nested verticalScroll that crashed with infinite-height constraints

## Prior State

CapturePageReview's root Column had Modifier.verticalScroll(rememberScrollState()), making it a scrollable child of MainScaffold's LazyColumn item — which provides unbounded height to its children. This caused an IllegalStateException crash whenever the OCR review screen rendered.

## Trigger

OCR capture pipeline (camera bind + shutter + ML Kit OCR) was confirmed working on a fresh emulator, but the review screen crashed immediately with 'Vertically scrollable component measured with infinity maximum height constraints'

## Decision

Removed Modifier.verticalScroll(rememberScrollState()) from CapturePageReview's root Column. The outer LazyColumn in MainScaffold's ScaffoldRoute.CAPTURE branch already handles all vertical scrolling for the capture route — only one scroll container is needed.

## Consequences

- OCR capture pipeline now works end-to-end: camera → shutter → OCR → review screen renders → publish
- CapturePageReview Column only needs padding(16.dp); no nested scroll containers in the capture route
- All testTags and OCR/quote/publish logic untouched

## Open Tail

- Real-book OCR quality requires a physical device (pipeline proven on emulator)

## Evidence

- transcript lines 3177-3222

