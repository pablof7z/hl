---
type: research-record
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
source_lines: 5602-5672
agent_attribution: Adversarial review of OpRunner work subagent
has_preregistered_criteria: true
has_method: true
has_structured_report: true
characterization: "Adversarial review of actor-blocking fix: verdict SHIP with two should-fix UX gaps (JoinRoom and CurationWrite missing busy flags), systematic 9-dimension code audit with line-number citations"
captured_at: 2026-06-12T18:06:03Z
---

Adversarial review of actor-blocking fix: verdict SHIP with two should-fix UX gaps (JoinRoom and CurationWrite missing busy flags), systematic 9-dimension code audit with line-number citations

---

User: [Agent task result: Agent "Adversarial review of OpRunner work" completed]
`CurationWrite` (4433) also submits without a busy flag and without an emit on the success-path-to-submit (it only emits on validation error). No "adding to collection…" busy indicator during the up-to-30s op. Same gap class as JoinRoom — a slow network op with no busy feedback. The apply arm shows a success/error toast at the end. Less severe than JoinRoom (curation is lower-frequency) but same invariant-(ii) deviation.

So invariant (ii) is upheld for the majority but violated for: **JoinRoom, ArticleBookmarkToggle, CommentInteraction (Like+Bookmark), CurationWrite**. Of these, ArticleBookmark/Comment are defensible as optimistic-UI (no busy by design, §4.3). JoinRoom and CurationWrite are slow Class-A/B ops with no optimistic update and no busy flag — genuine gaps.

I have completed all nine dimensions. Let me compile the findings report.

---

# Adversarial Review — Actor-Blocking Fix (Highlighter Rust Core)

**Verdict: SHIP** — with two should-fix UX gaps logged. No blockers. The core liveness invariant (single writer, off-actor network work, truthful timeout re-entry, supersession, logout cancellation) holds under audit, the lint passes, the design doc is updated, and the acceptance tests are load-bearing. The defects found are bounded UX/observability gaps and minor debt, none of which reintroduce actor blocking or data corruption.

## What I verified holds (with citations)

**Dimension 1 — Design conformance.**
- (i) All 46 `ops.submit_op(` futures (`nmp_app.rs`, counted exactly 46 in production) capture only `Arc<HighlighterCore>` + owned data. I scanned every `async move` block; the apparent `state.write`/`emit` hits were all in the *trailing* handler arm after the closure, never inside it. Confirmed clean at the helper-fn sites too (`nmp_app.rs:7092,7299,7576,10899`).
- (iii) Deadlines correct: `OP_DEADLINE_NETWORK = 30s` (`nmp_app.rs:2392`), `OP_DEADLINE_RELAY_PROBE = 6s` (2394), `OP_DEADLINE_RELAY_IMPORT = 5s` (2395). RelayProbe/Import sites use the bounded ones (5273, 5307); everything else 30s.
- (iv) `block_on_local` wrapper (`nmp_app.rs:2036`) is the only blocking primitive; 80 call sites, all Class-D. Spot-audited 11 (refresh_comments, hydrate_app_chrome, refresh_network_settings, refresh_profile_view, refresh_room_detail, hydrate_profile, load_book_picker_recents, apply_network_connectivity_policy, ensure_signed_in_app_scope, refresh_feedback_threads, StartNostrConnect@4249) — all ndb-read/sync-registration. `start_nostr_connect` (`client.rs:161-174`) spawns the pairing wait rather than awaiting it, so it is genuinely local.
- (v) `apply_op_outcome` (`nmp_app.rs:2595`) is the sole off-actor state-mutation point; the `OpResolved` arm (2539-2550) gates on `is_stale` then calls it.
- (vi) Logout (`nmp_app.rs:4280-4296`) calls `ops.cancel_all()` (2495), which aborts all in-flight and bumps every generation; late resolutions dropped by `is_stale` (2440). `Stop` also calls `cancel_all` (2570).

**Dimension 2 — Timeout-outcome correctness (the post-hoc fix).** Audited all 46 sites. The deleted `op_timed_out` gen-0/empty-payload helper has zero survivors. Every timeout outcome carries the live generation and real keys: Auth (3388/3438 carry generation+nsec/uri+persist/clear flags), JoinRoom (4536 real group_id/room_name), FollowToggle (4783 truthful desired/previous revert), RelayProbe (5273 real url + `document:None`), SearchLocal/UsernameCheck/AccountCreate/Isbn/WebMetadata all carry live generation+real requested string. The `Box::new(Err(op_timeout_message(...)))` sites (7297, 8725, 8795) preserve the boxed payload shape. The FollowToggle inner redundant `tokio::time::timeout` was correctly removed (comment 4783-4787).

**Dimension 4 — Resolution races / dual counters.** The legacy counters (`auth_generation`, `search_runtime.generation`, `username_generation`, `create_generation`) and OpRunner per-domain generations are lock-step: each legacy counter is bumped exactly once per submission on the actor thread, immediately before/inside the paired `submit_op`. No bump-without-submit: `prepare_create_account_request` (`nmp_app.rs:7261`) bumps create_generation only after all validation early-returns (7233/7241/7256); `update_create_account_username` (7004) bumps only when it returns `Some` (the same value passed to `submit_username_availability_op`). The double-gate (OpRunner `is_stale` + legacy `generation != runtimes.auth_generation` at 7151/7192) is redundant but safe — OpRunner's counter is always ≥ the legacy one after a `cancel_all`, so it is strictly stricter and cannot let a stale resolution through.

**Dimension 3 — Supersession/aborts.** Hashed-target domains (`op_target_hash`, `nmp_app.rs:2157`, SipHash) collision is ~2^-64 and benign: IsbnPreview/WebMetadata apply arms re-check the real `requested` string (`nmp_app.rs:8740+`, `remove(&requested)`), and the retained `pending_isbn_lookups`/`pending_web_metadata` sets guard submit-time. CommentInteraction collision at worst drops one toggle and refreshes all comments. Auth shared slot (nsec+bunker → `OpDomain::Auth`) correctly preserves legacy cross-method supersession (3363-3368, 3428-3429).

**Dimension 7 — Actor loop.** `ACTOR_MAX_HANDLER_MS` gauge (1990) + `record_handler_duration` (2001) is a lock-free monotonic max, warns >250ms. `OpResolved` handler never network-blocks (apply arms use `block_on_local` for ndb refreshes only). `handle_core_delta` is now a sync `fn` (5458), Phase 3 complete.

**Dimension 8 — Tests are load-bearing.** `auth_supersession_nsec_then_bunker_drops_stale_resolution` (13618) uses the credential-persist callback as a clean canary for "stale resolution dropped" and documents what it deliberately doesn't assert and why. `join_room_timeout_resolves_with_live_payload` (13693) waits the full 30s deadline and asserts the Err arm fires — exactly the post-hoc regression. `acceptance_account_creation_regression` (12994), `acceptance_supersession_drops_first_generation` (13046), `acceptance_logout_cancels_in_flight` (13103) all drive the real actor against a real black-hole TCP listener and assert observable state. None assert implementation details tightly enough to break on legitimate refactors. The `regression_inline_block_on_starves_queued_resolution` (13192) proves the mechanism by construction with a bounded wedge.

**Dimension 9 — Residual debt.** Zero TODO/FIXME/HACK introduced. Design doc Status is **"Implemented"** (`docs/architecture/actor-blocking-fix.md:3`) — the prompt's premise that it still says "Proposed" is stale; nothing to flag there. Lint passes on the current tree. The `OpOutcome` match in `apply_op_outcome` is exhaustive (no wildcard arms); every variant is constructed ≥3×.

## Findings

### Should-fix

**1. `RequestJoinRoom` submits a 30s Class-A network op with no busy flag and no pre-submit emit — violates design invariant §4.1(2).**
`nmp_app.rs:4513-4547`. The success path sets no joining indicator and does not `emit` before `submit_op`. `pending_joins` is runtime-internal and only populated in the apply arm *on success* (`nmp_app.rs:2613-2617`). Between tap and resolution (up to 30s on a slow network) the UI shows nothing; then the room appears or an error toast fires. The design §4.6 specified an acceptance test asserting "the join busy-flag is set" — that assertion was never implemented (`join_room_timeout_resolves_with_live_payload` asserts liveness + eventual error, not a busy flag), so the gap is unguarded. Fix: add an `is_joining` snapshot field set+emitted before submit, cleared in the apply arm; assert it in the join test.

**2. `SetAddressInCurationSet` / `CreateCurationSetAndAdd` submit a Class-B publish with no busy flag and no success-path pre-submit emit.**
`nmp_app.rs:4432-4443` and 4475. Same class as JoinRoom (slow op, no optimistic update, no "adding…" feedback). Lower frequency than join but the same invariant-(ii) deviation. Fix: add a curation-write busy flag set+emitted before submit.

### Nit

**3. Dead field `InFlightOp.started`.** `nmp_app.rs:2399-2400` — set on every submit (2487) but never read (`grep '.started'` returns nothing). Explicitly `#[allow(dead_code)]`. The design spec included it presumably for an in-flight stall watchdog that was never built. Remove the field (and the `Instant::now()` call) or wire the watchdog.

**4. Useless trace tag `"core"` at the StartNostrConnect `block_on_local`.** `nmp_app.rs:4249` — every other of the 80 sites has a descriptive tag (e.g. `"refresh_network_settings"`); this one says `"core"`, which is uninformative in a slow-handler warning. Rename to `"start_nostr_connect"`.

### Observations (no action required, but logged for "no debt anywhere")

**5. Aborted-mid-wait NMP ops leak a `waiters` HashMap entry.** `nmp_runtime.rs:342-352`. When OpRunner aborts an op parked in `wait_for` (`tokio::time::timeout(timeout, rx).await`, line 346), the future is dropped without running the cleanup at 352, so the `waiters` entry persists until a matching row arrives (302) or the process exits. Bounded per user action (not a runaway), and the `completed` cache is LRU-capped (`NMP_ACTION_RESULT_CACHE_LIMIT`, 311), so this is not unbounded accumulation. But it is not zero-leak. A `Drop` guard on the waiter, or pruning stale waiters on insert, would close it.

**6. nsec login `spawn_blocking` side effects cannot be cancelled.** `nmp_app.rs:3403-3409`. The design (§4.2) acknowledges abort only abandons *waiting*, not side effects. The narrow race: a logout interleaved with an in-flight nsec login could let the blocking `login_nsec` install a signer into core *after* `core.logout()`. The synchronous `apply_nsec_identity` (3377) and the staleness gate prevent the *resolution* from writing snapshot state, but the core-level signer install is outside actor control. Pre-existing behavior class, not introduced here; the `auth_supersession` test deliberately documents (13612-13616) that core-level sign-in is independent of facade supersession by design.

**7. `clear_network_action_error` (success path) does not clear `is_saving`.** `nmp_app.rs:6287-6291`. Harmless today only because `apply_op_outcome` unconditionally runs `refresh_network_settings` afterward (`nmp_app.rs:3089`), which sets `is_saving = false` (6193). Latent fragility: if that refresh were ever made conditional, `is_saving` would leak on a successful relay write. Consider clearing it in the success arm directly.

**8. Lint bypass surface.** `scripts/lint-actor-blocking.sh` is line-based substring matching on `.block_on(`. It would catch `Handle::block_on` (contains `.block_on(`), but a multi-line-formatted `runtime\n.block_on(` would slip past, and the wrapper-allow is an exact-string match (`let out = runtime.block_on(fut);`) that breaks if the wrapper is reformatted (fails closed — acceptable). A `rustfmt`-normalized or AST-based check (or `clippy` lint) would be more robust. Low priority given the wrapper discipline + CI gate already in place (`.github/workflows/core.yml:48-50`).

**9. `CommentInteraction` and `ArticleBookmarkToggle` submit without a busy flag** (`nmp_app.rs:3783+`, 4312+) — unlike JoinRoom/CurationWrite, these are defensible under the §4.3 optimistic-UI exemption (no busy concept by design; success re-hydrates, failure toasts). Logged only to note the invariant-(ii) wording ("every submitting handler sets a busy flag") is not literally universal — it has an intentional optimistic-update carve-out that the design text doesn't explicitly enumerate.

## Relevant files
- `/Users/pablofernandez/Work/hl/app/core/src/nmp_app.rs` — OpRunner primitive (2404-2503), `submit_op` (2450), `apply_op_outcome` (2595), the 46 submit sites, findings #1-#4, #7, #9.
- `/Users/pablofernandez/Work/hl/app/core/src/nmp_runtime.rs` — `NmpActionResultsState` (287-359), finding #5.
- `/Users/pablofernandez/Work/hl/app/core/src/relays.rs` — test seam (`set_relay_policy_override`, 55-121); clean, no findings.
- `/Users/pablofernandez/Work/hl/scripts/lint-actor-blocking.sh` — finding #8.
- `/Users/pablofernandez/Work/hl/.github/workflows/core.yml` — CI gate wiring (correct); note the known-flake retry at 61-65 is documented, not introduced by this work.
- `/Users/pablofernandez/Work/hl/docs/architecture/actor-blocking-fix.md` — Status correctly updated to "Implemented".

Blocking list for FIX-FIRST: **none.** The two should-fix items (#1 JoinRoom, #2 CurationWrite busy flags) are UX-feedback gaps, not liveness or correctness regressions — they can ship and be fast-followed, but if "no technical debt anywhere" is enforced literally as a merge gate, #1 should be closed first since it's a user-visible 30s dead-spot on a primary action with a silently-dropped acceptance assertion.
