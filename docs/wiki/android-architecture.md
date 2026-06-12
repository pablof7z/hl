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
updated: 2026-06-12
verified: 2026-06-12
compiled-from: conversation
sources:
  - session:0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
---

# Android Architecture

## File Organization & Architecture

The Android app's single-file MainActivity.kt (3,317 lines) is refactored into 23 cohesive files with per-feature packages, each panel constructing its own HighlighterAppAction and dispatching it. The app uses a 2-parameter (state, dispatch) AppScreen. The app uses a 3-tab Material3 NavigationBar (Highlights/Rooms/Search) with a top bar (avatar→profile, gear→settings, status subtitle), a Capture FAB, and per-tab dispatch lifecycle (e.g. OpenHomeFeed/CloseHomeFeed). FEEDBACK_PROJECT_COORDINATE is located in AppConfig.kt in the same package as MainActivity.

<!-- citations: [^0c7b6-48] [^0c7b6-59] [^0c7b6-71] [^0c7b6-105] [^0c7b6-120] [^0c7b6-176] -->
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
