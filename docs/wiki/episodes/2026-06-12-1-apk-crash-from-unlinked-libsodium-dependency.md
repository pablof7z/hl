---
type: episode-card
date: 2026-06-12
session: fc140a10-b623-435c-9e69-364f38ce9541
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/fc140a10-b623-435c-9e69-364f38ce9541.jsonl
salience: root-cause
status: active
subjects:
  - highlighter-core
  - android-build
  - libsodium
  - native-deps
supersedes: []
related_claims: []
source_lines:
  - 103-179
captured_at: 2026-06-12T07:59:33Z
---

# Episode: APK crash from unlinked libsodium dependency in Rust core

## Prior State

The Android APK was being built and deployed assuming libhighlighter_core.so was self-contained; no awareness that a dynamic libsodium dependency was missing from the APK bundle

## Trigger

User reported app immediately crashes on Android; emulator testing confirmed a FATAL EXCEPTION at HighlighterViewModel instantiation

## Decision

Root cause identified: libhighlighter_core.so dynamically references the libsodium symbol crypto_stream_chacha20_ietf_xor_ic, but libsodium is not bundled in the APK, causing dlopen failure and ViewModel crash at startup

## Consequences

- The build pipeline must be changed to either statically link libsodium into libhighlighter_core.so or include libsodium's .so in the APK's native lib directory
- Any future Rust core dependency on external C libraries must be audited for similar dynamic-linking gaps

## Open Tail

- Fix not yet implemented — need to determine whether to statically link libsodium via Rust's sodiumoxide/sodium-sys crate or bundle the shared library in Gradle jniLibs

## Evidence

- transcript lines 103-179
