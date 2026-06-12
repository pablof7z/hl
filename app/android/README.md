# Highlighter for Android

Native Android client for Highlighter, built on the shared Rust core (`app/core`)
via the NMP app facade. All business logic, state, and networking live in Rust;
this module is a Jetpack Compose UI that dispatches `HighlighterAppAction`s and
renders `HighlighterAppState` snapshots pushed through a UniFFI reconciler.

## Requirements

- JDK 17
- Android SDK 35 (+ NDK; any recent NDK works)
- Rust stable with the Android targets you build for, e.g.
  `rustup target add aarch64-linux-android`
- [`cargo-ndk`](https://github.com/bbqsrc/cargo-ndk): `cargo install cargo-ndk`

## Building

```sh
./gradlew assembleDebug          # debug APK (arm64-v8a by default)
./gradlew assembleRelease        # minified release APK
```

The Gradle build drives everything: it compiles the Rust core with `cargo ndk`,
generates the Kotlin bindings with `uniffi-bindgen`, and packages the `.so`
into the APK. No manual codegen step.

### ABIs

Only `arm64-v8a` is built right now, which covers real devices and Apple
Silicon emulators. Add further ABIs in `app/build.gradle.kts` (the cargo task
and `abiFilters`) when an x86_64 emulator or 32-bit support is needed; each
ABI also needs its rustup target installed.

### Versioning

`versionName`/`versionCode` default to the values in `app/build.gradle.kts` and
can be overridden for CI/release builds:

```sh
./gradlew assembleRelease -Phighlighter.versionName=0.2.0 -Phighlighter.versionCode=3
```

## Release signing

Release builds are signed if `keystore.properties` exists next to this README
(git-ignored), otherwise the APK is built unsigned:

```properties
storeFile=/absolute/path/to/release.keystore
storePassword=...
keyAlias=...
keyPassword=...
```

## Code layout

```
app/src/main/java/com/highlighter/app/
  MainActivity.kt            activity, deep links (highlighter://nip46)
  HighlighterViewModel.kt    NMP app + state reconciler -> StateFlow
  ui/AppScreen.kt            root scaffold + top bar
  ui/theme/                  colors + Material3 theme
  ui/components/             shared primitives (Panel, Chip, ...)
  ui/<feature>/              one package per feature panel
  util/                      formatting helpers
```

The UI is intentionally thin: composables take state snapshots plus a single
`dispatch: (HighlighterAppAction) -> Unit`. If you need new behavior, add it to
the Rust core (`app/core/src/nmp_app.rs`) first.

## CI

`.github/workflows/android.yml` builds the debug APK and runs Android Lint on
every push/PR touching `app/android` or `app/core`, and uploads the APK as an
artifact.
