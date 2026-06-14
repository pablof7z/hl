---
title: Android Session Persistence
slug: android-session-persistence
topic: nmp-app
summary: The session persistence issue where the app reverts to an unauthenticated state on back-key press (observed during harness validation) remains an open item to i
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

# Android Session Persistence

## Session Persistence

The session persistence issue where the app reverts to an unauthenticated state on back-key press (observed during harness validation) remains an open item to investigate. (Previously: credentials were persisted via EncryptedSharedPreferences, surviving force-stop/relaunch.) onState guards the syncNetworkCallback OS call to only fire when wifiOnlyEnabled actually changes, instead of on every state emission.

<!-- citations: [^0c7b6-39] [^0c7b6-52] [^0c7b6-67] [^0c7b6-95] [^0c7b6-112] [^0c7b6-127] [^0c7b6-137] [^0c7b6-186] [^84748-43] [^84748-88] [^84748-158] -->
