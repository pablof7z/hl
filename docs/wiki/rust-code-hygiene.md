---
title: Rust Code Hygiene
slug: rust-code-hygiene
topic: native-dependencies
summary: The Rust core must not contain todo!() panics or dead legacy API modules
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

# Rust Code Hygiene

## Code Hygiene

The Rust core must not contain todo!() panics or dead legacy API modules. Two todo!() panics and dead legacy code (cache.rs module, dead hydrate stub, two dead legacy methods) are removed from the Rust core, with all 251 core tests still passing. There is a pre-existing flaky core test that also fails on the untouched tree. The Rust core's tracing output is routed to logcat on Android and stderr/Xcode console on iOS via new initPlatformLogging()/initPlatformLoggingWithFilter() exports, replacing the previous silent-drop behavior.

The Rust core's Cargo.toml references a sibling path dependency ../../../nostr-multi-platform which requires a dual-checkout in CI.

<!-- citations: [^0c7b6-170] [^0c7b6-85] [^0c7b6-34] [^0c7b6-57] [^0c7b6-115] [^0c7b6-129] [^0c7b6-193] -->
