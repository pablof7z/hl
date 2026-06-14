---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: superseded
subjects:
  - android-jank
  - emit-hz-rate-limiter
  - lazy-list-virtualization
  - onState-os-call-guard
supersedes:
  - 2026-06-13-1-feed-jank-multi-layered-root-cause
related_claims: []
source_lines:
  - 1536-1618
  - 1619-1673
  - 1727-1800
  - 1829-1846
  - 1847-1930
captured_at: 2026-06-13T16:02:40Z
---

# Episode: Jank root-cause chain: unimplemented emit_hz + eager Column + unguarded OS calls

## Prior State

App was severely janky (72–80% janky frames, 100–379 frame skips) causing navigation to be completely unresponsive. Initial fix attempt (Kotlin-side .sample(16ms) on StateFlow) was insufficient. The emit_hz config parameter existed (30 Hz) but was never implemented in the Rust actor loop — every OpResolved emitted a full state clone+push. HomeFeedPanel rendered all ~140 cards eagerly inside a single LazyColumn item, defeating virtualization. onState unconditionally called syncNetworkCallback (expensive OS registerNetworkCallback/unregisterNetworkCallback) on every state emission.

## Trigger

Validation on a fresh cold-booted emulator confirmed jank was a real app bug, not emulator degradation (72.56% janky, 220 slow draw commands). Investigation traced three root causes: (1) emit_hz dead code, (2) eager Column composing all cards at once, (3) onState making OS calls on every emission regardless of value change.

## Decision

Three-layer fix: (1) Implement recv_timeout-based emit_hz rate limiter in nmp_app.rs with trailing-emit guarantee (latest state always delivered within one interval), (2) Guard syncNetworkCallback in HighlighterViewModel.onState with a lastWifiOnly change-detection field, (3) Convert HomeFeedPanel from an eager Column { items.forEach {} } inside a single LazyColumn item to a LazyListScope extension emitting each card as its own keyed lazy item. Secondary: memoize joinedRoomIds with derivedStateOf, guard debug log allocation with Log.isLoggable.

## Consequences

- Janky frames dropped from ~72% to 48%, worst frame skip from 379→36 frames
- Navigation taps now register; rooms tab works on clean emulator
- Residual 48% jank is per-card image-decode cost (polish tier), not the all-at-once composition storm
- The emit_hz mechanism is now a real rate-limiter in the Rust core, reusable for future throttling needs
- Full Rust rebuild required whenever nmp_app.rs changes (the .so goes stale)

## Open Tail

- Residual 48% jank could be further reduced by image-load tuning and per-card composition lightening
- Optional core-side emit_hz tuning (currently 30 Hz) can be adjusted if smoother frame delivery is needed

## Evidence

- transcript lines 1536-1618
- transcript lines 1619-1673
- transcript lines 1727-1800
- transcript lines 1829-1846
- transcript lines 1847-1930

