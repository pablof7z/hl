---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: superseded
subjects:
  - android-feed-jank
  - home-feed-panel
  - highlighter-view-model
  - nmp-app-actor
supersedes: []
related_claims: []
source_lines:
  - 1446-1495
  - 1577-1595
  - 1619-1672
  - 1743-1797
  - 1827-1832
  - 1847-1931
captured_at: 2026-06-13T15:54:26Z
---

# Episode: Feed jank: multi-layered root cause from unimplemented rate-limiter to eager composition

## Prior State

UI jank attributed solely to StateFlow emit frequency; initial fix was .sample(16ms) on the Kotlin side, which only partially improved the problem (21% janky but still 222–379 frame skips, navigation unresponsive).

## Trigger

After the sample fix, validation still showed severe jank and taps not registering. Cold-boot test on a fresh emulator confirmed 72% janky frames was a real app bug, not emulator exhaustion. Profiling and code inspection revealed two additional root causes layered underneath.

## Decision

Three fixes applied in sequence: (1) emit_hz was configured at 30Hz but never implemented — the actor loop called emit() unconditionally on every OpResolved. Implemented a proper recv_timeout-based rate limiter with trailing-emit guarantee. (2) HighlighterViewModel.onState called syncNetworkCallback (expensive OS register/unregister calls) on every state emission regardless of whether wifiOnly changed — guarded with lastWifiOnly comparison. (3) HomeFeedPanel was hosted as a single LazyColumn item containing an eager Column with all ~140 cards, defeating virtualization entirely — converted to a LazyListScope extension emitting each card as its own keyed lazy item.

## Consequences

- Janky frames dropped from ~72% to 48%; worst frame skip from 379 to 36 frames
- Navigation taps now register (Rooms tab, room detail, tab switching all work on clean emulator)
- Residual 48% janky is per-card image decode/render cost on visible items, not all-at-once realization
- emit_hz rate limiter is now a durable invariant — all future op resolutions are bounded
- The .sample(16ms) on Kotlin side remains as an additional coalescing layer
- RoomDetailPanel tabs were already LazyColumn — only HomeFeedPanel needed conversion

## Open Tail

- Remaining 48% jank is per-card composition cost (image decode, profile hydration) — tunable via image-load strategy but not a structural issue
- Optional core-side emit_hz at the nmp_app.rs:11093 boundary was left as a future lever

## Evidence

- transcript lines 1446-1495
- transcript lines 1577-1595
- transcript lines 1619-1672
- transcript lines 1743-1797
- transcript lines 1827-1832
- transcript lines 1847-1931

