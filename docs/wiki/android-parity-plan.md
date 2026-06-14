---
title: Android Parity Plan
slug: android-parity-plan
topic: nmp-app
summary: The Android parity plan is persisted at `Plans/android-parity-plan.md` containing an exhaustive iOS feature inventory with file refs, 38 concrete validation flo
tags:
  - capture
volatility: warm
confidence: medium
created: 2026-06-13
updated: 2026-06-14
verified: 2026-06-13
compiled-from: conversation
sources:
  - session:847487cd-e15b-4222-85ee-4a5a2b6f590b
---

# Android Parity Plan

## Overview

The Android parity plan is persisted at `Plans/android-parity-plan.md` containing an exhaustive iOS feature inventory with file refs, 38 concrete validation flows with step-by-step outcomes, an Android gap audit (MISSING/BROKEN/PARTIAL/WORKING per flow), a UI/IA reorganization plan, a validation-harness plan, and a phased implementation roadmap. Both iOS and Android apps are thin renderers over a single Rust state machine (`dispatch(HighlighterAppAction)` + `state()` snapshot tree); every iOS feature already exists in the core as actions + snapshot fields, so parity is rendering/IA/navigation work, not new business logic. The feed-empty-fix spec is at `Plans/feed-empty-fix.md`; the responsiveness-and-toast-fix spec is at `Plans/responsiveness-and-toast-fix.md`.

<!-- citations: [^84748-22] [^84748-37] [^84748-52] [^84748-97] [^84748-111] [^84748-123] [^84748-132] [^84748-196] -->
## Parity Gaps

OCR camera capture is genuinely missing from Android (no CAMERA permission, no camera, no OCR, no ISBN entry/lookup) and is the largest parity gap. The CapturePanel was rewritten to support book picker recents (RequestBookPickerRecents(24)), manual ISBN entry with normalizeIsbn validation and dedup against recents, ISBN preview lookup (RequestIsbnPreview), and a 'Use' button that commits a Pending artifact with all iOS-parity catalog fields.

<!-- citations: [^84748-23] [^84748-131] -->
## Implementation Roadmap

The implementation roadmap is organized into 8 phases (0–7), dependency-ordered, where Phases 1–3 close the basic functionality (rooms openable, create-room relocated, feed visible), Phase 4 builds OCR, and Phases 5–7 cover social/reading/podcast polish. PR #6 (feat/android-ios-parity) contains the full parity work: 39 files, +5575/−508, merged to main as commit 45382ee; iOS file changes in the working tree (Self.firstNonEmpty, project.yml build number, pbxproj) were not committed as they are pre-existing/parallel changes.

<!-- citations: [^84748-24] [^84748-185] [^84748-197] -->
## Validation Harness

The validation harness uses the `HighlighterTest` AVD (android-34, google_apis, arm64-v8a) which matches the arm64-only APK constraint. Maestro flows (one per numbered validation flow) are the recommended harness, with adb/uiautomator fallback; nsec login (not signup) is used to avoid the known account-creation hang. The Android emulator has no camera, so OCR flow validation must use injected images with iOS reference baselines. Compose UI nodes are discoverable by text-based selectors via `uiautomator` for Maestro/automation test driving.

<!-- citations: [^84748-25] [^84748-53] -->
## Known Issues

Known core/server issues include a signup hang, NIP-05 404 errors, and App Links verification failures for `beta.highlighter.com`. <!-- [^84748-26] -->

## Test Data

The seeded test account (`npub1sle0h9fqdffs2qh3lfzax2zaer5cn7v9phtl4uls93t808qaws2std326a`) follows 16 highlighters with 115 verified kind:9802 highlight events on the app's read relays, and its credentials are stored outside the git repo at `~/Builds/test-account.txt`. Validation screenshots and logs are persisted at `~/Builds/validation-before/` (pre-fix baseline) and `~/Builds/validation-after/` (post-fix with logged-in feed). <!-- [^84748-27] -->
