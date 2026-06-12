---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: active
subjects:
  - account-creation
  - nip05-validation
  - signup-flow
supersedes:
  - 2026-06-12-2-account-creation-bricked-when-nip-05
  - 2026-06-12-2-signup-blocked-on-nip-05-api
related_claims: []
source_lines:
  - 4447-4456
captured_at: 2026-06-12T16:31:21Z
---

# Episode: Account creation impossible when NIP-05 check fails

## Prior State

can_submit required the NIP-05 username check to return Available. Since the UI auto-fills username from display name and the NIP-05 API (beta.highlighter.com/api/nip05) returns 404, account creation was impossible.

## Trigger

On-device testing found signup bricked when the username check fails.

## Decision

Allow proceeding on a failed NIP-05 check (the claim is skipped rather than blocking submission). Added 30s deadline + HTTP timeouts to account creation; removed dead code (todo!() panics).

## Consequences

- Account creation now works offline and when NIP-05 is unavailable
- Dead todo!() panics removed from account creation path
- initPlatformLogging() added so Rust logs reach logcat/Xcode

## Open Tail

- NIP-05 API route missing server-side (returns 404) — should be deployed

## Evidence

- transcript lines 4447-4456
