---
title: Android Image Loading
slug: android-image-loading
topic: nmp-app
summary: "The HighlighterApplication class implements SingletonImageLoader.Factory to provide a shared Coil ImageLoader, registered in the AndroidManifest via android:nam"
tags:
  - capture
volatility: warm
confidence: medium
created: 2026-06-13
updated: 2026-06-14
verified: 2026-06-13
compiled-from: conversation
sources:
  - session:847487cd-e15b-4222-85ee-4a5a2b6f590b
---

# Android Image Loading

## Shared Singleton Image Loader

The HighlighterApplication class implements SingletonImageLoader.Factory to provide a shared Coil ImageLoader, registered in the AndroidManifest via android:name. The loader is configured with a 25% memory cache, 100MB disk cache, correctly-sized thumbnail decodes (targetSize) for fixed-size slots, and crossfade preserved at the loader level.

<!-- citations: [^84748-170] [^84748-184] [^84748-209] -->
