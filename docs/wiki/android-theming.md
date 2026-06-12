---
title: Android Theming & Brand Palette
slug: android-theming
topic: ui-components
summary: The brand palette maps to Material theme tokens as Paperâbackground, InkâonSurface, MutedâonSurfaceVariant, Lineâoutline, Mossâprimary, Goldâseconda
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

# Android Theming & Brand Palette

## Brand Palette Mapping

The brand palette maps to Material theme tokens as Paper→background, Ink→onSurface, Muted→onSurfaceVariant, Line→outline, Moss→primary, Gold→secondary, Clay→tertiary. The Android app supports full dark mode with Material3 color schemes derived from the brand palette, using a values-night window theme. All 16 Android UI files must use MaterialTheme color tokens with zero hardcoded color literals remaining outside the theme definition, except the lone inline literal Color(0xFFF1EFE6) (warm off-white chip fill) which is mapped to surfaceVariant. The Android app must have an adaptive vector launcher icon recreating the iOS quote-marks mark in terracotta, including a monochrome layer for themed icons.

<!-- citations: [^0c7b6-31] [^0c7b6-23] [^0c7b6-30] [^0c7b6-53] [^0c7b6-80] [^0c7b6-163] -->
