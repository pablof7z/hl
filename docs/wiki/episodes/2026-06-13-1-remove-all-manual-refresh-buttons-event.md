---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: active
subjects:
  - refresh-buttons
  - home-feed
  - rooms-explorer
  - comments-panel
  - article-reader
  - bookmarks
  - profile-panel
  - feedback-panel
  - media-settings
  - room-invite
supersedes:
  - 2026-06-13-1-remove-all-manual-refresh-buttons-from
related_claims: []
source_lines:
  - 2533-2544
  - 2550-2628
captured_at: 2026-06-13T18:06:05Z
---

# Episode: Remove all manual Refresh buttons — event-driven Nostr client anti-pattern

## Prior State

Manual Refresh OutlinedButtons existed on 9 screens (HomeFeed, RoomExplorer, Comments, ArticleReader, MediaSettings, BookmarkLibrary, Feedback, Profile, RoomInvite), dispatching RefreshX actions.

## Trigger

User correction: manual Refresh buttons are an anti-pattern in an event-driven Nostr client — the core already pushes live updates via listenForUpdates subscriptions.

## Decision

Remove all manual Refresh OutlinedButton UI from all 9 screens. Keep on-appear Open* dispatches (already present on every screen) and pull-to-refresh gestures (swipe-down in HighlightsTab and RoomsTab).

## Consequences

- 9 screens modified with OutlinedButton + surrounding Row imports removed
- Standing preference saved to memory (hl-no-refresh-buttons.md) to prevent re-introduction
- Pull-to-refresh gestures intentionally left (MainScaffold lines 352-373 and 389-417) but flagged for potential removal under same principle
- Every affected screen already had DisposableEffect-based Open* dispatches, so live content loading is unchanged

## Open Tail

- Whether to also remove the two remaining pull-to-refresh gestures under the same event-driven principle

## Evidence

- transcript lines 2533-2544
- transcript lines 2550-2628

