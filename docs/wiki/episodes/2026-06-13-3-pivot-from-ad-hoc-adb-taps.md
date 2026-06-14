---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: workflow
status: active
subjects:
  - android-validation
  - maestro-flows
  - testtags-as-resourceid
supersedes:
  - 2026-06-13-4-maestro-validation-harness-replacing-ad-hoc
related_claims: []
source_lines:
  - 2334-2398
captured_at: 2026-06-13T17:52:11Z
---

# Episode: Pivot from ad-hoc adb taps to deterministic Maestro flows

## Prior State

Validation was done by agents computing adb tap coordinates and screenshot-matching — repeatedly hitting wrong targets (tapped a reading card instead of a highlight card, never exercised highlight-detail)

## Trigger

Improvisational adb-tap agent tapped the wrong card type ('DMs are dead' article instead of a highlight card) and never exercised the actual highlight-detail screen; the approach was fundamentally flaky

## Decision

Switch to deterministic Maestro flows: add testTagsAsResourceId = true to RootScene's root Box modifier so all existing testTags become addressable, then create 8 declarative YAML flow files (login, feed, highlight-detail, comments, rooms-explorer, open-room, create-room, search-nav)

## Consequences

- testTagsAsResourceId flag propagates to every existing testTag in the codebase via the root Box modifier
- 8 Maestro flow YAML files created under app/android/maestro/
- Reproducible pass/fail + screenshots per flow replaces coordinate-guessing
- Suite is reusable for every remaining flow (reader, profile, share, bookmarks)

## Open Tail

- Search flow (33-search-nav.yaml) was gutted by the runner agent to just assert 'Search tab opens'; needs restoration of person-row→profile navigation assertions

## Evidence

- transcript lines 2334-2398

