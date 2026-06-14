---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: superseded
subjects:
  - rooms-tab-lifecycle
  - main-scaffold
supersedes:
  - 2026-06-13-2-room-dismissal-on-tab-switch-removed
related_claims: []
source_lines:
  - 443-449
captured_at: 2026-06-13T12:25:09Z
---

# Episode: RoomsTab onDispose stray CloseRoom dispatch — root cause of 'opening rooms does nothing'

## Prior State

Opening a room appeared to do nothing — room detail collapsed immediately on any tab switch or recomposition

## Trigger

Audit found `MainScaffold.kt:320` — `RoomsTab` DisposableEffect.onDispose dispatched `CloseRoom`, collapsing `roomDetail` state on every tab switch or recomposition

## Decision

Removed the `CloseRoom` dispatch from `RoomsTab.onDispose` (body is now a no-op comment). `CloseRoom` ownership stays with `RootScene.Overlays` and `RoomDetailPanel`'s Close button — both unchanged

## Consequences

- Rooms stay open across tab switches and recompositions, matching iOS behavior
- Single-line fix with highest user-visible impact per the audit

## Open Tail

*(none)*

## Evidence

- transcript lines 443-449

