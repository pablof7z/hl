---
title: UI Components
slug: ui-components
topic: ui-components
summary: The AuthorAvatar view (Profile/AuthorAvatar.swift) is a custom SwiftUI component, not sourced from an nmp UI library
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
---

# UI Components

## AuthorAvatar

The AuthorAvatar view (Profile/AuthorAvatar.swift) is a custom SwiftUI component, not sourced from an nmp UI library. It displays a monogram fallback when no profile matches, and loads the actual profile picture via Kingfisher when a URL is present. Avatars render at 32dp in comments/chat and 28dp in discussions. Chat and comment avatars must render via state.profiles lookup, not a missing API.

<!-- citations: [^d9710-4] [^0c7b6-47] [^0c7b6-70] -->
## Android Modularization

The Android app UI must be modularized from a single 3300-line MainActivity.kt into well-organized files comparable to iOS's 105 Swift files. <!-- [^0c7b6-7] -->

## Podcast Mini-Player

The Android app requires a dedicated podcast mini-player UI to match iOS feature parity. <!-- [^0c7b6-8] -->

## Standalone Book View

The Android app requires a standalone book view to match iOS feature parity. <!-- [^0c7b6-9] -->

## Navigation & Routing

ProfileDestination must conform to Identifiable for use with .navigationDestination(item:). The iOS CommentRow has a 'View profile' menu item that navigates to ProfileView, using a local @State profileDestination and .navigationDestination(item:) pattern matching ThreadView's existing contract.

<!-- citations: [^0c7b6-46] [^0c7b6-58] -->
