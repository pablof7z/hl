---
title: NMP App Facade
slug: nmp-app-facade
topic: nmp-app
summary: The app uses the NMP app facade (nmp_app.rs) from core, with HighlighterStore conforming to the NostrProfileHost protocol via an adapter file (HighlighterStore+
tags:
  - capture
volatility: warm
confidence: medium
created: 2026-06-11
updated: 2026-06-12
verified: 2026-06-11
compiled-from: conversation
sources:
  - session:d9710893-bea1-487e-9bb2-499a23d553a6
  - session:0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
  - session:f54b4a16-dacb-41e6-b32f-b737d606254f
  - session:cd5f3967-ddef-43db-91ca-0d6b810bcfea
---

# NMP App Facade

## NMP App Facade Architecture

The app uses the NMP app facade (nmp_app.rs) from core, with HighlighterStore conforming to the NostrProfileHost protocol via an adapter file (HighlighterStore+NostrProfileHost.swift) located in the Core directory; App.swift injects the HighlighterStore as the nostrProfileHost into the SwiftUI environment, providing profile data to NMP UI components. The store holds a HighlighterNmpApp instance and a HighlighterAppStateReconciler that receives state pushes via the onState callback. iOS screens have migrated from old per-feature stores (BookmarkStore, ProfileStore, etc.) to data flowing through nmpState: HighlighterAppState on the store, with the old per-feature stores deleted. The NMP pattern (fire-and-forget action dispatch, bounded state snapshots via reconciler) is fully adopted across Rust core, iOS, and Android; the web app intentionally uses NDK directly without NMP. The Android app shares business logic with iOS through the HighlighterNmpApp Rust facade rather than reimplementing it. On iOS, PodcastPlayerStore (AVPlayer position) and CaptureStore (local OCR pipeline) are legitimate device-local state exceptions that remain outside the Rust core. The committed codebase includes an NMP-style app facade in core, iOS store consolidation, an Android skeleton, and share_links.

The NMP app module is exposed to Swift via UniFFI.

The app does not use any external NMP package (no nmp UI, no nmp nip29 crate).

NMP must provide NIP-11 relay information (including the icon) directly to apps so that Highlighter iOS and Android require no extra work, no direct HTTP requests, no parsing, and no awareness of NIP-11. When the NIP-11 relay information feature lands on the master branch of ~/Work/nostr-multi-platform, a new NMP version is deployed and Highlighter is updated to use it.

The web app is intentionally outside the NMP architecture, using NDK directly with no NMP integration, duplicating logic the Rust core owns. A decision must be made explicitly on whether the web app stays permanently outside NMP or adopts the Rust core via a WASM build.

Every Android UI panel must construct its own HighlighterAppAction and dispatch it via the NMP pattern (state + dispatch), rather than relying on wrapper callbacks. The HighlighterAppScreen composable takes only (state, dispatch) parameters; every panel constructs its own HighlighterAppAction and dispatches it directly.

Visibility for cross-file symbols is internal; everything used within a single file stays private.

The Android app must register an EventBridge (setCoreEventCallback) before dispatching Bootstrap to receive relay status changes, NIP-46 login completion, and live data deltas from the Rust core, mirroring the iOS registerEventBridge() pattern so events are no longer silently dropped. The iOS app must call the Rust core event bridge (setCoreEventBridge) at startup before bootstrap, matching the same pattern. Both platforms call initPlatformLogging() at startup to route the Rust core's tracing output to the platform logging system (logcat on Android, stderr/Xcode console on iOS). The relay connection status bug is fixed: the kernel's built-in relay-diagnostics projection travels inside emitted snapshot frames, which highlighter-core now decodes via nmp_app_set_update_callback → frame decode → diagnostics state, driving connection status to reach "Online" on both Android and iOS.

Android profile lookups use profileFor(pubkey) with trim+lowercase normalization matching iOS HighlighterStore.profile, provided via LocalProfiles CompositionLocal, rendering avatars in CommentRow, ChatRow, and DiscussionRow with monogram fallback.

Per-tab dispatch lifecycle maps: Highlights (OpenHomeFeed/CloseHomeFeed, PullToRefresh→RefreshHomeFeed), Rooms (OpenRoomExplorer/CloseRoom, RefreshRoomExplorer), Search (SearchOpened/SearchClosed), Settings (OpenMediaSettings+OpenNetworkSettings enter / Close leave), Bookmarks (OpenBookmarks/CloseBookmarks), Feedback (OpenFeedback(coord)/CloseFeedback).

The edit profile screen has fields for displayName, name, about, picture, banner, nip05, website, lud16 with live upload spinners and error handling, gated by a host-held boolean dispatched via OpenEditProfile/CloseEditProfile.

The curation menu is a ModalBottomSheet driven by state.curationMenu, with checkmark on member sets, toggle via SetAddressInCurationSet, inline new-collection field via CreateCurationSetAndAdd, and dismiss via CloseCurationMenu.

~92 blocking network awaits inside the NMP actor loop are an architectural defect that causes the actor to wedge on dead networks; a Fable agent is researching the proper fix with a phased design doc.

<!-- citations: [^0c7b6-83] [^d9710-1] [^d9710-2] [^d9710-3] [^0c7b6-6] [^0c7b6-5] [^0c7b6-14] [^0c7b6-18] [^0c7b6-33] [^0c7b6-44] [^0c7b6-56] [^0c7b6-69] [^0c7b6-82] [^0c7b6-97] [^0c7b6-165] [^f54b4-2] [^cd5f3-1] [^cd5f3-4] -->
