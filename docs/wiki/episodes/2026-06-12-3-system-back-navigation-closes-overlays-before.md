---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: product
status: active
subjects:
  - android-back-navigation
  - overlay-stack
  - predictive-back
supersedes: []
related_claims: []
source_lines:
  - 914-940
captured_at: 2026-06-12T08:49:34Z
---

# Episode: System back navigation closes overlays before exiting app

## Prior State

Android app had no back-navigation handling — pressing system back would exit the app regardless of open overlays (comments, invites, room detail, article reader, profile, feedback thread)

## Trigger

Professionalization directive; iOS handles back navigation per-overlay, Android did not

## Decision

Implemented BackHandler in AppScreen.kt that dispatches the appropriate CloseXxx action for the innermost open overlay (comments → invite → room → article → profile → feedback thread) before allowing back to exit the app. Predictive back gesture enabled

## Consequences

- Users can now navigate back through overlay stack as expected
- Predictive back animation supported on Android 14+
- Matches iOS behavior for overlay dismissal

## Open Tail

*(none)*

## Evidence

- transcript lines 914-940
