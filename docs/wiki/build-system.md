---
title: Build System
slug: build-system
topic: build-system
summary: The Android app's package ID is com.highlighter.app (version 0.1.0, target SDK 35, min SDK 26)
tags:
  - capture
volatility: warm
confidence: medium
created: 2026-06-12
updated: 2026-06-13
verified: 2026-06-12
compiled-from: conversation
sources:
  - session:fc140a10-b623-435c-9e69-364f38ce9541
  - session:0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
  - session:f54b4a16-dacb-41e6-b32f-b737d606254f
  - session:847487cd-e15b-4222-85ee-4a5a2b6f590b
---

# Build System

## Build Configuration

The Android app's package ID is com.highlighter.app (version 0.1.0, target SDK 35, min SDK 26). The Android build produces a debug APK at ~/Builds/highlighter-debug.apk (approximately 52–54 MB, arm64-v8a only, self-signed); no keystore.properties is present for release signing. Builds must be full clean rebuilds rather than incremental updates. The FEEDBACK_PROJECT_COORDINATE constant must reside in AppConfig.kt. The Gradle cargoBuildArm64 task sets AR and RANLIB environment variables to the NDK's llvm-ar and llvm-ranlib respectively. The file-size check gate failure is due to master's own baseline drift (not by the PR), and is resolved by the owner via a post-merge baseline-refresh PR following the same precedent as PR #1196 for #1192.

NMP releases follow a manifest-driven process: the release-train manifest (release/nmp-release.toml) lists all public crates, and merge requires a full cargo test --workspace gate plus cross-target checks.

FlatBuffers bindings are pinned to flatc 25.12.19, and a drift check gate ensures the checked-in generated code stays in sync.

<!-- citations: [^f54b4-18] [^f54b4-19] [^fc140-1] [^0c7b6-13] [^fc140-3] [^f54b4-9] [^f54b4-17] [^84748-20] [^84748-33] [^84748-82] -->
