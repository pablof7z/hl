---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - create-room-ux
  - rooms-navigation
supersedes:
  - 2026-06-13-3-create-room-ia-redesign-inline-form
related_claims: []
source_lines:
  - 186-208
  - 450-466
  - 493-498
captured_at: 2026-06-13T12:42:52Z
---

# Episode: Create-room UX — inline form replaced by FAB + modal bottom sheet

## Prior State

CreateRoomPanel was rendered as the permanent first item of the Rooms LazyColumn, dumping a creation form at the top of the list — the user's explicit 'Create Room form dumped at top' complaint.

## Trigger

Gap audit identified this as the #2 critical fix; iOS uses a '+' toolbar button → modal sheet (line 199).

## Decision

Removed the inline CreateRoomPanel from RoomsTab's LazyColumn. Added a FloatingActionButton on the Rooms tab that opens a new CreateRoomSheet (ModalBottomSheet). On successful creation (createRoom.createdGroupId non-null), the sheet auto-dispatches OpenRoomInvite + ClearCreateRoomResult and dismisses — mirroring iOS CreateRoomSheet's routing to RoomInviteView.

## Consequences

- Rooms tab opens directly on the explorer list with no creation form visible.
- Room creation is now an intentional action behind a FAB, matching iOS convention.
- Test tag create_room_fab added for automation.

## Open Tail

*(none)*

## Evidence

- transcript lines 186-208
- transcript lines 450-466
- transcript lines 493-498

