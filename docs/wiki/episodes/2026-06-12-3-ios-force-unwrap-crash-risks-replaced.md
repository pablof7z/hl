---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: active
subjects:
  - ios-crash-risks
  - swift-safety
  - force-unwrap
supersedes: []
related_claims: []
source_lines:
  - 1790-1828
captured_at: 2026-06-12T08:57:33Z
---

# Episode: iOS force-unwrap crash risks replaced with safe patterns

## Prior State

Five locations in iOS used force unwraps (`as!`, `!`) that could trap at runtime: BookScannerView layer cast, MarkdownRenderer mutableCopy and URL(string:), OCRStructureReconstructor max-bucket, ShareToCommunitySheet title and image URL

## Trigger

Systematic gap-finding goal to address all technical debt and crash risks across iOS/Android

## Decision

Replace all five with safe patterns: `guard let ... as?` with fallback, `as?` with NSMutableAttributedString fallback, optional URL creation with `.flatMap(URL.init(string:))`

## Consequences

- Eliminates 5 runtime crash paths in production
- Happy paths unchanged (layerClass still guarantees type; URL creation skips malformed links gracefully)
- CommentRow.swift stub replaced with 'View profile' navigation wired through existing NavigationStack pattern

## Open Tail

*(none)*

## Evidence

- transcript lines 1790-1828
