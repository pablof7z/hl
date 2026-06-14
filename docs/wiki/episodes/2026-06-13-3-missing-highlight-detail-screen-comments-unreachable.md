---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - highlight-detail-screen
  - comments-entry-point
supersedes:
  - 2026-06-13-4-highlight-detail-screen-comments-reachability-from
related_claims: []
source_lines:
  - 2113-2189
captured_at: 2026-06-13T16:12:24Z
---

# Episode: Missing highlight detail screen — comments unreachable from feed

## Prior State

Feed highlight cards opened OpenArticleReader directly. OpenComments was only dispatched from inside rooms (RoomDetailPanel). No highlight detail screen existed. iOS has HighlightDetailView with quote center-stage, byline→profile, and comment/share/bookmark actions. The newly-built threaded CommentsPanel was correct but unreachable from the feed.

## Trigger

Validation showed no comment UI accessible from any highlight in the feed; code audit confirmed OpenComments only dispatched from RoomDetailPanel, while HomeFeedPanel dispatched OpenArticleReader on card tap.

## Decision

Created HighlightDetailScreen (host-side local navigation via selectedHighlight state + BackHandler in HighlightsTab, same pattern as Settings/Bookmarks/Capture). Feed card tap now opens detail screen instead of article reader. Detail screen provides: full quote display, tappable author→OpenProfile, Comment action→OpenComments (opening existing CommentsPanel via RootScene overlay stack), source header→OpenArticleReader, system share sheet, ToggleArticleBookmark for 30023 articles.

## Consequences

- Comments threading now reachable from feed via highlight detail
- Article reader still accessible via source header within detail (not lost)
- ShareHighlightRepost omitted (no room-picker sheet in Android yet)
- Web reader link omitted (no OpenWebReader action in binding)
- Matches iOS navigation pattern for highlight detail

## Open Tail

- Room picker sheet (ShareToCommunitySheet parity) needed for full share flow
- Web reader navigation destination not yet available in Android binding

## Evidence

- transcript lines 2113-2189

