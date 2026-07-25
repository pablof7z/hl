# AGENTS.md — Highlighter Mobile & Desktop Apps

> The Highlighter native apps are Rust/NMP-owned products with thin native shells. App runtime, event ingestion, relay routing, protocol actions, durable product state, and screen-shaped projections belong in the Rust core through `nmp` (NMP). Platform-specific UI layers (Kotlin for Android, Swift for iOS, Tauri/native for desktop) render projections and execute bounded OS capabilities only.

## Tech Stack

| Layer | Technology | Purpose |
|---|---|---|
| **Rust core** | Rust + NMP | Nostr runtime, event store, NIP-29 groups, signing, sync, routing, durable projections |
| **Android** | Kotlin + Jetpack Compose | Native Android UI |
| **iOS** | Swift + SwiftUI | Native iOS UI |
| **Desktop** | Tauri or Rust-native | Desktop app (macOS, Windows, Linux) |
| **FFI bridge** | C ABI (via `uniffi` or `cbindgen`) | Exposes Rust core to Kotlin/Swift |

## Setup Commands

### Rust Core

```bash
cd app/core

# Build the Rust core library
cargo build

# Run tests
cargo test

# Run clippy lints
cargo clippy -- -D warnings

# Format check
cargo fmt --check

# Build for Android targets
cargo build --target aarch64-linux-android
cargo build --target armv7-linux-androideabi

# Build for iOS targets
cargo build --target aarch64-apple-ios
```

### Android

```bash
cd app/android

# Install dependencies
./gradlew assembleDebug

# Run on connected device/emulator
./gradlew installDebug

# Run Android tests
./gradlew test

# Lint
./gradlew lint
```

### iOS

```bash
cd app/ios

# Install dependencies
pod install

# Build (via xcodebuild or Xcode)
xcodebuild -workspace Highlighter.xcworkspace -scheme Highlighter -sdk iphonesimulator

# Run tests
xcodebuild test -workspace Highlighter.xcworkspace -scheme Highlighter -sdk iphonesimulator -destination 'platform=iOS Simulator,name=iPhone 16'
```

### Desktop

```bash
cd app/desktop

# Build
cargo build --release

# Run
cargo run
```

## Project Structure

```
app/
├── core/                      # Shared Rust core library
│   ├── src/
│   │   ├── lib.rs             # Library root, public API
│   │   ├── kernel/            # TEA actor, typed actions, snapshots, capability requests
│   │   ├── nmp_*/             # NMP app wiring and Nostr substrate integration
│   │   ├── groups/            # NIP-29 projections/actions owned by Rust/NMP
│   │   ├── auth/              # Signer/key capability requests and session projection
│   │   ├── highlights/        # Highlight and discussion product projections/actions
│   │   └── ffi/               # Bounded FFI bridge definitions (UniFFI/cbindgen)
│   ├── Cargo.toml
│   └── Cargo.lock
├── android/                   # Kotlin + Jetpack Compose
│   ├── app/
│   │   └── src/main/java/     # Kotlin source
│   ├── build.gradle.kts
│   └── gradle/
├── ios/                       # Swift + SwiftUI
│   ├── Highlighter/
│   │   └── Views/             # SwiftUI views
│   ├── Highlighter.xcodeproj
│   └── Podfile
├── desktop/                   # Tauri or native Rust desktop
│   ├── src/
│   ├── Cargo.toml
│   └── tauri.conf.json       # (if Tauri)
└── AGENTS.md
```

## Key Concepts

### FFI Bridge

The Rust core exposes a C ABI interface that platform layers consume:

- **Android**: `uniffi` generates Kotlin bindings from Rust — run `cargo run --bin uniffi-bindgen` after Rust changes
- **iOS**: `uniffi` generates Swift modules — import the generated `.swift` file into the Xcode project
- **Desktop**: Direct Rust API (no FFI needed for Tauri; direct calls for native)

When modifying the Rust core's public API:
1. Update the Rust code
2. Regenerate FFI bindings for all platforms
3. Rebuild platform-specific projects

### Native Architecture Contract

The Rust/NMP core provides:
- **Single app source of truth** for Nostr runtime, event ingestion, event store, relay routing, protocol actions, and durable product projections.
- **Bounded snapshots** shaped for the currently open app chrome and screens. Native code must not receive or mirror the whole event store.
- **Typed actions and capability requests**. Native dispatches user intent and reports raw OS results; Rust/NMP decides policy, retry, routing, privacy, and user-visible error state.
- **Offline/sync behavior** through the NMP substrate. Do not add a second app-client database or native product cache to work around missing projections.

Accepted native storage/capability exceptions:
- iOS Keychain and Android Keystore/encrypted session storage for secrets and signer material.
- NIP-55 app IPC handles, camera/file-picker handles, AVPlayer/ExoPlayer media handles, push/display permission state, and share-sheet/App Group handoff payloads.
- Rendering and media caches such as image caches, waveform caches, and temporary captured image files, provided product policy remains Rust/NMP-owned and logout/deletion behavior is explicit.
- Presentation-only state such as selected tab, sheet visibility, scroll position while a view is alive, focus, local text-field drafts before dispatch, and animation state.

### Auth on Mobile

| Platform | Method | Details |
|---|---|---|
| Android | NIP-55 | Android signer app (Amber, etc.) plus Keystore-backed local secrets when needed |
| iOS | Local keypair | Key stored in iOS Keychain |
| Both | NIP-46 | Remote signer (Nostr Connect) for cross-device |

## Testing

```bash
# Rust core — full suite
cd app/core && cargo test

# Rust core — specific module
cargo test --lib nostr
cargo test --lib groups

# Android
cd app/android && ./gradlew test

# iOS
cd app/ios && xcodebuild test -scheme Highlighter -sdk iphonesimulator -destination 'platform=iOS Simulator,name=iPhone 16'

# Desktop
cd app/desktop && cargo test
```

### Test Patterns

- **Rust unit tests**: Colocated in `#[cfg(test)] mod tests` within each module
- **Integration tests**: `app/core/tests/` — test full Nostr flows against a local test relay
- **Platform UI tests**: Platform-specific test directories (`androidTest`, iOS test target)
- **Always run `cargo test` before committing Rust changes**

## Build & Deployment

### Android
```bash
# Debug APK
./gradlew assembleDebug

# Release APK (requires signing config)
./gradlew assembleRelease

# Bundle for Play Store
./gradlew bundleRelease
```

### iOS
```bash
# Archive for distribution
xcodebuild archive -scheme Highlighter -archivePath build/Highlighter.xcarchive

# Export IPA
xcodebuild -exportArchive -archivePath build/Highlighter.xcarchive -exportPath build/
```

### Desktop
```bash
# Build release binary
cargo build --release

# Platform-specific packaging (Tauri)
cargo tauri build
```

## Code Style

### Rust Core
- Follow `rustfmt` defaults — `cargo fmt` before every commit
- Clippy warnings are errors — `cargo clippy -- -D warnings`
- Use `thiserror` for error types in the public API
- Use `tracing` for all logging (no `println!` in library code)
- Document all public items with `///` doc comments

### Kotlin (Android)
- Follow Kotlin style guide
- Use Jetpack Compose for all UI
- ViewModels adapt Rust/NMP projections to Compose state; they do not own product facts, relay policy, or durable app state

### Swift (iOS)
- Follow Swift API Design Guidelines
- Use SwiftUI for all views
- `@Observable` / `@State` are allowed for rendering and transient presentation state, not for owning product facts that belong in Rust/NMP

## What's New Changelog

The iOS app ships a bundled `app/ios/Highlighter/Sources/Highlighter/Resources/whats-new.json` that drives an in-app "What's New" sheet shown once per cold launch.

**Rule: every user-facing change committed to the iOS app must have a corresponding entry in `whats-new.json`.**

Entry format:
```json
{
  "shipped_at": "2026-05-14T20:03:00Z",
  "lines": [
    "One sentence describing the user-visible change."
  ]
}
```

- `shipped_at` is ISO-8601 UTC (`Z` suffix). It is the primary key — **must be unique across all entries**. Use the next minute if you need to disambiguate.
- Add the entry at the top of the `entries` array (newest first is conventional, though the service re-sorts at runtime).
- Do not add entries for internal refactors, test changes, or non-visible fixes.

## Common Patterns

- **Adding a new Rust API**: Prefer a typed NMP/kernel action, bounded snapshot, or capability request. Regenerate bindings only when the FFI surface changes.
- **Adding a new screen**: Create native rendering on each platform, but source product state from Rust/NMP projections.
- **Nostr event handling**: Use typed Rust/NMP actions and projections. Native code must not build protocol JSON, choose relays, or publish directly.
- **Persistence changes**: App-client product persistence belongs in Rust/NMP. Native persistence requires an explicit OS capability, presentation-state, or cache exception with deletion/logout behavior.
