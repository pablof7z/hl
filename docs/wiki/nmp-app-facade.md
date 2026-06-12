---
title: NMP App Facade
slug: nmp-app-facade
topic: nmp-app
summary: The app uses the NMP app facade (nmp_app.rs) from core, with HighlighterStore holding a HighlighterNmpApp instance and a HighlighterAppStateReconciler that rece
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

# NMP App Facade

## NMP App Facade Architecture

The app uses the NMP app facade (nmp_app.rs) from core, with HighlighterStore holding a HighlighterNmpApp instance and a HighlighterAppStateReconciler that receives state pushes via the onState callback. iOS screens have migrated from old per-feature stores (BookmarkStore, ProfileStore, etc.) to data flowing through nmpState: HighlighterAppState on the store, with the old per-feature stores deleted. The committed codebase includes an NMP-style app facade in core, iOS store consolidation, an Android skeleton, and share_links. <!-- [^d9710-1] -->

The NMP app module is exposed to Swift via UniFFI. <!-- [^d9710-2] -->

The app does not use any external NMP package (no nmp UI, no nmp nip29 crate). <!-- [^d9710-3] -->
