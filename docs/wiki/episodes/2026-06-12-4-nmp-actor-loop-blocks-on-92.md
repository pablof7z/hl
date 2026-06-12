---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: superseded
subjects:
  - nmp-actor
  - blocking-awaits
  - dead-network
supersedes: []
related_claims: []
source_lines:
  - 4463-4484
captured_at: 2026-06-12T13:29:48Z
---

# Episode: NMP actor loop blocks on ~92 network awaits

## Prior State

The NMP actor loop was assumed to handle all dispatched actions responsively; on a live network this appeared to work.

## Trigger

Account creation hung indefinitely on a dead/emulated network; tracing revealed ~92 `block_on` sites inside the NMP actor loop that await network I/O, wedging the entire actor when the network is unreachable.

## Decision

Documented as a known architectural defect; added a 30s deadline + HTTP timeouts as a partial mitigation for the create-account path specifically; spawned a Fable Architect agent to research a proper fix across all 92 sites.

## Consequences

- Create-account no longer hangs indefinitely (30s deadline), but other network-blocking paths in the actor loop still wedge on dead networks
- A phased design doc is being produced at docs/architecture/actor-blocking-fix.md
- The single-threaded actor model needs spawn-and-message-back or multi-threaded handler pool to avoid blocking

## Open Tail

- Full audit of all 92 block_on sites classifying harmless (local ndb) vs network-unbounded
- Implement spawn-and-message-back pattern generalizing the create-account worker
- Deterministic dead-network test harness

## Evidence

- transcript lines 4463-4484
