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
---

# Build System

## Build Configuration

Debug APKs are placed at ~/Builds/app-debug.apk. Builds must be full clean rebuilds rather than incremental updates. <!-- [^fc140-1] -->

The FEEDBACK_PROJECT_COORDINATE constant must reside in AppConfig.kt. <!-- [^0c7b6-13] -->

The Gradle cargoBuildArm64 task sets AR and RANLIB environment variables to the NDK's llvm-ar and llvm-ranlib respectively. <!-- [^fc140-3] -->
