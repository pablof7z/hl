---
title: iOS Safety & Navigation
slug: ios-safety-and-navigation
topic: ui-components
summary: "iOS fixes five crash risks: BookScannerView layer cast, MarkdownRenderer attributedString cast and three URL(string:) force-unwraps, OCRStructureReconstructor m"
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

# iOS Safety & Navigation

## Safety and Navigation

iOS fixes five crash risks: BookScannerView layer cast, MarkdownRenderer attributedString cast and three URL(string:) force-unwraps, OCRStructureReconstructor max-by unwrap, and ShareToCommunitySheet displayTitle/imageURL force-unwraps — all replaced with guard-let/fallback. CommentRow's 'View profile' is wired via .navigationDestination(item:) with ProfileDestination conforming to Identifiable, matching ThreadView's existing model. iOS routes inbound share links (nevent/nprofile/naddr) by decoding and routing them, plus universal-link entitlement; previously iOS only handled nip46 and share-extension handoff and never consumed the share links it minted, while Android now routes them and iOS has been given the same routing. iOS adds ~32 VoiceOver accessibility labels and traits across 20 files: Capture (3 labels), Podcast (7), Comments (2), Communities (10), Profile (2), Settings (3), Article reader (2), Feedback (1); decorative icons are skipped. iOS converts 11 previously silent try? failure sites to do/catch with .error logging, covering keychain saves, podcast position encode/decode, waveform cache, capture crop/OCR, bundled Readability.js, highlight JSON, nostr event-tags, and transcript parsing. The iOS test suite has 22 unit tests covering TranscriptParser and CommentTreeBuilder, and must use the Swift Testing framework (import Testing, @Test, #expect) with @testable import Highlighter.

<!-- citations: [^0c7b6-41] [^0c7b6-42] [^0c7b6-43] [^0c7b6-32] [^0c7b6-40] [^0c7b6-54] [^0c7b6-68] [^0c7b6-81] [^0c7b6-96] [^0c7b6-113] [^0c7b6-128] [^0c7b6-138] [^0c7b6-149] [^0c7b6-164] [^0c7b6-187] -->
