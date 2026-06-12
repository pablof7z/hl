---
title: Platform-Local State
slug: platform-local-state
topic: nmp-app
summary: iOS retains PodcastPlayerStore (AVPlayer position) and CaptureStore (local OCR pipeline) as transient device-local state outside the Rust core.
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

# Platform-Local State

## Device-Local State

iOS retains PodcastPlayerStore (AVPlayer position) and CaptureStore (local OCR pipeline) as the only remaining Swift-side stores, holding transient device-local state that legitimately doesn't belong in the Rust core. Android podcast playback uses Media3 ExoPlayer with a mini-player bar, full listening screen with chapters/speed/skip, and a simplified clip composer; waveform and transcript views remain iOS-only.

<!-- citations: [^0c7b6-25] [^0c7b6-45] [^0c7b6-84] [^0c7b6-98] [^0c7b6-114] -->
## Platform Logging

The Rust core's initPlatformLogging() routes tracing output to logcat on Android and stderr/Xcode console on iOS, so Rust logs are visible on both platforms, and is wired at startup on both platforms.

<!-- citations: [^0c7b6-99] [^0c7b6-139] -->
