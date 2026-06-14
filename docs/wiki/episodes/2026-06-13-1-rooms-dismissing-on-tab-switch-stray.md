---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: active
subjects:
  - rooms-tab-lifecycle
  - room-persistence
supersedes:
  - 2026-06-13-2-roomstab-ondispose-stray-closeroom-dispatch-root
related_claims: []
source_lines:
  - 196-200
  - 440-448
  - 503-506
captured_at: 2026-06-13T12:42:52Z
---

# Episode: Rooms dismissing on tab switch — stray CloseRoom in onDispose

## Prior State

RoomsTab's DisposableEffect.onDispose dispatched CloseRoom, collapsing roomDetail whenever the user switched tabs or the Compose recomposed — making it appear that 'opening a room does nothing.'

## Trigger

Opus gap audit identified this as the #1 critical fix: MainScaffold.kt:320 wrongly dispatching CloseRoom on disposal (line 198).

## Decision

Removed the CloseRoom dispatch from RoomsTab.onDispose entirely (onDispose body is now a no-op). CloseRoom remains owned by RoomDetailPanel's close button and RootScene.Overlays — the correct exit points.

## Consequences

- Rooms stay open across tab switches and recompositions — the original 'rooms don't open' complaint resolved.
- No other CloseRoom call site was changed; room-close affordances still work as designed.
- Test tag room_explorer_list added to RoomsTab LazyColumn for validation.

## Open Tail

- Flow #12 (open a room, see its content, tab away and back) still needs a clean tap-an-existing-tile validation — the Haiku validator tested room creation, not room opening.

## Evidence

- transcript lines 196-200
- transcript lines 440-448
- transcript lines 503-506

