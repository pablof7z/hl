# AGENTS.md — Highlighter Mobile & Desktop Apps

> The Highlighter native apps share a Rust core that handles Nostr protocol logic, NIP-29 group operations, local data, and sync. Platform-specific UI layers (Kotlin for Android, Swift for iOS, Tauri/native for desktop) consume the Rust core via FFI bridge.

## Tech Stack

| Layer | Technology | Purpose |
|---|---|---|
| **Rust core** | Rust (no_std where possible) | Nostr client, NIP-29 groups, signing, sync, local DB |
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
│   │   ├── nostr/             # Nostr client (relay mgmt, event signing)
│   │   ├── groups/            # NIP-29 group operations (join, leave, membership)
│   │   ├── auth/              # NIP-42 authentication, key management
│   │   ├── events/            # Event construction & validation
│   │   ├── db/                # Local SQLite storage & sync
│   │   ├── highlight/         # Highlight extraction & management
│   │   └── ffi/               # FFI bridge definitions (uniffi/cbindgen)
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

### Offline & Sync

The Rust core provides:
- **Local event database** (SQLite) — all data available offline
- **Background sync** — reconcile when connectivity returns
- **Optimistic UI** — post events locally, confirm when relay accepts
- **Conflict resolution** — latest timestamp wins for replaceable events (Nostr convention)

### Auth on Mobile

| Platform | Method | Details |
|---|---|---|
| Android | NIP-55 | Android signer app (Amber, etc.) |
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
- ViewModels for screen state, single source of truth via `StateFlow`

### Swift (iOS)
- Follow Swift API Design Guidelines
- Use SwiftUI for all views
- `@Observable` / `@State` for view state

## Common Patterns

- **Adding a new Rust API**: Define in `core/src/`, expose via FFI in `core/src/ffi/`, regenerate bindings, implement UI on each platform
- **Adding a new screen**: Create the screen in each platform's UI layer, use the Rust core for data
- **Nostr event handling**: Always construct events via `core/src/events/` helpers — never build raw JSON manually
- **Local database changes**: Add migration in `core/src/db/migrations/`, update schema version