# Design: Eliminating Actor-Thread Blocking in the Highlighter NMP Facade

**Status:** Implemented (all four phases + caller-supplied timeout outcomes; see Implementation notes below)
**Date:** 2026-06-12 (designed and implemented)
**Defect:** Single kernel actor thread in `app/core/src/nmp_app.rs` wedges on network-dependent `runtime.block_on(...)` calls; on a dead network the UI freezes in intermediate states (confirmed on-device: account creation stuck at "Creating…" forever even though the 30 s worker-side timeout fired — the actor could not dequeue `AccountCreateResolved` because it was blocked in another handler).
**Scope of change:** `app/core/src/nmp_app.rs` (~12,004 lines, 92 `block_on` sites), plus a small test seam in `app/core/src/relays.rs`. No changes to the embedded `nostr-multi-platform` framework are required.

---

## 1. The blocking surface, mapped

### 1.1 Actor structure (as shipped)

- Actor thread `highlighter-nmp-actor` spawned at `app/core/src/nmp_app.rs:1944-1947`; it builds a **current-thread** tokio runtime (`nmp_app.rs:1948-1952`) and drains a `Receiver<KernelMsg>` with blocking `rx.recv()` (`nmp_app.rs:1954`).
- `KernelMsg` (`nmp_app.rs:1837-1884`): `Action`, `CoreDelta`, nine `*Resolved` variants (Isbn, WebMetadata, UsernameAvailability, NsecSignIn, BunkerSignIn, AccountCreate, OnboardingFollows, DefaultBlossomInit, SearchLocal), and `Stop`.
- `Action` → `handle_action` (sync, contains the big match with most `block_on` sites, `nmp_app.rs:1958`); `CoreDelta` → `runtime.block_on(handle_core_delta(...))` (`nmp_app.rs:1961`); `*Resolved` arms are sync and never block (`nmp_app.rs:1963-2074`).
- State is `Arc<parking_lot::RwLock<HighlighterAppState>>`; handlers `state.write()` then `bump()` a revision; `emit()` (`nmp_app.rs:~10093`) clones the state under a read lock and calls `reconciler.on_state(...)`. **Every blocked handler therefore also blocks all snapshot emission** — the loop is the only emitter.

The single-message-at-a-time loop is the right shape (it is exactly nmp-core's single-writer actor). The defect is *what happens inside one message*.

### 1.2 Classification of the 92 `block_on` sites

The classification below was verified by reading the helper bodies down into legacy core facade (`app/core/src/client.rs`) and the runtime layers (`app/core/src/nmp_runtime.rs`, `app/core/src/nostr_runtime.rs`). An important correction to naive triage: **the majority of `get_*` core methods are pure legacy app event store reads, not relay queries** — e.g. `get_user_profile` (`client.rs:473-478`), `get_article` (`client.rs:506`), `get_comments_for_reference` (`client.rs:542-554`), `get_follows` (`client.rs:1514-1517`), `get_joined_communities` (`client.rs:423-428`), `get_following_highlights` (`client.rs:459-466`), `get_feedback_threads` (`client.rs:1062-1070`), `get_featured_rooms` (`client.rs:1318-1323`), `get_relay_diagnostics` / `get_auto_connected_relays` (`client.rs:1683-1710`, in-memory snapshot). Likewise the `subscribe_*` methods are synchronous interest registration (`client.rs:236-243`) and `disconnect_all`/`reconnect_all` are non-waiting nudges (`client.rs:1723-1732`). Network delivery is push-based: deltas arrive later via the event callback.

That leaves four genuinely distinct classes:

#### Class A — NMP protocol-action waits, **360 s** worst case (the killers)

`legacy runtime::dispatch_nmp_action_for_result` → `nmp_runtime.rs:774` waits on a correlation row with `NMP_PROTOCOL_ACTION_TIMEOUT = Duration::from_secs(360)` (`nmp_runtime.rs:56`). On a dead network the relay ack never arrives and the actor wedges for **six minutes per message**. In-actor `block_on` sites:

| Helper | Core path | block_on sites (nmp_app.rs) | Trigger |
|---|---|---|---|
| `upload_create_room_cover` | `core.upload_photo` → `blossom.rs:247` | 2288 | CreateRoom cover pick |
| `upload_edit_profile_image` | same | 2656 | Edit-profile avatar pick |
| `upload_capture_photo` | same | 2976 | Capture flow photo |
| `publish_room_chat_message` | `chat.rs:101` (`nmp.nip29.post_chat_message`) | 3264 | Sending a chat message |
| `request_join_room` | `groups.rs:220` (`nmp.nip29.join`) | 2911 | Tapping Join |

#### Class B — NMP sign waits, **65 s** worst case per sign, on every publish helper

Every event publish signs through `NmpNostrSigner::sign_event` (`nmp_runtime.rs:377-390`) → `sign_unsigned_event`, which awaits the NMP kernel with `NMP_SIGN_TIMEOUT = Duration::from_secs(65)` (`nmp_runtime.rs:54`, wait at `:651`). The subsequent publish itself is fire-and-forget (`dispatch_publish`, `nmp_runtime.rs:787+`). Severity is signer-dependent: a local key resolves in microseconds; a **NIP-46 bunker signer on a bad network stalls the actor up to 65 s per signature** — the exact V-90 bug class nmp-core fixed for itself in ADR-0040. Multi-sign helpers compound it (`groups::create_room` signs 2+ events sequentially, `groups.rs:278-330`).

In-actor sites in this class (~24): `submit_create_room` (2317), `mint_room_invite_link` (2355, 2405), `submit_room_invite_members` (2409), `publish_comment_from_draft` (2455), `toggle_comment_like` (2463), `toggle_comment_bookmark` (2467), `publish_feedback_note_from_state` (2508, 2547), `persist_media_settings` (2579, 2587, 2598), `submit_edit_profile` (2675), `toggle_article_bookmark` (2767), `set_address_in_curation_set` (2866), `create_curation_set_and_add` (2878), `toggle_profile_follow` (3087), `publish_article_highlight` (3142), `publish_artifact_share` (3160), `publish_url_share` (3170), `share_highlight_repost` (3181), `publish_room_discussion` (3231), `publish_capture_highlight` (2992), `publish_capture_picture` (3009), `publish_clip_highlight` (3026), and the relay-list writes in `upsert_network_relay` / `apply_network_import_relays` (3341, 3390).

#### Class C — bounded in-actor HTTP / fetch-and-wait, 4–6 s each (serial accumulation)

- `probe_network_relay_nip11` — `NIP11_PROBE_TIMEOUT = 6 s` (`relay_polish.rs:16`), site 3370.
- `fetch_network_import_relays` — `IMPORT_FETCH_TIMEOUT = 5 s` (`relay_polish.rs:17`, `:136-141`), site 3380.
- Nostr-entity resolution — `open_nmp_filter_once_and_wait` with `NOSTR_ENTITY_FETCH_TIMEOUT = 4 s` (`nostr_entities.rs:27`, called at `client.rs:1494-1499`); reachable from paste/clip/share resolution paths.

Individually survivable, but the loop is serial: five queued messages that each stall 5 s freeze the UI for 25 s with zero snapshots.

#### Class D — local-only (the remaining ~58 sites: keep as-is)

`hydrate_app_chrome`, `ensure_signed_in_app_scope`, `handle_app_foregrounded` (verified: `disconnect_all` is non-waiting, `client.rs:1729-1732`; `hydrate_joined_communities` is ndb-only, `nmp_app.rs:7496-7533`), `refresh_home_feed`, `refresh_room_detail`, `refresh_profile_view`, `refresh_article_reader`, `refresh_comments`, `refresh_feedback_*`, `refresh_bookmarks_library`, `refresh_room_explorer_*`, `refresh_network_settings`, `load_book_picker_recents`, etc. — these compose ndb reads + sync subscription registration. They are correct to run inline; converting them would churn snapshot ordering for no benefit.

### 1.3 Startup / lifecycle exposure

`Bootstrap` (`nmp_app.rs:2135-2152`), `RefreshAppChrome` (2153-2167) and `AppForegrounded` (2168-2182) are Class D plus a *spawned* blossom-init worker (`nmp_app.rs:4985-5005`) — they are not themselves the six-minute wedges. The unconditional hazards on common paths are: any queued Class A/B action ahead of a resolution (the reproduced account-creation freeze — `AccountCreateResolved` starves behind a blocked handler), and `handle_core_delta` cascades (`nmp_app.rs:3464+`) which run under `block_on` at 1961 and can re-enter refresh helpers while further deltas queue behind them. Phase 0 instrumentation (below) pins the exact on-device culprit; the design removes the entire class regardless.

### 1.4 The facade already contains the correct pattern, nine times

`nmp_app.rs` ships nine spawn-and-message-back workers: local search (spawn 3701), username availability (5212), nsec sign-in (5287), bunker sign-in (5318), **account creation (5488, with the only explicit deadline — 30 s at 5500-5512)**, default-blossom init (4985), ISBN preview (7361), web metadata (7440), onboarding follow publish (10018). Each mints a generation (e.g. `auth_generation` at 2191-2192), spawns a thread with its own current-thread runtime, sends a `KernelMsg::*Resolved` back, and the actor-side handler drops stale resolutions by comparing generations (e.g. 5355, 3789, 10054). The fix below is this pattern, generalized and de-duplicated — not a new architecture.

---

## 2. What the framework intends (nostr-multi-platform)

The embedded framework is explicit that **an actor must never block**, and it has already ratified the exact remediation:

- **Doctrine D3/D8** (`docs/builder-guide/03-doctrine-d0-d8.md`, canonical text `docs/product-spec/doctrine.md`): D8 forbids polling/stalls at any layer; the single-writer actor advances only by dequeuing commands. Substrate trait docs repeat the contract verbatim: tick observers "MUST be cheap and non-blocking (D8: no I/O, no mutex waits…)" (`docs/builder-guide/05a-substrate-traits.md:114`).
- **ADR-0024 — Async capability protocol** (`docs/decisions/0024-async-capability-protocol.md`): "Blocking the actor for seconds violates the single-actor invariant (D3) — while it waits, no other ActorCommand runs, no snapshot tick emits." Decision: two-phase protocol — fire-and-forget dispatch with an executor-minted `correlation_id`, completion re-enters the actor as a command (`ActorCommand::CapabilityResultReady`). Explicitly rejected alternatives: "keep dispatch synchronous" and "thread pool holding result state outside the actor" (the actor must remain the single owner of progress).
- **ADR-0040 — Capability-worker seam** (`docs/decisions/0040-capability-worker-seam.md`, Accepted 2026-05-31): closed V-90, *the same bug class as this defect* — in-actor `op.wait(...)` stalls of up to ~24 s. It ratifies the **serialized worker thread that feeds results back into the actor**, and documents the canonical non-blocking sign pattern: a pending remote sign is *parked* (`crates/nmp-core/src/actor/pending_sign.rs`), polled with non-blocking `try_recv` each idle tick, and timed out after `PENDING_SIGN_TIMEOUT` (5 s) into a D6 toast.
- **ADR-0031** (signer-broker): the precedent for *worker-feeds-actor* — a worker re-enters via `ActorCommand`, "never blocking or mutating kernel state from the worker."
- **ADR-0028** (actor-liveness probe): actor stalls are treated as observable defects with dedicated FFI probing.
- **D6** (`03-doctrine-d0-d8.md`, row D6): failures surface as toast state + busy flags, never as exceptions across FFI.

**Verdict:** the framework already provides and mandates the right primitive — *off-actor work + correlation/generation-tagged re-entry message + deadline at the worker + D6 error surfacing*. Highlighter's facade violates it in ~34 network-class sites; its own nine workers prove the pattern fits this codebase. (Re-platforming legacy core facade onto nmp-core's `ActionModule` registry would be the maximal interpretation of "use the framework primitive," but legacy core facade is a parallel nostr-sdk-based runtime — that is a re-architecture, not a fix, and is out of scope here.)

---

## 3. Strategy evaluation

| Strategy | Correctness | Risk across 92 sites | Incremental? | Testability | Shell-visible change |
|---|---|---|---|---|---|
| **(a) Spawn-and-message-back everywhere** | Good — state mutated only in actor | High churn: ~58 local sites converted needlessly; every action becomes two-phase; snapshot ordering changes everywhere | Poor (all-or-nothing semantics shift) | 248-test suite largely assumes synchronous handler completion | Large |
| **(b) Per-`block_on` deadlines** | Unchanged | Low per-site, but proven insufficient: the 30 s account-create timeout already failed because *other* messages stall the loop; serial stalls remain (n × deadline); violates D3/D8 | Yes | Easy | Small |
| **(c) Multi-thread runtime, handlers as concurrent `async fn`** | **Bad** — handlers assume exclusive `state.write()` between emit points (read-modify-write sequences; `pending_joins` and the per-panel `ActorRuntimes` are `&mut`); concurrent scheduling breaks the single-writer invariant (D4) and reorders emits | Highest | No | Hardest (races) | Unpredictable |
| **(d) Hybrid: classify; local stays sync, network ops become registered off-actor operations** | Good — identical to the nine shipped workers; actor remains single writer | Bounded: ~34 sites, mechanical after plumbing | **Yes — top wedges first** | Matches existing `*Resolved` test patterns | Localized, per-domain busy flags |

**Recommendation: (d)**, with the off-actor mechanics standardized into one small primitive instead of nine bespoke copies. This is also exactly what ADR-0040 chose for nmp-core itself ("serialized worker — not per-site sagas") and what ADR-0024 prescribes for slow capabilities (correlation-ID re-entry). Strategy (b) is retained *inside* (d) as defense-in-depth: every off-actor operation gets a deadline, enforced in the worker where it can actually fire.

---

## 4. Design

### 4.1 The `OpRunner` primitive

One shared worker runtime owned by the actor context, replacing per-request `thread::Builder + new_current_thread runtime` boilerplate (e.g. `nmp_app.rs:5488-5494`):

```rust
/// Off-actor operation runner. Lives in ActorContext.
struct OpRunner {
    /// 2-worker multi-thread runtime: enough parallelism that a stuck
    /// upload cannot starve a publish; bounded so we never build a
    /// thread-per-tap zoo. Workers never touch HighlighterAppState.
    runtime: tokio::runtime::Runtime,           // Builder::new_multi_thread().worker_threads(2)
    actor_tx: Sender<KernelMsg>,
    in_flight: HashMap<OpDomain, InFlightOp>,   // owned by the actor thread
}

struct InFlightOp {
    generation: u64,
    abort: tokio::task::AbortHandle,
    started: Instant,
}

/// Domain key — one slot per UI surface so a second request supersedes the first.
#[derive(Hash, Eq, PartialEq, Clone, Copy)]
enum OpDomain {
    JoinRoom, RoomChatPublish, BlossomUpload(UploadSlot),  // RoomCover | ProfileImage | Capture
    CommentPublish, FeedbackPublish, HighlightPublish, SharePublish,
    FollowToggle, BookmarkToggle, CurationWrite, MediaSettingsWrite,
    ProfileEditSubmit, RoomCreate, RoomInvite, RelayProbe, RelayImport,
    EntityResolve, NetworkRelayWrite, /* extend per phase */
}
```

One new `KernelMsg` variant covers all migrated operations (the nine legacy variants can fold into it over time):

```rust
KernelMsg::OpResolved {
    domain: OpDomain,
    generation: u64,
    outcome: Box<OpOutcome>,   // enum: per-domain typed payload, mirrors today's per-worker Result payloads
}
```

Submission, on the actor thread:

```rust
fn submit_op<F>(ops: &mut OpRunner, domain: OpDomain, deadline: Duration, fut: F)
where F: Future<Output = OpOutcome> + Send + 'static {
    let generation = ops.bump_generation(domain);          // supersession
    if let Some(prev) = ops.in_flight.get(&domain) { prev.abort.abort(); }
    let tx = ops.actor_tx.clone();
    let handle = ops.runtime.spawn(async move {
        let outcome = match tokio::time::timeout(deadline, fut).await {
            Ok(o) => o,
            Err(_) => OpOutcome::timed_out(domain),         // D6 message, e.g. account-create's existing copy
        };
        let _ = tx.send(KernelMsg::OpResolved { domain, generation, outcome: Box::new(outcome) });
    });
    ops.in_flight.insert(domain, InFlightOp { generation, abort: handle.abort_handle(), started: Instant::now() });
}
```

Actor-side resolution (single writer preserved — this is the only place state mutates):

```rust
KernelMsg::OpResolved { domain, generation, outcome } => {
    if runtimes.ops.is_stale(domain, generation) { /* superseded or logged out: drop */ }
    else { apply_op_outcome(&ctx, &mut runtimes, domain, *outcome); emit(&ctx.state, &ctx.reconciler); }
}
```

**Invariants:**
1. Workers receive `Arc<LegacyCoreFacade>` and input data only — never `state`, never `ActorRuntimes`. All snapshot mutation happens in `apply_op_outcome` on the actor thread, exactly like today's nine `*Resolved` handlers.
2. Every handler that submits an op sets the domain's busy flag and emits *before* returning (the existing `set_signing_in` / `set_bootstrapping` pattern, `nmp_app.rs:2136-2137, 2188-2189`). Carve-out: optimistic-update toggles (comment like/bookmark, article bookmark) intentionally have no busy flag — the optimistic state IS the feedback; failure reverts + toasts (§4.3).
3. Deadlines per class: Class A ops 30 s (not 360 — the UI cannot wait six minutes; the underlying `dispatch_action_for_result` future is simply abandoned at the worker deadline and its eventual row discarded), Class B publishes 30 s, Class C probes keep their existing 4–6 s.
4. `block_on` survives only for Class D, renamed through a thin wrapper `block_on_local(...)` carrying a duration guard (log/debug-assert > 50 ms), so the lint in Phase 0 can ban naked `runtime.block_on` in this file.

### 4.2 Cancellation and supersession

- **User taps refresh twice / retypes a query:** the second `submit_op` for the same `OpDomain` bumps the generation and aborts the previous task. Even if abort races completion, the stale `OpResolved` carries the old generation and is dropped — the same discipline as `auth_generation` (`nmp_app.rs:2191-2192`, checked at 5355) and search (3789).
- **Logout mid-flight:** the existing logout arm clears `pending_joins` (`nmp_app.rs:2749`); it additionally calls `ops.cancel_all()` — abort every in-flight task and bump every generation. Late resolutions are dropped by the staleness check; nothing can write a logged-out user's data into a fresh session.
- **Abort semantics:** aborting a publish-in-flight does not "unsend" a relay message already dispatched (fire-and-forget below us); abort only abandons *waiting*. That matches current behavior — today's timeouts abandon waits too.

### 4.3 Error surfacing

Follow D6 and the codebase's two existing channels:
- **User-initiated, fire-once operations** (publish, join, upload, follow toggle): failure/timeout → `set_toast(state, HighlighterToast { kind: Error, .. })` (pattern at `nmp_app.rs:3449-3455`); busy flag cleared. Timeout copy mirrors the shipped account-create message ("… timed out. Check your connection and try again.", `nmp_app.rs:5509`).
- **Panel refreshes** (Class C probes, entity resolve): per-panel error/empty fields in the snapshot (e.g. a NIP-11 probe result column in network settings), no toast.
- Optimistic UI is unchanged where it exists today (e.g. a comment appears locally, the relay echo reconciles via delta).

### 4.4 Migration plan (phased)

**Phase 0 — Observability + guardrails (no behavior change).**
- Wrap the actor loop body with per-message duration logging; warn at > 250 ms with the message tag (mirrors ADR-0028's liveness philosophy).
- Introduce `block_on_local` and mechanically rename all 92 sites (pure rename; classification is encoded by the later phases).
- Add a CI grep gate (modeled on nmp's `doctrine-lint`) rejecting `runtime.block_on(` in `nmp_app.rs` outside the wrapper and `OpRunner`.
- *Scope: ~1–2 days. Exit: an on-device trace identifies the actual wedge sites by name.*

**Phase 1 — Kill the Class A wedges + Class C (the top-10 list).**
Land `OpRunner`, `OpDomain`, `KernelMsg::OpResolved`, and migrate exactly these handlers:
1. `request_join_room` (2911) — keeps `pending_joins` bookkeeping in `apply_op_outcome`.
2. `publish_room_chat_message` (3264).
3. `upload_create_room_cover` (2288), 4. `upload_edit_profile_image` (2656), 5. `upload_capture_photo` (2976).
6. `probe_network_relay_nip11` (3370), 7. `fetch_network_import_relays` (3380).
8. Entity-resolve call sites (via `client.rs:1494`).
9. `submit_create_room` (2317) and 10. `mint_room_invite_link` (2355/2405) — multi-sign + room bootstrap, the worst Class B compounds.
- *Scope: ~4–6 days including plumbing. Exit: the dead-network harness (4.6) shows snapshots keep emitting and `AccountCreateResolved` is processed within deadline + ε while any Phase-1 action is in flight.*

**Phase 2 — Class B publish helpers (~24 sites).**
Mechanical after Phase 1: each helper splits into `prepare_*` (actor: read state, build draft, set busy, emit) → `submit_op` (sign + publish off-actor) → `apply_op_outcome` (actor: clear busy, toast on error, local hydrate). Fold `start_onboarding_follow_publish` (10018) into `OpRunner` while touching follows.
- *Scope: ~1.5–2 weeks, parallelizable by domain.*

**Phase 3 — `handle_core_delta` hardening.**
The delta handler (1961 → 3464+) cascades into refresh helpers. After Phases 1–2 those cascades are local-only; change `handle_core_delta`'s remaining awaits to `block_on_local` and assert the duration guard. Optionally make it a sync fn to prove it.
- *Scope: ~3–4 days.*

**Phase 4 — Consolidation (optional but recommended).**
Migrate the nine legacy workers onto `OpRunner`/`OpResolved` (delete eight bespoke thread+runtime constructions and five `KernelMsg` variants), keeping their generation semantics. Pure refactor under existing tests.
- *Scope: ~1 week.*

### 4.5 Behavioral changes visible to iOS/Android

- Migrated actions become two-phase: an immediate snapshot with `busy = true`, then a completion snapshot. Shells already consume this shape for sign-in, account-create, search, ISBN, and web metadata — no new platform contract, but **per-domain busy/error fields are added to `HighlighterAppState`** where missing (room-join pending already exists via `pending_joins`; uploads/publishes gain flags).
- Worst-case latency for a stuck operation drops from a 360 s / 65 s silent freeze to a 30 s busy state ending in a toast, with the rest of the UI live throughout.
- Snapshot *ordering* between unrelated domains can interleave where it previously serialized (a refresh completing while an upload is in flight). The reconciler contract (full-state snapshots with a revision bump) already tolerates this.

### 4.6 Test strategy

**Existing suite:** the in-file tests (`nmp_app.rs:10629+`) drive a real actor via `TestReconciler` (10644-10666), `test_app()` (10668-10680), and `next_state` polling with 5 s receive timeouts (10755-10767). Class D handlers are untouched, so the bulk passes as-is; migrated-domain tests adapt by pumping `next_state` until the completion snapshot — the same loop they already use for sign-in (e.g. `valid_sign_in_…` at 11035).

**New: deterministic dead-network harness.** Two gaps today:
1. **Relay URLs are compile-time baked** — `relay_policy()` uses `include_str!("relay_policy.json")` (`relays.rs:46-56`). Add a test-only seam: `HighlighterAppConfig` (currently `data_dir` / `visible_limit` / `emit_hz` only, `nmp_app.rs:80-90`) gains an optional `relay_policy_json: Option<String>` (or an env override read once in `relay_policy()`), letting tests point every relay role at harness URLs. NMP itself happily accepts arbitrary URLs — its own tests use `wss://relay.test` (`nostr-multi-platform/crates/nmp-testing/tests/e2e_full_pipeline.rs:83`).
2. **A black-hole relay**: a `TcpListener` on `127.0.0.1:0` that accepts connections and never completes the WebSocket handshake — deterministic "network up, relay dead." For live-relay behavior, reuse the framework's precedent of an in-process `nak serve` relay (`nostr-multi-platform/crates/nmp-testing/tests/real_relay_nip17_cold_start_kernel.rs:23`). HTTP black-holing (NIP-05, NIP-11, blossom) uses the same listener trick with the relevant base URLs made injectable (`NIP05_API_URL` is a const in `nmp_app.rs` today).

**Acceptance tests (Phase 1 exit gates):**
- *Liveness under wedge:* point all relays at the black hole; dispatch `RequestJoinRoom`, then 10 unrelated local actions. Assert each local action's snapshot arrives < 1 s, the join busy-flag is set, and a join-timeout toast appears at the deadline.
- *Account-creation regression (the on-device repro):* black-hole relays + black-hole NIP-05; dispatch `SubmitCreateAccount` interleaved with a Phase-1 action; assert the `AccountCreateResolved` outcome reaches a snapshot within 30 s + ε — this test fails on today's code by construction.
- *Supersession:* two `ProbeNetworkRelayNip11` for the same URL against a slow-then-fast harness; assert only the second generation's result lands.
- *Logout mid-flight:* start an upload against the black hole, dispatch sign-out; assert no post-logout snapshot contains the op's domain data and busy flags are cleared.
- *Loop-stall watchdog as a test assertion:* the Phase-0 per-message timer exported through a test hook; assert max handler duration < 250 ms across the suite (the D8 regression guard, in miniature).

---

## 5. Why this is the right long-term shape

The fundamental constraint is the single-writer actor: it buys snapshot consistency and lock-free state discipline, and it is non-negotiable in both this facade and the framework underneath (D3/D4). Once an actor is the sole writer *and* the sole emitter, any await inside it is a liveness bug by construction — deadline tuning only rescales the freeze. Every mature actor system lands on the same resolution this design adopts: effects run off-loop and re-enter as messages with correlation discipline. The framework reached that conclusion twice (ADR-0024 by design, ADR-0040 after shipping the V-90 stall), and this facade independently reached it nine times in miniature. The work here is not inventing a pattern — it is finishing the migration to the one already chosen, and installing the lint and the harness that keep the loop non-blocking permanently.


---

## 6. Implementation notes (post-landing)

All four phases landed 2026-06-12, plus one design amendment discovered by the
acceptance tests:

- **Caller-supplied timeout outcomes** (§4.1 amendment): the designed
  `OpOutcome::timed_out(domain)` generic fallback was insufficient — domains
  whose resolvers re-check per-domain generation counters (Auth, SearchLocal,
  UsernameCheck, AccountCreate) dropped the zeroed-generation fallback as
  stale, wedging busy flags exactly like the original defect; domains with
  keyed bookkeeping (JoinRoom's `pending_joins`, IsbnPreview/WebMetadata dedup
  maps, RelayProbe rows) corrupted state on empty-key fallbacks. `submit_op`
  now requires the caller to construct the timeout outcome with the live
  generation and real keys; the generic constructor was deleted. Regression
  tests: `auth_supersession_nsec_then_bunker_drops_stale_resolution`,
  `join_room_timeout_resolves_with_live_payload`.
- **`handle_core_delta` is a sync fn** (Phase 3's optional hardening was
  taken): the non-blocking-delta invariant is structural.
- **`login_nsec` runs under `spawn_blocking`**: it is a sync core method that
  internally `block_on`s the NMP runtime; on OpRunner's multi-thread runtime a
  nested `block_on` panics.
- **Supersession semantics note**: facade supersession governs resolution
  side effects (credential persistence, toasts, busy flags) — not core-level
  identity. A superseded-but-successful `login_nsec` still surfaces through
  the `SignerConnected` delta by long-standing design; a later failed bunker
  pairing must not silently log out a successfully signed-in user.
