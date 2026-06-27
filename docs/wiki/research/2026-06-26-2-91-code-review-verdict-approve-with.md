---
type: research-record
date: 2026-06-26
session: 9ae03596-fa74-4208-88c6-a90bd3b176e4
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/9ae03596-fa74-4208-88c6-a90bd3b176e4.jsonl
source_lines: 2676-2696
agent_attribution: review91
has_preregistered_criteria: true
has_method: true
has_structured_report: true
characterization: "#91 code review verdict APPROVE WITH NITS: verified no over-deletion of live code, decouple safety, callback drop safe, FFI structs correct, identified non-blocking nits"
captured_at: 2026-06-26T20:11:13Z
---

#91 code review verdict APPROVE WITH NITS: verified no over-deletion of live code, decouple safety, callback drop safe, FFI structs correct, identified non-blocking nits

---

## #91 VERDICT: APPROVE WITH NITS

Deletion is correct. No live code over-deleted; client.rs decouple is safe; lib compiles (`cargo check --lib` → warnings only, no errors).

### Verified correct
(a) **No over-deletion.** All live projection helpers preserved: highlights `build_highlight_event`/`build_repost_event`/`build_imeta_tag` (used by capture_draft.rs), artifacts `build_share_event`/`artifact_record_from_event`, profile `parse_metadata`, all feedback `*_projection` fns + 11 structs. I extracted every deleted `pub fn`/`pub struct` and grepped the tree — only hits are doc-comments + an unrelated Swift `hydrate()`. Deleted items all take `&Ndb`/`&NostrRuntime` or are their private helpers.

(b) **client.rs decouple safe.** UniFFI surface unchanged — only the *private* `runtime` field → `data_dir: PathBuf` and the non-exported `pub(crate) runtime()` → `data_dir()`. No `#[uniffi::export]`/constructor touched. `default_data_dir()` move is byte-faithful: both produce `dirs::data_dir()?.join("highlighter").join("ndb")` with same `CoreError::Cache` fallback.

(c) **Dropping install_diagnostics_callback is safe (statically verified).** `install_diagnostics_callback`/`enable_relay_diagnostics_events`/`relay_diagnostics_snapshot`/`sync_relay_diagnostics` were referenced ONLY inside nostr_runtime.rs — zero callers in ffi/ or Swift. Live relay-diagnostics UI is fed by kernel `relay_diagnostics.rs` + `RelayDiagnosticsViewSnapshot` (HighlighterAppKernel.swift, NetworkSettingsView.swift). Legacy callback was dead.

(d) **Deleted FFI structs** (ArtifactPublishSnapshot, FeedbackRoot/ReplyPublishSnapshot): no hand-written Swift refs; matching `pub use` removal in lib.rs is consistent. **route.rs repoint** nostr_ndb::nostr::* → nostr::* is identical types (nostr-ndb just re-exported the nostr crate; Cargo pins nostr 0.44).

### Nits (non-blocking)
1. **PR-introduced dead-code warning**: highlights.rs:1608-1619 `ShareEventSpec`+`share_event()` are now unused (their only callers were deleted tests at 578/610). Delete them. (Build still passes — project doesn't deny(warnings) and already carries many.)
2. **Stale comments** referencing nostrdb/NostrRuntime/NdbFilter in events.rs:187/198, profile.rs:1, models.rs:671/712, kernel action.rs:1875, actor.rs:2456, effect.rs:838, feed.rs:187, omnibox.rs:10, articles.rs:561, room_home.rs (2522/2680/2804/2880), search.rs (several). Doc cleanup.
3. **Test-count correction**: actual reduction is 40 test fns (755→715 source-level), not "14". All 40 are nostrdb/NostrRuntime-exercising (isolated_ndb/ndb_with_events/open_ndb builders + nostr_runtime live tests, 2 `#[ignore]`). No live projection test removed. Your reported `--lib 715 passed` is the correct gate.

### FYI — pre-existing, NOT this PR
`cargo check --lib --tests` fails compiling integration tests tests/session_nsec.rs + tests/nostr_connect.rs (login_nsec, classify_login_input, pair_bunker, current_user, restore_session_snapshot, start_default_nostr_connect, AuthSessionSnapshot/RestoreSnapshot — E0432/E0599). This is pre-existing breakage: PR doesn't touch tests/ (confirmed via --stat), and none of those auth symbols exist in origin/main's lib src either, so those binaries couldn't compile on main before this PR. An earlier kernel-migration moved auth into kernel/domains/auth.rs. Worth knowing `cargo test` (without --lib) is already red on main; `--lib` is the only green gate. Not a #91 blocker.
</teammate-message>
