---
title: Android Build & CI
slug: android-build
topic: android-build
summary: The Android app targets only the arm64-v8a ABI
tags:
  - capture
volatility: warm
confidence: medium
created: 2026-06-12
updated: 2026-06-12
verified: 2026-06-12
compiled-from: conversation
sources:
  - session:0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
---

# Android Build & CI

## Build Configuration

The Android app targets only the arm64-v8a ABI. Android CI must account for the Rust core's path dependency on ../../../nostr-multi-platform outside the repository. Release builds use R8 minification and resource shrinking, reducing APK size from 37 MB to 29 MB, with ProGuard keep rules for JNA/UniFFI. Release build signing is configured via a git-ignored keystore.properties file, and versionName/versionCode are property-driven.

<!-- citations: [^0c7b6-1] [^0c7b6-2] [^0c7b6-3] [^0c7b6-10] [^0c7b6-15] [^0c7b6-19] [^0c7b6-73] [^0c7b6-108] -->
## CI Pipeline

The Android project requires a CI pipeline setup, as only iOS Xcode Cloud scripts currently exist. The Android app must have a GitHub Actions CI workflow that runs build verification on every push and pull request. The CI workflow at .github/workflows/android.yml builds the APK, runs unit tests and lint on every push/PR touching app/android or app/core, and includes a sibling checkout of nostr-multi-platform for Rust core path dependencies. The Cargo.lock sibling-repo dependency requires nostr-multi-platform to be checked out alongside hl for both CI and local builds; the CI workflows handle this with dual checkouts. Android CI requires an NDK llvm-ar override for libsodium linking to avoid runtime crashes. A seed test suite of unit tests exists for the formatter utility functions.

The Android app must route Rust core logs to logcat so the app is debuggable on-device.

<!-- citations: [^0c7b6-35] [^0c7b6-11] [^0c7b6-16] [^0c7b6-177] -->
