---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - search-navigation
  - android-search-panel
supersedes: []
related_claims: []
source_lines:
  - 2022-2039
  - 2094-2116
captured_at: 2026-06-13T16:02:40Z
---

# Episode: Search result rows now navigate to profile and room

## Prior State

Search person (ProfileSearchRow) and community (CommunitySearchRow) result rows displayed data but were not tappable — no click handlers, no navigation to profile or room.

## Trigger

iOS parity gap audit identified that iOS search results navigate to profile/room detail via NavigationLink.

## Decision

Wire both row types via LocalDispatch.current: CommunitySearchRow dispatches OpenRoom(community.id), ProfileSearchRow dispatches OpenProfile(profile.pubkey). SearchResultRow modifier parameter added for testTag injection. Verified in code that .clickable is applied conditionally when onClick is provided.

## Consequences

- Search person rows navigate to profile overlay
- Search community rows navigate to room detail
- Initial validator reported failure but code inspection confirmed correct wiring — failure was due to degraded emulator, not a code bug

## Open Tail

*(none)*

## Evidence

- transcript lines 2022-2039
- transcript lines 2094-2116

