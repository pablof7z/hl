---
title: Platform-Local State
slug: platform-local-state
topic: nmp-app
summary: iOS retains PodcastPlayerStore (AVPlayer position) and CaptureStore (local OCR pipeline) as the only remaining Swift-side stores, holding transient device-local
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

# Platform-Local State

## Device-Local State

iOS retains PodcastPlayerStore (AVPlayer position) and CaptureStore (local OCR pipeline) as the only remaining Swift-side stores, holding transient device-local state that legitimately doesn't belong in the Rust core. Android podcast playback uses Media3 ExoPlayer with a mini-player bar, full listening screen with chapters/speed/skip, and a simplified clip composer; waveform and transcript views remain iOS-only. State.isbnPreviews is provided via a LocalIsbnPreviews composition local from the root CompositionLocalProvider in MainActivity, alongside LocalProfiles and LocalWebMetadata, and a previewForIsbn(isbn) extension on List<HighlighterIsbnPreview> returns ArtifactPreview? mirroring iOS app.isbnPreview(isbn:).

<!-- citations: [^0c7b6-25] [^0c7b6-45] [^0c7b6-84] [^0c7b6-98] [^0c7b6-114] [^84748-101] [^84748-125] -->
## Platform Logging

The Rust core's initPlatformLogging() routes tracing output to logcat on Android and stderr/Xcode console on iOS, so Rust logs are visible on both platforms, and is wired at startup on both platforms.

<!-- citations: [^0c7b6-99] [^0c7b6-139] -->
