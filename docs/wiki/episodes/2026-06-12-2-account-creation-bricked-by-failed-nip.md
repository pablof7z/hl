---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: product
status: superseded
subjects:
  - signup
  - nip05
  - create-account
supersedes: []
related_claims: []
source_lines:
  - 3999-4150
captured_at: 2026-06-12T13:29:48Z
---

# Episode: Account creation bricked by failed NIP-05 availability check

## Prior State

Account creation required `username_status == Available` before `can_submit` became true; any NIP-05 check failure or unreachable API left the submit button permanently disabled.

## Trigger

On-device testing showed account creation hanging on 'Creating…' indefinitely; diagnosis revealed `beta.highlighter.com/api/nip05` returns 404 and the NIP-05 check transitions to `Error`, blocking submission entirely.

## Decision

Changed `recompute_create_account_submit` so a failed username check no longer blocks submission — the NIP-05 claim is simply skipped when the check can't complete.

## Consequences

- Users can now create accounts even when the NIP-05 API is down or returns errors
- Username uniqueness is no longer verified client-side before account creation; the claim is best-effort
- A 30s deadline and HTTP timeouts were also added to the create-account worker to prevent indefinite hangs

## Open Tail

- Server-side NIP-05 route needs to be restored at beta.highlighter.com

## Evidence

- transcript lines 3999-4150
