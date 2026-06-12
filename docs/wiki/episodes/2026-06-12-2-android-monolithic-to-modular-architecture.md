---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: architecture
status: active
subjects:
  - android-app
  - modularization
  - nmp-architecture
supersedes: []
related_claims: []
source_lines:
  - 232-234
  - 754-803
captured_at: 2026-06-12T08:19:17Z
---

# Episode: Android: Monolithic to Modular Architecture

## Prior State

All Android UI code in a single 3,317-line MainActivity.kt with ~70 lambda callbacks threaded through HighlighterAppScreen

## Trigger

Professionalization directive required structural cleanup to make the app maintainable

## Decision

Split into 23 cohesive files: each panel owns its own dispatch calls, HighlighterAppScreen reduced to 2 parameters (state, dispatch), visibility changed from private to internal only for cross-file symbols

## Consequences

- Each feature panel (Auth, Profile, Search, Rooms, etc.) is now independently editable
- Callback wiring moved from centralized MainActivity to each panel's callsite
- No onXxx dispatch-wrapper functions remain — panels construct actions directly
- Total lines grew slightly (~3,317 → ~3,486) due to per-file package/import blocks, which is expected

## Open Tail

*(none)*

## Evidence

- transcript lines 232-234
- transcript lines 754-803
