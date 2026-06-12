---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: superseded
subjects:
  - relay-diagnostics
  - nmp-architecture
  - cross-platform-bug
supersedes: []
related_claims: []
source_lines:
  - 2567-2600
  - 3194-3288
  - 3290-3300
  - 3477-3482
captured_at: 2026-06-12T10:50:18Z
---

# Episode: Cross-platform relay status stuck at UNKNOWN: core never decoded diagnostics from snapshot frames

## Prior State

Connection status showed 'Ready'/UNKNOWN on both Android and iOS despite data flowing (24 rooms, 89 highlights, 4 relays). The kernel's built-in relay-diagnostics typed projections only traveled inside emitted snapshot frames, but highlighter-core never decoded them — the diagnostics state was starved on both platforms.

## Trigger

Empirical observation on emulator that status stayed at UNKNOWN with 4 connected relays; traced through NMP kernel code discovering that builtins_diagnostics projections are sidecar data in snapshot frames, not in the host-registered projection registry, so the existing drain never consumed them

## Decision

Added a SnapshotFrameSidecar struct that registers via nmp_app_set_update_callback, decodes each emitted frame to extract relay diagnostics and action results from the typed_projections sidecar, and feeds them into the existing NmpRelayDiagnosticsState and NmpActionResultsState

## Consequences

- Relay connection status now correctly shows 'Online' on both Android and iOS
- Relay management UI in Android Settings → Network shows live status dots per relay
- This was a cross-platform core bug — iOS had the same starved code path, fixed for free
- Any future built-in typed projection will be automatically consumed by the same sidecar mechanism

## Open Tail

*(none)*

## Evidence

- transcript lines 2567-2600
- transcript lines 3194-3288
- transcript lines 3290-3300
- transcript lines 3477-3482
