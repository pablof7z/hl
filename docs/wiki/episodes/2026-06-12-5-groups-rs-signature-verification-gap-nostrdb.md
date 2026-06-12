---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: active
subjects:
  - nostrdb-signatures
  - group-membership
  - security
supersedes: []
related_claims: []
source_lines:
  - 1309-1335
captured_at: 2026-06-12T08:57:33Z
---

# Episode: groups.rs signature verification gap: nostrdb strips signatures

## Prior State

groups.rs code comment stated it uses a 'placeholder sig' and relies on `Event::from_json` which does NOT verify signatures. The code path queries nostrdb, re-hydrates events into nostr-sdk Event objects by splicing a valid-shape placeholder signature.

## Trigger

Systematic review during gap-finding; the code comment itself flagged the concern

## Decision

Identified but not resolved in this session — the comment was read and flagged as a security concern

## Consequences

- Group membership metadata is trusted without cryptographic verification
- Any relay could inject forged group admin/member events that would be accepted as valid

## Open Tail

- Must implement proper signature verification for nostrdb-hydrated events, or use an alternate query path that preserves signatures

## Evidence

- transcript lines 1309-1335
