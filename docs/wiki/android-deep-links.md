---
title: Android Deep Links
slug: android-deep-links
topic: nmp-app
summary: "Android deep links use two manifest intent-filters: a verified autoVerify App Link for `https://beta.highlighter.com/highlight/{token}` and a `highlighter://hig"
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
  - session:847487cd-e15b-4222-85ee-4a5a2b6f590b
---

# Android Deep Links

## Manifest Configuration

Android deep links use two manifest intent-filters: a verified autoVerify App Link for `https://beta.highlighter.com/highlight/{token}` and a `highlighter://highlight/{token}` custom-scheme fallback, leaving the existing nip46 signer callback untouched. App Links autoVerify for `beta.highlighter.com` requires hosting `/.well-known/assetlinks.json` on the server with the app's package name (`com.highlighter.app`, version 0.1.0, targetSdk 35, minSdk 26, arm64-v8a only) and signing-cert SHA-256.

<!-- citations: [^0c7b6-90] [^0c7b6-76] [^0c7b6-88] [^0c7b6-110] [^0c7b6-123] [^0c7b6-181] [^84748-58] -->
## Token Decoding and Routing

Android handles ACTION_SEND share intents and deep links (`https://beta.highlighter.com/highlight/{token}` and `highlighter://highlight/{token}`), decoding the bech32 token via the core's `decodeNostrEntity`; an Event (nevent) result dispatches `OpenComments("e", eventId, kindHint ?: 9802)`, a Profile (nprofile) result dispatches `OpenProfile`, and an Address (naddr) result is a TODO with no usable single dispatch target in the core today. Share-in via ACTION_SEND composes highlights or articles from incoming URLs.

<!-- citations: [^0c7b6-77] [^0c7b6-89] [^0c7b6-111] [^0c7b6-124] [^0c7b6-146] [^0c7b6-182] -->

## Server-Side Verification Prerequisites

Currently `beta.highlighter.com/api/nip05` returns 404 (server-side route missing), and App Links/universal links require server-side `assetlinks.json`/AASA files deployed to activate verification. <!-- [^0c7b6-183] -->

## Highlight Detail Navigation

The HighlightDetailScreen serves as a host-side local navigation destination from the feed: tapping a highlight card opens the detail screen instead of the article reader. Available actions on this screen include OpenComments, OpenProfile, OpenArticleReader, ToggleArticleBookmark, and a system share sheet. <!-- [^84748-183] -->
