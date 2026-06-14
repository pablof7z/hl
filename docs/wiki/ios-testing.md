---
title: iOS Testing
slug: ios-testing
topic: nmp-app
summary: The iOS first unit test suite contains 22 tests in 2 suites (TranscriptParserTests with 14 tests and CommentTreeBuilderTests with 8 tests) using the Swift Testi
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

# iOS Testing

## iOS Test Target

The iOS first unit test suite contains 22 tests in 2 suites (TranscriptParserTests with 14 tests and CommentTreeBuilderTests with 8 tests) using the Swift Testing framework.

A seeded test Nostr account was created for validation with 16 followed highlighters and 115 confirmed real kind:9802 highlight events, credentials stored at ~/Builds/test-account.txt outside the repo. <!-- [^84748-63] -->

<!-- citations: [^0c7b6-55] [^0c7b6-150] -->

## Maestro E2E Testing

The testTagsAsResourceId semantics flag is set on the root composable container, enabling Maestro to address all testTags by resource-id.

Maestro flow files are stored at app/android/maestro/ covering login (00), feed (06), highlight-detail (08), comments (30), rooms-explorer (11), open-room (12), create-room (19), and search-nav (33).

<!-- citations: [^84748-135] [^84748-136] [^84748-147] -->
