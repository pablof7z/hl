---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - highlight-detail-screen
  - comments-reachability
  - feed-card-navigation
supersedes: []
related_claims: []
source_lines:
  - 2110-2117
  - 2136-2189
captured_at: 2026-06-13T16:02:40Z
---

# Episode: Highlight detail screen + comments reachability from feed

## Prior State

Tapping a highlight card in the feed opened the article reader directly (OpenArticleReader). No highlight detail screen existed. The threaded CommentsPanel was built but unreachable from the feed — OpenComments was only dispatched from inside rooms.

## Trigger

Validation found no comment UI accessible from the feed. Code grep confirmed OpenComments only dispatched from RoomDetailPanel. iOS has HighlightDetailView (quote center-stage, author→profile, comment/share/bookmark) which Android lacked entirely.

## Decision

Add HighlightDetailScreen as a host-side local navigation state (selectedHighlight in HighlightsTab, same pattern as Settings/Bookmarks). Feed card tap now opens the detail screen (not OpenArticleReader). Detail screen includes: highlight quote, tappable author→OpenProfile, source header→OpenArticleReader for 30023 articles, Comment button→OpenComments opening threaded CommentsPanel, share via Android Intent.ACTION_SEND, bookmark via ToggleArticleBookmark. CommentsPanel already supports threaded replies via buildCommentTree from core's childLinks/topLevelEventIds.

## Consequences

- Threaded comments are now reachable from the home feed via highlight detail
- Feed card navigation changed: tap → detail screen, not article reader
- Article reader still accessible from within the detail screen's source header
- Web reader tap intentionally omitted (no OpenWebReader action in binding)
- Share-to-community (ShareHighlightRepost) omitted (no room picker sheet on Android)

## Open Tail

- Web reader from highlight detail source header requires a future OpenWebReader action in the binding

## Evidence

- transcript lines 2110-2117
- transcript lines 2136-2189

