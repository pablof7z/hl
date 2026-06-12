---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: active
subjects:
  - ci-sibling-repo
  - cargo-path-dependency
  - android-ci
supersedes: []
related_claims: []
source_lines:
  - 804-806
  - 959-962
captured_at: 2026-06-12T08:49:34Z
---

# Episode: CI sibling-repo path dependency discovered and fixed

## Prior State

Rust core's Cargo.toml has path dependencies pointing to ../../../nostr-multi-platform/crates/... (a sibling repo). CI only checked out the main repo, so any PR touching app/core would fail to build. The refactor agent's isolated worktree also failed to build for the same reason

## Trigger

The modularization agent's worktree build failed because the relative path to nostr-multi-platform didn't resolve. This surfaced a CI gap that would also break any PR touching the Rust core

## Decision

Added a checkout step for the sibling nostr-multi-platform repo in .github/workflows/android.yml so the path dependencies resolve. Also documented the sibling-repo dependency in saved memory

## Consequences

- CI now checks out both repos, so Android/core PRs will build correctly
- The sibling-repo coupling is now explicit rather than implicit
- Future worktree-based agents need the same symlink or checkout setup

## Open Tail

- The sibling path dependency could be replaced with git/Cargo registry references for more robust CI

## Evidence

- transcript lines 804-806
- transcript lines 959-962
