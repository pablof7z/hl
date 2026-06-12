---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: active
subjects:
  - ios-crash-safety
  - force-unwrap-removal
supersedes: []
related_claims: []
source_lines:
  - 1783-1828
captured_at: 2026-06-12T09:08:45Z
---

# Episode: iOS force-unwrap crash risks eliminated across five views

## Prior State

Five iOS views contained force-unwraps (`as!`, `URL(string:)!`, `.max(by:)!`) that could crash in production: BookScannerView's AVCaptureVideoPreviewLayer cast, MarkdownRenderer's NSMutableAttributedString cast and three URL(string!) calls, OCRStructureReconstructor's max-by comparison, ShareToCommunitySheet's title/imageURL unwraps, and CommentRow's mute-action stub.

## Trigger

Systematic audit during the 'professionalize Android' session uncovered crash-risk patterns while comparing iOS and Android implementations. The force-unwraps were identified as production crash vectors.

## Decision

Replaced all five force-unwraps with safe alternatives: guard-let with fallbacks, Optional.flatMap(URL.init(string:)), conditional .link attachment, and empty-collection guards. CommentRow's mute stub replaced with a wired 'View profile' navigation action matching ThreadView's pattern.

## Consequences

- Malformed URLs (footnote links, profile links) no longer crash the app
- AVCaptureVideoPreviewLayer failure degrades gracefully instead of trapping
- Empty OCR bucket lists return 0 instead of crashing on force-unwrap
- CommentRow now navigates to profiles via .navigationDestination(item:) pattern

## Open Tail

*(none)*

## Evidence

- transcript lines 1783-1828
