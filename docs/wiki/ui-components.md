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
updated: 2026-06-11
verified: 2026-06-11
compiled-from: conversation
sources:
  - session:d9710893-bea1-487e-9bb2-499a23d553a6
---

# UI Components

## AuthorAvatar

The AuthorAvatar view (Profile/AuthorAvatar.swift) is a custom SwiftUI component, not sourced from an nmp UI library. It displays a deterministic gradient fallback derived from the pubkey hash with an initial letter when no picture URL is available, and loads the actual profile picture via Kingfisher when a URL is present. <!-- [^d9710-4] -->
