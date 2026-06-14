---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: active
subjects:
  - android-room-detail
  - room-tab-structure
supersedes:
  - 2026-06-13-3-room-detail-bare-forms-replaced-with
related_claims: []
source_lines:
  - 1382-1434
captured_at: 2026-06-13T16:02:40Z
---

# Episode: Room detail parity: pill tabs, modal composer, gated chat tab

## Prior State

Room detail screen used bare inline forms with no tab structure, no room name in the header, and no modal for creating discussions. Chat tab was always visible regardless of activity.

## Trigger

iOS parity audit identified room detail as a major structural gap vs iOS RoomHomeView.

## Decision

Room detail now has: room name in TopAppBar header, pill-segment tabs (Home/Library/Discussions/Chat with Chat only shown when chatMessageCount > 0, matching iOS hasChatActivity guard), modal BottomSheet for discussion composer opened by + IconButton and FAB (auto-dismisses on lastPublishedDiscussionId), CloseRoom on back. Profile hydration deduped via LaunchedEffect keyed on pubkey with local-cache guard. Test tags added for all interactive elements.

## Consequences

- Room detail matches iOS structural parity (tabs, composer, gating)
- Discussion inline form fields completely removed in favor of modal
- Chat tab correctly hidden when no chat activity exists

## Open Tail

*(none)*

## Evidence

- transcript lines 1382-1434

