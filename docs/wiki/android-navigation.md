---
title: Android Navigation & Back Stack
slug: android-navigation
topic: ui-components
summary: System back navigation closes the innermost open overlay (comments → invite → room → article → profile → feedback thread) before exiting, with predictive back e
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

# Android Navigation & Back Stack

## System Back Navigation

The Android app must support system back navigation that closes the innermost open overlay (comments → invite → room → article → profile → feedback thread) before exiting, with predictive back gesture support.

<!-- citations: [^0c7b6-21] [^0c7b6-27] [^0c7b6-36] [^0c7b6-50] [^0c7b6-92] -->
## Navigation Architecture

The Android UI must be structured with root-scene gating: welcome screen → login/create-account → full-screen onboarding interests → main app, instead of a single-scroll dump of all panels. The main app uses a 3-tab Material3 bottom navigation bar containing Highlights, Rooms, and Search tabs, with top-bar avatar → Profile and gear → Settings. Each tab must dispatch its own Open/Close actions on enter/leave rather than firing all panels open at startup. Article reader, room detail, comments, and invites must be full-screen destinations with back navigation; the share composer must be a sheet. A Capture FAB navigates to a DestinationScaffold-wrapped CapturePanel, since no OpenCapture/CloseCapture action exists in the core. Android deep links support both https://beta.highlighter.com/highlight/ (with autoVerify) and highlighter://highlight/{token} custom-scheme URLs, routing bech32 tokens through decodeNostrEntity. App Links autoVerify requires hosting a .well-known/assetlinks.json on beta.highlighter.com with the package name com.highlighter.app and signing-cert SHA-256. The naddr deep-link route has no usable single dispatch target in the core and remains a TODO.

<!-- citations: [^0c7b6-62] [^0c7b6-63] [^0c7b6-64] [^0c7b6-28] [^0c7b6-37] [^0c7b6-49] [^0c7b6-65] [^0c7b6-91] -->
