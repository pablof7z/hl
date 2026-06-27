---
type: research-record
date: 2026-06-26
session: 9ae03596-fa74-4208-88c6-a90bd3b176e4
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/9ae03596-fa74-4208-88c6-a90bd3b176e4.jsonl
source_lines: 2506-2545
agent_attribution: coder91b
has_preregistered_criteria: true
has_method: true
has_structured_report: true
characterization: "#91 final report: per-module deletions verified, 715 cargo tests pass, build clean, deps dropped, two gates incomplete (bindings regen + iOS build)"
captured_at: 2026-06-26T20:11:13Z
---

#91 final report: per-module deletions verified, 715 cargo tests pass, build clean, deps dropped, two gates incomplete (bindings regen + iOS build)

---

**#91 Final Report — feat/91-delete-nostrruntime-lane**

**Final commit:** `4ed6c64d` — pushed to origin.

---

**Per-module deletions (each its own commit, cargo test --lib green after each):**

1. `feedback.rs` (db871806) — deleted 855 lines: query_threads_snapshot, query_thread_snapshot, query_threads, query_thread_events, query_first_agent_pubkey, publish_note, ensure_feedback_relay, 7 private helpers, nostrdb/tempfile imports, TEST_COORD, 5 ndb helpers, 9 ndb tests, FeedbackRootPublishSnapshot, FeedbackReplyPublishSnapshot structs.

2. `artifacts.rs` (46462e6b) — deleted 499 lines: publish, publish_snapshot, query_for_group, search_cached, ArtifactPublishSnapshot struct, scan_cap/contains_ci/starts_ci/artifact_match_rank/artifact_identity helpers, nostrdb import, isolated_ndb/ingest/make_share test helpers. Also removed `pub use artifacts::ArtifactPublishSnapshot` from lib.rs. Kept build_share_event + SearchableShare (still tested).

3. `highlights.rs` (1d300a3e) — deleted 705 lines: publish_and_share, hydrate, query_for_article, publish, query_for_reference, query_for_book_catalog, query_for_group, query_highlights_by_author, 4 private helpers, nostrdb import, 3 test helpers, 7 ndb tests. Kept record_from_cached_event, build_highlight_event, build_repost_event, build_imeta_tag (kernel-called).

4. `relays.rs` (6d427f0f) — deleted 122 lines: publish_nip65, publish_app_data, set_relays, upsert_relay, remove_relay, set_relay_roles (all took &NostrRuntime). Removed `use crate::nostr_runtime::NostrRuntime` import. Kept query_relays/latest_nip65/latest_app_data/nostrdb import temporarily (still referenced from nostr_runtime.rs at the time).

5. `client.rs` (d463f420) — Decouple: replaced `runtime: Arc<NostrRuntime>` field with `data_dir: PathBuf`. Moved `default_data_dir()` from nostr_runtime.rs into client.rs. Dropped `install_diagnostics_callback` — confirmed kernel owns relay diagnostics via `RelayDiagnosticsViewSnapshot` in HighlighterAppKernel.swift (openRelayDiagnostics/closeRelayDiagnostics lifecycle, line 519–528, snapshot received at line 595). HighlighterStore.swift holds `core: HighlighterCore` only as a stub (comment: "Legacy core handle retained for #95 relay-list import path") — no methods called on it. Removed `runtime()` pub(crate) accessor (was #[allow(dead_code)]).

6. Final cleanup (4ed6c64d) — deleted `nostr_runtime.rs` + `test_ndb.rs`; deleted `query_relays`, `latest_nip65`, `latest_app_data` from relays.rs (now fully orphaned); dropped `nostrdb` import from relays.rs; repointed `kernel/domains/route.rs` from `nostr_ndb::nostr::` to `nostr::` (PublicKey + ToBech32); removed `pub mod nostr_runtime` + `#[cfg(test)] pub mod test_ndb` from lib.rs; dropped `nostrdb` + `nostr-ndb` from Cargo.toml + Cargo.lock patch section.

---

**cargo test --lib:** `test result: ok. 715 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` (down from 729 — the 14 deleted tests were nostr_runtime + test_ndb module tests, as expected).

**cargo build --lib:** Clean, no errors.

**Grep gate** (`rg -n "nostr_sdk|nostrdb|nostr-ndb|NostrRuntime|NdbFilter|Ndb::new" app/core/src app/core/Cargo.toml`): Zero hits for NostrRuntime, nostrdb, nostr-ndb, NdbFilter, Ndb::new. All remaining nostr_sdk hits are legitimate kernel/FFI code (highlights, artifacts, share_links, ffi/ios, kernel test helpers — all keeping nostr_sdk for event building).

**Cargo.toml:** `nostrdb` and `nostr-ndb` dependencies dropped. `[patch.crates-io]` nostrdb override also removed. Cargo.lock confirmed zero nostrdb/nostr-ndb entries.

---

**Incomplete / still needed before merge:**

- **Generated Swift bindings diff** (`generate-swift-bindings.sh`): NOT run — requires Xcode toolchain + iOS SDK unavailable in this environment. Expected to be ~empty since the UniFFI-exported HighlighterCore surface (new, plan_podcast_playback_session, record_podcast_playback_position, get_podcast_playback_rehydration_snapshot, set_event_callback) is unchanged. The deleted structs (FeedbackRootPublishSnapshot, FeedbackReplyPublishSnapshot, ArtifactPublishSnapshot) were previously in generated bindings but have no hand-written Swift usage — their removal should be ~empty diff on the Swift side, but needs visual confirmation.

- **Xcode build** (`xcodebuild build -project Highlighter.xcodeproj -scheme Highlighter -sdk iphonesimulator -destination 'platform=iOS Simulator,name=iPhone 16 ci'`): NOT run — requires Xcode/simulator toolchain. The modified Swift files in the working tree (HighlighterAppKernel.swift, ImportRelaysSheet.swift, NetworkSettingsStore.swift) were pre-existing modifications unrelated to #91; they need to compile cleanly against the updated `highlighter_coreFFI.h` after bindings regeneration.

Everything on the Rust side is verifiably clean. The two remaining gates are iOS toolchain steps.
</teammate-message>
