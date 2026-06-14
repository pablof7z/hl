---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - rooms-tab-lifecycle
  - create-room-ux
  - home-feed-rendering
supersedes: []
related_claims: []
source_lines:
  - 433-490
captured_at: 2026-06-13T12:01:56Z
---

# Episode: Three surgical basics fixes: room persistence, create-room IA, feed truncation

## Prior State

Rooms dismissed on any tab switch/recomposition (CloseRoom in RoomsTab onDispose); CreateRoomPanel rendered as permanent first item of rooms LazyColumn; HomeFeedPanel silently capped at 8 items with no loading-vs-empty distinction

## Trigger

Gap audit identified these as the top 3 critical defects producing the 'rooms don't open', 'create room form dumped at top', and 'feed looks broken' user experiences

## Decision

Fix 1: Remove CloseRoom dispatch from RoomsTab.onDispose (ownership stays with RootScene overlays and RoomDetailPanel close button). Fix 2: Delete inline CreateRoomPanel from LazyColumn; add FAB → ModalBottomSheet with auto-routing to RoomInvite on success (mirrors iOS CreateRoomSheet). Fix 3: Remove take(8) cap; replace single EmptyPanel with distinct loading indicator ('Syncing highlights…') vs empty state ('No highlights yet')

## Consequences

- Rooms persist across tab switches and recompositions
- Create-room follows iOS modal-sheet convention with success routing
- Full uncapped feed renders with proper loading/empty UX states
- Test tags added (create_room_fab, room_explorer_list, feed_loading, feed_item_list) for validation harness

## Open Tail

- Validation with real authenticated data still pending (seeded account created but login verification not yet completed)

## Evidence

- transcript lines 433-490

