---
title: Native Dependencies
slug: native-dependencies
topic: native-dependencies
summary: The native library libhighlighter_core.so dynamically links against libsodium and requires the symbol crypto_stream_chacha20_ietf_xor_ic.
tags:
  - capture
volatility: warm
confidence: medium
created: 2026-06-12
updated: 2026-06-12
verified: 2026-06-12
compiled-from: conversation
sources:
  - session:fc140a10-b623-435c-9e69-364f38ce9541
  - session:0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
---

# Native Dependencies

## Native Dependencies

The native library libhighlighter_core.so dynamically links against libsodium and requires the symbol crypto_stream_chacha20_ietf_xor_ic. The app must not crash on launch due to unresolved libsodium symbols. The Android build requires an NDK llvm-ar override to link libsodium correctly. To guard against unresolved symbols, a post-build task (fixLibsodiumAndRelink) checks whether libsodium.a is empty, re-creates it with llvm-ar from the compiled .o files, and re-links if needed. This libsodium fix logic is implemented in an external shell script (fix-libsodium.sh) invoked from Gradle rather than embedded inline.

<!-- citations: [^fc140-2] [^fc140-4] [^0c7b6-24] -->
