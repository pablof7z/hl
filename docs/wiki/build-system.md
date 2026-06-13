---
title: Build System
slug: build-system
topic: build-system
summary: Debug APKs are placed at ~/Builds/app-debug.apk
tags:
  - capture
volatility: warm
confidence: medium
created: 2026-06-12
updated: 2026-06-12
verified: 2026-06-12
compiled-from: conversation
sources:
  - session:fc140a10-b623-435c-9e69-364f38ce9541
  - session:0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
  - session:f54b4a16-dacb-41e6-b32f-b737d606254f
---

# Build System

## Build Configuration

Debug APKs are placed at ~/Builds/app-debug.apk. Builds must be full clean rebuilds rather than incremental updates. The FEEDBACK_PROJECT_COORDINATE constant must reside in AppConfig.kt. The Gradle cargoBuildArm64 task sets AR and RANLIB environment variables to the NDK's llvm-ar and llvm-ranlib respectively. The file-size check gate failure is due to master's own baseline drift (not by the PR), and is resolved by the owner via a post-merge baseline-refresh PR following the same precedent as PR #1196 for #1192.

NMP releases follow a manifest-driven process: the release-train manifest (release/nmp-release.toml) lists all public crates, and merge requires a full cargo test --workspace gate plus cross-target checks. <!-- [^f54b4-18] -->

FlatBuffers bindings are pinned to flatc 25.12.19, and a drift check gate ensures the checked-in generated code stays in sync. <!-- [^f54b4-19] -->

<!-- citations: [^fc140-1] [^0c7b6-13] [^fc140-3] [^f54b4-9] [^f54b4-17] -->
