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
updated: 2026-06-13
verified: 2026-06-12
compiled-from: conversation
sources:
  - session:0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
  - session:847487cd-e15b-4222-85ee-4a5a2b6f590b
---

# Android Build & CI

## Build Configuration

The Android app targets only the arm64-v8a ABI. The package ID is com.highlighter.app, version 0.1.0 (versionCode=1), targeting SDK 35 with a minimum SDK of 26. The app does not declare CAMERA permission, blocking OCR camera capture. Android CI must account for the Rust core's path dependency on ../../../nostr-multi-platform outside the repository. Release builds use R8 minification and resource shrinking, reducing APK size from 37 MB to 29 MB, with ProGuard keep rules for JNA/UniFFI. Release build signing is configured via a git-ignored keystore.properties file, and versionName/versionCode are property-driven.

<!-- citations: [^0c7b6-1] [^0c7b6-2] [^0c7b6-3] [^0c7b6-10] [^0c7b6-15] [^0c7b6-19] [^0c7b6-73] [^0c7b6-108] [^84748-47] [^84748-93] -->
## CI Pipeline

The Android project requires a CI pipeline setup, as only iOS Xcode Cloud scripts currently exist. The Android app must have a GitHub Actions CI workflow that runs build verification on every push and pull request. The CI workflow at .github/workflows/android.yml builds the APK, runs unit tests and lint on every push/PR touching app/android or app/core, and includes a sibling checkout of nostr-multi-platform for Rust core path dependencies. The Cargo.Lock sibling-repo dependency requires nostr-multi-platform to be checked out alongside hl for both CI and local builds; the CI workflows handle this with dual checkouts. Android CI requires an NDK llvm-ar override for libsodium linking to avoid runtime crashes. A seed test suite of unit tests exists for the formatter utility functions. The validation harness uses a HighlighterTest AVD (arm64-v8a, google_apis, android-34) matching the arm64-only APK, with adb, emulator, and Maestro all installed and operational. The validation harness (HighlighterTest AVD, arm64-v8a/google_apis/android-34) is operational: emulator boots, APK installs, app launches, Compose nodes are discoverable by text/uiautomator for automation, and adb screencap produces valid PNGs. Validation agents are kept to short, single-purpose runs with hard time caps instead of marathon sessions, because long sessions on the emulator cause agent death and unreliable results. On a fresh cold-booted emulator, navigation (Rooms tab, room detail, tab switching) registers correctly, confirming that prior unresponsiveness was caused by emulator degradation rather than app bugs. The root Box in RootScene has semantics { testTagsAsResourceId = true } so all testTags propagate as resource IDs for Maestro. Maestro flow files are created for 8 flows: login (00-login.yaml), feed (06-feed.yaml), highlight-detail (08-highlight-detail.yaml), comments (30-comments.yaml), rooms-explorer (11-rooms-explorer.yaml), open-room (12-open-room.yaml), create-room (19-create-room.yaml), search-nav (33-search-nav.yaml). The emulator has no camera, so OCR flow validation requires an injected image + iOS reference rather than true camera capture; a physical device is needed for end-to-end camera testing. The CAMERA permission is absent from the Android manifest, blocking OCR camera capture (Phase 4 blocker). A seeded test account exists at ~/Builds/test-account.txt (outside the git repo), containing nsec, npub (npub1sle0h9fqdffs2qh3lfzax2zaer5cn7v9phtl4uls93t808qaws2std326a), and hex credentials, following 16 highlighters with 115 confirmed highlight events on the app's default read relays. The Android app must route Rust core logs to logcat so the app is debuggable on-device.

<!-- citations: [^0c7b6-35] [^0c7b6-11] [^0c7b6-16] [^0c7b6-177] [^84748-8] [^84748-12] [^84748-48] [^84748-67] [^84748-83] [^84748-94] [^84748-103] -->
