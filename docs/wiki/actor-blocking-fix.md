---
title: Actor Blocking Fix
slug: actor-blocking-fix
topic: native-dependencies
summary: The deep architectural issue of ~92 blocking network awaits inside the NMP actor loop causes the actor to wedge on a dead network, preventing even the 30s timeo
tags:
  - capture
volatility: warm
confidence: medium
created: 2026-06-12
updated: 2026-06-13
verified: 2026-06-12
compiled-from: conversation
sources:
  - session:0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
  - session:847487cd-e15b-4222-85ee-4a5a2b6f590b
---

# Actor Blocking Fix

## Defect Analysis

The deep architectural issue of ~92 blocking network awaits inside the NMP actor loop causes the actor to wedge on a dead network, preventing even the 30s timeout from processing; an actor-side audit is needed but was too invasive for this session. The actor-blocking defect is in highlighter-core (app/core), not the NMP framework; NMP's doctrine D3/D8 forbids blocking the actor thread and its ADR-0040 fixed the identical bug class (V-90) inside nmp-core using a serialized worker + parked-operation pattern, and highlighter's facade violates the doctrine while already implementing the correct pattern in nine bespoke workers. The facade calls runtime.block_on() on network-dependent operations inside the single-threaded actor loop, waiting up to 360s for relay ACKs and 65s per signature, violating the framework's own D3/D8 non-blocking doctrine. The blocking surface is classified into: Class A (5 sites, 360s waits for uploads/chat-publish/room-join), Class B (~24 sites, 65s sign waits), Class C (3 sites, 4-6s bounded HTTP fetches), and Class D (~58 local-only nostrdb reads that are safe as-is). Aborted-mid-wait NMP ops leak a waiters HashMap entry in nmp_runtime.rs because the future is dropped without running cleanup; a Drop guard or stale-waiter pruning on insert would close it.

The core actor runs on a dedicated background thread (highlighter-nmp-actor); dispatch is non-blocking (try_send into a bounded channel), so the host must NOT serialize calls itself. The real cause of skipped frames/ANR is a recomposition flood: the core's emit calls onState synchronously with a full HighlighterAppState clone on every resolved op, and per-card LaunchedEffects in HomeFeedPanel don't check whether the datum is already in state, amplifying the burst.

<!-- citations: [^84748-44] [^0c7b6-101] [^0c7b6-117] [^0c7b6-130] [^0c7b6-140] [^0c7b6-159] [^0c7b6-172] [^84748-56] -->
## Recommended Fix

The fix uses a single OpRunner primitive: a shared 2-thread tokio runtime, OpDomain-keyed in-flight registry with generation-based supersession (tap refresh twice aborts the first op) and AbortHandle cancellation, a single KernelMsg::OpResolved message back to the actor, and all state mutation stays on the actor thread via apply_op_outcome. The core actor's emit_hz rate limiting was initially configured at 30 Hz but never implemented — the actor loop called emit() unconditionally on every OpResolved, causing full-state-clone emission bursts that triggered recomposition floods. It was subsequently implemented with recv_timeout-based batching and a trailing-emit guarantee that the final state is always delivered within one interval after a burst. Each migrated OpRunner domain reuses an existing busy flag (is_cover_uploading, is_creating, is_sending_chat_message, etc.) rather than adding new state fields; the only additive UniFFI change is HighlighterAppConfig.relay_policy_json: Option<String> (defaults to None) for the test seam. RequestJoinRoom submits a 30s network op with no busy flag and no pre-submit emit, violating design invariant §4.1(2); the fix is to add an is_joining snapshot field set+emitted before submit, cleared in the apply arm. clear_network_action_error on the success path does not clear is_saving; it relies on a subsequent refresh_network_settings call to do so, which is a latent fragility. The resolve_nostr_entity method was intentionally not migrated to OpRunner because it is a #[uniffi::export(async_runtime = 'tokio')] async method running on UniFFI's own tokio runtime, not the kernel actor loop, so it cannot wedge the actor and already carries a 4s timeout. The recomposition flood is fixed by (1a) sampling/conflating the collected StateFlow to coalesce bursts to ~1 recomposition per frame, and (1b) guarding each hydration dispatch on absence from LocalProfiles/LocalIsbnPreviews/LocalWebMetadata in HomeFeedPanel.kt.

<!-- citations: [^84748-45] [^0c7b6-102] [^0c7b6-118] [^0c7b6-131] [^0c7b6-173] [^84748-66] [^84748-92] [^84748-127] -->
## Phased Migration Plan

The design document lives at docs/architecture/actor-blocking-fix.md (status: Implemented, covering diagnosis, OpRunner design, migration phases, and test harness) and prescribes a phased migration with Phase 0 (instrumentation + lint), Phase 1 (top 10 wedge sites), Phase 2 (~24 publish helpers), Phase 3 (handle_core_delta hardening), Phase 4 (fold legacy workers into OpRunner). Phase 0 added per-message duration instrumentation in the actor loop with a static AtomicU64 max-duration gauge and tracing::warn above 250ms, a block_on_local wrapper that warns above 50ms, and all 75 actor-side runtime.block_on( sites were mechanically renamed to it with 9 off-actor worker-runtime sites tagged lint-allow: block_on. A CI lint script (scripts/lint-actor-blocking.sh) bans raw .block_on( outside the block_on_local wrapper and allowlist, enforced in .github/workflows/core.yml alongside cargo test and lint. Phase 1 migrated the 10 worst wedge sites (5 Class A 360s waits, 3 Class C bounded fetches, and 2 additional relay writes) onto OpRunner with 30s deadlines (6s/5s for Class C probes), deleting 9 orphaned async handler bodies. Phase 2 migrated all ~24 Class B publish/write helpers onto OpRunner with 30s deadlines, achieving zero Class-B block_on_local sites remaining — no block_on_local site awaits a sign/publish core method. The three remaining network-adjacent block_on_local calls are correct to keep inline: apply_network_connectivity_policy (non-waiting reconnect/disconnect nudges), hydrate_search_relays (in-memory snapshot read), and start_nostr_connect (builds connect URI synchronously; bunker wait is spawned off-actor). The block_on_local call at StartNostrConnect uses the uninformative trace tag 'core' instead of 'start_nostr_connect'; it should be renamed. Integration-test debt from Phase 1's additive relayPolicyJson field (broken tests/nostr_connect.rs and tests/session_nsec.rs) was fixed directly before Phase 3. Phases 3+4 convert handle_core_delta to a sync fn to make the non-blocking invariant structural, eliminate all three pre-existing clippy warnings in nmp_app.rs, and consolidate the eight remaining bespoke workers onto OpRunner with their dead KernelMsg variants deleted. The highlighter-core actor logs slow handler warnings (382ms and 1098ms for core_delta) indicating potential responsiveness issues under load.

<!-- citations: [^0c7b6-103] [^0c7b6-119] [^0c7b6-132] [^0c7b6-141] [^0c7b6-160] [^0c7b6-174] [^84748-46] -->
## Dead-Network Test Harness

A deterministic dead-network test harness is designed with a relay_policy_json config seam (HighlighterAppConfig field defaulting to None in production, replacing compile-time-baked relay URLs), a black-hole TCP listener that accepts connections but never responds, and per-test relay policy install/reset for test isolation. The actor max-handler-ms watchdog threshold is set at 2000ms to absorb parallel-suite scheduler variance, with the steady-state target remaining <250ms. Acceptance tests prove the OpRunner fix: a regression test shows inline block_on starves queued resolutions for ≥2.5s while OpRunner resolves in <500ms; liveness tests complete 10 local actions in ~0.3s while a probe is wedged off-actor. The adversarial review verdict is SHIP with zero blocking findings; the core liveness invariant (single writer, off-actor network work, truthful timeout re-entry, supersession, logout cancellation) holds under audit.

<!-- citations: [^0c7b6-104] [^0c7b6-133] [^0c7b6-142] [^0c7b6-161] [^0c7b6-175] -->
