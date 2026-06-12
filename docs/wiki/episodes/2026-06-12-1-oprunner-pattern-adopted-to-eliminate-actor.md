---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: architecture
status: superseded
subjects:
  - nmp-actor-blocking
  - oprunner-pattern
  - nmp-app-facade
supersedes:
  - 2026-06-12-1-nmp-facade-actor-thread-blocks-on
  - 2026-06-12-1-nmp-actor-thread-blocking-defect-diagnosed
  - 2026-06-12-3-nmp-actor-thread-blocks-on-network
related_claims: []
source_lines:
  - 4469-4538
  - 4553-4634
  - 4658-4741
  - 4840-4860
captured_at: 2026-06-12T16:31:21Z
---

# Episode: OpRunner pattern adopted to eliminate actor-thread blocking

## Prior State

The NMP facade actor loop called runtime.block_on() on network-dependent operations (~92 sites), which could wedge the single actor thread for up to 360 seconds on a dead network. On-device, account creation stuck at "Creating…" forever — the actor was blocked in another handler and couldn't dequeue the resolution message, even after the timeout fired.

## Trigger

On-device testing revealed the "Creating…" hang persisted past the 30s worker timeout; root-cause research classified 92 block_on sites into Class A (5 sites, 360s relay-ACK waits for uploads/chat-publish/room-join), Class B (~24 sites, 65s NIP-46 signer waits), Class C (3 bounded HTTP fetches), and Class D (~58 safe local reads). NMP framework's own ADR-0040 had already fixed the identical bug class (V-90) inside nmp-core using a serialized worker + parked-operation pattern.

## Decision

Adopt OpRunner primitive: a shared 2-thread tokio runtime, domain-keyed (OpDomain) in-flight registry with generation-based supersession and AbortHandle cancellation, a single KernelMsg::OpResolved message back to the actor, and all state mutation staying on the actor thread. All Class A (5 sites) and Class B (~26 sites with extras) migrated off-actor. ~58 local-only block_on sites wrapped in block_on_local with instrumentation. CI lint (lint-actor-blocking.sh) bans raw block_on in production code. Dead-network test harness with configurable relay_policy_json seam and black-hole TCP listener.

## Consequences

- Empirically proven: old inline-block_on shape starves queued resolutions ≥2.5s; new submit-off-thread shape observes them in <500ms
- 6 acceptance tests including a by-construction regression test that fails on the old shape
- Generation-based supersession: double-tap on same target aborts prior op; different targets are independent slots
- OpDomain keying preserves existing single-slot semantics (e.g. one RoomChatPublish slot, per-target ArticleBookmarkToggle)
- Timeout fallback surfaces D6 toast messages on both platforms
- Zero new state fields — all migrated domains reuse existing busy flags
- Phased migration: Phase 0 (instrumentation + lint), Phase 1 (top-10 wedges + harness), Phase 2 (all publish helpers), Phase 3 (handle_core_delta hardening), Phase 4 (fold 8 legacy workers into OpRunner)

## Open Tail

- Phase 3 (handle_core_delta hardening) and Phase 4 (consolidate legacy workers) still in progress
- Pre-existing moving flake in subscribe_joined_communities test (not introduced by OpRunner)
- NMP framework could add a debug assertion or callback-style API to prevent future apps from repeating the block_on-in-actor mistake

## Evidence

- transcript lines 4469-4538
- transcript lines 4553-4634
- transcript lines 4658-4741
- transcript lines 4840-4860
