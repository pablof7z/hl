---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: architecture
status: active
subjects:
  - nmp-adoption
  - web-architecture
  - source-of-truth
supersedes: []
related_claims: []
source_lines:
  - 110-133
captured_at: 2026-06-12T08:57:33Z
---

# Episode: NMP adoption: 100% on native, 0% on web — deliberate architectural boundary or debt

## Prior State

Unclear whether NMP (Native Model Pattern) was fully adopted across the codebase

## Trigger

User asked 'is NMP already fully adopted throughout?'

## Decision

Finding: NMP is at 100% where it applies (Rust core owns all business logic and state; iOS dispatches via nmpApp.dispatch; Android built NMP-native from start). The web app (SvelteKit) uses NDK directly with no NMP integration — duplicating logic the Rust core owns. This appears deliberate but creates architectural debt.

## Consequences

- Web app duplicates business logic that Rust core already owns
- Any future behavior change must be implemented twice (Rust + NDK/TypeScript)
- A WASM build of the Rust core could unify this, but no such build exists

## Open Tail

- Decide explicitly whether web stays outside NMP permanently or gets a WASM bridge

## Evidence

- transcript lines 110-133
