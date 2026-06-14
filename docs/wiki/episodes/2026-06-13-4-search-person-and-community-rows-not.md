---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - search-navigation
  - profile-navigation
  - search-panel
supersedes: []
related_claims: []
source_lines:
  - 1985-2109
captured_at: 2026-06-13T16:12:24Z
---

# Episode: Search person and community rows not navigable

## Prior State

Person and community search result rows were display-only; tapping did nothing. iOS dispatches NavigationLink(value: c.id) for communities and NavigationLink(value: ProfileDestination.pubkey) for people.

## Trigger

iOS parity gap identified in audit; initial validation on degraded emulator falsely reported rows as broken, but code review confirmed they were simply not wired to dispatch actions.

## Decision

Added LocalDispatch.current to ProfileSearchRow (dispatches OpenProfile(profile.pubkey)) and CommunitySearchRow (dispatches OpenRoom(community.id)). Made rows clickable via SearchResultRow modifier pattern.

## Consequences

- Person rows navigate to profile overlay matching iOS ProfileDestination
- Community rows navigate to room detail matching iOS RoomHomeView
- SearchPanel continues to compile without changes to other callers (LocalDispatch.current, not new parameter)

## Open Tail

*(none)*

## Evidence

- transcript lines 1985-2109

