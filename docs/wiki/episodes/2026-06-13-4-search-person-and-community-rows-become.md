---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: active
subjects:
  - search-navigation
  - search-panel
supersedes:
  - 2026-06-13-3-search-person-and-community-rows-made
related_claims: []
source_lines:
  - 2023-2048
captured_at: 2026-06-13T17:33:54Z
---

# Episode: Search person and community rows become navigable

## Prior State

Person and community rows in SearchPanel were not tappable — tapping did nothing.

## Trigger

iOS parity gap — iOS NavigationLink makes person rows navigate to ProfileDestination and community rows to room views.

## Decision

Wired CommunitySearchRow to dispatch OpenRoom(community.id) and ProfileSearchRow to dispatch OpenProfile(profile.pubkey) via LocalDispatch.current. Added modifier parameter to SearchResultRow for testTag injection without breaking existing callers.

## Consequences

- Search person rows navigate to profile overlay; community rows navigate to room detail
- Used LocalDispatch.current rather than adding new callback parameters, so ProfilePanel (which also imports CommunitySearchRow) compiles without changes
- Runtime validation blocked by emulator relay instability (drops to Offline); code correctness verified by inspection of .clickable wiring at lines 281-288 of SearchPanel.kt

## Open Tail

- Search row navigation needs runtime validation on a device with stable relay connectivity

## Evidence

- transcript lines 2023-2048

