---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: reversal
status: active
subjects:
  - android-app-architecture
  - android-navigation
  - android-auth-flow
supersedes: []
related_claims: []
source_lines:
  - 1854-1858
  - 2219-2232
captured_at: 2026-06-12T09:08:45Z
---

# Episode: Android app restructured from single-file dump to navigation architecture

## Prior State

Android app was a single 3,300-line MainActivity.kt with every panel stacked in one LazyColumn — auth/onboarding, settings, communities, all scrolling together with no tab bar, no auth gate, and no screen-level lifecycle management.

## Trigger

User explicitly rejected the state: 'it's all just a long view… no tabs for settings, it doesn't even connect to relays, it is basically missing everything — there's no flow, it's just a dumping ground of things.'

## Decision

Full restructure mirroring iOS RootSceneView: welcome → login/create-account → onboarding interests → bottom-tabbed main app (Highlights, Rooms, Search) with profile/settings screens and proper back navigation. Screens own their lifecycle, dispatching Open/Close actions on enter/leave.

## Consequences

- Android UI must be organized into multiple files and composable destinations instead of one monolith
- Auth state now gates the entire UI tree — unauthenticated users see only the welcome flow
- Bottom tab bar replaces the single-scroll approach permanently
- Share composer becomes a sheet rather than an inline section

## Open Tail

- Rebuild agent still in progress — on-emulator acceptance test (login, kill, relaunch still logged in) not yet verified
- Podcast playback UI (Media3 ExoPlayer + mini player) still queued in Wave B

## Evidence

- transcript lines 1854-1858
- transcript lines 2219-2232
