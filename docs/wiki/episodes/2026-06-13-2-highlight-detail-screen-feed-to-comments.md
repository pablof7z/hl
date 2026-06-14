---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - highlight-detail-screen
  - feed-navigation
  - comments-entry
supersedes:
  - 2026-06-13-2-highlight-detail-screen-added-feed-cards
related_claims: []
source_lines:
  - 2110-2201
captured_at: 2026-06-13T16:48:47Z
---

# Episode: Highlight detail screen: feed-to-comments navigation entry was missing

## Prior State

Feed highlight cards opened the article reader directly via OpenArticleReader; CommentsPanel with threaded replies existed but was only reachable from room detail screens. No highlight-detail screen existed in the Android app (iOS has HighlightDetailView with quote, author→profile, comment/share/bookmark).

## Trigger

Validation found no way to access comments from feed highlights; OpenComments was only dispatched from RoomDetailPanel. The newly-built threaded-comments UI was correct but unreachable from the primary user flow.

## Decision

Added HighlightDetailScreen as host-side local navigation (selectedHighlight state + BackHandler + DestinationScaffold, same pattern as Settings/Bookmarks/Capture). Feed card tap now opens the detail screen instead of the article reader. Detail screen provides: quote display, tappable author→OpenProfile, comment action→OpenComments, system share sheet, bookmark toggle. Article reader remains accessible from the detail screen's source header.

## Consequences

- Threaded-comments UI is now reachable from the feed via highlight detail → Comment button
- Feed card routing changed: HighlightFeedCard.clickable → onOpenDetail (was OpenArticleReader); ReadingFeedCard unchanged (still → OpenArticleReader)
- ShareHighlightRepost omitted — Android has no room-picker sheet equivalent to iOS ShareToCommunitySheet; system share sheet used instead
- Web reader link omitted — no OpenWebReader action exists in the UniFFI binding

## Open Tail

- Search person→profile navigation still unproven on-device (emulator relay state flaky), though code is verified correct
- OCR capture (camera + ML Kit) remains unbuilt — emulator has no camera; needs physical device for true camera validation

## Evidence

- transcript lines 2110-2201

