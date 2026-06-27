---
title: Relay-Import Preview
slug: relay-import-preview
topic: relay-import-preview
summary: Relay-import preview is a cached interface over NMP's mailbox cache that displays available relays without requiring a synchronous network fetch
tags:
  - capture
volatility: warm
confidence: medium
created: 2026-06-26
updated: 2026-06-26
verified: 2026-06-26
compiled-from: conversation
sources:
  - session:019f039e-4ac7-7c32-8ff5-c8be2da7ee01
---

# Relay-Import Preview

## Overview

Relay-import preview is a cached interface over NMP's mailbox cache that displays available relays without requiring a synchronous network fetch. Highlighter's implementation uses NMP's mailbox-cache handle instead of HighlighterCore.fetchRelaysForPubkey.

<!-- citations: [^019f0-8a590] [^019f0-59c2c] -->
## Behavior and Constraints

Relay-import preview is read-only and cache-bounded, with no raw events, additional relay client, or second mailbox cache. When a user's kind:10002 event is not present in NMP's cache, the preview displays the existing empty-result UI. <!-- [^019f0-5d611] -->
