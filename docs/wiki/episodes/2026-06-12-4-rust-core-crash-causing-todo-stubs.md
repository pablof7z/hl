---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: active
subjects:
  - rust-todo-removal
  - cache-module
  - hydrate-stub
supersedes: []
related_claims: []
source_lines:
  - 1170-1297
captured_at: 2026-06-12T08:49:34Z
---

# Episode: Rust core crash-causing todo!() stubs removed

## Prior State

Two todo!() macros in the Rust core would panic at runtime if reached: cache.rs::highlight_counts_by_artifact (used nowhere) and highlights.rs::hydrate (dead code with no callers). The cache module was a shell wrapping an uninitialized _db: () field

## Trigger

Gap-finding directive surfaced todo!() panics via grep; these are crash bugs that would surface unpredictably

## Decision

Removed the entire cache.rs module (pub mod cache from lib.rs) since it was an empty shell with a todo!() crash. Removed the hydrate() dead stub from highlights.rs. Kept subscribe_vault and get_my_highlights which had test coverage despite the dead-code warning, adding doc notes instead

## Consequences

- No runtime panic paths remain in the Rust core from todo!() macros
- 251/251 core tests still pass after removal
- Cache module's intended nostrdb initialization is documented but not yet implemented — a conscious gap rather than a hidden crash

## Open Tail

- nostrdb-backed Cache implementation remains a future task

## Evidence

- transcript lines 1170-1297
