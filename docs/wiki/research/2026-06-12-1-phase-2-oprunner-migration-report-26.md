---
type: research-record
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
source_lines: 4658-4740
agent_attribution: OpRunner Phase 2 publishes subagent
has_preregistered_criteria: true
has_method: true
has_structured_report: true
characterization: "Phase 2 OpRunner migration report: 26 publish handlers migrated to OpRunner, five verification gates green, zero Class-B blocking sites remaining, with test results and deviation analysis"
captured_at: 2026-06-12T18:06:03Z
---

Phase 2 OpRunner migration report: 26 publish handlers migrated to OpRunner, five verification gates green, zero Class-B blocking sites remaining, with test results and deviation analysis

---

User: [Agent task result: Agent "OpRunner Phase 2 publishes" completed]
The diff stat reflects the working-tree state (which includes both Phase 1 and Phase 2 since neither is committed). All work is in the main tree at `/Users/pablofernandez/Work/hl/app/core/src/nmp_app.rs`. Per instructions, I have NOT committed.

Let me write the final report.

---

📋 SUMMARY: Phase 2 of the actor-blocking fix is complete — all ~24 Class B publish helpers (plus 2 sibling relay writes and the onboarding worker) are migrated onto the Phase 1 OpRunner primitive, with new acceptance coverage and all five verification gates green.

🔍 ANALYSIS: Every migrated handler now splits into prepare-on-actor (read state, validate, set busy flag, emit) → `submit_op` with a 30s deadline (sign+publish off-actor, future captures only `Arc<HighlighterCore>` + owned input) → `apply_op_outcome` on the actor (clear busy, optimistic/local hydrate, D6 toast). The off-actor-future capture invariant was verified programmatically: zero futures reference state/runtimes/reconciler/ops.

⚡ ACTIONS: Extended `OpDomain`/`OpOutcome`/`op_timeout_message`/`op_timed_out`; migrated 26 handler call sites via the main tree (Edit tool was sandboxed to a stale worktree as predicted, so all edits applied via Bash); deleted 23 orphaned async helpers; folded onboarding into OpRunner and removed the bespoke `OnboardingFollowsResolved` plumbing; added 4 new tests; fixed disk-space exhaustion mid-build.

✅ RESULTS: All gates pass. Android `BUILD SUCCESSFUL`. Class-B-remaining count = **ZERO**.

📊 STATUS: Working tree only (not committed, per instruction). File: `/Users/pablofernandez/Work/hl/app/core/src/nmp_app.rs`.

📁 CAPTURE: The `tests/nostr_connect.rs` and `tests/session_nsec.rs` integration tests were left broken by Phase 1 (missing the additive `relay_policy_json` field); pre-existing, out of Phase 2 scope, and don't affect the `cargo test --lib` gate.

➡️ NEXT: Phase 3 (`handle_core_delta` hardening) and Phase 4 (consolidate the 8 remaining legacy workers onto OpRunner). Optionally fix the two pre-existing integration tests and the named flaky test.

---

### Migrated handlers (name → OpDomain → deadline)

| Handler | OpDomain | Deadline |
|---|---|---|
| `SubmitRoomInviteMembers` | `RoomInviteMembers` | 30s |
| `PublishComment` | `CommentPublish` | 30s |
| `ToggleCommentLike` | `CommentInteraction{target,kind:Like}` | 30s |
| `ToggleCommentBookmark` | `CommentInteraction{target,kind:Bookmark}` | 30s |
| `PublishFeedbackNewThread` | `FeedbackPublish` | 30s |
| `PublishFeedbackReply` | `FeedbackPublish` | 30s |
| `AddBlossomServer` / `RemoveBlossomServer` / `MoveBlossomServers` (3 sites) | `MediaSettingsWrite` | 30s |
| `SubmitEditProfile` | `ProfileEditSubmit` | 30s |
| `ToggleArticleBookmark` | `ArticleBookmarkToggle{target}` | 30s |
| `SetAddressInCurationSet` | `CurationWrite` | 30s |
| `CreateCurationSetAndAdd` | `CurationWrite` | 30s |
| `ToggleProfileFollow` | `FollowToggle` | 30s (self-timeout) |
| `PublishArticleHighlight` | `HighlightPublish` | 30s |
| `PublishArtifactShare` | `SharePublish` | 30s |
| `PublishUrlShare` | `SharePublish` | 30s |
| `ShareHighlightRepost` | `SharePublish` | 30s |
| `PublishRoomDiscussion` | `RoomDiscussionPublish` | 30s |
| `PublishCaptureHighlight` | `CapturePublish` | 30s |
| `PublishCapturePicture` | `CapturePublish` | 30s |
| `PublishClipHighlight` | `CapturePublish` | 30s |
| `UpsertNetworkRelay` | `NetworkRelayWrite{Upsert}` | 30s |
| `RemoveNetworkRelay` | `NetworkRelayWrite{Remove}` | 30s |
| `SetNetworkRelayRoles` | `NetworkRelayWrite{SetRoles}` | 30s |
| `ApplyNetworkImportRelays` | `NetworkRelayWrite{ImportApply}` | 30s |
| `CompleteOnboarding` (follow publish) | `OnboardingFollows` | 30s |

I migrated `RemoveNetworkRelay` and `SetNetworkRelayRoles` beyond the named list because they sign+publish the same relay list (kind 10002) — leaving them as `block_on_local` would have failed the "ZERO Class-B-remaining" gate.

### OpDomain keying decisions (toggle supersession)

- **`OpDomain` stays `Copy`**, so per-target keys use a `u64` hash (`op_target_hash`) of the identifier; the actual id rides in the `OpOutcome` payload.
- **`CommentInteraction { target: u64, kind: CommentInteractionKind }`** — keyed by `hash(event_id)` + Like/Bookmark. A like on comment A and a like on comment B get distinct keys (no cross-target abort); a rapid double-tap on the *same* comment+kind shares a key → abort+regenerate → last-tap-wins. Different kinds on the same comment are independent slots. Proven by `comment_interaction_keying_is_per_target_and_kind`.
- **`ArticleBookmarkToggle { target: u64 }`** — keyed by `hash(address)`; same per-target rationale.
- **`FollowToggle`** — single unkeyed slot (only one profile view is open at a time); rapid double-tap supersedes, last-tap-wins. To keep the optimistic-revert correct even on timeout (the generic `op_timed_out` can't know `previous_following`), the FollowToggle future self-times-out via `tokio::time::timeout` so it always carries the real revert state.
- **`NetworkRelayWrite`** — single slot for all four relay-list writes (they mutate one list); supersession serializes them, then the apply-arm refresh reconciles.
- **`CapturePublish`/`SharePublish`/`CurationWrite`** — single slot each (one composer/menu open at a time).

### New state fields
**None added.** Every busy flag already existed (`is_publishing`, `is_saving`, `is_mutating_follow`, `is_adding_members`, `is_publishing_discussion`, `is_sending_chat_message`, etc.). No uniffi Record change → existing Kotlin untouched (confirmed by `BUILD SUCCESSFUL`).

### Test results
- `cargo test --lib -- --test-threads=1` run 4× total: clean 254-pass on the majority; two runs surfaced the **pre-existing moving flake** (run 1 hit a test I then fixed for the new ordering; later runs hit `curation_menu_open…` and `subscribe_joined_communities…` — both named in your known-issues note, both pass in isolation). **No new persistent failures.**
- New tests, all green: `comment_interaction_keying_is_per_target_and_kind`, `rapid_double_toggle_last_tap_wins`, `acceptance_publish_under_dead_network`, `acceptance_phase2_actions_keep_loop_live`.
- Adapted `article_reader_publish_without_article_surfaces_error_as_state` to the new validation-first ordering (validation now runs before the busy flag is set, eliminating a spurious `is_publishing` flash — a behavioral improvement).
- Lint: PASS. Clippy: 2 nmp_app.rs warnings, both verified verbatim in the pre-Phase-1 baseline (MutexGuard-across-await in `RelayStatusChanged`; Option::map in `set_room_invite_query`) → **zero net-new**. Android gradle: `BUILD SUCCESSFUL`.

### Deviations from the design doc (with justification)
1. **Migrated `RemoveNetworkRelay` + `SetNetworkRelayRoles`** (not explicitly named) — they sign+publish kind-10002 like `upsert`; required to hit the zero-Class-B gate.
2. **FollowToggle self-timeout** — added an inner `tokio::time::timeout` because the generic timeout fallback can't preserve `previous_following` for a correct optimistic revert.
3. **Validation-before-busy ordering** for publish helpers (e.g. article highlight) — synchronous input validation now precedes the busy flag, avoiding a UI flash; required adapting one test.
4. **Toggle test modeled at the `OpRunner` layer** — the last-tap-wins supersession test drives `OpRunner` directly rather than a full signed-in fixture, exercising the exact mechanism without auth scaffolding.

### Class-B-remaining count: **ZERO**
A surgical grep confirms no `block_on_local` site awaits a sign/publish core method. The three remaining network-adjacent `block_on_local` calls are all Class C/D and correct to keep inline: `apply_network_connectivity_policy` (non-waiting reconnect/disconnect nudges), `hydrate_search_relays` (in-memory snapshot read), and `start_nostr_connect` (builds the connect URI synchronously; the bunker wait is spawned off-actor inside the core).

🎯 COMPLETED: Phase two actor-blocking migration done; all five gates green, zero Class B remaining.
