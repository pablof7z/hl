---
title: Android App Professionalization
slug: android-professionalization
topic: android-build
summary: The Android app must be professionalized from a single-file reference implementation into a real, production-quality app, fixed properly and completely with no
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

# Android App Professionalization

## Professionalization

The Android app must be professionalized from a single-file reference implementation into a real, production-quality app, fixed properly and completely with no technical debt allowed anywhere. The Android app must address all gaps, technical debt, missing/incomplete/stubbed features across both iOS and Android. The Android app must connect to relays for functionality. The Android app's UI panels must be organized into feature-scoped files rather than a single monolithic file. The entire UI is refactored from a single 3,317-line MainActivity.kt into 23 cohesive files organized by feature, with MainActivity.kt reduced to 64 lines and feature-specific panels organized under ui/ packages. Cross-file symbols use internal visibility while symbols used within a single file remain private. The app uses dark/light Material3 color schemes derived from the brand palette, with all 16 UI files using theme tokens and zero hardcoded color literals remaining outside the theme layer. The brand palette maps to Material3 tokens as: Paper→background, Ink→onSurface, Muted→onSurfaceVariant, Line→outline, Moss→primary, Gold→secondary, Clay→tertiary. The app has a proper adaptive vector launcher icon recreating the iOS quote-marks mark, including a monochrome layer for themed icons. System back navigation closes the innermost open overlay (comments → invite → room → article → profile → feedback thread) before exiting the app, with predictive back enabled. No deep-link handling exists in the app; the original onCreate only does enableEdgeToEdge() and setContent. FEEDBACK_PROJECT_COORDINATE lives in AppConfig.kt in the same package as MainActivity. The Android app includes a Settings screen with Network, Media, Library (Bookmarks/Feedback), What's New, and Sign-out sections. The app has unit tests for formatters (pure functions) as a seed test suite. Android podcast playback v1 uses a Media3/ExoPlayer podcast player with a mini-player bar and full listening screen with chapters/speed/skip, matching iOS's podcast functionality (waveform extraction excluded as platform-local). Account creation no longer bricks when the NIP-05 availability check fails; a failed check lets the user proceed (the claim is skipped), and a 30-second deadline plus HTTP timeouts are applied to the creation request. Android release builds must use R8 minification with resource shrinking, reducing APK size from 37 MB to 29 MB, ProGuard keep rules for JNA/UniFFI, optional signing via git-ignored keystore.properties, and property-driven versionName/versionCode. Android Edit Profile is a full-screen destination with banner/avatar pickers dispatching UploadEditProfileImage, text fields for displayName/name/about/nip05/website/lud16, live upload spinners, error display, and SubmitEditProfile gated while busy. The remaining known gaps include: user-facing strings still hardcoded rather than in strings.xml, single ABI (arm64-v8a) only, dark theme deserves a visual once-over on emulator, podcast waveform/transcript views are iOS-only, naddr deep-link routing is TODO, App Links require server-side assetlinks.json deployment, and beta.highlighter.com/api/nip05 returns 404.

<!-- citations: [^0c7b6-38] [^0c7b6-4] [^0c7b6-12] [^0c7b6-17] [^0c7b6-22] [^0c7b6-29] [^0c7b6-51] [^0c7b6-66] [^0c7b6-78] [^0c7b6-93] [^0c7b6-125] [^0c7b6-136] [^0c7b6-147] [^0c7b6-162] [^0c7b6-184] -->
