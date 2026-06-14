---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - room-detail-panel
  - root-scene
  - android-ui-parity
supersedes:
  - 2026-06-13-4-room-detail-screen-rebuilt-to-ios
related_claims: []
source_lines:
  - 1347-1434
  - 1766-1770
captured_at: 2026-06-13T15:54:26Z
---

# Episode: Room detail: bare forms replaced with structured parity screen

## Prior State

The Android room screen was a bare panel with inline discuss and chat composers, a hardcoded "Room" header, and no tab structure — far behind the iOS room detail.

## Trigger

iOS parity audit identified room detail as a major gap: iOS has room name in header, pill tabs (Home/Library/Discussions/Chat), and a discussion composer behind a modal/FAB.

## Decision

Full rewrite of RoomDetailPanel (~560 lines) to a Scaffold-based screen with: room name resolved from chrome.joinedCommunities (falling back to "Room"), pill-tab navigation (Home/Library/Discussions/Chat with content per tab), discussion composer as a ModalBottomSheet triggered by + IconButton or FAB, chat tab shown only when chatMessageCount > 0, and profile hydration deduped per tab.

## Consequences

- Room detail now matches iOS structure with named header and tabbed content
- Discussion composer moved from inline fields to modal (no more inline forms)
- Chat tab conditionally shown (matching iOS hasChatActivity guard)
- Room name pulled from joined community metadata, not hardcoded
- Validated on clean emulator: name + pill tabs render, empty states work

## Open Tail

*(none)*

## Evidence

- transcript lines 1347-1434
- transcript lines 1766-1770

