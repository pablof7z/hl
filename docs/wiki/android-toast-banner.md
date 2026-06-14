---
title: Android Toast Banner
slug: android-toast-banner
topic: ui-components
summary: The welcome/toast banner is positioned below the status bar and TopAppBar (WindowInsets.statusBars + 64.dp + 8.dp) instead of overlapping the header
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

# Android Toast Banner

## Positioning

The welcome/toast banner is positioned below the status bar and TopAppBar (WindowInsets.statusBars + 64.dp + 8.dp) instead of overlapping the header. The 'not found' toast banner no longer appears on benign lookup failures (ISBN preview, web metadata, profile subscription); the core-side set_toast calls were replaced with tracing::debug!, and an Android-side 4-second auto-expire LaunchedEffect clears any remaining toasts.

<!-- citations: [^84748-79] [^84748-80] [^84748-81] [^84748-89] [^84748-100] [^84748-116] [^84748-134] [^84748-146] [^84748-159] [^84748-187] [^84748-199] [^84748-212] -->
