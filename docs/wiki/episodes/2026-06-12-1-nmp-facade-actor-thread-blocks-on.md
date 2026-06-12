---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: architecture
status: superseded
subjects:
  - actor-blocking
  - op-runner
  - nmp-app-facade
  - highlighter-core
supersedes: []
related_claims: []
source_lines:
  - 4176-4426
  - 4487-4504
  - 4539-4551
  - 4581-4633
captured_at: 2026-06-12T15:08:36Z
---

# Episode: NMP facade actor thread blocks on network calls, wedging UI indefinitely

## Prior State

The Highlighter NMP facade's single kernel actor thread called runtime.block_on() for ~92 sites, including ~5 network-dependent operations that could wait up to 360s (blossom uploads, chat publish, room join via dispatch_action_for_result) and ~24 publish helpers that could wait 65s per NIP-46 bunker signature. On a dead network, the actor froze entirely — even timeouts couldn't resolve because the actor was too blocked to dequeue resolution messages.

## Trigger

On-device testing showed account creation stuck at 'Creating…' forever even after the 30s worker-side timeout fired; diagnosis confirmed the actor couldn't dequeue AccountCreateResolved because it was blocked mid-handler. The NMP framework's own ADR-0040 had already fixed the identical bug class (V-90) inside nmp-core itself using a serialized worker + parked-operation pattern.

## Decision

Adopt an OpRunner primitive: a shared 2-thread tokio runtime with an OpDomain-keyed in-flight registry, generation-based supersession (double-tap aborts prior op), AbortHandle cancellation, and a single KernelMsg::OpResolved variant back to the actor. All state mutation stays on the actor thread (preserving the exclusive-write-between-emits invariant). Phase 0 added per-message duration instrumentation, a block_on_local() wrapper that warns above 50ms, and a CI lint banning raw block_on in the actor. Phase 1 migrated the 10 worst wedge sites (5 Class A at 360s, 3 Class C bounded HTTP, 2 room create/invite).

## Consequences

- Empirically proven: old inline-block_on shape starves queued resolutions for ≥2.5s; new submit-off-thread shape observes them in <500ms
- 6 acceptance tests including a regression that fails on today's code by construction
- ~58 of the 92 block_on sites are pure local nostrdb reads (safe); only ~32 need migration
- Phases 2–4 remain: ~24 publish helpers, handle_core_delta hardening, folding 9 legacy bespoke workers into OpRunner
- Relay URLs are compile-time baked via include_str! — needs a relay_policy_json config seam for dead-network testing

## Open Tail

- Phase 2 (~24 publish helpers) in progress
- Phase 3 handle_core_delta hardening queued
- Phase 4 fold 9 legacy workers into OpRunner queued
- Watchdog bound is 2000ms to absorb parallel-suite variance; steady-state target remains <250ms

## Evidence

- transcript lines 4176-4426
- transcript lines 4487-4504
- transcript lines 4539-4551
- transcript lines 4581-4633
