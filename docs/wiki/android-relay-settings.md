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
  - session:f54b4a16-dacb-41e6-b32f-b737d606254f
---

# Android Relay Settings

## Relay List

Android Settings → Network lists all configured relays with live status dots and roles, with add/remove capability. The Android settings screen also includes media, library (bookmarks/feedback), what's new, and sign-out sections. The relay list UI displays the `icon` field from NIP-11 documents for each relay, falling back to a monogram avatar when no icon is available. Relay probe operations are keyed by a stable hash of the relay URL so that probes for different relays execute independently and re-probing the same URL still supersedes an in-flight probe for that same URL.

<!-- citations: [^0c7b6-79] [^0c7b6-94] [^0c7b6-126] [^0c7b6-148] [^0c7b6-185] [^f54b4-1] -->
