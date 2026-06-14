---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - refresh-button-removal
  - nostr-event-driven-ui
supersedes:
  - 2026-06-13-3-manual-refresh-buttons-removed-nostr-live
related_claims: []
source_lines:
  - 2533-2629
captured_at: 2026-06-13T17:52:11Z
---

# Episode: Remove all manual Refresh buttons from event-driven Nostr client

## Prior State

9 screens had manual Refresh OutlinedButtons (HomeFeed, RoomExplorer, Comments, ArticleReader, MediaSettings, Bookmarks, Feedback threads/thread-detail, Profile, RoomInvite) that dispatched RefreshX actions

## Trigger

Recognition that in an event-driven Nostr client with listenForUpdates subscriptions, manual refresh is an anti-pattern — the core already pushes live updates automatically

## Decision

Remove all Refresh OutlinedButtons from all 9 screens; preserve pull-to-refresh swipe gestures (MainScaffold) and on-appear Open* dispatches which already load data on screen entry

## Consequences

- Screens still auto-load via DisposableEffect(Unit) { dispatch(OpenX) } on appear
- Pull-to-refresh swipe gestures intentionally kept on Highlights and Rooms tabs
- Standing preference saved to memory (hl-no-refresh-buttons.md) to prevent re-introduction
- All 9 panels compile clean with unused imports removed

## Open Tail

- Pull-to-refresh gestures still exist on HighlightsTab and RoomsTab — may also want removal under same anti-pattern principle

## Evidence

- transcript lines 2533-2629

