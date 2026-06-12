---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: superseded
subjects:
  - nmp-typed-projections
  - relay-diagnostics
  - highlighter-core
supersedes: []
related_claims: []
source_lines:
  - 2850-3290
captured_at: 2026-06-12T11:35:05Z
---

# Episode: NMP typed-projection drain was unwired, starving relay status on both platforms

## Prior State

Relay connection status was not displayed (or stuck in a non-Online state) on Android, and the same code path was starved on iOS. The assumption was that this was an Android-specific UI gap.

## Trigger

Investigation revealed the kernel's built-in relay-diagnostics projection only travels inside emitted snapshot frames (not in `run_typed_snapshot_projections`, which runs only host-registered closures). highlighter-core never decoded the typed-projections sidecar from those frames, so diagnostics data was produced by the Rust actor but never consumed by the host bridge.

## Decision

Created a `TypedProjectionSidecar` struct and drain-based consumption pattern in `nmp_runtime.rs`: on each snapshot tick, the host-side callback decodes the frame's typed-projections sidecar (relay diagnostics + action results) and pushes them into dedicated state holders. This required a new `decode_snapshot_typed_projections` import (re-exported through `nmp_core`) and a sidecar struct wired into `HighlighterNmpRuntime`.

## Consequences

- Relay status line now reads 'Online' on Android emulator, verified live
- iOS gets the same fix for free since it shares the same Rust core; its relay rows were equally starved
- Establishes the architectural contract: built-in NMP projections are consumed via frame-envelope decoding, not via the host-registered projection registry
- Any future built-in typed projection must be drained through this same sidecar mechanism

## Open Tail

- The web app (SvelteKit/NDK) is intentionally outside NMP and has no equivalent projection consumption

## Evidence

- transcript lines 2850-3290
