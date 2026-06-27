---
title: Kernel Domains
slug: kernel-domains
topic: kernel-domains
summary: NMP-backed kernel domains (`kernel/domains/`) are already in use via typed projections and mailbox cache for all product reads of highlights, artifacts, profile
tags:
  - capture
volatility: warm
confidence: medium
created: 2026-06-26
updated: 2026-06-26
verified: 2026-06-26
compiled-from: conversation
sources:
  - session:9ae03596-fa74-4208-88c6-a90bd3b176e4
---

# Kernel Domains

## Status

NMP-backed kernel domains (`kernel/domains/`) are already in use via typed projections and mailbox cache for all product reads of highlights, artifacts, profiles, feedback, relays, and relay diagnostics. No upstream blocking on new NMP APIs exists. Remaining `nostrdb` references in `nostr_runtime.rs` and five feature modules are dead code with zero product callers and are suitable for deletion. <!-- [^9ae03-90c0b] -->
