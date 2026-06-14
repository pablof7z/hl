---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: active
subjects:
  - rooms-create
  - rooms-detail
  - room-ia
supersedes:
  - 2026-06-13-4-create-room-ux-fab-modal-bottom
related_claims: []
source_lines:
  - 800-845
  - 1347-1433
captured_at: 2026-06-13T13:20:33Z
---

# Episode: Rooms IA rebuilt to match iOS: modal creation + pill-tab detail

## Prior State

Room creation was an inline form dumped on the explorer list. Room detail was a bare panel with a hardcoded "Room" header and inline discuss/chat composers.

## Trigger

iOS parity audit showed create-room uses a bottom-sheet modal behind a FAB, and room detail has pill tabs (Home/Library/Discussions/Chat) with a modal discussion composer.

## Decision

Create-room changed to a ModalBottomSheet behind a "+" FAB and TopAppBar icon button (dismisses on successful publish via LaunchedEffect on lastPublishedDiscussionId). Room detail rebuilt as a full-screen Scaffold with resolved room name in TopAppBar, pill-tab navigation (Home/Library/Discussions/Chat — chat only shown when hasChatActivity), and discussion composer behind a ModalBottomSheet.

## Consequences

- Room creation no longer replaces the explorer list — clean modal UX matching iOS
- Room detail shows real room name from state.chrome.joinedCommunities instead of hardcoded "Room"
- Discussion composer auto-dismisses on publish success
- Profile hydration deduped per-tab using remembered hydratedPubkeys set
- Chat tab conditionally shown matching iOS hasChatActivity guard

## Open Tail

- Room navigation tab-switch regression needs verification (emulator input flakiness interfered with validation)
- Unnamed rooms still display abbreviated hex — depends on room metadata hydration

## Evidence

- transcript lines 800-845
- transcript lines 1347-1433

