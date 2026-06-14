---
title: Android Validation Harness
slug: android-validation-harness
topic: nmp-app
summary: The validation harness uses a HighlighterTest AVD (android-34, google_apis, arm64-v8a), adb, and Maestro for deterministic pass/fail validation with screenshots
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

# Android Validation Harness

## Validation Harness

The validation harness uses a HighlighterTest AVD (android-34, google_apis, arm64-v8a), adb, and Maestro for deterministic pass/fail validation with screenshots and view hierarchies saved under ~/Builds/validation-before/ and ~/Builds/validation-after/. A test account (nsec) stored out-of-band at ~/Builds/test-account.txt (not committed to the repo) is required for authentication-dependent flow validation; the emulator has no camera, so OCR flow is validated via injected image. Login (not signup) is used to avoid the known account-creation hang.

<!-- citations: [^84748-160] [^84748-175] [^84748-213] -->
## Test Tag Configuration

The app uses testTagsAsResourceId = true on the root Box, enabling deterministic testTag-based selectors for all flows. This semantics flag propagates to every testTag in the codebase from the root Box, requiring no other code changes.

<!-- citations: [^84748-161] [^84748-200] -->
## Maestro Validation Flows

38 Maestro flows covering auth, rooms, reading/books, profiles, comments, share, search, settings, and podcast flows are defined as YAML files under app/android/maestro/. A regression smoke suite on the latest-NMP APK confirmed 7 flows pass, verifying that feed projection, NIP-29 room projection, and the registered CreatePublicGroup action work correctly after the ADR-0052 migration.

<!-- citations: [^84748-162] [^84748-176] [^84748-188] [^84748-201] [^84748-214] -->

## Test Tag Inventory

Test tags added across the app include: create_room_fab, room_explorer_list, feed_loading, feed_item_list, feed_highlight_card, feed_reading_card, card_cover, card_author, room_tile_cover, room_tile_name, room_detail_name, room_tab_home, room_tab_library, room_tab_discussions, room_tab_chat, room_new_discussion_fab, capture_book_recents, capture_recent_book, capture_isbn_field, capture_isbn_preview, capture_isbn_use, capture_camera_button, capture_take_photo, capture_ocr_text, capture_select_quote, capture_publish, capture_scan_barcode, highlight_detail, highlight_detail_author, highlight_detail_comment, highlight_detail_share, highlight_detail_bookmark, comment_reply_button, comment_row, comment_reply_row, search_person_row, search_community_row, profile_screen, profile_edit_button, profile_follow_button, bookmarks_screen. <!-- [^84748-215] -->
