---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: active
subjects:
  - highlight-detail-screen
  - feed-navigation
  - comments-entry
supersedes:
  - 2026-06-13-2-highlight-detail-screen-feed-to-comments
related_claims: []
source_lines:
  - 2110-2190
captured_at: 2026-06-13T17:33:54Z
---

# Episode: Highlight Detail Screen adds feed-to-comments navigation path

## Prior State

Feed highlight cards dispatched OpenArticleReader directly; no highlight-detail screen existed. OpenComments was only dispatched from RoomDetailPanel, making comments unreachable from the home feed. iOS had HighlightDetailView (quote center-stage, byline→profile, comment/share/bookmark).

## Trigger

Validation discovered that the just-built threaded CommentsPanel was unreachable from the feed — OpenComments only existed in RoomDetailPanel, and highlight cards bypassed any detail view straight to the article reader.

## Decision

Built HighlightDetailScreen with host-side local navigation (var selectedHighlight state in HighlightsTab inside MainScaffold, same BackHandler/DestinationScaffold pattern as Settings/Bookmarks). Feed cards now open the detail screen first; article reader reachable from within via source header tap. Comment/share/bookmark actions dispatch through the core overlay stack.

## Consequences

- Feed card routing changed from OpenArticleReader to opening HighlightDetailScreen
- Comment action dispatches OpenComments(rootTagName="e", rootTagValue=highlight.eventId, rootKind=9802u) making threaded replies accessible from the feed
- Author byline taps dispatch OpenProfile; source header taps dispatch OpenArticleReader; share uses Android system share sheet
- ShareHighlightRepost (room picker) omitted — no room-picker sheet exists on Android
- Web reader link omitted — no OpenWebReader action in the binding

## Open Tail

- ShareHighlightRepost needs a room-picker sheet for full iOS parity
- Web reader destination needs a binding action to be added

## Evidence

- transcript lines 2110-2190

