---
title: UI Components
slug: ui-components
topic: ui-components
summary: The AuthorAvatar view (Profile/AuthorAvatar.swift) is a custom SwiftUI component, not sourced from an NMP UI library
tags:
  - capture
volatility: warm
confidence: medium
created: 2026-06-11
updated: 2026-06-13
verified: 2026-06-11
compiled-from: conversation
sources:
  - session:d9710893-bea1-487e-9bb2-499a23d553a6
  - session:0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
  - session:cd5f3967-ddef-43db-91ca-0d6b810bcfea
  - session:847487cd-e15b-4222-85ee-4a5a2b6f590b
---

# UI Components

## AuthorAvatar

The AuthorAvatar view (Profile/AuthorAvatar.swift) is a custom SwiftUI component, not sourced from an NMP UI library. It displays a monogram fallback when no profile matches, and loads the actual profile picture via Kingfisher when a URL is present. Avatars render at 32dp in comments/chat and 28dp in discussions. Chat and comment avatars must render via state.profiles lookup, not a missing API. The `HighlightFeedCardView` uses `NostrAvatar` and `NostrProfileName` components instead of a custom `AuthorAvatar` and manual `app.profile()` calls. The `NostrAvatar` component owns the `claimProfile` lifecycle, calling `requestProfile` on mount and re-claiming on pubkey change, reading profile data live from the NostrProfileHost to ensure profiles populate as Rust resolves kind:0 events.

<!-- citations: [^d9710-4] [^0c7b6-47] [^0c7b6-70] [^cd5f3-3] [^cd5f3-6] -->
## Android Modularization

The Android app UI must be modularized from a single 3300-line MainActivity.kt into well-organized files comparable to iOS's 105 Swift files. <!-- [^0c7b6-7] -->

## Podcast Mini-Player

The Android app requires a dedicated podcast mini-player UI to match iOS feature parity. <!-- [^0c7b6-8] -->

## Standalone Book View

The Android app requires a standalone book view to match iOS feature parity. <!-- [^0c7b6-9] -->

## Navigation & Routing

ProfileDestination must conform to Identifiable for use with .navigationDestination(item:). The iOS CommentRow has a 'View profile' menu item that navigates to ProfileView, using a local @State profileDestination and .navigationDestination(item:) pattern matching ThreadView's existing contract.

Search person rows dispatch OpenProfile(pubkey) and community rows dispatch OpenRoom(groupId), making both tappable like iOS. <!-- [^84748-148] -->

<!-- citations: [^0c7b6-46] [^0c7b6-58] -->

## NMP UI Components

Highlights feed uses NMP UI components (NostrAvatar and NostrProfileName) instead of the custom AuthorAvatar view and manual profile resolution. NMP UI components are installed via the `nmp add component` CLI command, which copies source to the standard `Components/NostrUser/` path and writes an `nmp.components.lock` file for future updates; manual vendoring is incorrect. The NostrAvatar component in the NMP registry uses Kingfisher (KFImage) by default rather than AsyncImage. NostrAvatar owns the claimProfile lifecycle, calling requestProfile on mount and re-claiming on pubkey change, reading profile data live from the NostrProfileHost. The app root injects the store as a `nostrProfileHost` environment value (`.environment(\.nostrProfileHost, store)`) so NMP UI components can resolve profile data.

<!-- citations: [^cd5f3-2] [^cd5f3-5] -->
