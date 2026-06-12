---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: active
subjects:
  - android-build
  - libsodium
  - cross-compilation
supersedes: []
related_claims: []
source_lines:
  - 519-539
captured_at: 2026-06-12T08:19:17Z
---

# Episode: Android Build: NDK llvm-ar Required for Libsodium Cross-Compilation

## Prior State

Android debug builds succeeded locally but the build.gradle.kts had no cross-compilation toolchain configuration for native dependencies

## Trigger

Subagent's modularization build failed because libsodium's autoconf used macOS 'ar' which creates empty archives for Android object files

## Decision

Added NDK's llvm-ar and llvm-ranlib to the cargo build environment in build.gradle.kts, ensuring correct cross-compilation archiving of native dependencies

## Consequences

- Android CI builds will correctly link libsodium on Linux runners
- Any future native Rust dependency with autoconf will also benefit from the correct AR/RANLIB

## Open Tail

*(none)*

## Evidence

- transcript lines 519-539
