---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: active
subjects:
  - android-rooms-tab
  - room-navigation
supersedes: []
related_claims: []
source_lines:
  - 443-449
captured_at: 2026-06-13T12:58:25Z
---

# Episode: RoomsTab — stray CloseRoom on tab switch removed

## Prior State

Switching away from the Rooms tab dispatched CloseRoom in DisposableEffect.onDispose, causing any open room to close unexpectedly on tab switch or recomposition

## Trigger

Identified as flow #12 in the Android parity plan ('opening rooms does nothing')

## Decision

Removed the CloseRoom dispatch from RoomsTab's onDispose; CloseRoom ownership remains with RootScene.Overlays and RoomDetailPanel's close button (both unchanged)

## Consequences

- Open rooms persist across tab switches and recompositions
- Emulator validation confirmed: open room → switch to Highlights → return → room explorer displays correctly
- Back button still correctly dismisses room detail

## Open Tail

*(none)*

## Evidence

- transcript lines 443-449

