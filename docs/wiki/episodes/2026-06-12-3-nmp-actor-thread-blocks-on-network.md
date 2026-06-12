---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: architecture
status: superseded
subjects:
  - nmp-actor-blocking
  - oprunner
  - nmp-architecture
supersedes: []
related_claims: []
source_lines:
  - 4447-4536
captured_at: 2026-06-12T13:54:23Z
---

# Episode: NMP actor thread blocks on network I/O — OpRunner design

## Prior State

92 block_on sites in the NMP actor loop; 5 Class-A sites (blossom upload, chat publish, room join) wait up to 360s on relay ACKs, freezing the entire actor and all UI snapshots; ~24 Class-B sites wait up to 65s on NIP-46 bunker signing

## Trigger

On-device 'Creating…' hang persisted even after adding a 30s worker-side timeout because the actor thread was blocked in another handler and couldn't dequeue the AccountCreateResolved message

## Decision

Hybrid OpRunner strategy: shared 2-thread tokio runtime, OpDomain-keyed in-flight registry with generation supersession + AbortHandle cancellation, single KernelMsg::OpResolved variant back to actor, all state mutation stays on the actor thread; 30s worker-side deadlines; D6 toast/busy-flag error surfacing

## Consequences

- Phased migration: Phase 0 (instrumentation + block_on_local lint), Phase 1 (top-10 wedge sites), Phase 2 (~24 publish helpers), Phase 3 (handle_core_delta hardening), Phase 4 (fold 9 legacy workers into OpRunner)
- relay_policy_json config seam needed (relay URLs are currently compile-time baked via include_str!)
- Deterministic dead-network test harness designed with black-hole TCP listener
- ADR-0040 precedent validated — highlighter facade violates doctrine the framework already fixed for itself

## Open Tail

- Phase 0 + Phase 1 implementation pending
- Class A 360s timeout sites are the critical path
- naddr deep links need a core dispatch action

## Evidence

- transcript lines 4447-4536
