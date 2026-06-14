---
title: Android Composition Patterns
slug: android-composition-patterns
topic: ui-components
summary: Leaf row composables (HydratedHighlightRow, DiscussionRow, FeedbackThreadRow, CommentRow, ArtifactPickerRow) keep local () -> Unit parameters composed by their
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

# Android Composition Patterns

## Leaf Row Composables

Leaf row composables (HydratedHighlightRow, DiscussionRow, FeedbackThreadRow, CommentRow, ArtifactPickerRow) keep local () -> Unit parameters composed by their parent panels rather than passing dispatch directly. The unconditional Log.i with joinToString on feed.items is guarded by Log.isLoggable() to skip the string allocation in production.

<!-- citations: [^0c7b6-20] [^84748-69] [^84748-105] -->
## CurationMenuSheet

CurationMenuSheet is a ModalBottomSheet driven by state.curationMenu, with loading/empty/error/list states, checkmark on member sets, toggle via SetAddressInCurationSet, inline 'New collection' field via CreateCurationSetAndAdd, and dismiss via CloseCurationMenu. The CurationMenu constructs the NIP-33 a-tag as "30023:${article.pubkey}:${article.identifier}" to match the iOS contract, since ArticleRecord has no address field.

<!-- citations: [^0c7b6-74] [^0c7b6-135] [^0c7b6-178] -->
## WebLinkPreview Composable

WebLinkPreview composable extracts the first http(s) URL from comment/discussion bodies, dispatches RequestWebMetadata(url), and renders a title/site/image card from state.webMetadata. Web metadata rendering in comments and room detail uses this composable. Web-metadata in bookmark rows was deferred.

<!-- citations: [^0c7b6-75] [^0c7b6-180] -->
## Avatar Resolution

Android comment/chat/discussion rows render author avatars and display names via a profileFor(pubkey) lookup with trim+lowercase normalization matching iOS, using a LocalProfiles CompositionLocal provided in MainActivity. Monogram fallback is used when no profile matches.

The HighlighterAppState snapshot fields profiles, webMetadata, and isbnPreviews are provided to the Compose tree via CompositionLocalProvider so feed cards, room tiles, and other panels can access them for hydration without prop drilling.

<!-- citations: [^84748-70] [^0c7b6-87] [^0c7b6-109] [^0c7b6-134] [^0c7b6-145] [^0c7b6-179] [^84748-106] [^84748-207] -->
## ToastBanner

The ToastBanner in RootScene is positioned below the status bars + TopAppBar height + 8dp gap (WindowInsets.statusBars + 64.dp + 8.dp) so it no longer overlaps the header/avatar/status chrome when edge-to-edge is active. A keyed LaunchedEffect with delay + ClearToast dispatch is added in RootScene.kt as an Android safety net to auto-expire toasts, since Android's ToastBanner has no built-in auto-expiry.

The 'not found' toast is caused by three benign per-card hydration paths (handle_web_metadata_resolved, handle_isbn_preview_resolved, request_profile subscribe failure) that surface CoreError::NotFound as a global Error toast, which persists because Android's ToastBanner has no auto-expiry. It is fixed primarily by removing set_toast calls in those three benign core paths (log/insert a negative marker instead), and secondarily by the auto-expiry safety net described above.

<!-- citations: [^84748-34] [^84748-49] [^84748-50] [^84748-108] -->
## State Flow Coalescing

StateFlow emissions are coalesced to ~1 per frame via sample(16.milliseconds) to prevent recomposition floods from burst op resolutions. The ViewModel's state flow is coalesced via .sample(16.milliseconds).stateIn(viewModelScope, SharingStarted.Eagerly, _state.value), bounding recomposition to ~1 per frame regardless of burst count. The ANR/jank root cause is a recomposition flood: the core emits a full HighlighterAppState clone on every resolved op (uncoalesced via onState), and per-card LaunchedEffects don't dedupe, causing whole-tree recompositions on Main — not a UI-thread block.

RemoteImage and AvatarImage both use Coil AsyncImage with crossfade(true), so image loading is already asynchronous and not a cause of UI jank.

joinedCommunities-to-Set is memoized via remember+derivedStateOf in RoomsTab to avoid allocating a new Set on every recomposition. <!-- [^84748-144] -->

<!-- citations: [^84748-57] [^84748-72] [^84748-107] -->
## CommentsPanel

CommentsPanel supports threaded replies: a buildCommentTree() function mirrors iOS CommentTreeBuilder, ReplyContextBanner shows 'Replying to @name' with a Cancel button, and depth-1 rows get a 2dp thread rail. <!-- [^84748-71] -->
