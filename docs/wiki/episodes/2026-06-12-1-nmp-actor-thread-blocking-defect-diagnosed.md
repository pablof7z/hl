---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: superseded
subjects:
  - actor-blocking
  - nmp-facade
  - highlighter-core
supersedes:
  - 2026-06-12-4-nmp-actor-loop-blocks-on-92
related_claims: []
source_lines:
  - 4463-4551
captured_at: 2026-06-12T14:10:16Z
---

# Episode: NMP Actor-Thread Blocking Defect Diagnosed

## Prior State

The NMP actor loop in highlighter-core called runtime.block_on() on network-dependent operations inside a single-threaded actor, with waits up to 360s for relay ACKs and 65s per signature. This was assumed acceptable and consistent with the NMP framework's conventions.

## Trigger

On-device testing revealed account creation hung indefinitely on dead networks — even after a 30s worker-side timeout fired, the actor could not dequeue AccountCreateResolved because it was blocked in another handler. Systematic investigation reclassified 92 block_on sites into 4 classes: A (5 sites, 360s waits for relay ACK), B (~24 sites, 65s sign waits), C (3 sites, bounded HTTP), D (~58 local-only, safe).

## Decision

Adopted OpRunner hybrid strategy: shared 2-thread tokio runtime, domain-keyed in-flight registry with generation supersession + AbortHandle, single KernelMsg::OpResolved variant, all state mutation stays on actor thread. Follows NMP framework's own ADR-0040 precedent for the identical bug class (V-90). Zero changes to NMP framework required.

## Consequences

- Phased migration required: Phase 0 (instrumentation + block_on_local wrapper + CI lint), Phase 1 (top-10 wedge sites incl. all Class A), Phase 2 (~24 publish helpers), Phases 3-4 (handle_core_delta hardening + fold 9 legacy workers into OpRunner)
- The 9 existing bespoke worker threads (account-create, search, bunker sign-in, etc.) will be folded into OpRunner in Phase 4
- A deterministic dead-network test harness is needed, requiring a relay_policy_json config seam (currently compile-time baked via include_str!)
- Timeouts-only approach was rejected as insufficient (30s account-create timeout already proved inadequate on-device)
- NMP's APIs make this mistake easy — dispatch_nmp_action_for_result hands you a future that can pend for 6 minutes with nothing preventing on-actor await

## Open Tail

- Phase 0+1 implementation begun via agent; Phases 2-4 queued

## Evidence

- transcript lines 4463-4551
