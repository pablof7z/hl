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
updated: 2026-06-12
verified: 2026-06-12
compiled-from: conversation
sources:
  - session:0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
---

# Android Composition Patterns

## Leaf Row Composables

Leaf row composables (HydratedHighlightRow, DiscussionRow, FeedbackThreadRow, CommentRow, ArtifactPickerRow) keep local () -> Unit parameters composed by their parent panels rather than passing dispatch directly. <!-- [^0c7b6-20] -->

## CurationMenuSheet

CurationMenuSheet is a ModalBottomSheet driven by state.curationMenu, with loading/empty/error/list states, checkmark on member sets, toggle via SetAddressInCurationSet, inline 'New collection' field via CreateCurationSetAndAdd, and dismiss via CloseCurationMenu. The CurationMenu constructs the NIP-33 a-tag as "30023:${article.pubkey}:${article.identifier}" to match the iOS contract, since ArticleRecord has no address field.

<!-- citations: [^0c7b6-74] [^0c7b6-135] [^0c7b6-178] -->
## WebLinkPreview Composable

WebLinkPreview composable extracts the first http(s) URL from comment/discussion bodies, dispatches RequestWebMetadata(url), and renders a title/site/image card from state.webMetadata. Web metadata rendering in comments and room detail uses this composable. Web-metadata in bookmark rows was deferred.

<!-- citations: [^0c7b6-75] [^0c7b6-180] -->
## Avatar Resolution

Android comment/chat/discussion rows render author avatars and display names via a profileFor(pubkey) lookup with trim+lowercase normalization matching iOS, using a LocalProfiles CompositionLocal provided in MainActivity. Monogram fallback is used when no profile matches.

<!-- citations: [^0c7b6-87] [^0c7b6-109] [^0c7b6-134] [^0c7b6-145] [^0c7b6-179] -->
