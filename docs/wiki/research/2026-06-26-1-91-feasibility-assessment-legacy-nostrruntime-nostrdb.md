---
type: research-record
date: 2026-06-26
session: 9ae03596-fa74-4208-88c6-a90bd3b176e4
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/9ae03596-fa74-4208-88c6-a90bd3b176e4.jsonl
source_lines: 1626-1668
agent_attribution: assess91
has_preregistered_criteria: true
has_method: true
has_structured_report: true
characterization: "#91 feasibility assessment: legacy NostrRuntime/nostrdb reads proven dead code via grep verification; all product reads already migrated to NMP-backed kernel domains; verdict FULLY ACTIONABLE"
captured_at: 2026-06-26T19:03:42Z
---

#91 feasibility assessment: legacy NostrRuntime/nostrdb reads proven dead code via grep verification; all product reads already migrated to NMP-backed kernel domains; verdict FULLY ACTIONABLE

---

# #91 feasibility — VERDICT: FULLY ACTIONABLE (NOT upstream-blocked)

The "blocked:upstream-nmp" assumption is **false**, and verified so. The crux question ("does NMP expose read APIs to replace hl's direct nostrdb reads?") is moot: hl **already migrated every product read to NMP-backed kernel domains**. The legacy nostrdb read sites are **dead code** — zero kernel/ffi callers; they survive only as test-only helpers and as intermediate helpers for equally-dead legacy `publish` functions. No new NMP API is required.

## Pin reality
`app/core/Cargo.toml:25-41` pins NMP by `branch="master"`; `Cargo.lock` locks rev `1e7750f0` (2026-06-26). Local NMP checkout HEAD `46954afb` is only **5 commits ahead**, all `nmp-sqlite-wasm` work — irrelevant to read APIs. The checkout faithfully represents the built NMP.

## Per read site — all already converted (each legacy helper has 0 kernel/ffi callers, grep-verified)

| Legacy read (hl file) | Queried | Already served by (NMP-backed) |
|---|---|---|
| `highlights.rs` ndb helpers (k9802/k11) | highlights by group/author/article | `kernel/domains/highlight_feed.rs` (snapshot projection + `NmpApp`), `articles.rs` |
| `artifacts.rs:263` `query_for_group`, `:316` `search_cached` (k11) | artifact shares / search | `kernel/domains/articles_feed.rs` + `share.rs` snapshot projections |
| `profile.rs:646` `query_raw_metadata_json` (k0) | profile metadata | `kernel/domains/profiles.rs` — `ProfileSnapshot` typed projection, `nmp_core::typed_projections::decode_profile`, `nmp_core::refs::RefProfileStore` |
| `feedback.rs` `query_thread_snapshot`/`query_first_agent_pubkey` | feedback threads/agent | `kernel/domains/feedback.rs` NMP list projection |
| `relays.rs:1349/1233/1303` `query_relays`/`latest_nip65`(k10002)/`latest_app_data`(k30078) | relay-config merge | `kernel/domains/relays.rs` (`NmpApp`), `relay_diagnostics.rs` (`decode_relay_diagnostics` sidecar); relay **import** via `ffi/ios.rs` NMP `mailbox_cache.snapshot(&author)` |

Bridge mechanism is proven and in-tree: `KernelEventObserver` + `nmp_ref.register_live_event_tap` / `register_typed_snapshot_projection`, used by `reactions.rs:461`, `discussions.rs:425`, `bookmark_sets.rs`, `follows.rs`. The dead publish fns are explicitly marked superseded: `kernel/effect.rs:782` and `kernel/domains/share.rs:459` say *"Replaces the bespoke `artifacts::publish`."*

## Is NostrRuntime load-bearing for product reads? NO — vestigial for reads
- Every `runtime:&NostrRuntime` fn is a **write** (`*::publish*`, `relays::set_relays`/`upsert_relay`/etc.) — all have **0 product callers**.
- `client.rs:105` `runtime()` is `#[allow(dead_code)]`.
- BUT `NostrRuntime` is **still instantiated at runtime**: `HighlighterStore.swift:71` constructs `HighlighterCore()` (`client.rs`) which calls `NostrRuntime::new()` (spins `nostr_sdk::Client`+`Ndb`) solely for two **non-read** jobs: `data_dir()` (feeds `OnboardingStore`/`PodcastPositionStore`) and `install_diagnostics_callback()`. The main app object is the separate `HighlighterApp` kernel lane (`App.swift:8`→`HighlighterAppKernel`).

## Deletion gating
- Read/publish helper deletions are **NOT gated** on anything (already dead) — land immediately.
- Deleting `NostrRuntime` itself **is** gated on retiring its two surviving non-read jobs in `client.rs`: give onboarding/podcast a plain data-dir provider, and drop `install_diagnostics_callback` (confirm kernel `relay_diagnostics.rs` already feeds the UI — likely redundant).
- Dropping `nostr-ndb`/`nostrdb` deps also needs repointing one transitive type re-export: `kernel/domains/route.rs:8-9` `nostr_ndb::nostr::{ToBech32,PublicKey}` → use the `nostr` crate (trivial). `feed.rs:187`/`articles.rs:561` are comments only; active kernel lane has zero real nostrdb usage.

## Ordered TDD slice plan (each independently landable, #102-#108 pattern)
1. Delete dead **highlights** read+publish lane (`&Ndb`/`&NostrRuntime` fns + tests); `cargo test` green.
2. Delete dead **artifacts** lane (`query_for_group`, `search_cached`, `publish`).
3. Delete dead **profile** lane (`query_raw_metadata_json`, `publish_profile`).
4. Delete dead **feedback** lane (`query_thread_snapshot`, `query_first_agent_pubkey`, `publish_note`).
5. Delete dead **relays** lane (`query_relays`, `latest_nip65`, `latest_app_data`, all `set_*`/`publish_*`).
6. Decouple `client.rs` from `NostrRuntime`: minimal data-dir provider for onboarding/podcast; retire `install_diagnostics_callback` after confirming kernel `relay_diagnostics` parity. Keep `HighlighterCore` Swift surface (podcast/callback) unchanged.
7. Delete `nostr_runtime.rs` + `test_ndb.rs`; repoint `route.rs` imports to `nostr`; drop `nostr-ndb`+`nostrdb` from `Cargo.toml:19-20,62`. Run the issue's `nmp_architecture_scan.py` verification.

## What to post on #91/#64
Not blocked. AC *"reads moved to NMP store/projection APIs OR replaced by existing kernel snapshots/actions"* is **already satisfied** — every product read flows through `kernel/domains/{highlight_feed,articles_feed,profiles,feedback,relays,relay_diagnostics}.rs` over `NmpApp`/typed projections/mailbox cache. Remaining `nostrdb` refs (nostr_runtime.rs + 5 feature modules + test_ndb.rs) are dead-code/test-fixture deletions, not API gaps. **No upstream NMP issue required.**

No blocked sites; no missing NMP API to name.
</teammate-message>
