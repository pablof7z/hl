---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: architecture
status: superseded
subjects:
  - android-modularization
  - nmp-app-facade
supersedes: []
related_claims: []
source_lines:
  - 754-802
  - 1120-1127
captured_at: 2026-06-12T08:31:54Z
---

# Episode: Android monolith → NMP-aligned modular architecture

## Prior State

The entire Android UI lived in a single 3,317-line MainActivity.kt with a ~70-lambda HighlighterAppScreen signature threading callbacks through every panel.

## Trigger

Session-scoped directive to 'professionalize, productize, improve the Android app as a real app'; prior assessment confirmed the single-file structure was parity of capability but not of maintainability.

## Decision

Refactored into 23 cohesive files: one file per feature package (auth, home, search, reader, bookmarks, rooms, capture, comments, feedback, settings, whatsnew), shared theme/components/util packages, and a 2-parameter (state, dispatch) AppScreen — every panel now constructs its own HighlighterAppAction inline, matching the NMP fire-and-forget pattern.

## Consequences

- Each panel is independently editable; future feature work no longer requires navigating a 3300-line file
- The AppScreen surface API is minimal (state + dispatch), matching iOS's post-migration NMP pattern
- Visibility shifted: cross-file symbols became internal, single-file symbols stayed private
- FEEDBACK_PROJECT_COORDINATE extracted to AppConfig.kt

## Open Tail

- Podcast mini-player UI still missing (iOS has one)
- User-facing strings still hardcoded, not in strings.xml
- Single ABI (arm64-v8a) only

## Evidence

- transcript lines 754-802
- transcript lines 1120-1127
