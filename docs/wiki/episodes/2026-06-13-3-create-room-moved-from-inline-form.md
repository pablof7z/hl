---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - create-room
  - rooms-ux
supersedes: []
related_claims: []
source_lines:
  - 186-208
  - 452-465
captured_at: 2026-06-13T12:09:15Z
---

# Episode: Create-room moved from inline form to FAB + modal sheet

## Prior State

CreateRoomPanel rendered as permanent first item of Rooms LazyColumn — form dumped at top of the rooms list, mirroring no iOS convention

## Trigger

Opus audit identified this as misplaced IA; iOS uses a + toolbar button → modal sheet

## Decision

Removed inline CreateRoomPanel from LazyColumn; added FAB → ModalBottomSheet (CreateRoomSheet) pattern with auto-routing to RoomInviteView on success (mirrors iOS CreateRoomSheet behavior)

## Consequences

- Rooms tab opens directly on the room explorer, not a creation form
- Room creation is an intentional action via FAB, not visual noise
- Successful creation routes to invite screen, matching iOS flow

## Open Tail

*(none)*

## Evidence

- transcript lines 186-208
- transcript lines 452-465

