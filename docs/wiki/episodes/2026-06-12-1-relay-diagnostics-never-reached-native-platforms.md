---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: superseded
subjects:
  - relay-diagnostics
  - nmp-snapshot-projections
  - cross-platform-bug
supersedes: []
related_claims: []
source_lines:
  - 2593-3306
captured_at: 2026-06-12T11:17:28Z
---

# Episode: Relay diagnostics never reached native platforms — snapshot frame decoding gap

## Prior State

Relay connection status (Online/Offline/Connecting) was not displaying on Android or iOS. The NMP kernel computed relay diagnostics as a built-in typed projection, but highlighter-core's nmp_runtime.rs never decoded or consumed the typed-projection sidecars embedded in snapshot frames — the data was emitted but silently dropped on both platforms.

## Trigger

Investigating Android's missing relay status revealed that the kernel's built-in relay-diagnostics projection only travels inside emitted snapshot frames, and highlighter-core had no plumbing to extract them from the `nmp_app_set_update_callback` frame bytes.

## Decision

Added a `BuiltinDiagnosticsSidecar` struct, a C-ABI callback registration, and a drain thread in nmp_runtime.rs that decodes relay diagnostics and action results from typed-projection sidecars inside snapshot frames. The diagnostics state now feeds `relay_diagnostics_snapshot()` and the action-results state, both consumed by the native reconciler.

## Consequences

- Both Android and iOS now correctly display relay connection status (verified 'Online' on emulator)
- Action results from NMP dispatches also reach native UI for the first time
- The root cause was in the shared Rust core, not platform-specific code — any future NMP typed projection will now be automatically consumed by the same drain path

## Open Tail

- The drain uses a bounded sync channel with a per-tick wake; if typed projections grow in volume, backpressure behavior should be verified

## Evidence

- transcript lines 2593-3306
