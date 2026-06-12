---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: product
status: superseded
subjects:
  - create-account
  - nip05
  - signup-flow
supersedes: []
related_claims: []
source_lines:
  - 3999-4120
  - 4150-4158
captured_at: 2026-06-12T14:10:16Z
---

# Episode: Signup Blocked on NIP-05 API Failure

## Prior State

Account creation required the username NIP-05 availability check to return Available before the submit button was enabled. If the API returned an error, 404, or timed out, can_submit stayed false and account creation was permanently impossible.

## Trigger

On-device testing showed account creation stuck at 'Creating…' indefinitely; investigation revealed the NIP-05 API at beta.highlighter.com returns 404 (server route missing), completely bricking signup.

## Decision

Changed validation so a failed username availability check still allows proceeding — the NIP-05 claim is skipped on failure and account creation proceeds with a warning rather than blocking entirely.

## Consequences

- Account creation works even when NIP-05 API is down or missing
- Username NIP-05 verification becomes best-effort rather than gating
- Server-side nip05 route still needs deployment for full functionality

## Open Tail

- beta.highlighter.com/api/nip05 route needs server-side deployment

## Evidence

- transcript lines 3999-4120
- transcript lines 4150-4158
