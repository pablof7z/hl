---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - refresh-button-removal
  - nostr-live-update-doctrine
supersedes:
  - 2026-06-13-3-manual-refresh-buttons-removed-as-anti
related_claims: []
source_lines:
  - 2533-2544
  - 2550-2630
captured_at: 2026-06-13T16:54:53Z
---

# Episode: Manual Refresh buttons removed — Nostr live-update doctrine

## Prior State

OutlinedButton 'Refresh' controls existed on 9 Android screens (feed, rooms explorer, comments, reader, profile, bookmarks, settings, feedback, room invite), allowing users to manually reload content.

## Trigger

User directive: manual Refresh buttons are an anti-pattern in an event-driven Nostr client where listenForUpdates already pushes live updates via subscriptions. The core handles real-time updates; UI should not suggest manual refresh is needed.

## Decision

Removed all Refresh OutlinedButtons from 9 screens. Kept pull-to-refresh gestures (swipe-down) on feed and rooms tabs. On-appear Open* dispatches remain intact so content loads automatically. Saved standing preference to memory (hl-no-refresh-buttons.md) to prevent re-introduction.

## Consequences

- Users no longer see manual refresh affordances on any screen
- Pull-to-refresh gestures preserved on feed and rooms as acceptable discoverable fallbacks
- Standing memory doc prevents refresh buttons from creeping back in future sessions
- All on-appear auto-load confirmed intact — no functionality lost

## Open Tail

- Pull-to-refresh gestures on feed and rooms may also warrant removal under the same doctrine

## Evidence

- transcript lines 2533-2544
- transcript lines 2550-2630

