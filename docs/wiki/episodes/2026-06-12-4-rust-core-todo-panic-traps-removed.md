---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: active
subjects:
  - rust-core
  - todo-macro
  - cache-module
supersedes: []
related_claims: []
source_lines:
  - 1170-1200
  - 1258-1300
  - 1670-1700
captured_at: 2026-06-12T08:57:33Z
---

# Episode: Rust core todo!() panic traps removed

## Prior State

Two `todo!()` macros in the Rust core that panic at runtime: `Cache::highlight_counts_by_artifact()` and `highlights::hydrate()`, plus the entire `cache.rs` module that was a stub with empty `_db: ()` field. Three dead methods on HighlighterCore (subscribe_vault, get_my_highlights, is_article_bookmarked) that compiled but were never called.

## Trigger

Systematic grep for `todo!()` during gap-finding revealed crash bugs

## Decision

Removed the entire `cache.rs` module (it was unpopulated: `_db: ()`), removed the `hydrate()` stub, removed 3 dead methods. Kept `subscribe_vault` because it has live test coverage and FFI ties, adding a doc note instead.

## Consequences

- No runtime panic traps remain in the `cache` or `highlights` modules
- 251/251 core tests still pass after removal
- highlight_counts_by_artifact feature is now absent rather than crash-prone — needs reimplementation backed by real nostrdb
- Vault subscription functionality preserved with documented caveat

## Open Tail

- Cache module needs real nostrdb-backed implementation
- highlight_counts_by_artifact removed without replacement

## Evidence

- transcript lines 1170-1200
- transcript lines 1258-1300
- transcript lines 1670-1700
