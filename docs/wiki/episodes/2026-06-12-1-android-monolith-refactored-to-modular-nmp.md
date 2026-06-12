---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: architecture
status: superseded
subjects:
  - android-modularization
  - nmp-architecture
  - mainactivity-refactor
supersedes: []
related_claims: []
source_lines:
  - 754-802
captured_at: 2026-06-12T08:49:34Z
---

# Episode: Android monolith refactored to modular NMP architecture

## Prior State

The entire Android UI lived in a single 3,317-line MainActivity.kt — a 'reference implementation' with ~70 lambda callbacks threaded through a single composable screen function, rather than a production app following the NMP dispatch pattern

## Trigger

Assessment revealed Android had feature parity through the shared Rust core but was structurally a monolith, not a maintainable app; the 'professionalize Android' directive required decomposing it into real architecture

## Decision

Refactored into 23 cohesive files: one file per feature panel (auth, home, search, reader, bookmarks, rooms, capture, comments, feedback, settings, whatsnew), shared components, theme, and formatters. The ~70-lambda HighlighterAppScreen signature collapsed to (state, dispatch) — every panel now constructs its own HighlighterAppAction and dispatches it directly, matching the iOS NMP pattern

## Consequences

- Android now mirrors iOS's NMP architecture: panels own their action construction instead of receiving pre-wired callbacks
- New feature panels can be added without touching MainActivity or any other panel
- Visibility changed from private to internal only for cross-file symbols; leaf-row composables retain local () -> Unit params
- Build-verified (debug + release APK, unit tests, lint all pass)

## Open Tail

- Podcast mini-player panel still missing; strings not yet extracted to strings.xml

## Evidence

- transcript lines 754-802
