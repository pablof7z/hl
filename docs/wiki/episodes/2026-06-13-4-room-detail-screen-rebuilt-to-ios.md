---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - room-detail-panel
  - room-detail-tabs
  - discussion-composer
supersedes: []
related_claims: []
source_lines:
  - 1347-1433
captured_at: 2026-06-13T13:15:31Z
---

# Episode: Room detail screen rebuilt to iOS parity

## Prior State

Room detail was a bare Panel with inline discuss+chat composers and a hardcoded 'Room' header, unlike iOS which shows the room name, pill-tab navigation (Home/Library/Discussions/Chat), and a modal discussion composer.

## Trigger

iOS parity audit showed the room detail screen needed full restructuring: room name in header, pill tabs, discussion composer behind an affordance (not inline), chat tab conditioned on activity.

## Decision

Full rewrite of RoomDetailPanel (~560 lines) as a Scaffold-based screen: room name resolved from state.chrome.joinedCommunities in TopAppBar, pill-tab navigation (Home/Library/Discussions/Chat with Chat conditioned on hasChatActivity), discussion composer as a ModalBottomSheet triggered by + IconButton and FAB (auto-dismisses on lastPublishedDiscussionId), profile hydration deduped per tab via hydratedPubkeys set check.

## Consequences

- Room detail matches iOS structure with named header, organized tab content, and hidden composer
- Open-room-stays-open behavior validated: room opens, stays open across interaction, back returns to explorer
- Discussion composer no longer dumped inline; modal dismisses on successful publish
- All four room-detail tabs have content: Home (highlights), Library (artifacts), Discussions (with modal composer), Chat (conditional)

## Open Tail

- Room detail back-navigation must only dispatch CloseRoom from back arrow/system back (verified in code)

## Evidence

- transcript lines 1347-1433

