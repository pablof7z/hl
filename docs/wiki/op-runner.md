---
title: OpRunner & Async Migration
slug: op-runner
topic: native-dependencies
summary: The OpRunner primitive uses a shared 2-thread tokio runtime, a domain-keyed in-flight registry with generation-based supersession and AbortHandle cancellation,
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
  - session:f54b4a16-dacb-41e6-b32f-b737d606254f
  - session:847487cd-e15b-4222-85ee-4a5a2b6f590b
---

# OpRunner & Async Migration

## Architecture

The OpRunner primitive uses a shared 2-thread tokio runtime, a domain-keyed in-flight registry with generation-based supersession and AbortHandle cancellation, a single KernelMsg::OpResolved variant back to the actor, and all state mutation stays on the actor thread via apply_op_outcome. Logout and Stop both call ops.cancel_all() to abort in-flight operations and bump every generation so late resolutions are dropped by is_stale. Entity-resolve (resolve_nostr_entity) was intentionally not migrated to OpRunner because it is a UniFFI async method running on its own tokio runtime, not the kernel actor loop, and cannot wedge the single-writer loop. The nsec login spawn_blocking side effects cannot be cancelled; a logout interleaved with an in-flight nsec login could let login_nsec install a signer into core after core.logout(), though the staleness gate prevents the resolution from writing snapshot state. A relay_policy_json config seam (HighlighterAppConfig field defaulting to None) allows per-test relay policy injection; production keeps the existing OnceLock, tests use AtomicPtr<RelayPolicy> with leaked-box static lifetime. The dead-network test harness uses spawn_black_hole_listener binding 127.0.0.1:0 that accepts connections but never responds, and an account-creation regression test that fails on today's code by construction. Validation-before-busy-flag ordering is used for publish helpers so synchronous input validation precedes the busy flag, avoiding a UI flash. OpRunner submissions have domain-specific deadlines: 30s for Class A/B operations (uploads, publishes, sign-ups), 4–6s for Class C HTTP fetches (relay probe, relay import). The core emit stream is rate-limited to 30 Hz via a recv_timeout-based loop in spawn_actor, with a trailing-emit guarantee ensuring the latest state is always delivered within one interval after a burst. syncNetworkCallback in HighlighterViewModel.onState is only called when state.network.wifiOnlyEnabled actually changes from the last known value, preventing hundreds of spurious OS calls per second during emit bursts.

<!-- citations: [^84748-90] [^0c7b6-167] [^0c7b6-151] [^0c7b6-152] [^0c7b6-153] [^0c7b6-154] [^0c7b6-166] [^0c7b6-188] [^84748-124] -->
## Domain Keying

Login nsec sign-in and bunker sign-in share a single OpDomain::Auth slot, so a nsec→bunker supersession correctly aborts the stale in-flight resolution. RelayProbe uses a stable hash of the relay URL as its OpDomain key, so different relays probe independently while re-probing the same URL still supersedes. CommentInteraction and ArticleBookmarkToggle use per-target hashed OpDomain keys so rapid double-taps supersede on the same target but don't cross-abort different targets. CommentInteraction and ArticleBookmarkToggle submit without a busy flag under an intentional optimistic-UI carve-out: success re-hydrates, failure toasts, and no busy concept exists by design. NetworkRelayWrite uses a single OpDomain slot for all four relay-list writes (upsert, remove, set-roles, import-apply) since they mutate one list; supersession serializes them and the apply-arm refresh reconciles.

<!-- citations: [^0c7b6-155] [^0c7b6-156] [^0c7b6-168] [^0c7b6-189] [^f54b4-24] -->
## Optimistic Revert

FollowToggle uses a single unkeyed slot with a self-timeout that carries the real revert state (previous_following) so optimistic revert is correct even on timeout, since the generic op_timed_out fallback cannot know previous_following.

<!-- citations: [^0c7b6-157] [^0c7b6-169] [^0c7b6-190] -->

## Busy Flag Policy

SetAddressInCurationSet and CreateCurationSetAndAdd submit Class-B publishes with no busy flag and no success-path pre-submit emit; a curation-write busy flag should be set+emitted before submit. <!-- [^0c7b6-191] -->

## InFlightOp.started Field

The InFlightOp.started field is set on every submit but never read; it should be removed or wired to an in-flight stall watchdog. <!-- [^0c7b6-192] -->
