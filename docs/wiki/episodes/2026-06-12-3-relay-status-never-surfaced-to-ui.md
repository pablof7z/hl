---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: superseded
subjects:
  - relay-status
  - nmp-diagnostics
  - settings-ui
supersedes: []
related_claims: []
source_lines:
  - 4454-4456
captured_at: 2026-06-12T15:08:36Z
---

# Episode: Relay status never surfaced to UI — diagnostics projection trapped inside actor frames

## Prior State

The kernel's built-in diagnostics projection (relay online/offline status) only traveled inside emitted snapshot frames, which the Highlighter core never decoded. Relay status dots in Settings showed nothing.

## Trigger

On-device verification during the Android professionalization pass revealed relay status was always absent in the UI despite the kernel producing the data.

## Decision

Wired nmp_app_set_update_callback to decode diagnostics frames and surface them as state. Verified on-device: relays now show Online status with live indicator dots in Settings.

## Consequences

- Relay status now visible on both platforms (shared Rust core fix)
- Settings UI can show per-relay connectivity state

## Open Tail

*(none)*

## Evidence

- transcript lines 4454-4456
