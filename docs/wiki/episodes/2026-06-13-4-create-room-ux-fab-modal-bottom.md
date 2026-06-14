---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - android-rooms-tab
  - create-room
supersedes:
  - 2026-06-13-2-create-room-ux-inline-form-replaced
related_claims: []
source_lines:
  - 452-466
captured_at: 2026-06-13T12:58:25Z
---

# Episode: Create-room UX — FAB + modal bottom sheet replaces inline form

## Prior State

CreateRoomPanel was rendered inline as the first item in the Rooms list LazyColumn, matching neither iOS behavior nor intended IA

## Trigger

Parity audit identified the inline form as mismatched with iOS CreateRoomSheet (a modal triggered from a button)

## Decision

Removed inline CreateRoomPanel from the Rooms LazyColumn; added a FloatingActionButton ('New room') that opens a ModalBottomSheet containing CreateRoomPanel; on successful creation the sheet auto-dispatches OpenRoomInvite + ClearCreateRoomResult + onDismiss, mirroring iOS CreateRoomSheet routing

## Consequences

- Rooms tab opens directly on the explorer (no form dumped on the list)
- Create-room is a modal interaction matching iOS
- Success auto-routes to invite screen and dismisses sheet
- Added testTags: create_room_fab, room_explorer_list

## Open Tail

*(none)*

## Evidence

- transcript lines 452-466

