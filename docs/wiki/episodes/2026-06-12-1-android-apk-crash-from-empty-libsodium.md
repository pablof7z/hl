---
type: episode-card
date: 2026-06-12
session: fc140a10-b623-435c-9e69-364f38ce9541
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/fc140a10-b623-435c-9e69-364f38ce9541.jsonl
salience: root-cause
status: active
subjects:
  - libsodium-cross-compile
  - android-build
  - cargo-ndk
  - llvm-ar
supersedes: []
related_claims: []
source_lines:
  - 103-103
  - 137-179
  - 448-448
  - 757-791
  - 884-928
  - 983-1000
  - 1141-1163
  - 1183-1186
captured_at: 2026-06-12T08:44:43Z
---

# Episode: Android APK crash from empty libsodium static archive in cross-compilation

## Prior State

The Android build produced an APK that crashed immediately at launch with dlopen failed: cannot locate symbol "crypto_stream_chacha20_ietf_xor_ic". The libsodium static archive (libsodium.a) was 96 bytes (empty) because macOS's host ar tool was used during libsodium's ./configure && make install, which cannot properly archive Android-target .o files. The cdylib crate type meant the unresolved symbol didn't cause a link-time error.

## Trigger

User reported the app crashes immediately on Android. Logcat showed: java.lang.RuntimeException → dlopen failed: cannot locate symbol "crypto_stream_chacha20_ietf_xor_ic" referenced by libhighlighter_core.so. Investigation revealed libsodium.a was empty (96 bytes) despite 116 .o files being compiled.

## Decision

Added AR and RANLIB environment variables to the cargoBuildArm64 Gradle task, pointing to the NDK's llvm-ar and llvm-ranlib. Also created a fix-libsodium.sh post-build task (fixLibsodiumAndRelink) that detects an empty libsodium.a, recreates it from .o files using llvm-ar, and re-links the Rust library as a safety net.

## Consequences

- Android builds now produce working APKs where libsodium symbols are statically resolved (lowercase 't' local text symbols, no longer undefined 'U')
- The fix is durable across clean rebuilds — verified with a full from-scratch rebuild
- The fixLibsodiumAndRelink task acts as a safety net since libsodium-sys's build script cleans .o files after make install, making the AR/RANLIB env var approach the primary fix
- build.gradle.kts now resolves NDK path from android.sdkDirectory and android.ndkVersion at execution time (doFirst) to avoid configuration-time resolution errors

## Open Tail

- The fixLibsodiumAndRelink task currently reports 'No .o files found, skipping' because libsodium-sys cleans .o files post-install — the primary fix relies on AR/RANLIB env vars propagating correctly. If those stop working, the safety net script won't help either.
- The fix has only been verified for arm64-v8a; adding other ABIs (armeabi-v7a, x86, x86_64) would need analogous llvm-ar paths

## Evidence

- transcript lines 103-103
- transcript lines 137-179
- transcript lines 448-448
- transcript lines 757-791
- transcript lines 884-928
- transcript lines 983-1000
- transcript lines 1141-1163
- transcript lines 1183-1186
