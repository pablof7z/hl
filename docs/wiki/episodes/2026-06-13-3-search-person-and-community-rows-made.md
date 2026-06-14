---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - search-navigation
  - profile-search-row
  - community-search-row
supersedes:
  - 2026-06-13-4-search-person-and-community-rows-not
  - 2026-06-13-5-search-result-rows-now-navigate-to
related_claims: []
source_lines:
  - 2023-2039
captured_at: 2026-06-13T16:24:41Z
---

# Episode: Search person and community rows made tappable

## Prior State

ProfileSearchRow and CommunitySearchRow in SearchPanel had no onClick handlers — tapping them did nothing. iOS dispatches NavigationLink to profile or room destinations.

## Trigger

iOS parity audit identified that search result rows should navigate to profile (person) or room (community) destinations.

## Decision

Wired both rows with LocalDispatch.current: ProfileSearchRow dispatches OpenProfile(profile.pubkey), CommunitySearchRow dispatches OpenRoom(community.id). Added onClick parameter to SearchResultRow with clickable modifier.

## Consequences

- Search person rows now navigate to the profile overlay
- Search community rows now navigate to the room detail screen
- testTags added: search_person_row, search_community_row

## Open Tail

*(none)*

## Evidence

- transcript lines 2023-2039

