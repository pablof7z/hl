---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - highlight-detail-screen
  - comments-entry
  - feed-card-navigation
supersedes:
  - 2026-06-13-3-missing-highlight-detail-screen-comments-unreachable
related_claims: []
source_lines:
  - 2110-2118
  - 2136-2190
captured_at: 2026-06-13T16:24:41Z
---

# Episode: Highlight detail screen added — feed cards now open detail with comment/share/bookmark instead of article reader

## Prior State

Tapping a highlight card in the feed opened OpenArticleReader directly. There was no highlight detail screen — no way to view the quote center-stage, tap author→profile, or access comment/share/bookmark actions. The threaded CommentsPanel existed but was unreachable from the feed (only dispatched from RoomDetailPanel).

## Trigger

iOS parity audit found iOS has a HighlightDetailView with quote, author byline→profile, comment, share, and bookmark actions. Validation confirmed comments UI was built but had no entry point from the home feed.

## Decision

Created HighlightDetailScreen.kt — a host-side local navigation screen (selectedHighlight state in HighlightsTab, BackHandler, DestinationScaffold pattern matching Settings/Bookmarks). Feed card tap now opens this detail screen instead of OpenArticleReader. Detail screen shows the highlight quote, tappable author→OpenProfile, source header→OpenArticleReader for article-backed highlights, Comment action→OpenComments (making threaded replies reachable), Share (Android system share sheet with highlighter.com URL), and Bookmark (ToggleArticleBookmark for 30023 highlights).

## Consequences

- Comments are now reachable from the home feed via highlight detail → Comment action
- Article reader is still accessible but only via the source header inside the detail screen
- ShareHighlightRepost (room picker) omitted — Android has no room-picker sheet
- Web reader from source header omitted — no OpenWebReader action in the binding
- testTags added: highlight_detail, highlight_detail_author, highlight_detail_comment, highlight_detail_share, highlight_detail_bookmark

## Open Tail

- ShareHighlightRepost needs a room-picker sheet for full iOS parity
- Web URL tap in source header is recognized but non-functional (no OpenWebReader binding)

## Evidence

- transcript lines 2110-2118
- transcript lines 2136-2190

