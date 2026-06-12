---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: product
status: active
subjects:
  - android-back-navigation
  - nmp-app-facade
supersedes: []
related_claims: []
source_lines:
  - 914-938
captured_at: 2026-06-12T08:31:54Z
---

# Episode: System back navigation closes overlays in order

## Prior State

Pressing the Android system back button would exit the app regardless of which overlay panels (comments, invite, room, article reader, profile, feedback thread) were open.

## Trigger

Professionalization directive; proper back handling is baseline for a real Android app.

## Decision

Implemented ordered back-dispatch: system back now closes the innermost open overlay (comments → invite → room → article → profile → feedback thread) before exiting the app. Predictive back gesture enabled.

## Consequences

- Users can navigate back through overlay panels without losing their place
- Maps directly to NMP Close actions (CloseComments, CloseRoomInvite, CloseRoom, CloseArticleReader, CloseProfile, CloseFeedbackThread)

## Open Tail

*(none)*

## Evidence

- transcript lines 914-938
