---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: superseded
subjects:
  - create-account
  - nip05
  - signup-flow
supersedes:
  - 2026-06-12-2-account-creation-bricked-by-failed-nip
related_claims: []
source_lines:
  - 3876-4158
captured_at: 2026-06-12T13:54:23Z
---

# Episode: Account creation bricked when NIP-05 API is unreachable

## Prior State

can_submit required username_status == Available; the NIP-05 check API at beta.highlighter.com/api/nip05 returns 404, making account creation impossible for all users

## Trigger

On-device testing showed 'Creating…' hang; traced to recompute_create_account_submit gating on Available status when the check can never succeed

## Decision

Allow submission when the username check fails (skip the NIP-05 claim on failure) and add a 30s deadline + HTTP timeouts to account creation; remove dead todo!() panics

## Consequences

- Account creation works in degraded/offline mode without NIP-05
- Users who skip the username field or fail the check still get a valid account
- The NIP-05 404 remains a server-side issue to fix separately

## Open Tail

- Server-side nip05 route needs to be deployed
- Actor-blocking means even the 30s timeout can't resolve if the actor is wedged

## Evidence

- transcript lines 3876-4158
