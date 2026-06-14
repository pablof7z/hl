---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: active
subjects:
  - android-jank
  - emit-hz-rate-limiter
  - home-feed-virtualization
  - onstate-os-call-guard
supersedes:
  - 2026-06-13-2-feed-eager-column-lazycolumn-virtualization
  - 2026-06-13-1-android-emit-storm-emit-hz-configured
related_claims: []
source_lines:
  - 1624-1674
  - 1787-1802
  - 1830-1845
  - 1919-1940
captured_at: 2026-06-13T16:24:41Z
---

# Episode: Android jank: emit_hz was configured but never implemented + eager feed rendering

## Prior State

The app was severely janky (72% janky frames, 100–379 frame skips) and navigation-blocking. Two root causes: (1) emit_hz=30 was configured in HighlighterViewModel but the Rust actor loop emitted full state on every resolved op unconditionally — the rate limiter was dead code, only referenced in a tracing::debug on actor exit. (2) HomeFeedPanel was hosted as a single LazyColumn item, so all ~140 cards composed eagerly at once. (3) onState called syncNetworkCallback (expensive OS calls) on every state emission regardless of change. (4) joinedCommunities allocated a new Set on every recomposition. (5) Unconditional Log.i with joinToString on every composition.

## Trigger

Validation agents found Rooms tab navigation completely unresponsive due to multi-second frame freezes; cold-boot testing on a fresh emulator confirmed the jank was a real app bug (not emulator exhaustion). Profiling showed 222–379 skipped frames, 72% janky, slow UI thread dominating.

## Decision

(1) Implemented emit_hz rate limiter in spawn_actor: recv_timeout-based loop with trailing-emit guarantee (dirty flag + timeout flush), so the latest state is always delivered within one interval. (2) Rewrote HomeFeedPanel from a @Composable returning a Column (hosted as one giant lazy item) into a LazyListScope extension emitting each card as its own keyed lazy item. (3) Guarded syncNetworkCallback in onState with a lastWifiOnly change-detection so OS calls only fire when the value actually changes. (4) Memoized joinedCommunities with derivedStateOf. (5) Guarded debug log with Log.isLoggable.

## Consequences

- Janky frames dropped from ~72% to ~48%; worst frame skip from 379 to 36 (roughly 10× improvement on worst-case freeze)
- Navigation-blocking freezes eliminated — Rooms tab, tab switching all register promptly
- The trailing-emit guarantee means no op result is lost: either emitted immediately or on first timeout after burst
- Residual ~48% jank is per-card image-decode (expected, not all-at-once realization)
- Image loading was already async (Coil AsyncImage) — not a cause, ruling out that path

## Open Tail

- Further jank reduction would need per-card composition lightening and image-load tuning
- The emit_hz implementation is in Rust core, requiring full rebuild on any change

## Evidence

- transcript lines 1624-1674
- transcript lines 1787-1802
- transcript lines 1830-1845
- transcript lines 1919-1940

