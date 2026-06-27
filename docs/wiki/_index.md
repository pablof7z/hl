# Wiki Index

> Derived cache — do not hand-edit. Rebuilt by proactive-context after each capture.

Last updated: 2026-06-27

## article-reader (1 guide)

| Slug | Title | Summary | Tags | Volatility | Verified | Topic |
|------|-------|---------|------|------------|----------|-------|
| [article-reader](guides/article-reader.md) | Article Reader | Article reading progress is calculated as a scroll-position fraction (contentOffset / scrollableHeight, clamped [0,1]) | capture | warm | 2026-06-26 | article-reader |

## artifacts (1 guide)

| Slug | Title | Summary | Tags | Volatility | Verified | Topic |
|------|-------|---------|------|------------|----------|-------|
| [artifacts](guides/artifacts.md) | Artifacts | Artifacts are books, articles, podcasts, or videos shared and annotated within communities. | capture | warm | 2026-06-26 | artifacts |

## autonomous-loop (1 guide)

| Slug | Title | Summary | Tags | Volatility | Verified | Topic |
|------|-------|---------|------|------------|----------|-------|
| [autonomous-loop](guides/autonomous-loop.md) | Autonomous Loop | When `/loop` is invoked with no prompt or interval, the system runs the autonomous check immediately and then self-paces the next iteration via ScheduleWakeup. | capture | warm | 2026-06-26 | autonomous-loop |

## bookmark-sets (1 guide)

| Slug | Title | Summary | Tags | Volatility | Verified | Topic |
|------|-------|---------|------|------------|----------|-------|
| [bookmark-sets](guides/bookmark-sets.md) | Bookmark Sets | Bookmark set edit actions (rename, delete) are gated to sets owned by the active user | capture | warm | 2026-06-26 | bookmark-sets |

## communities (1 guide)

| Slug | Title | Summary | Tags | Volatility | Verified | Topic |
|------|-------|---------|------|------------|----------|-------|
| [communities](guides/communities.md) | Communities | Highlighter communities are NIP-29 groups with portable user ownership that organize around source content rather than ephemeral posts | capture | warm | 2026-06-26 | communities |

## content-events (1 guide)

| Slug | Title | Summary | Tags | Volatility | Verified | Topic |
|------|-------|---------|------|------------|----------|-------|
| [content-events](guides/content-events.md) | Content Events | Content events use kind:11 threads for artifact shares, kind:9802 for highlights, and kind:1111 for discussion replies within NIP-29 groups. | capture | warm | 2026-06-26 | content-events |

## development-workflow (1 guide)

| Slug | Title | Summary | Tags | Volatility | Verified | Topic |
|------|-------|---------|------|------------|----------|-------|
| [development-workflow](guides/development-workflow.md) | Development Workflow | Resolve all GitHub issues to master and test them, working TDD style with opus planner, sonnet coder, and opus reviewer workflows. | capture | warm | 2026-06-26 | development-workflow |

## kernel-domains (1 guide)

| Slug | Title | Summary | Tags | Volatility | Verified | Topic |
|------|-------|---------|------|------------|----------|-------|
| [kernel-domains](guides/kernel-domains.md) | Kernel Domains | NMP-backed kernel domains (`kernel/domains/`) are already in use via typed projections and mailbox cache for all product reads of highlights, artifacts, profile | capture | warm | 2026-06-26 | kernel-domains |

## relay-import-preview (1 guide)

| Slug | Title | Summary | Tags | Volatility | Verified | Topic |
|------|-------|---------|------|------------|----------|-------|
| [relay-import-preview](guides/relay-import-preview.md) | Relay-Import Preview | Relay-import preview is a cached interface over NMP's mailbox cache that displays available relays without requiring a synchronous network fetch | capture | warm | 2026-06-26 | relay-import-preview |

## Research Records (31 records)

| Record | Date | Finding | Agent |
|--------|------|---------|-------|
| [2026-06-26-1-91-code-review-with-explicit-pre](research/2026-06-26-1-91-code-review-with-explicit-pre.md) | 2026-06-26 | #91 code review with explicit pre-registered criteria (a-d), empirically verified through grepping and code analysis, verdict APPROVE WITH NITS | review91 |
| [2026-06-26-1-91-feasibility-assessment-determined-legacy-nostrdb](research/2026-06-26-1-91-feasibility-assessment-determined-legacy-nostrdb.md) | 2026-06-26 | #91 feasibility assessment: determined legacy nostrdb lane is dead code (not upstream-blocked), verified by grep-confirmed zero kernel/FFI callers and file analysis, with ordered 7-slice TDD deletion plan | assess91 |
| [2026-06-26-1-91-feasibility-assessment-is-nostrruntime-nostrdb](research/2026-06-26-1-91-feasibility-assessment-is-nostrruntime-nostrdb.md) | 2026-06-26 | #91 feasibility assessment: is NostrRuntime/nostrdb lane blocking work execution? VERDICT: FULLY ACTIONABLE (not upstream-blocked; legacy nostrdb reads already migrated to NMP kernel domains, lane is dead code) | assess91 |
| [2026-06-26-1-91-feasibility-assessment-legacy-nostrruntime-nostrdb](research/2026-06-26-1-91-feasibility-assessment-legacy-nostrruntime-nostrdb.md) | 2026-06-26 | #91 feasibility assessment: legacy NostrRuntime/nostrdb reads proven dead code via grep verification; all product reads already migrated to NMP-backed kernel domains; verdict FULLY ACTIONABLE | assess91 |
| [2026-06-26-1-91-final-report-per-module-deletions](research/2026-06-26-1-91-final-report-per-module-deletions.md) | 2026-06-26 | #91 final report: per-module deletions verified, 715 cargo tests pass, build clean, deps dropped, two gates incomplete (bindings regen + iOS build) | coder91b |
| [2026-06-26-1-article-reader-62-investigation-current-state](research/2026-06-26-1-article-reader-62-investigation-current-state.md) | 2026-06-26 | Article Reader #62 investigation: current-state evaluation vs. issue claims with findings and risk assessment | ab194b7564f5844ac |
| [2026-06-26-1-code-evaluation-of-91-deletion-correctness](research/2026-06-26-1-code-evaluation-of-91-deletion-correctness.md) | 2026-06-26 | Code evaluation of #91 deletion correctness against pre-registered criteria (no over-deletion, FFI surface preservation, diagnostics callback safety, struct/test removal validity), with specific verification methodology and APPROVE WITH NITS verdict | review91 |
| [2026-06-26-1-code-review-of-91-deletion-evaluated](research/2026-06-26-1-code-review-of-91-deletion-evaluated.md) | 2026-06-26 | Code review of #91 deletion: evaluated over-deletion risk, FFI surface preservation, callback safety, and struct deletion correctness | review91 |
| [2026-06-26-1-coder63-verified-rebase-onto-current-main](research/2026-06-26-1-coder63-verified-rebase-onto-current-main.md) | 2026-06-26 | coder63 verified rebase onto current main preserves work integrity and compiles against new NMP (753 cargo + 45 Swift tests pass, BUILD SUCCEEDED) | coder63 |
| [2026-06-26-1-verification-of-91-rust-side-completion](research/2026-06-26-1-verification-of-91-rust-side-completion.md) | 2026-06-26 | Verification of #91 Rust-side completion: cargo test (715 pass), cargo build clean, grep gate zero hits, deps dropped | coder91b |
| [2026-06-26-2-89-ios-highlightercore-removal-scope-investigation](research/2026-06-26-2-89-ios-highlightercore-removal-scope-investigation.md) | 2026-06-26 | #89 iOS HighlighterCore removal scope investigation with grep-verified findings on deletability, relay-routing derivability, and dead code, plus ordered 3-slice TDD plan | plan89 |
| [2026-06-26-2-91-code-review-verdict-approve-with](research/2026-06-26-2-91-code-review-verdict-approve-with.md) | 2026-06-26 | #91 code review verdict APPROVE WITH NITS: verified no over-deletion of live code, decouple safety, callback drop safe, FFI structs correct, identified non-blocking nits | review91 |
| [2026-06-26-2-assess91-investigated-whether-91-is-blocked](research/2026-06-26-2-assess91-investigated-whether-91-is-blocked.md) | 2026-06-26 | assess91 investigated whether #91 is blocked on upstream NMP; verdict: FULLY ACTIONABLE (not blocked; legacy reads already migrated to NMP kernel domains) | assess91 |
| [2026-06-26-2-code-review-of-89-ios-thin](research/2026-06-26-2-code-review-of-89-ios-thin.md) | 2026-06-26 | Code review of #89 iOS thin shell: evaluated joinRoom relay derivation correctness, fail-closed safety, test genuineness, and removal safety | review89 |
| [2026-06-27-1-debug-investigation-identified-wasm-boot-root](research/2026-06-27-1-debug-investigation-identified-wasm-boot-root.md) | 2026-06-27 | Debug investigation: identified WASM boot root cause (missing sqlite3 artifacts in static files), implemented fix, verified with Playwright tests (@wasm tier green) | coder65boot |
| [2026-06-27-1-debug-wasm-boot-failure-identified-sqlite](research/2026-06-27-1-debug-wasm-boot-failure-identified-sqlite.md) | 2026-06-27 | Debug WASM boot failure: identified sqlite module import chain issue, vendored missing artifacts, enabled @wasm E2E tier → 2 tests PASS | coder65boot |
| [2026-06-27-1-dod-definition-of-done-evaluation-of](research/2026-06-27-1-dod-definition-of-done-evaluation-of.md) | 2026-06-27 | DoD (Definition of Done) evaluation of #65 Slice 1 WASM bootstrap implementation - reports on build gate, test, artifact build status with PASS/SKIP/GREEN verdicts and deviations | coder65s1 |
| [2026-06-27-1-dod-verification-of-slice-1-wasm](research/2026-06-27-1-dod-verification-of-slice-1-wasm.md) | 2026-06-27 | DoD verification of Slice 1 WASM bootstrap: pre-defined gates (build, fallback test, additive-ness), test execution, verdicts (GREEN/PASS/SKIPPED) | coder65s1 |
| [2026-06-27-1-root-cause-investigation-of-wasm-bridge](research/2026-06-27-1-root-cause-investigation-of-wasm-bridge.md) | 2026-06-27 | Root cause investigation of WASM bridge boot failure; identified missing transitive dependency (sqlite3.mjs), implemented fix with vendoring, @wasm E2E tests green | coder65boot |
| [2026-06-27-1-slice-1-implementation-dod-report-verifies](research/2026-06-27-1-slice-1-implementation-dod-report-verifies.md) | 2026-06-27 | Slice 1 implementation DoD report: verifies additive WASM scaffold with build/test execution, zero NDK edits, ready for review | coder65s1 |
| [2026-06-27-1-slice-1-wasm-bootstrap-implementation-verification](research/2026-06-27-1-slice-1-wasm-bootstrap-implementation-verification.md) | 2026-06-27 | Slice 1 WASM bootstrap implementation verification - PRIMARY GATE green, fallback playwright passes, additive-only changes verified | coder65s1 |
| [2026-06-27-1-wasm-bridge-boot-integration-debugging-root](research/2026-06-27-1-wasm-bridge-boot-integration-debugging-root.md) | 2026-06-27 | Wasm bridge boot integration debugging: root-cause analysis of missing sqlite3.mjs/sqlite3.wasm in dynamic imports, fix via artifact vendoring and build-script update, verification with empirical test execution (2/2 passing, secp256k1 signature confirmed) | subagent |
| [2026-06-27-2-investigation-of-upstream-signer-implementation-nip](research/2026-06-27-2-investigation-of-upstream-signer-implementation-nip.md) | 2026-06-27 | Investigation of upstream signer implementation: NIP-07 wired in WASM runtime, NIP-46 and nsec signing blocked (upstream #2119/#2068); TDD plan provided for NIP-07 path | plan65s2 |
| [2026-06-27-2-root-cause-investigation-of-worker-module](research/2026-06-27-2-root-cause-investigation-of-worker-module.md) | 2026-06-27 | Root cause investigation of worker module-load failure: identified missing sqlite3.mjs/wasm dependencies, applied fix, verified @wasm E2E tier green | coder65boot |
| [2026-06-27-2-slice-1-code-audit-verified-additive](research/2026-06-27-2-slice-1-code-audit-verified-additive.md) | 2026-06-27 | Slice 1 code audit - verified additive-ness, SSR-safety, degradation honesty, no secrets - APPROVE WITH NITS verdict | review65s1 |
| [2026-06-27-2-slice-1-code-review-approve-with](research/2026-06-27-2-slice-1-code-review-approve-with.md) | 2026-06-27 | Slice 1 code review: APPROVE WITH NITS verdict with structured criteria verification (additive-ness, SSR-safety, degradation, vendoring) | review65s1 |
| [2026-06-27-3-gating-investigation-single-ref-reads-profile](research/2026-06-27-3-gating-investigation-single-ref-reads-profile.md) | 2026-06-27 | Gating investigation: single-ref reads (profile/event) doable with generic vendored WASM kernel builtins; hl's real feeds upstream-blocked on NMP app-composition crate | plan65s3 |
| [2026-06-27-3-slice-1-5-dod-report-debugs](research/2026-06-27-3-slice-1-5-dod-report-debugs.md) | 2026-06-27 | Slice 1.5 DoD report: debugs real-boot integration failure, vendors wasm artifact with sqlite dependencies, enables @wasm tier—both test tiers green | coder65boot |
| [2026-06-27-3-slice-1-5-wasm-boot-debugging](research/2026-06-27-3-slice-1-5-wasm-boot-debugging.md) | 2026-06-27 | Slice 1.5 WASM boot debugging - root cause identified (missing sqlite3 imports), fixes applied, both playwright tiers green | coder65boot |
| [2026-06-27-4-slice-1-5-code-review-approve](research/2026-06-27-4-slice-1-5-code-review-approve.md) | 2026-06-27 | Slice 1.5 code review: APPROVE WITH NITS with provenance verification, build reproducibility assessment, artifact size justification | review65boot |
| [AGENTS](research/AGENTS.md) |  |  |  |

## Episode Cards (1 card)

| Card | Date | Title | Salience | Status |
|------|------|-------|----------|--------|
| [2026-06-26-1-62-footnote-rendering-plaintext-recovery-strategy](episodes/2026-06-26-1-62-footnote-rendering-plaintext-recovery-strategy.md) | 2026-06-26 | #62 footnote rendering: plaintext recovery strategy identified | root-cause | active |

## Nouns (79 entities)

| Noun | Name | Origin | Definition |
|------|------|--------|------------|
| [91-delete-legacy-app-core-nostrruntime-nostrdb-lane](nouns/91-delete-legacy-app-core-nostrruntime-nostrdb-lane.md) | #91 (Delete legacy app-core NostrRuntime/nostrdb lane) | extracted | fully actionable (not upstream-NMP-blocked); requires removal of dead nostrdb read+publish functions across 5 feature modules (highlights, artifacts, profile, feedback, relays) and decoupling `HighlighterCore` from `NostrRuntime` instantiation while preserving its FFI surface for iOS onboarding/diagnostics |
| [91-github-issue](nouns/91-github-issue.md) | #91 (GitHub issue) | extracted | FULLY ACTIONABLE (NOT upstream-blocked) — legacy nostrdb read sites are dead code with zero kernel/ffi callers, already superseded by NMP-backed kernel domains |
| [appstate-discovered-groups](nouns/appstate-discovered-groups.md) | AppState.discovered_groups | extracted | retained in core; holds discovered groups from the active discovery relay, decoded from typed sidecar, populated when AppAction::StartRoomDiscovery is dispatched |
| [article-body-renderer](nouns/article-body-renderer.md) | article body renderer | extracted | renders from the kernel content_tree via ContentTreeBodyRenderer in the live path (a hybrid approach, not the deleted ArticleBodyView.swift path) |
| [article-body-rendering](nouns/article-body-rendering.md) | article body rendering | extracted | Pipeline where the body renders from the kernel content_tree via ContentTreeBodyRenderer, which flattens prose into NSAttributedString segments displayed in ArticleBodyView |
| [article-body-rendering-hl-ios](nouns/article-body-rendering-hl-ios.md) | article body rendering (hl iOS) | extracted | Hybrid approach: body renders from kernel `content_tree` via `ContentTreeBodyRenderer`, which flattens prose into `NSAttributedString` segments displayed in the bespoke `ArticleBodyView` (`UITextView`) |
| [article-body-rendering-system](nouns/article-body-rendering-system.md) | article body rendering system | extracted | A hybrid architecture where the body renders from the kernel content_tree via ContentTreeBodyRenderer, which flattens prose into NSAttributedString segments displayed in ArticleBodyView |
| [article-footnotes-current-state-before-62](nouns/article-footnotes-current-state-before-62.md) | article footnotes (current state before #62) | extracted | broken via the live path—ContentTreeBodyRenderer hard-returns empty footnotes/footnoteAnchors; old MarkdownRenderer/FootnotePreprocessor paths no longer run because content_tree has no footnote node kind |
| [article-reader-in-62](nouns/article-reader-in-62.md) | article reader (in #62) | extracted | hybrid rendering system that uses kernel `content_tree` (CommonMark format) via `ContentTreeBodyRenderer` to flatten prose into `NSAttributedString` segments displayed in `ArticleBodyView` (`UITextView`), with text selection via Edit Menu, footnotes via inline references, and reading progress overlay |
| [article-reader-in-current-main](nouns/article-reader-in-current-main.md) | article reader (in current main) | extracted | is a hybrid: body renders from kernel `content_tree` via `ContentTreeBodyRenderer` (flattens to NSAttributedString segments) displayed in bespoke `ArticleBodyView` (UITextView) |
| [articlebodyview](nouns/articlebodyview.md) | ArticleBodyView | extracted | a UITextView wrapper that displays article body text with selectable content, custom Edit Menu (Highlight/Highlight with note), and footnote tap routing |
| [autonomous-loop](nouns/autonomous-loop.md) | autonomous loop | extracted | Invoked on a timer while user is away; keeps work moving forward without user driving every step; steward of established work, not an initiator |
| [autonomous-loop-autonomous-check](nouns/autonomous-loop-autonomous-check.md) | autonomous loop / autonomous check | extracted | invoked on a timer while the user is away; keeps work moving forward without user driving every step; acts as steward of established work, not initiator; must not invent new work without clear authorization |
| [autonomous-loop-check](nouns/autonomous-loop-check.md) | Autonomous loop check | extracted | timer-invoked stewardship while user is away/occupied; keeps work moving on already-started tasks, PRs, and problems without initiating new work |
| [autonomous-loop-dynamic](nouns/autonomous-loop-dynamic.md) | <<autonomous-loop-dynamic>> | extracted | the dynamic-mode sentinel that expands at fire time to full instructions (on first fire / post-compact / loop.md edit) or dynamic-pacing-specific short reminder (subsequent fires) |
| [bookmarksetrow-setcoordinate](nouns/bookmarksetrow-setcoordinate.md) | BookmarkSetRow.setCoordinate | extracted | computed property extracting the relay coordinate as the string format kind:pubkey:d-tag |
| [bridge-mechanism](nouns/bridge-mechanism.md) | Bridge mechanism | extracted | Proven and in-tree pattern: `KernelEventObserver` + `nmp_ref.register_live_event_tap` / `register_typed_snapshot_projection`, used by kernel domains (reactions, discussions, bookmark_sets, follows) to connect app state to NMP. |
| [bridge-shape-for-65-web-nmp-migration](nouns/bridge-shape-for-65-web-nmp-migration.md) | bridge shape (for #65 web NMP migration) | extracted | WASM-in-worker architecture via nmp-browser-runtime + vendored runtime-web; main thread spawns Worker that boots the WASM runtime, drives it with FlatBuffers messages, receives UpdateFrame bytes back. |
| [content-tree](nouns/content-tree.md) | content_tree | extracted | CommonMark format representation of article content that does not model footnote syntax ([^id]); footnote references and definitions survive as literal .text content inside paragraph nodes |
| [contenttreebodyrenderer](nouns/contenttreebodyrenderer.md) | ContentTreeBodyRenderer | extracted | the live body renderer in the hl iOS shell that flattens prose from content_tree into NSAttributedString segments for display via ArticleBodyView |
| [contenttreefootnotes](nouns/contenttreefootnotes.md) | ContentTreeFootnotes | extracted | Rust enum that scans a content_tree for footnote definition paragraphs (matching pattern `^\s*\[^id\]:\s*body`) and returns paired definitions plus root indices to exclude from body rendering |
| [curation-set-share-base-url](nouns/curation-set-share-base-url.md) | CURATION_SET_SHARE_BASE_URL | extracted | `https://highlighter.com/note/` — the web route base for bookmark-set (kind:30004) naddr share links, mapped to the web app's generic `/note/<naddr>` handler |
| [current-article-body-renderer](nouns/current-article-body-renderer.md) | current article body renderer | extracted | a hybrid: kernel content_tree rendered via ContentTreeBodyRenderer (which flattens prose into NSAttributedString) displayed in the bespoke ArticleBodyView UITextView wrapper |
| [d3-doctrine-d3-routing](nouns/d3-doctrine-d3-routing.md) | D3 (doctrine:d3-routing) | extracted | hl architecture constraint: HighlighterApp/NMP lane never constructs relay URLs itself; all relay routing derives from AppState or is supplied by the caller (externalized to avoid tight coupling) |
| [feedback-rs-module](nouns/feedback-rs-module.md) | feedback.rs module | extracted | is NOT wholesale dead — its uniffi-exported types (14 total: FeedbackThreadsSnapshot, FeedbackMessageRowProjection, etc.) and projection functions are live, consumed by iOS app and kernel |
| [footnotepreprocessor](nouns/footnotepreprocessor.md) | FootnotePreprocessor | extracted | Parser providing Definition type and GFM footnote parsing logic; extracts footnote definitions and references from text |
| [footnotes-article-reader-feature](nouns/footnotes-article-reader-feature.md) | footnotes (article reader feature) | extracted | Broken via live path — ContentTreeBodyRenderer hard-returns empty footnotes (NSAttributedString()) and empty anchors dict; content_tree has no footnote node kind, so old MarkdownRenderer/FootnotePreprocessor path never runs |
| [footnotes-feature-in-62](nouns/footnotes-feature-in-62.md) | footnotes (feature in #62) | extracted | broken via live path; old MarkdownRenderer/FootnotePreprocessor markdown path no longer runs; ContentTreeBodyRenderer hard-returns empty footnotes; need recovery from content_tree literal text in paragraph nodes |
| [footnotes-in-62-context](nouns/footnotes-in-62-context.md) | Footnotes (in #62 context) | extracted | CommonMark paragraph content: literal `[^id]` reference tokens and `[^id]: body` definition lines survive as plain text inside paragraph nodes (not a CommonMark native construct); recovered in hl shell via regex scan and rendered with superscript anchors + back-links |
| [footnotes-in-article-body](nouns/footnotes-in-article-body.md) | footnotes (in article body) | extracted | Currently broken in live path; ContentTreeBodyRenderer hard-returns empty footnotes/anchors because content_tree has no footnote node kind; content is flat |
| [highlighter-ios-project](nouns/highlighter-ios-project.md) | Highlighter iOS project | extracted | xcodegen-driven project (source of truth in app/ios/Highlighter/project.yml) with no .xcworkspace or Podfile, built via pre-build script that generates Rust bindings |
| [highlighter-project-ios-build](nouns/highlighter-project-ios-build.md) | Highlighter project (iOS build) | extracted | is xcodegen-driven; app/ios/Highlighter/project.yml is source of truth; no workspace, no Podfile; pre-build script generates Swift FFI bindings from Rust |
| [highlighterapp](nouns/highlighterapp.md) | HighlighterApp | extracted | is the main app object — the separate `HighlighterApp` kernel lane, distinct from the legacy `HighlighterCore` FFI stub |
| [highlighterapp-kernel-lane](nouns/highlighterapp-kernel-lane.md) | HighlighterApp kernel lane | extracted | the main app object; separate from the legacy HighlighterCore |
| [highlightercore](nouns/highlightercore.md) | HighlighterCore | extracted | A legacy iOS FFI interface instantiated by HighlighterStore solely to provide data_dir for onboarding/podcast position stores and a diagnostics event callback; not used for product surface reads/writes |
| [highlightercore-post-91](nouns/highlightercore-post-91.md) | HighlighterCore (post-#91) | extracted | thin Swift stub; Rust object that was previously instantiated for podcast playback (not used, PodcastPlayerStore is pure-Swift) and diagnostics callback (now owned by kernel); becomes fully deletable after iOS removes the field |
| [hl-s-product-reads](nouns/hl-s-product-reads.md) | hl's product reads | extracted | Already migrated to NMP-backed kernel domains (highlight_feed, articles_feed, profiles, feedback, relays) — no upstream NMP API gap remains |
| [hl-s-read-infrastructure](nouns/hl-s-read-infrastructure.md) | hl's read infrastructure | extracted | has already migrated every product read to NMP-backed kernel domains (highlights/artifacts/profiles/feedback/relays via kernel/domains/* + NmpApp/typed projections/mailbox cache) |
| [hl-s-real-feeds-and-writes](nouns/hl-s-real-feeds-and-writes.md) | hl's real feeds and writes | extracted | upstream-blocked on an nmp-app-highlighter composition crate in the NMP repo |
| [hl-s-real-feeds-highlights-articles-room-timelines](nouns/hl-s-real-feeds-highlights-articles-room-timelines.md) | hl's real feeds (highlights/articles/room timelines) | extracted | blocked on upstream NMP app-composition; the generic vendored wasm runtime cannot serve them without an nmp-app-highlighter composition crate in NMP |
| [ios-test-framework](nouns/ios-test-framework.md) | iOS test framework | extracted | Swift Testing (import Testing; struct …Tests { @Test func … }), not XCTest |
| [issue-62](nouns/issue-62.md) | issue #62 | extracted | Article body renderer: text selection, footnotes, overlay (NMP #1695 gap) |
| [issue-95-slice-nmp-mob-014](nouns/issue-95-slice-nmp-mob-014.md) | issue #95 (SLICE-NMP-MOB-014) | extracted | Move iOS relay-list import off HighlighterCore |
| [kernel-domains](nouns/kernel-domains.md) | Kernel domains | extracted | NMP-backed domain modules (highlight_feed, articles, profiles, feedback, relays, relay_diagnostics) that serve all product reads via typed projections/snapshots; replaced the legacy nostrdb read paths |
| [kind-30004](nouns/kind-30004.md) | kind:30004 | extracted | Nostr protocol: curation set event (bookmark set management); addressable kind carrying title, description, and membership (`a` tags for curated items) |
| [legacy-nostrdb-read-publish-helpers](nouns/legacy-nostrdb-read-publish-helpers.md) | legacy nostrdb read/publish helpers | extracted | dead code—zero kernel or FFI callers; survive only as test-fixture utilities and intermediate helpers for equally-dead legacy publish functions; every product read already flows through NMP-backed kernel domains |
| [legacy-nostrdb-read-publish-helpers-hl-core](nouns/legacy-nostrdb-read-publish-helpers-hl-core.md) | Legacy nostrdb read/publish helpers (hl core) | extracted | Dead code — zero kernel/ffi callers; survive only as test-only helpers and intermediate helpers for equally-dead legacy publish functions |
| [legacy-nostrdb-read-publish-lane](nouns/legacy-nostrdb-read-publish-lane.md) | legacy nostrdb read/publish lane | extracted | Dead code; query_* and publish_* functions across highlights/profile/artifacts/feedback/relays modules have zero kernel/FFI callers; survive only as test-only helpers and superseded implementations |
| [legacy-nostrdb-read-sites](nouns/legacy-nostrdb-read-sites.md) | legacy nostrdb read sites | extracted | dead code; all product reads have been migrated to NMP-backed kernel domains, leaving these reader/publish functions with zero kernel/FFI callers |
| [loadwasmbridge](nouns/loadwasmbridge.md) | loadWasmBridge() | extracted | A ready-made function that dynamically imports the WASM glue, runs initialization, and constructs `new NmpWasmRuntime()` within a Web Worker. |
| [loop](nouns/loop.md) | /loop | extracted | autonomous default with dynamic pacing; invoked with no prompt and no interval, runs check immediately, then self-paces next iteration via ScheduleWakeup (no cron) |
| [loop-command-loop](nouns/loop-command-loop.md) | loop command (/loop) | extracted | Autonomous default mode that runs the autonomous check now, then self-paces the next iteration via ScheduleWakeup (not a recurring cron) |
| [main-7014eece](nouns/main-7014eece.md) | main @ 7014eece | extracted | hybrid architecture where body renders from kernel content_tree via ContentTreeBodyRenderer, flattening prose into NSAttributedString segments displayed in bespoke ArticleBodyView UITextView |
| [markdownrenderer](nouns/markdownrenderer.md) | MarkdownRenderer | extracted | Component that owns the Output type, footnote attribute keys, and renderFootnotes method; converts markdown to attributed strings with footnote metadata |
| [monitor](nouns/monitor.md) | Monitor | extracted | Persistent-mode task construct that arms an event observer; its events immediately wake the loop, bypassing the ScheduleWakeup deadline |
| [naddr-web-route-curation-sets](nouns/naddr-web-route-curation-sets.md) | naddr web route (curation sets) | extracted | Route IS https://highlighter.com/note/<naddr> (same as articles); naddr bech32 carries kind=30004 so any nostr-aware client decodes it correctly regardless of path |
| [nmp-backed-kernel-domains-vs-legacy-nostrdb-lane](nouns/nmp-backed-kernel-domains-vs-legacy-nostrdb-lane.md) | NMP-backed kernel domains (vs legacy nostrdb lane) | extracted | set of Rust domains (`highlight_feed.rs`, `articles_feed.rs`, `profiles.rs`, `feedback.rs`, `relays.rs`, `relay_diagnostics.rs`) that serve all live product reads via `KernelEventObserver` + NMP's typed projections and mailbox cache; the authority superseding hl's direct nostrdb queries |
| [nmp-browser-runtime](nouns/nmp-browser-runtime.md) | nmp-browser-runtime | extracted | The composition root for the WASM runtime (Rust crate exports `#[wasm_bindgen] struct NmpWasmRuntime`); built via wasm-pack `--target web`, distinct from `nmp-wasm` which is ABI-glue only. |
| [nmp-nostrwirenode](nouns/nmp-nostrwirenode.md) | NMP NostrWireNode | extracted | has no footnote concept; must not be changed |
| [nmp-wasm-bridge-the-shape-chosen-for-65](nouns/nmp-wasm-bridge-the-shape-chosen-for-65.md) | NMP WASM bridge (the shape chosen for #65) | extracted | A Worker-resident NmpWasmRuntime that handles sign requests via `set_identity` + async `begin_sign`/`deliver_signer_response` round-trips, with NIP-07 extension signing wired and NIP-46 bunker/local-key deferred to upstream NMP |
| [nostrruntime](nouns/nostrruntime.md) | NostrRuntime | extracted | legacy Rust module providing direct nostrdb/nostr-sdk client access for read and write; vestigial for product reads (all already replaced by NMP-backed kernel domains); still instantiated at app startup solely for `data_dir()` (feeds OnboardingStore/PodcastPositionStore) and `install_diagnostics_callback()` |
| [nostrruntime-functions](nouns/nostrruntime-functions.md) | NostrRuntime functions | extracted | exclusively write operations (publish*, set_relays, upsert_relay, remove_relay) with zero product callers; all product reads already migrated to NMP-backed kernel domains |
| [nostrruntime-in-91-context](nouns/nostrruntime-in-91-context.md) | NostrRuntime (in #91 context) | extracted | legacy Rust struct for writes only; every function taking &NostrRuntime is a write op (publish/set/upsert) with zero product callers; survives only for non-read jobs (data_dir, diagnostics callback) |
| [nostrruntime-instantiation](nouns/nostrruntime-instantiation.md) | NostrRuntime instantiation | extracted | in the pre-#91 system, HighlighterCore constructs NostrRuntime solely for two non-read jobs: data_dir() (onboarding/podcast position storage) and install_diagnostics_callback() (relay diagnostics hook) |
| [nostrwirenode](nouns/nostrwirenode.md) | NostrWireNode | extracted | NMP type that has no footnote concept and must not be changed to support footnotes in hl |
| [project-build-system](nouns/project-build-system.md) | project build system | extracted | xcodegen-driven, not xcworkspace-based; source of truth is app/ios/Highlighter/project.yml |
| [reading-progress-overlay](nouns/reading-progress-overlay.md) | reading progress overlay | extracted | Fully absent; no scroll-offset tracking in ArticleReaderView; ScrollAnchor state set but never consumed |
| [reading-progress-overlay-feature-in-62](nouns/reading-progress-overlay-feature-in-62.md) | reading progress overlay (feature in #62) | extracted | fully absent; no scroll-offset tracking in ArticleReaderView; ScrollAnchor state set but never consumed |
| [readingprogress](nouns/readingprogress.md) | ReadingProgress | extracted | Presentation-only state tracking scroll position (scroll offset while a view is alive); allowed exception per app/AGENTS.md |
| [relay-owned-storage](nouns/relay-owned-storage.md) | Relay-owned storage | extracted | Separate relay infrastructure (Croissant MMM event store, Bleve search, Blossom media, LiveKit, relay settings, NIP-29 moderation) distinct from app-client persistence and not in scope for mobile NMP migration |
| [runtime-nostrruntime-function](nouns/runtime-nostrruntime-function.md) | runtime:&NostrRuntime function | extracted | a write operation (publish, set_relays, etc.) with zero product callers |
| [share-url-web-links-for-articles-sets](nouns/share-url-web-links-for-articles-sets.md) | Share URL (web links for articles/sets) | extracted | https://highlighter.com/note/<naddr> — the web app's single naddr handler route for all addressable content (articles, curation sets, etc.); naddr bech32 encodes kind, author, identifier, relay hint |
| [text-selection-article-reader-feature](nouns/text-selection-article-reader-feature.md) | text selection (article reader feature) | extracted | Works — ArticleBodyView (UITextView, isSelectable) with custom Edit Menu (Highlight/Highlight-with-note) dispatching to onPublishHighlight/onRequestNote; selection-projection logic is untested and un-extracted |
| [text-selection-feature-in-62](nouns/text-selection-feature-in-62.md) | text selection (feature in #62) | extracted | works in current codebase; ArticleBodyView (UITextView, isSelectable) with custom Edit Menu (Highlight/Highlight-with-note); needs extraction of selection-projection logic into pure, tested function |
| [text-selection-in-articles](nouns/text-selection-in-articles.md) | text selection (in articles) | extracted | Currently working; implemented via ArticleBodyView (UITextView, isSelectable) with custom Edit Menu (Highlight / Highlight-with-note actions); paragraph-context projection logic extracts quote and surrounding context |
| [the-wasm-bridge-s-signer-broker](nouns/the-wasm-bridge-s-signer-broker.md) | the WASM bridge's signer broker | extracted | wired upstream for NIP-07 only; NIP-46 (bunker) and nsec are runtime-blocked |
| [web-app-naddr-routing](nouns/web-app-naddr-routing.md) | web app naddr routing | extracted | All naddr links route to /note/<naddr>, not /a/<naddr>; the pattern is used for kind:30004 curation sets and articles alike |
| [web-bridge](nouns/web-bridge.md) | web bridge | extracted | runs the generic NMP kernel (not hl's native kernel) |
| [web-chirp](nouns/web-chirp.md) | web/chirp | extracted | A working web reference app (SolidJS+Vite) that boots nmp-wasm and consumes the WASM runtime end-to-end, serving as the usage specification for the NMP web bridge. |

