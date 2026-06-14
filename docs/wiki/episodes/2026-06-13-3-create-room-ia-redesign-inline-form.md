---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - create-room-ux
  - rooms-tab
  - main-scaffold
supersedes:
  - 2026-06-13-3-create-room-moved-from-inline-form
related_claims: []
source_lines:
  - 452-466
captured_at: 2026-06-13T12:25:09Z
---

# Episode: Create-room IA redesign — inline form replaced by FAB + modal sheet

## Prior State

CreateRoomPanel was rendered as the permanent first item of the Rooms LazyColumn, always visible at the top of the rooms list

## Trigger

Audit identified the misplaced 'Create Room form dumped at top' — iOS uses a toolbar + button → modal sheet pattern, not an inline form

## Decision

Removed inline CreateRoomPanel from RoomsTab LazyColumn. Added a FAB on the ROOMS tab that opens a ModalBottomSheet containing CreateRoomPanel. On successful creation (createRoom.createdGroupId non-null), auto-dispatch OpenRoomInvite + ClearCreateRoomResult + dismiss, mirroring iOS CreateRoomSheet routing

## Consequences

- Rooms tab opens directly on the room explorer list, not the create form
- Create-room is behind a discoverable affordance matching iOS convention
- Test tags added (create_room_fab, room_explorer_list) for validation automation

## Open Tail

*(none)*

## Evidence

- transcript lines 452-466

