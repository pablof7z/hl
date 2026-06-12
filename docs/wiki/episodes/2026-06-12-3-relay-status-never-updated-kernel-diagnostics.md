---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: active
subjects:
  - relay-status
  - nmp-update-callback
  - diagnostics-projection
supersedes:
  - 2026-06-12-3-relay-status-never-surfaced-to-ui
  - 2026-06-12-1-nmp-typed-projection-drain-was-unwired
  - 2026-06-12-3-relay-status-never-updated-in-ui
  - 2026-06-12-3-cross-platform-relay-status-stuck-at
related_claims: []
source_lines:
  - 4447-4456
captured_at: 2026-06-12T16:31:21Z
---

# Episode: Relay status never updated — kernel diagnostics projection not decoded

## Prior State

Relay status showed no updates in Settings on either platform. The kernel's built-in diagnostics projection only traveled inside emitted snapshot frames, which the core never decoded.

## Trigger

On-device testing revealed relays showing no live status dots.

## Decision

Wired nmp_app_set_update_callback through frame decode so diagnostics reach the reconciler. Fixed both platforms at once.

## Consequences

- Relay status now reaches Online on-device; Settings lists every relay with live status dots
- The architectural insight — that snapshot frames carry diagnostics that must be decoded — applies to any future NMP facade consumer

## Open Tail

*(none)*

## Evidence

- transcript lines 4447-4456
