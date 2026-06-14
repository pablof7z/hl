---
type: episode-card
date: 2026-06-14
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: architecture
status: superseded
subjects:
  - nmp-runtime
  - adr-0052
  - core-dependency
supersedes: []
related_claims: []
source_lines:
  - 3577-3603
captured_at: 2026-06-14T08:09:37Z
---

# Episode: NMP ADR-0052 builder typestate migration

## Prior State

nmp_runtime.rs called builder.storage_path(…).start(RunConfig::default()) and register_action::<T>() without a module instance — the old NMP API

## Trigger

After merging PR #6 and updating to latest NMP, cargo check failed with 6 errors: the builder gained a required ProjectionsDeclared transition step before .start(), and register_action now takes a module instance by value

## Decision

Added .consume_all_builtin_projections() between .storage_path(…) and .start(…), and updated register_action calls to pass the module by value — matching the nmp-defaults/examples/minimal_app.rs reference pattern

## Consequences

- Core compiles against latest NMP; all 7 Maestro smoke flows pass (feed projections, NIP-29 rooms, create-room action)
- ADR-0053 informational warning logged at runtime: 'host not declaring consumed projections' (consume_all_builtin_projections is the broadest declaration, not the tightest)
- Future cleanup: narrow to declare_consumed_projections([...]) for only the projections hl actually consumes

## Open Tail

- ADR-0053 debt — tighten projection declaration to only the consumed set

## Evidence

- transcript lines 3577-3603

