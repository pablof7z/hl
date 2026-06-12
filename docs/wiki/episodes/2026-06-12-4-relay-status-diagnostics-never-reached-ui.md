---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: product
status: superseded
subjects:
  - relay-status
  - diagnostics-projection
  - nmp-callbacks
supersedes: []
related_claims: []
source_lines:
  - 4447-4456
captured_at: 2026-06-12T13:54:23Z
---

# Episode: Relay status diagnostics never reached UI

## Prior State

The kernel's built-in diagnostics projection was emitted inside snapshot frames but the core never decoded them; Settings showed no relay status on either platform

## Trigger

Professionalization effort discovered relay status was computed but invisible to users

## Decision

Wired nmp_app_set_update_callback → frame decode → diagnostics projection; both platforms now receive and display live relay status (online/offline dots)

## Consequences

- Both platforms show relay status in Settings
- Fix is in the shared Rust core so both platforms benefit simultaneously

## Open Tail

*(none)*

## Evidence

- transcript lines 4447-4456
