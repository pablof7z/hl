---
type: episode-card
date: 2026-06-14
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: architecture
status: active
subjects:
  - nmp-runtime
  - highlighter-core
  - adr-0052
supersedes:
  - 2026-06-14-3-nmp-adr-0052-builder-typestate-migration
related_claims: []
source_lines:
  - 3577-3610
captured_at: 2026-06-14T08:39:07Z
---

# Episode: NMP ADR-0052 core migration: ProjectionsDeclared typestate

## Prior State

Core's nmp_runtime.rs used the old NMP builder chain: builder.storage_path(...).start(RunConfig::default()) — missing the required ProjectionsDeclared typestate step

## Trigger

Latest NMP (ADR-0052) made ProjectionsDeclared a required builder step before .start(), and register_action now takes the module instance by value — cargo check produced 6 compile errors

## Decision

Added .consume_all_builtin_projections() between .storage_path(...) and .start(RunConfig::default()); updated register_action calls to pass module by value; used consume_all_builtin_projections() rather than explicit declaration because hl's snapshot-tick observer reads both relay diagnostics and action results

## Consequences

- Core compiles and runs against latest NMP
- All smoke tests pass: feed projections, NIP-29 rooms projections, create-room registered action all verified
- ADR-0053 DEBT warning logged: host not declaring consumed projections explicitly (informational, not blocking)

## Open Tail

- ADR-0053 DEBT: should eventually declare specific consumed projections rather than consume_all_builtin_projections for tighter API contract

## Evidence

- transcript lines 3577-3610

