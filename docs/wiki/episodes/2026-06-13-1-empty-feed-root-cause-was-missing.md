---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: active
subjects:
  - android-feed
  - home-feed-population
supersedes: []
related_claims: []
source_lines:
  - 698-761
captured_at: 2026-06-13T13:20:33Z
---

# Episode: Empty-feed root cause was missing data, not render bug

## Prior State

The Android home feed appeared blank, assumed to be a render/sync bug requiring a code fix in the feed pipeline.

## Trigger

Creating a seeded test account with 16 follows and completing onboarding caused 143 real highlights to populate; validator confirmed the render path works with data present.

## Decision

The feed render pipeline was never broken. Emptiness was caused by accounts having zero follows and incomplete onboarding, not a UI or core sync defect.

## Consequences

- Effort redirected from feed-render debugging to presentation/hydration parity with iOS
- Onboarding completion is a hard prerequisite for feed population — any future automated test must complete onboarding before checking feed state
- The feed-sync code path was validated as working end-to-end with relay connections

## Open Tail

- Onboarding completion rate as a UX concern — users who skip it see a dead feed

## Evidence

- transcript lines 698-761

