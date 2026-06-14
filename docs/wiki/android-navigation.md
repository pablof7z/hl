---
title: Android Navigation & Back Stack
slug: android-navigation
topic: ui-components
summary: The Android app must support system back navigation that closes the innermost open overlay (comments â invite â room â article â profile â feedback th
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

# Android Navigation & Back Stack

## System Back Navigation

The Android app must support system back navigation that closes the innermost open overlay (comments → invite → room → article → profile → feedback thread) before exiting, with predictive back gesture support.

<!-- citations: [^0c7b6-21] [^0c7b6-27] [^0c7b6-36] [^0c7b6-50] [^0c7b6-92] -->
## Navigation Architecture

The Android UI must be structured with root-scene gating: welcome screen → login/create-account → full-screen onboarding interests → main app, instead of a single-scroll dump of all panels. The main app uses a 3-tab Material3 bottom navigation bar containing Highlights, Rooms, and Search tabs, with top-bar avatar → Profile and gear → Settings. Each tab must dispatch its own Open/Close actions on enter/leave rather than firing all panels open at startup. The RoomsTab must not dispatch CloseRoom on dispose; the stray CloseRoom dispatch in RoomsTab.onDispose (MainScaffold.kt:320) was removed, preventing an open room from collapsing on tab switch or recomposition, and room closing is handled only by the RootScene overlays close button and the RoomDetailPanel close button. Flow #12 (open a room and it stays open across tab switches) is verified PASS — room detail opens, persists, and returns cleanly to the explorer on back press. The CreateRoomPanel must not appear as a permanent inline first item of the Rooms LazyColumn; it must be relocated behind a FAB/toolbar affordance that opens a modal sheet, mirroring the iOS + toolbar button convention. Search result rows for people dispatch OpenProfile(pubkeyHex) and community rows dispatch OpenRoom(groupId), making them tappable to navigate to the profile overlay or room detail respectively. Tapping a feed highlight card opens the highlight detail screen (not the article reader directly); tapping a reading card still dispatches OpenArticleReader directly. The highlight detail screen shows the quote, an author byline (tappable → OpenProfile, consuming the tap independently so the card action does not fire), a source header (tappable → OpenArticleReader for articles), a comment action (→ OpenComments), bookmark (ToggleArticleBookmark), and share (Android system share sheet with a highlighter.com URL); web reader navigation from the source header is omitted because no OpenWebReader or equivalent action exists in the binding. Manual Refresh buttons are removed from all screens (feed, rooms explorer, comments, reader, profile, bookmarks, settings, feedback, room invite); on-appear Open* dispatches (OpenHomeFeed, OpenRoomExplorer, OpenBookmarks, OpenFeedback, OpenMediaSettings) ensure screens load data live, while Comments, Profile, ArticleReader, and RoomInvite are core-state-driven overlays that load via the calling screen's prior Open action. Pull-to-refresh swipe gestures in MainScaffold are preserved. Article reader, room detail, comments, and invites must be full-screen destinations with back navigation; the share composer must be a sheet. A Capture FAB navigates to a DestinationScaffold-wrapped CapturePanel, since no OpenCapture/CloseCapture action exists in the core. Android deep links support both https://beta.highlighter.com/highlight/ (with autoVerify) and highlighter://highlight/{token} custom-scheme URLs, routing bech32 tokens through decodeNostrEntity. App Links autoVerify requires hosting a .well-known/assetlinks.json on beta.highlighter.com with the package name com.highlighter.app and signing-cert SHA-256. The naddr deep-link route has no usable single dispatch target in the core and remains a TODO.

<!-- citations: [^0c7b6-62] [^0c7b6-63] [^0c7b6-64] [^0c7b6-28] [^0c7b6-37] [^0c7b6-49] [^0c7b6-65] [^0c7b6-91] [^84748-9] [^84748-15] [^84748-36] [^84748-74] [^84748-86] [^84748-110] [^84748-122] [^84748-130] [^84748-195] [^84748-210] -->
