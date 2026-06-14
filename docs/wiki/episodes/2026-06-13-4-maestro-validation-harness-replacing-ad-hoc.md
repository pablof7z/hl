---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: architecture
status: superseded
subjects:
  - validation-harness
  - maestro-flows
  - testtag-semantics
supersedes: []
related_claims: []
source_lines:
  - 2336-2347
  - 2349-2398
captured_at: 2026-06-13T16:24:41Z
---

# Episode: Maestro validation harness replacing ad-hoc agent tap-testing

## Prior State

Validation was done by dispatching agents that computed adb tap coordinates and screenshots. This was unreliable: agents died mid-run on long sessions, taps misregistered on degraded emulators, and coordinate-guessing missed specific targets (e.g. tapping a Reading card instead of a Highlight card). Results were inconsistent and unrepeatable.

## Trigger

Repeated validation failures — agents dying, login buttons not responding on fresh boots, wrong card types tapped, 67-call thrashing — made it clear that ad-hoc coordinate-based validation was not trustworthy for pass/fail decisions.

## Decision

Added testTagsAsResourceId = true to the root composable in RootScene.kt (single semantics modifier propagating to all testTags). Created 8 declarative Maestro flow files in app/android/maestro/: 00-login, 06-feed, 08-highlight-detail, 30-comments, 11-rooms-explorer, 12-open-room, 19-create-room, 33-search-nav. Each flow targets testTags and visible-text selectors for deterministic interaction.

## Consequences

- All existing testTags (feed_highlight_card, highlight_detail, room_tile_name, create_room_fab, search_person_row, etc.) are now addressable via Maestro's resourceId selector
- Validation is reproducible: maestro test yields deterministic pass/fail per flow with screenshots
- New interactive elements must include testTags for Maestro addressability — implicit contract
- Profile screen assertions use visible-text 'Profile' (no root testTag on ProfilePanel yet)

## Open Tail

- ProfilePanel needs a root testTag for more specific assertions
- Onboarding chip taps use index-based selectors which may be fragile across UI changes

## Evidence

- transcript lines 2336-2347
- transcript lines 2349-2398

