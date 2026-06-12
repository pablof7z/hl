# Wiki Index

> Derived cache — do not hand-edit. Rebuilt by proactive-context after each capture.

Last updated: 2026-06-12

## android-build (2 guides)

| Slug | Title | Summary | Tags | Volatility | Verified | Topic |
|------|-------|---------|------|------------|----------|-------|
| [android-build](android-build.md) | Android Build & CI | The Android app targets only the arm64-v8a ABI | capture | warm | 2026-06-12 | android-build |
| [android-professionalization](android-professionalization.md) | Android App Professionalization | The Android app must be professionalized from a single-file reference implementation into a real, production-quality app, fixed properly and completely with no | capture | warm | 2026-06-12 | android-build |

## build-system (1 guide)

| Slug | Title | Summary | Tags | Volatility | Verified | Topic |
|------|-------|---------|------|------------|----------|-------|
| [build-system](build-system.md) | Build System | Debug APKs are placed at ~/Builds/app-debug.apk | capture | warm | 2026-06-12 | build-system |

## native-dependencies (5 guides)

| Slug | Title | Summary | Tags | Volatility | Verified | Topic |
|------|-------|---------|------|------------|----------|-------|
| [actor-blocking-fix](actor-blocking-fix.md) | Actor Blocking Fix | The deep architectural issue of ~92 blocking network awaits inside the NMP actor loop causes the actor to wedge on a dead network, preventing even the 30s timeo | capture | warm | 2026-06-12 | native-dependencies |
| [native-dependencies](native-dependencies.md) | Native Dependencies | The native library libhighlighter_core.so dynamically links against libsodium and requires the symbol crypto_stream_chacha20_ietf_xor_ic. | capture | warm | 2026-06-12 | native-dependencies |
| [op-runner](op-runner.md) | OpRunner & Async Migration | The OpRunner primitive uses a shared 2-thread tokio runtime, a domain-keyed in-flight registry with generation-based supersession and AbortHandle cancellation, | capture | warm | 2026-06-12 | native-dependencies |
| [rust-code-hygiene](rust-code-hygiene.md) | Rust Code Hygiene | The Rust core must not contain todo!() panics or dead legacy API modules | capture | warm | 2026-06-12 | native-dependencies |
| [rust-visibility-conventions](rust-visibility-conventions.md) | Rust Visibility Conventions | Cross-file symbols are marked internal while symbols used within a single file remain private. | capture | warm | 2026-06-12 | native-dependencies |

## nmp-app (8 guides)

| Slug | Title | Summary | Tags | Volatility | Verified | Topic |
|------|-------|---------|------|------------|----------|-------|
| [account-creation](account-creation.md) | Account Creation | Account creation no longer bricks when the NIP-05 availability check fails; a failed check lets the user proceed (the claim is skipped) | capture | warm | 2026-06-12 | nmp-app |
| [android-architecture](android-architecture.md) | Android Architecture | The Android app's single-file MainActivity.kt (3,317 lines) is refactored into 23 cohesive files with per-feature packages, each panel constructing its own High | capture | warm | 2026-06-12 | nmp-app |
| [android-deep-links](android-deep-links.md) | Android Deep Links | Android deep links use two manifest intent-filters: a verified autoVerify App Link for `https://beta.highlighter.com/highlight/{token}` and a `highlighter://hig | capture | warm | 2026-06-12 | nmp-app |
| [android-relay-settings](android-relay-settings.md) | Android Relay Settings | Android Settings â Network lists all configured relays with live status dots and roles, with add/remove capability | capture | warm | 2026-06-12 | nmp-app |
| [android-session-persistence](android-session-persistence.md) | Android Session Persistence | Android session credentials are persisted via EncryptedSharedPreferences, surviving force-stop/relaunch with the same user identity restored from cache. | capture | warm | 2026-06-12 | nmp-app |
| [ios-testing](ios-testing.md) | iOS Testing | The iOS first unit test suite contains 22 tests in 2 suites (TranscriptParserTests with 14 tests and CommentTreeBuilderTests with 8 tests) using the Swift Testi | capture | warm | 2026-06-12 | nmp-app |
| [nmp-app-facade](nmp-app-facade.md) | NMP App Facade | The app uses the NMP app facade (nmp_app.rs) from core, with HighlighterStore holding a HighlighterNmpApp instance and a HighlighterAppStateReconciler that rece | capture | warm | 2026-06-11 | nmp-app |
| [platform-local-state](platform-local-state.md) | Platform-Local State | iOS retains PodcastPlayerStore (AVPlayer position) and CaptureStore (local OCR pipeline) as transient device-local state outside the Rust core. | capture | warm | 2026-06-12 | nmp-app |

## ui-components (5 guides)

| Slug | Title | Summary | Tags | Volatility | Verified | Topic |
|------|-------|---------|------|------------|----------|-------|
| [android-composition-patterns](android-composition-patterns.md) | Android Composition Patterns | Leaf row composables (HydratedHighlightRow, DiscussionRow, FeedbackThreadRow, CommentRow, ArtifactPickerRow) keep local () -> Unit parameters composed by their | capture | warm | 2026-06-12 | ui-components |
| [android-navigation](android-navigation.md) | Android Navigation & Back Stack | System back navigation closes the innermost open overlay (comments → invite → room → article → profile → feedback thread) before exiting, with predictive back e | capture | warm | 2026-06-12 | ui-components |
| [android-theming](android-theming.md) | Android Theming & Brand Palette | The brand palette maps to Material theme tokens as Paperâbackground, InkâonSurface, MutedâonSurfaceVariant, Lineâoutline, Mossâprimary, Goldâseconda | capture | warm | 2026-06-12 | ui-components |
| [ios-safety-and-navigation](ios-safety-and-navigation.md) | iOS Safety & Navigation | iOS fixes five crash risks: BookScannerView layer cast, MarkdownRenderer attributedString cast and three URL(string:) force-unwraps, OCRStructureReconstructor m | capture | warm | 2026-06-12 | ui-components |
| [ui-components](ui-components.md) | UI Components | The AuthorAvatar view (Profile/AuthorAvatar.swift) is a custom SwiftUI component, not sourced from an nmp UI library | capture | warm | 2026-06-11 | ui-components |

## Research Records (4 records)

| Record | Date | Finding | Agent |
|--------|------|---------|-------|
| [2026-06-12-1-android-app-rebuild-acceptance-testing-persistence](research/2026-06-12-1-android-app-rebuild-acceptance-testing-persistence.md) | 2026-06-12 | Android app rebuild acceptance testing: persistence test PASSED (force-stop/relaunch retains login), build verified on emulator with 89 highlights and 24 rooms loaded | Rebuild Android app navigation |
| [2026-06-12-1-phase-0-1-oprunner-implementation-evaluation](research/2026-06-12-1-phase-0-1-oprunner-implementation-evaluation.md) | 2026-06-12 | Phase 0+1 OpRunner implementation evaluation: 10 handler migrations verified against design-doc criteria, 250 tests pass, lint clean, clippy clean, Android build green, empirical proof old block_on starves ≥2.5s while new shape resolves <500ms, one documented deviation (entity-resolve correctly not migrated) | a076365ff131365d2 |
| [2026-06-12-1-phase-2-oprunner-migration-report-26](research/2026-06-12-1-phase-2-oprunner-migration-report-26.md) | 2026-06-12 | Phase 2 OpRunner migration report: 26 publish handlers migrated to OpRunner, five verification gates green, zero Class-B blocking sites remaining, with test results and deviation analysis | OpRunner Phase 2 publishes subagent |
| [2026-06-12-2-adversarial-review-of-actor-blocking-fix](research/2026-06-12-2-adversarial-review-of-actor-blocking-fix.md) | 2026-06-12 | Adversarial review of actor-blocking fix: verdict SHIP with two should-fix UX gaps (JoinRoom and CurationWrite missing busy flags), systematic 9-dimension code audit with line-number citations | Adversarial review of OpRunner work subagent |

## Episode Cards (61 cards)

| Card | Date | Title | Salience | Status |
|------|------|-------|----------|--------|
| [2026-06-12-1-android-apk-crash-from-empty-libsodium](episodes/2026-06-12-1-android-apk-crash-from-empty-libsodium.md) | 2026-06-12 | Android APK crash from empty libsodium static archive in cross-compilation | root-cause | active |
| [2026-06-12-1-android-app-professionalization-directive](episodes/2026-06-12-1-android-app-professionalization-directive.md) | 2026-06-12 | Android app professionalization directive | architecture | active |
| [2026-06-12-1-android-app-restructured-from-single-file](episodes/2026-06-12-1-android-app-restructured-from-single-file.md) | 2026-06-12 | Android app restructured from single-file dump to navigation architecture | reversal | active |
| [2026-06-12-1-android-app-single-column-debug-harness](episodes/2026-06-12-1-android-app-single-column-debug-harness.md) | 2026-06-12 | Android app: single-column debug harness → real navigation architecture | reversal | active |
| [2026-06-12-1-android-monolith-nmp-aligned-modular-architecture](episodes/2026-06-12-1-android-monolith-nmp-aligned-modular-architecture.md) | 2026-06-12 | Android monolith → NMP-aligned modular architecture | architecture | superseded |
| [2026-06-12-1-android-monolith-refactored-to-modular-nmp](episodes/2026-06-12-1-android-monolith-refactored-to-modular-nmp.md) | 2026-06-12 | Android monolith refactored to modular NMP architecture | architecture | superseded |
| [2026-06-12-1-android-rebuilt-from-single-file-skeleton](episodes/2026-06-12-1-android-rebuilt-from-single-file-skeleton.md) | 2026-06-12 | Android rebuilt from single-file skeleton to real Compose app | architecture | active |
| [2026-06-12-1-android-reference-skeleton-production-track](episodes/2026-06-12-1-android-reference-skeleton-production-track.md) | 2026-06-12 | Android: Reference Skeleton → Production Track | reversal | active |
| [2026-06-12-1-apk-crash-from-unlinked-libsodium-dependency](episodes/2026-06-12-1-apk-crash-from-unlinked-libsodium-dependency.md) | 2026-06-12 | APK crash from unlinked libsodium dependency in Rust core | root-cause | active |
| [2026-06-12-1-ios-share-links-minted-but-never](episodes/2026-06-12-1-ios-share-links-minted-but-never.md) | 2026-06-12 | iOS share links minted but never consumed | reversal | active |
| [2026-06-12-1-nmp-actor-thread-blocking-defect-diagnosed](episodes/2026-06-12-1-nmp-actor-thread-blocking-defect-diagnosed.md) | 2026-06-12 | NMP Actor-Thread Blocking Defect Diagnosed | root-cause | superseded |
| [2026-06-12-1-nmp-facade-actor-thread-blocks-on](episodes/2026-06-12-1-nmp-facade-actor-thread-blocks-on.md) | 2026-06-12 | NMP facade actor thread blocks on network calls, wedging UI indefinitely | architecture | superseded |
| [2026-06-12-1-nmp-typed-projection-drain-was-unwired](episodes/2026-06-12-1-nmp-typed-projection-drain-was-unwired.md) | 2026-06-12 | NMP typed-projection drain was unwired, starving relay status on both platforms | root-cause | superseded |
| [2026-06-12-1-oprunner-actor-architecture-eliminates-nmp-blocking](episodes/2026-06-12-1-oprunner-actor-architecture-eliminates-nmp-blocking.md) | 2026-06-12 | OpRunner actor architecture eliminates NMP blocking | architecture | active |
| [2026-06-12-1-oprunner-pattern-adopted-to-eliminate-actor](episodes/2026-06-12-1-oprunner-pattern-adopted-to-eliminate-actor.md) | 2026-06-12 | OpRunner pattern adopted to eliminate actor-thread blocking | architecture | superseded |
| [2026-06-12-1-relay-diagnostics-never-reached-native-platforms](episodes/2026-06-12-1-relay-diagnostics-never-reached-native-platforms.md) | 2026-06-12 | Relay diagnostics never reached native platforms — snapshot frame decoding gap | root-cause | superseded |
| [2026-06-12-1-relay-diagnostics-starved-by-undecoded-snapshot](episodes/2026-06-12-1-relay-diagnostics-starved-by-undecoded-snapshot.md) | 2026-06-12 | Relay diagnostics starved by undecoded snapshot frames | root-cause | superseded |
| [2026-06-12-2-account-creation-bricked-by-failed-nip](episodes/2026-06-12-2-account-creation-bricked-by-failed-nip.md) | 2026-06-12 | Account creation bricked by failed NIP-05 availability check | product | superseded |
| [2026-06-12-2-account-creation-bricked-when-nip-05](episodes/2026-06-12-2-account-creation-bricked-when-nip-05.md) | 2026-06-12 | Account creation bricked when NIP-05 API is unreachable | root-cause | superseded |
| [2026-06-12-2-android-app-rebuilt-from-single-file](episodes/2026-06-12-2-android-app-rebuilt-from-single-file.md) | 2026-06-12 | Android app rebuilt from single-file reference to production quality | product | active |
| [2026-06-12-2-android-app-restructuring-from-single-file](episodes/2026-06-12-2-android-app-restructuring-from-single-file.md) | 2026-06-12 | Android app restructuring from single-file reference to production architecture | reversal | active |
| [2026-06-12-2-android-dark-mode-hardcoded-colors-materialtheme](episodes/2026-06-12-2-android-dark-mode-hardcoded-colors-materialtheme.md) | 2026-06-12 | Android dark mode: hardcoded colors → MaterialTheme tokens across 16 files | product | active |
| [2026-06-12-2-android-dark-mode-via-material-theme](episodes/2026-06-12-2-android-dark-mode-via-material-theme.md) | 2026-06-12 | Android dark mode via Material theme tokens | product | active |
| [2026-06-12-2-android-eventbridge-root-cause-setcoreeventcallback-was](episodes/2026-06-12-2-android-eventbridge-root-cause-setcoreeventcallback-was.md) | 2026-06-12 | Android EventBridge root cause: setCoreEventCallback was never called | root-cause | active |
| [2026-06-12-2-android-gains-full-dark-mode-via](episodes/2026-06-12-2-android-gains-full-dark-mode-via.md) | 2026-06-12 | Android gains full dark mode via Material3 theme token sweep | product | active |
| [2026-06-12-2-android-monolithic-to-modular-architecture](episodes/2026-06-12-2-android-monolithic-to-modular-architecture.md) | 2026-06-12 | Android: Monolithic to Modular Architecture | architecture | active |
| [2026-06-12-2-android-nmp-event-bridge-missing-relay](episodes/2026-06-12-2-android-nmp-event-bridge-missing-relay.md) | 2026-06-12 | Android NMP event bridge missing — relay state and login deltas silently dropped | root-cause | active |
| [2026-06-12-2-ios-never-consumed-its-own-share](episodes/2026-06-12-2-ios-never-consumed-its-own-share.md) | 2026-06-12 | iOS never consumed its own share links — Android became first, then iOS caught up | product | active |
| [2026-06-12-2-share-link-routing-android-first-platform](episodes/2026-06-12-2-share-link-routing-android-first-platform.md) | 2026-06-12 | Share link routing — Android first platform to consume bech32 links end-to-end | product | active |
| [2026-06-12-2-signup-blocked-on-nip-05-api](episodes/2026-06-12-2-signup-blocked-on-nip-05-api.md) | 2026-06-12 | Signup Blocked on NIP-05 API Failure | product | superseded |
| [2026-06-12-3-cross-platform-relay-status-stuck-at](episodes/2026-06-12-3-cross-platform-relay-status-stuck-at.md) | 2026-06-12 | Cross-platform relay status stuck at UNKNOWN: core never decoded diagnostics from snapshot frames | root-cause | superseded |
| [2026-06-12-3-ios-cannot-route-the-share-links](episodes/2026-06-12-3-ios-cannot-route-the-share-links.md) | 2026-06-12 | iOS cannot route the share links it mints | architecture | active |
| [2026-06-12-3-ios-force-unwrap-crash-risks-replaced](episodes/2026-06-12-3-ios-force-unwrap-crash-risks-replaced.md) | 2026-06-12 | iOS force-unwrap crash risks replaced with safe patterns | root-cause | active |
| [2026-06-12-3-nmp-actor-thread-blocks-on-network](episodes/2026-06-12-3-nmp-actor-thread-blocks-on-network.md) | 2026-06-12 | NMP actor thread blocks on network I/O — OpRunner design | architecture | superseded |
| [2026-06-12-3-nmp-fully-adopted-on-native-platforms](episodes/2026-06-12-3-nmp-fully-adopted-on-native-platforms.md) | 2026-06-12 | NMP Fully Adopted on Native Platforms; Web Intentionally Excluded | architecture | active |
| [2026-06-12-3-relay-status-never-surfaced-to-ui](episodes/2026-06-12-3-relay-status-never-surfaced-to-ui.md) | 2026-06-12 | Relay status never surfaced to UI — diagnostics projection trapped inside actor frames | root-cause | superseded |
| [2026-06-12-3-relay-status-never-updated-in-ui](episodes/2026-06-12-3-relay-status-never-updated-in-ui.md) | 2026-06-12 | Relay Status Never Updated in UI | root-cause | superseded |
| [2026-06-12-3-relay-status-never-updated-kernel-diagnostics](episodes/2026-06-12-3-relay-status-never-updated-kernel-diagnostics.md) | 2026-06-12 | Relay status never updated — kernel diagnostics projection not decoded | root-cause | active |
| [2026-06-12-3-rust-core-tracing-was-silently-discarded](episodes/2026-06-12-3-rust-core-tracing-was-silently-discarded.md) | 2026-06-12 | Rust core tracing was silently discarded on both platforms — platform logging now wired | architecture | active |
| [2026-06-12-3-system-back-navigation-closes-overlays-before](episodes/2026-06-12-3-system-back-navigation-closes-overlays-before.md) | 2026-06-12 | System back navigation closes overlays before exiting app | product | active |
| [2026-06-12-3-system-back-navigation-closes-overlays-in](episodes/2026-06-12-3-system-back-navigation-closes-overlays-in.md) | 2026-06-12 | System back navigation closes overlays in order | product | active |
| [2026-06-12-4-account-creation-impossible-when-nip-05](episodes/2026-06-12-4-account-creation-impossible-when-nip-05.md) | 2026-06-12 | Account creation impossible when NIP-05 check fails | root-cause | active |
| [2026-06-12-4-android-build-ndk-llvm-ar-required](episodes/2026-06-12-4-android-build-ndk-llvm-ar-required.md) | 2026-06-12 | Android Build: NDK llvm-ar Required for Libsodium Cross-Compilation | root-cause | active |
| [2026-06-12-4-android-promoted-from-single-file-reference](episodes/2026-06-12-4-android-promoted-from-single-file-reference.md) | 2026-06-12 | Android promoted from single-file reference implementation to production app | reversal | active |
| [2026-06-12-4-branded-adaptive-launcher-icon-replaces-default](episodes/2026-06-12-4-branded-adaptive-launcher-icon-replaces-default.md) | 2026-06-12 | Branded adaptive launcher icon replaces default robot | product | active |
| [2026-06-12-4-ios-could-not-open-its-own](episodes/2026-06-12-4-ios-could-not-open-its-own.md) | 2026-06-12 | iOS Could Not Open Its Own Share Links | product | active |
| [2026-06-12-4-ios-force-unwrap-crash-risks-eliminated](episodes/2026-06-12-4-ios-force-unwrap-crash-risks-eliminated.md) | 2026-06-12 | iOS force-unwrap crash risks eliminated across five views | root-cause | active |
| [2026-06-12-4-nmp-actor-loop-blocks-on-92](episodes/2026-06-12-4-nmp-actor-loop-blocks-on-92.md) | 2026-06-12 | NMP actor loop blocks on ~92 network awaits | root-cause | superseded |
| [2026-06-12-4-platform-logging-root-cause-rust-tracing](episodes/2026-06-12-4-platform-logging-root-cause-rust-tracing.md) | 2026-06-12 | Platform logging root cause: Rust tracing output dropped on both platforms | root-cause | active |
| [2026-06-12-4-relay-status-diagnostics-never-reached-ui](episodes/2026-06-12-4-relay-status-diagnostics-never-reached-ui.md) | 2026-06-12 | Relay status diagnostics never reached UI | product | superseded |
| [2026-06-12-4-rust-core-crash-causing-todo-stubs](episodes/2026-06-12-4-rust-core-crash-causing-todo-stubs.md) | 2026-06-12 | Rust core crash-causing todo!() stubs removed | root-cause | active |
| [2026-06-12-4-rust-core-todo-panic-traps-removed](episodes/2026-06-12-4-rust-core-todo-panic-traps-removed.md) | 2026-06-12 | Rust core todo!() panic traps removed | root-cause | active |
| [2026-06-12-5-android-app-rebuilt-from-single-file](episodes/2026-06-12-5-android-app-rebuilt-from-single-file.md) | 2026-06-12 | Android app rebuilt from single-file skeleton to production | reversal | active |
| [2026-06-12-5-android-deep-links-first-platform-to](episodes/2026-06-12-5-android-deep-links-first-platform-to.md) | 2026-06-12 | Android deep links — first platform to consume share links end-to-end | product | active |
| [2026-06-12-5-android-upgraded-from-reference-skeleton-to](episodes/2026-06-12-5-android-upgraded-from-reference-skeleton-to.md) | 2026-06-12 | Android Upgraded from Reference Skeleton to Production App | reversal | active |
| [2026-06-12-5-ci-sibling-repo-path-dependency-discovered](episodes/2026-06-12-5-ci-sibling-repo-path-dependency-discovered.md) | 2026-06-12 | CI sibling-repo path dependency discovered and fixed | root-cause | active |
| [2026-06-12-5-encrypted-session-persistence-on-android](episodes/2026-06-12-5-encrypted-session-persistence-on-android.md) | 2026-06-12 | Encrypted session persistence on Android | product | active |
| [2026-06-12-5-groups-rs-signature-verification-gap-nostrdb](episodes/2026-06-12-5-groups-rs-signature-verification-gap-nostrdb.md) | 2026-06-12 | groups.rs signature verification gap: nostrdb strips signatures | root-cause | active |
| [2026-06-12-5-nmp-adoption-scope-complete-on-native](episodes/2026-06-12-5-nmp-adoption-scope-complete-on-native.md) | 2026-06-12 | NMP adoption scope: complete on native, web deliberately outside | architecture | active |
| [2026-06-12-6-nmp-adoption-100-on-native-0](episodes/2026-06-12-6-nmp-adoption-100-on-native-0.md) | 2026-06-12 | NMP adoption: 100% on native, 0% on web — deliberate architectural boundary or debt | architecture | active |
| [2026-06-12-7-android-relay-connection-failure-rust-core](episodes/2026-06-12-7-android-relay-connection-failure-rust-core.md) | 2026-06-12 | Android relay connection failure: Rust core never initializes Android logger | root-cause | active |

