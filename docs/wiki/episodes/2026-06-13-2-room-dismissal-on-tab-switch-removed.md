---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - rooms-tab
  - room-detail
supersedes:
  - 2026-06-13-2-three-surgical-basics-fixes-room-persistence
related_claims: []
source_lines:
  - 186-208
  - 442-448
captured_at: 2026-06-13T12:09:15Z
---

# Episode: Room dismissal on tab switch removed

## Prior State

Opening a room and switching tabs or recomposing caused the room to close — rooms appeared non-functional

## Trigger

Opus audit found RoomsTab.onDispose (MainScaffold.kt:320) dispatches CloseRoom, collapsing roomDetail on any tab switch or recomposition

## Decision

Removed CloseRoom dispatch from RoomsTab.onDispose; ownership of room closure stays with RootScene.Overlays and RoomDetailPanel's Close button

## Consequences

- Rooms stay open across tab switches and recompositions
- Room detail no longer disappears on navigation

## Open Tail

*(none)*

## Evidence

- transcript lines 186-208
- transcript lines 442-448

