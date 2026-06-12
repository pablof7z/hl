---
title: Android Relay Settings
slug: android-relay-settings
topic: nmp-app
summary: Android Settings â Network lists all configured relays with live status dots and roles, with add/remove capability
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

# Android Relay Settings

## Relay List

Android Settings → Network lists all configured relays with live status dots and roles, with add/remove capability. The Android settings screen also includes media, library (bookmarks/feedback), what's new, and sign-out sections.

Relay connection status now reaches Online by wiring nmp_app_set_update_callback → frame decode → diagnostics state in the Rust core, fixing both platforms at once.

Relay URLs are compile-time baked via include_str!("relay_policy.json"); the design adds a relay_policy_json config seam for testability.

<!-- citations: [^0c7b6-79] [^0c7b6-94] [^0c7b6-126] [^0c7b6-148] [^0c7b6-185] -->
