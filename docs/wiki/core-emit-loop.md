---
title: Core Emit Loop
slug: core-emit-loop
topic: native-dependencies
summary: The main-thread jank was caused by uncoalesced full-state emits on every resolved op (emit_hz was configured but never implemented in the actor loop) plus onSta
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

# Core Emit Loop

## Jank Root Cause

The main-thread jank was caused by uncoalesced full-state emits on every resolved op (emit_hz was configured but never implemented in the actor loop) plus onState calling syncNetworkCallback (expensive OS calls) on every emit; fixes include a sample(16ms) StateFlow coalescing, hydration dispatch guards, and a wifi-only change guard.

<!-- citations: [^84748-163] [^84748-189] -->
## State Sampling

The state StateFlow is coalesced via sample(16.milliseconds).stateIn(viewModelScope) to bound recomposition to ~1 per frame regardless of burst emissions.

<!-- citations: [^84748-164] [^84748-177] [^84748-202] -->
## Rate-Limited Emit Loop

The core's state emission rate is capped by an implemented emit_hz rate limiter (default 30 Hz) with a trailing-emit guarantee, preventing recomposition floods from per-op full-state snapshots.

<!-- citations: [^84748-165] [^84748-178] [^84748-190] [^84748-203] [^84748-216] -->
## OS Callback Guard

The HighlighterViewModel.onState callback only calls syncNetworkCallback when wifiOnlyEnabled has actually changed, preventing hundreds of spurious OS-level registerNetworkCallback calls per state emission.

<!-- citations: [^84748-179] [^84748-191] [^84748-204] [^84748-217] -->
## Debug Log Guard

The feed-loading debug log joinToString allocation is guarded behind Log.isLoggable so it is skipped in production builds. <!-- [^84748-180] -->
