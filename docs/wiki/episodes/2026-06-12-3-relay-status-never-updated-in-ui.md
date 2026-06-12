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
  - settings
supersedes:
  - 2026-06-12-1-relay-diagnostics-starved-by-undecoded-snapshot
  - 2026-06-12-4-relay-status-diagnostics-never-reached-ui
related_claims: []
source_lines:
  - 4453-4456
captured_at: 2026-06-12T14:10:16Z
---

# Episode: Relay Status Never Updated in UI

## Prior State

Relay status indicators in Settings showed static/stale state and never reflected actual relay connectivity.

## Trigger

On-device testing during Android professionalization showed relay status never reaching 'Online' even on a working network. Root cause: the kernel's diagnostics projection only traveled inside emitted snapshot frames, which the core never decoded.

## Decision

Wired nmp_app_set_update_callback → frame decode → diagnostics projection so relay status reaches the UI.

## Consequences

- Both iOS and Android now show live relay connectivity status in Settings
- Users can verify relay connections are working from the Settings UI

## Open Tail

*(none)*

## Evidence

- transcript lines 4453-4456
