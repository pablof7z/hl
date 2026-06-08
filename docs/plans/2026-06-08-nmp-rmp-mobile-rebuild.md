# NMP/RMP Mobile Rebuild Plan
## Version 1.0 | June 8, 2026

This plan rebuilds the Highlighter native app around a strict Rust Multi-Platform / Nostr Multi-Platform architecture. The current iOS app is the visual reference. The rebuilt iOS app must preserve that UI exactly unless a later product decision explicitly changes it. All product behavior, Nostr policy, state machines, validation, persistence, routing, retry, cache policy, and cross-platform invariants move into Rust. Swift and Kotlin render native UI and execute OS capabilities only.

Initial targets are iOS and Android. Web and relay work are out of scope unless a mobile contract exposes a documented protocol bug.

## Non-Negotiables

1. Preserve the existing iOS UI verbatim.
   - SwiftUI view layout, navigation hierarchy, copy, design tokens, visual density, and interaction shape are the baseline.
   - Refactors may change how views receive state and send actions, not what they render.
   - Android uses native Jetpack Compose equivalents that match the same product hierarchy and visual language while respecting Android navigation, back behavior, accessibility, and platform controls.
2. Rust is the single writer for app facts.
   - Native shells do not own business logic, protocol policy, cache invalidation, retry, relay routing, signer policy, bookmark membership, feed sorting, notification semantics, search semantics, comment threading, highlight publishing, room membership, or onboarding completion.
3. FFI is dispatch plus bounded snapshots.
   - Native dispatches actions and receives state updates.
   - Dispatch is fire-and-forget and does not return operation success.
   - Errors are represented in Rust-owned state as typed toast, inline diagnostic, field validation, or action-stage values.
   - No per-operation `Result`, thrown exception, or one-off query API crosses the final FFI boundary.
4. Native capabilities execute; Rust decides.
   - iOS and Android provide raw results for secure storage, signing handoff, camera, OCR, share intake, media playback, files/photos, network reachability, push, and OS URL opening.
   - Rust owns what the raw result means and what happens next.
5. No migrations, no compatibility bridge, no staged old path.
   - This is a clean rebuild. The final tree must not contain a second app architecture, shimmed business logic, fake data, temporary bridge APIs, or TODO debt.
   - If local app data must be reset because the schema is replaced, the reset is a deliberate clean-start decision, not a migration layer.
6. No mocks, fake stubs, or unwired surfaces.
   - Every shipped screen action is backed by real Rust behavior or a real native capability.
   - Screens with unavailable platform capability must expose a real disabled/unavailable state owned by Rust, not mock success.
7. Performance is a correctness gate.
   - Open-view snapshots are bounded.
   - No event store, signer state, relay cache, history, or unbounded list crosses FFI.
   - No polling or sleep-check loops in production Rust, Swift, Kotlin, or background jobs.
   - UI update cadence is coalesced and capped per view.

## Current Audit

The existing repository is not greenfield:

| Area | Current State | Rebuild Consequence |
|---|---|---|
| Rust core | `app/core` exists with a large UniFFI object and feature modules. | Reuse product/protocol knowledge, not the FFI shape. Replace query/subscribe APIs with a TEA kernel. |
| iOS shell | `app/ios/Highlighter` has a full SwiftUI app and share extension. | Preserve views and platform-specific UX. Delete Swift business stores as state owners. |
| Android shell | No `app/android` project exists. | Build Android first-class with Kotlin, Compose, Android signer support, and capability bridges. |
| Web app | SvelteKit production client exists. | Not part of the rebuild except as a type/reference source where current Rust mirrors web records. |
| Relay | Go relay exists. | Keep protocol assumptions aligned, but do not move mobile behavior into relay work. |

The NMP scanner and source audit found these blocking classes:

| Rule | Evidence | Required Fix |
|---|---|---|
| D3 relay routing | Hardcoded relay constants in `app/core/src/relays.rs`, `feedback.rs`, Nostr Connect setup, search defaults, and share hints. | Replace app-facing relay choices with Rust-owned routing policy, user/app relay config, outbox planning, and audited opt-outs. |
| D4 single source of truth | Swift `HighlighterStore`, feature stores, profile/web/ISBN caches, bookmark sets, pending joins, recent searches, App Group mirrors. | Rust owns all durable and derived app facts. Native holds only current projected view state and transient OS handles. |
| D5 bounded FFI | Current FFI exposes broad `get_*`, `search_*`, `subscribe_*`, `publish_*` calls and view stores re-query full slices. | Open/close views with bounded screen snapshots and typed updates. No event store or unbounded lists cross FFI. |
| D6 errors in state | UniFFI surface returns `Result<T, CoreError>` and Swift wraps it in `async throws`. | Final FFI exposes dispatch-only actions and state snapshots carrying typed failures. |
| D7 capability bridge | Swift uploads Blossom blobs, performs Keychain session restore decisions, owns share queue behavior, formats some policy values. | Native reports raw capability results; Rust decides policy, persistence, retry, and user-facing state. |
| D8 reactivity | Production Swift uses sleeps for toast reset, debounces, search loading fallbacks, invite copy state, and view-level subscriptions. Rust tests and some code use sleep loops. | Replace production sleep/poll loops with actor timers, injected clocks, subscription lifecycle events, and bounded coalescers. |
| D9 kernel time | Rust uses wall-clock reads in metadata/list/NIP-46 policy code. | Introduce injected kernel clock. Reducers, replay, expiration, replaceable resolution, and publish timestamps use the clock. |

## Target Architecture

### Package Layout

Final mobile tree:

```text
app/
  core/
    Cargo.toml
    src/
      lib.rs
      kernel/
        app.rs
        action.rs
        actor.rs
        clock.rs
        effect.rs
        snapshot.rs
        view.rs
      capabilities/
        audio.rs
        camera.rs
        files.rs
        keychain.rs
        nostr_signer.rs
        ocr.rs
        share.rs
        url_open.rs
      domain/
        artifacts/
        auth/
        bookmarks/
        capture/
        comments/
        discovery/
        feedback/
        groups/
        highlights/
        media/
        profile/
        reads/
        relays/
        search/
      nostr/
        event_store.rs
        outbox.rs
        private_events.rs
        routing.rs
        signer.rs
        subscriptions.rs
        sync.rs
      ffi/
        mod.rs
        android.rs
        ios.rs
  ios/
    Highlighter/
      Sources/
        Highlighter/
          Core/
          Features/
          Navigation/
        ShareExtension/
        Shared/
  android/
    settings.gradle.kts
    build.gradle.kts
    app/
      build.gradle.kts
      src/main/java/com/highlighter/app/
```

The Rust crate can remain named `highlighter-core` for package continuity, but its public API is rebuilt. Existing modules may be moved or rewritten only when they satisfy the new ownership boundaries.

### Runtime Model

Highlighter uses a single Rust actor:

```text
Native UI
  -> dispatch(AppAction)
Rust Kernel Actor
  -> reducer updates AppState
  -> effects start async protocol/capability work
  -> internal events return nondeterministic inputs
  -> view projections emit bounded snapshots
Native UI
  -> render snapshot, execute capability requests
```

Rules:

- `AppState` is the single app model. Feature submodels are co-located by domain.
- `AppAction` is the only app-facing input from native UI.
- `KernelEvent` is the only input from async Rust work or native capability results.
- Reducers never await.
- Effects are idempotent and cancellable by view/session lifecycle.
- `ViewId` scopes every open screen, sheet, and app chrome projection.
- Snapshots are full screen-shaped projections by default; deltas are allowed only after profiling and must be lossless.
- Native navigation can use native transitions, but Rust owns the route stack/tab/sheet semantic state.

### Final FFI Surface

The final UniFFI surface should be small and stable:

| API | Purpose |
|---|---|
| `HighlighterApp::new(config)` | Construct the kernel. Constructor may fail only for unrecoverable local initialization; recoverable failures are state. |
| `set_observer(observer)` | Register a platform observer for snapshots and capability requests. |
| `dispatch(action)` | Fire-and-forget user/platform action. No success return. |
| `open_view(view_id, route)` | Register a bounded projection. |
| `close_view(view_id)` | Drop projection and cancel view-scoped effects. |
| `current_snapshot(view_id)` | Read latest snapshot for rendering/recovery. |
| `resume()` / `suspend()` | Platform lifecycle input. |
| `provide_capability_result(result)` | Return raw native capability output. |
| `shutdown()` | Idempotent cleanup. |

Generated bindings may still contain low-level UniFFI implementation errors internally, but app-authored Swift/Kotlin must not call broad throwing product APIs.

### State And Ownership

| Fact | Single Writer | Native Role |
|---|---|---|
| Current user/session state | Rust auth domain | Securely store/load secret material only when Rust requests it. |
| Onboarding completion | Rust app state | Render onboarding route. |
| Joined communities | Rust groups domain | Render list/picker snapshots. |
| Pending joins and membership toasts | Rust groups domain | Render toast/banner state. |
| Profiles and metadata | Rust profile domain/event store | Render projected author fields and placeholders. |
| Web metadata and ISBN previews | Rust artifacts/media domains | Render projected card fields. |
| Bookmarks and curation sets | Rust bookmarks domain | Render menu/sheet state and dispatch user intent. |
| Search query, debounce, relay progress | Rust search domain | Render query text, sections, loading/progress state. |
| Room home, lanes, chat, discussions | Rust groups/comments/artifacts domains | Render bounded open-room snapshots. |
| Highlight composer and publish stages | Rust highlights/capture domains | Render field state and capability prompts. |
| OCR reconstruction | Rust capture domain | iOS/Android return raw OCR observations. |
| Audio clip semantics | Rust media domain | Native audio engine executes play/pause/seek and reports raw progress. |
| Relay roles/routing/diagnostics | Rust relays/nostr domains | Render diagnostics and execute raw NIP-11 probe only if requested as capability. |
| Share extension queue | Rust share domain | Extension writes raw incoming payload; app drains by dispatching a raw share-intake result. |

### Capability Bridges

| Capability | iOS | Android | Rust Owns |
|---|---|---|---|
| Secure storage | Keychain | Android Keystore / EncryptedSharedPreferences | What secrets exist, restore policy, logout cleanup, error display. |
| Signer handoff | URL scheme / Nostr Connect | NIP-55 signer intents plus NIP-46 | Auth state machine, permissions, retry, relay selection, signer errors. |
| Camera/photos | AVFoundation, PhotosUI | CameraX, Photo Picker | Capture flow state, accepted image policy, publish decision. |
| OCR | Vision text recognition | ML Kit Text Recognition | Reconstruction, selection, quote/context derivation, confidence policy. |
| Share intake | Share Extension App Group | Android Sharesheet intent | Parsing, dedupe, artifact preview pipeline, target community state. |
| Audio playback | AVPlayer/AVAudioSession | ExoPlayer/MediaSession | Playback state, clip range, resume policy, highlight draft creation. |
| Blossom HTTP upload | URLSession raw upload capability or Rust HTTP when feasible | OkHttp/raw upload capability or Rust HTTP when feasible | Server choice, NIP-98 signing, retry, upload result interpretation. |
| External URL | `openURL` | Intent launcher | Which URL may open and what route follows. |

Capability lifecycles must be idempotent: repeated start, stop, restart, app suspend, and app resume are legal.

## Feature Coverage

The rebuilt app must ship all currently visible iOS product surfaces:

| Surface | Rust Domains | Native UI |
|---|---|---|
| Auth/onboarding | auth, profile, relays | Existing SwiftUI onboarding/login, Compose equivalents. |
| Root navigation | kernel route state | Existing tab/root views, Compose root nav. |
| Communities/discover | groups, discovery, recommendations | Existing room explorer/list/card views, Compose equivalents. |
| Room home | groups, artifacts, highlights, comments, chat | Existing room header, lanes, tabs, cards, chat/discussion views. |
| Capture/book scanner | capture, artifacts, highlights, groups, media | Existing camera/OCR UI, Android CameraX flow. |
| Article/book/podcast readers | artifacts, reads, highlights, comments, media | Existing readers and podcast player UI, Android native equivalents. |
| Highlights/feed/detail | highlights, artifacts, comments, reactions, bookmarks | Existing cards/detail/action sheets. |
| Profile/edit/following | profile, follows, reads, groups | Existing profile views/sheets. |
| Search | search, relays, artifacts, profiles, groups, highlights | Existing search screens. |
| Bookmarks/vault | bookmarks, lists, artifacts | Existing bookmarks and set detail UI. |
| Settings/network/media/keys | relays, media, auth | Existing settings screens. |
| Feedback | feedback, comments | Existing feedback list/detail/new thread views. |
| Share extension | share, artifacts, groups | Existing iOS share UI plus Android sharesheet entry. |
| What's New | app/changelog state | Existing sheet; Rust owns seen state. |

Anything kept in the app must be wired to real behavior. If a surface is removed from scope, remove its UI entry point in the same change and record the product decision.

## Implementation Phases

### Phase 0: Freeze UI And Contracts

Deliverables:

- Capture the current iOS screen inventory and entry points.
- Record screenshot baselines for primary screens on a simulator.
- Add a `ui-preservation` checklist mapping each SwiftUI view to its new Rust projection.
- Freeze product event kinds and NIP usage against `docs/product-spec-v2.0.md` and `docs/technical-architecture.md`.

Gate:

- A documented view/projection inventory exists.
- iOS still builds before architecture replacement begins.

### Phase 1: Build The Rust Kernel Skeleton With Real App State

Deliverables:

- Replace the broad UniFFI object with the final dispatch/snapshot/capability FFI.
- Add `AppAction`, `KernelEvent`, `AppState`, `ViewRoute`, `ViewSnapshot`, `CapabilityRequest`, and `CapabilityResult`.
- Implement the actor, reducer, effect runner, view registry, coalesced observer updates, injected clock, and deterministic test harness.
- Port session restore, logout, onboarding route, root tab route, and app chrome state into Rust.

Gate:

- Rust tests prove dispatch does not return operation success, reducers do not await, snapshots are bounded by open views, and injected time controls route/session behavior.
- iOS root shell renders from Rust app/root snapshots with no Swift-owned app facts.

### Phase 2: Auth, Signers, Relays, And Event Store

Deliverables:

- Rebuild auth around Rust-owned session state.
- Implement iOS Keychain and Android secure-storage capabilities as raw secret storage only.
- Implement NIP-46 and Android NIP-55 capability flows with Rust-owned signer policy.
- Replace hardcoded relay decisions with Rust routing policy, outbox planning, app/user relay roles, and audited Highlighter relay bootstrap.
- Move relay diagnostics and network settings into Rust snapshots/actions.
- Keep nostrdb/event-store internals inside Rust.

Gate:

- No app-authored native throwing product API remains for auth or relays.
- Scanner no longer reports D3 hardcoded production relay decisions outside audited configuration modules.
- Login/logout/session restore work on iOS and Android with real signer/secret flows.

### Phase 3: App Chrome, Discovery, Rooms, And Profiles

Deliverables:

- Port joined communities, room explorer, featured/friends rooms, room home, profile cache, follows, and membership state.
- Native room/profile stores become projection adapters only or disappear.
- Rust owns placeholders for unavailable profile/metadata, not native loading gates.

Gate:

- Communities, discover, room home, profile, follow/unfollow, join requests, and membership toasts work from Rust projections.
- Snapshot sizes are bounded by visible shelves, tabs, and limits.

### Phase 4: Artifacts, Highlights, Comments, Bookmarks, And Search

Deliverables:

- Port artifact preview/build/share, highlight publish/share/detail, NIP-22 comments, reactions, bookmarks/curation sets, and search.
- Rust owns optimistic stages and reconciliation.
- Search debounce, relay progress, timeout, and history are Rust state/effects using injected time.

Gate:

- All visible article/book/highlight/comment/bookmark/search actions work without native business stores.
- No production sleeps or polling loops remain for search, comments, bookmark, toast, or subscription behavior.

### Phase 5: Capture, OCR, Media, Share, And Platform-Specific Capabilities

Deliverables:

- Move OCR reconstruction, ISBN preview caching, capture draft state, community picker state, and publish decisions to Rust.
- iOS returns raw Vision observations and image buffers; Android returns raw ML Kit observations and image buffers.
- Move podcast playback state, resume policy, clip semantics, transcript parsing policy, and clip publish flow to Rust.
- Native audio engines execute play/pause/seek and report raw progress at a bounded cadence.
- Rebuild iOS share extension drain and Android sharesheet intake through Rust share actions.
- Move What's New seen state into Rust.

Gate:

- Camera/OCR/book capture, podcast playback/clipping, share-to-community, media settings, and What's New all work from real capabilities.
- Native only holds transient camera/audio/share handles.

### Phase 6: Android First-Class App

Deliverables:

- Create `app/android` with Kotlin, Jetpack Compose, Gradle, UniFFI Kotlin bindings, Android resources, app icons, and package config.
- Implement Compose screens matching the iOS UI hierarchy and Highlighter design language.
- Wire Android secure storage, NIP-55, share intent, CameraX, ML Kit OCR, ExoPlayer, file/photo picker, and URL opening capabilities.
- Add Android unit/instrumentation tests for bridge lifecycle and critical flows.

Gate:

- Android debug build installs and all iOS-parity primary flows work against real Rust behavior.
- No Android-only product logic duplicates Rust facts.

### Phase 7: Delete Old Architecture And Enforce Doctrine

Deliverables:

- Delete `SafeHighlighterCore`, `EventBridge`, Swift feature stores that own facts, old generated API wrappers, broad Rust `get_*`/`publish_*`/`subscribe_*` FFI surface, native caches, and old query/subscribe view wiring.
- Keep only rendering adapters and capability executors in Swift/Kotlin.
- Add architecture lint tests that fail on native product stores, native `UserDefaults` app facts, hardcoded relay policy outside audited config, production sleeps, and broad throwing FFI product methods.

Gate:

- `nmp_architecture_scan.py /Users/customer/Work/hl` has no blocking findings for app-authored production code.
- Any generated binding false positives are documented in lint allowlist tests, not ignored informally.

## Verification Matrix

Required before calling the rebuild complete:

| Gate | Command / Method |
|---|---|
| Rust format | `cd app/core && cargo fmt --check` |
| Rust tests | `cd app/core && cargo test` |
| Rust clippy | `cd app/core && cargo clippy -- -D warnings` |
| NMP scan | `python3 /Users/customer/.codex/skills/nmp-app-architecture/scripts/nmp_architecture_scan.py /Users/customer/Work/hl` |
| iOS bindings | `cd app/core && ./scripts/generate-swift-bindings.sh` or equivalent rebuilt script |
| iOS build | `xcodebuild -project app/ios/Highlighter/Highlighter.xcodeproj -scheme Highlighter -sdk iphonesimulator -destination 'platform=iOS Simulator,name=iPhone 16' build` |
| iOS tests | `xcodebuild test -project app/ios/Highlighter/Highlighter.xcodeproj -scheme Highlighter -sdk iphonesimulator -destination 'platform=iOS Simulator,name=iPhone 16'` |
| Android build | `cd app/android && ./gradlew assembleDebug` |
| Android tests | `cd app/android && ./gradlew test lint` |
| UI preservation | Compare iOS screenshot baselines before/after for primary screens. |
| Manual smoke | Real login/session restore, browse/join room, share artifact, publish highlight, comment, bookmark, search, capture OCR, play podcast, share extension/sharesheet. |

## Completion Definition

The rebuild is complete only when:

- iOS and Android run from the same Rust-owned behavior.
- Existing iOS UI is preserved except for documented platform-correct Android equivalents.
- All retained visible features are wired to real Rust behavior and real native capabilities.
- No mocks, fake stubs, duplicate old paths, unwired controls, temporary migration bridges, or TODO debt remain.
- Architecture scanner and local build/test gates pass or have documented generated-code-only false positives.
- The final commit history includes the planning checkpoint and implementation commits with verification notes.
