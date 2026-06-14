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
updated: 2026-06-14
verified: 2026-06-12
compiled-from: conversation
sources:
  - session:0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
  - session:f54b4a16-dacb-41e6-b32f-b737d606254f
  - session:847487cd-e15b-4222-85ee-4a5a2b6f590b
---

# Android Relay Settings

## Relay List

Android Settings → Network lists all configured relays with live status dots and roles, with add/remove capability. The Android settings screen also includes media, library (bookmarks/feedback), what's new, and sign-out sections. The relay list UI displays the `icon` field from NIP-11 documents for each relay, falling back to a monogram avatar when no icon is available. Relay probe operations are keyed by a stable hash of the relay URL so that probes for different relays execute independently and re-probing the same URL still supersedes an in-flight probe for that same URL.

The app's default relays are: `wss://relay.highlighter.com` (NIP-29 group relay), `wss://relay.damus.io` (read+write), `wss://purplepag.es` (indexer only), and `wss://relay.primal.net` (indexer only). (Previously: the default set also included `wss://nos.lol` as an outbox-routing relay.)

The `onState` callback guards `syncNetworkCallback` to only invoke it when `wifiOnlyEnabled` has actually changed, preventing hundreds of spurious OS `registerNetworkCallback`/`unregisterNetworkCallback` calls per second. The Settings gear icon in MainScaffold has testTag 'settings_button' and the NetworkPanel has testTag 'network_settings' for Maestro targeting.

<!-- citations: [^84748-17] [^0c7b6-79] [^0c7b6-94] [^0c7b6-126] [^0c7b6-148] [^0c7b6-185] [^f54b4-1] [^84748-29] [^84748-75] [^84748-87] [^84748-211] -->
