---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - refresh-button-removal
  - feed-ux
  - rooms-ux
supersedes: []
related_claims: []
source_lines:
  - 2533-2548
captured_at: 2026-06-13T16:48:47Z
---

# Episode: Manual Refresh buttons removed as anti-pattern in event-driven Nostr client

## Prior State

Manual Refresh buttons existed in the feed header, rooms explorer, room detail, and comments screens, allowing users to manually reload content.

## Trigger

User correction: manual Refresh buttons are an anti-pattern in an event-driven Nostr client because the core already pushes live updates via listenForUpdates subscription — content updates automatically without user action.

## Decision

Remove all Refresh buttons across feed, rooms explorer, room detail, and comments screens. Retain on-appear Open* triggers so content still loads live via the subscription. Document as standing project preference (hl-no-refresh-buttons.md) to prevent re-introduction.

## Consequences

- All screens rely purely on subscription-driven live updates; no user-initiated refresh affordance anywhere
- On-appear triggers preserved so content loads when a screen is first shown
- Preference saved to project memory to prevent Refresh buttons from creeping back in

## Open Tail

*(none)*

## Evidence

- transcript lines 2533-2548

