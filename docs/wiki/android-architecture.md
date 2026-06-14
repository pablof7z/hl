---
title: Android Architecture
slug: android-architecture
topic: nmp-app
summary: The Android app's single-file MainActivity.kt (3,317 lines) is refactored into 23 cohesive files with per-feature packages, each panel constructing its own High
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

# Android Architecture

## File Organization & Architecture

The Android app's single-file MainActivity.kt (3,317 lines) is refactored into 23 cohesive files with per-feature packages, each panel constructing its own HighlighterAppAction and dispatching it. The app uses a 2-parameter (state, dispatch) AppScreen. The app uses a 3-tab Material3 NavigationBar (Highlights/Rooms/Search) with a top bar (avatar→profile, gear→settings, status subtitle), a Capture FAB, and per-tab dispatch lifecycle (e.g. OpenHomeFeed/CloseHomeFeed). FEEDBACK_PROJECT_COORDINATE is located in AppConfig.kt in the same package as MainActivity. The home feed must not silently cap items via `take(8)` and must distinguish a loading/syncing state from a truly empty state. The root Box in RootScene has semantics { testTagsAsResourceId = true } applied, making all testTags addressable by Maestro via resource-id selectors.

<!-- citations: [^0c7b6-48] [^0c7b6-59] [^0c7b6-71] [^0c7b6-105] [^0c7b6-120] [^0c7b6-176] [^84748-6] [^84748-119] -->
## Data Normalization

Profile lookups on Android trim and lowercase the pubkey hex to match the core's storage normalization. <!-- [^0c7b6-60] -->

## Deferred Features

Web metadata in bookmark rows and ISBN previews are deferred/not yet wired on Android. <!-- [^0c7b6-61] -->

## Auth & Profile

Android auth flow mirrors iOS RootSceneView: welcome screen → login / create-account screens → full-screen onboarding interests → main app, instead of a single LazyColumn. Android Edit Profile is a full-screen destination with banner+avatar pickers dispatching UploadEditProfileImage, text fields for displayName/name/about/nip05/website/lud16, live upload spinners, and SubmitEditProfile; the sheet is host-gated by an editProfileOpen boolean matching the core having no isOpen flag.

<!-- citations: [^0c7b6-72] [^0c7b6-106] [^0c7b6-121] [^0c7b6-143] -->
## Curation Menu

Android Curation menu is a ModalBottomSheet driven by state.curationMenu, showing loading/empty/error/list states, checkmarks on member sets, toggle membership via SetAddressInCurationSet, inline 'New collection' field dispatching CreateCurationSetAndAdd, per-row BookmarkAdd icon button, and dismiss via CloseCurationMenu.

<!-- citations: [^0c7b6-86] [^0c7b6-107] [^0c7b6-144] -->
## Podcast Player

Android has a podcast mini-player bar and full listening screen with chapters/speed/skip via Media3 ExoPlayer, skipping waveform extraction (iOS-only for now). <!-- [^0c7b6-122] -->

## Core Architecture Principle

Both iOS and Android apps are thin renderers over a single Rust state machine (`dispatch(HighlighterAppAction)` + `state()` snapshot tree); every iOS feature already exists in the core as actions plus snapshot fields, so parity is rendering/IA/navigation work, not new business logic. <!-- [^84748-3] -->

## Plan Document

The plan document at `Plans/android-parity-plan.md` contains the exhaustive iOS feature inventory, 38 concrete validation flows, the Android gap audit, UI/IA reorganization plan, validation-harness plan, and phased implementation roadmap. <!-- [^84748-4] -->

## Phased Roadmap

The roadmap consists of 8 phases (0–7), dependency-ordered, each sized for a Sonnet coding agent plus Haiku validator; Phases 1–3 close the basics (rooms openable, create-room relocated, feed visible), Phase 4 builds OCR, and Phases 5–7 are social/reading/podcast polish. <!-- [^84748-5] -->

## Feed Rendering & State Symmetry

The Android feed render path, UniFFI type mapping, enum/field names, state plumbing, and core bridges are byte-for-byte correct and symmetric with iOS; an empty feed can only mean `state.homeFeed.items` is empty at the snapshot level (data/auth/sync starvation in the core), not a Compose/mapping bug. The feed loading state ("Syncing highlights…") must be kept; it is original, identical to iOS, and does not swallow the populated case (the `else` branch fires whenever `items` is non-empty regardless of `isLoading`). <!-- [^84748-7] -->
