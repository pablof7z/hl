---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: architecture
status: superseded
subjects:
  - android-feed-virtualization
  - home-feed-panel
supersedes:
  - 2026-06-13-1-jank-root-cause-chain-unimplemented-emit
related_claims: []
source_lines:
  - 1796-1930
captured_at: 2026-06-13T16:12:24Z
---

# Episode: Feed eager Column → LazyColumn virtualization

## Prior State

HomeFeedPanel was a @Composable fun returning a Column that eagerly composed all ~140 feed cards at once, hosted as a single LazyColumn item in HighlightsTab. Every card — with its images, profile lookups, and hydration effects — composed and drew simultaneously, causing 72% janky frames and 100–379 frame skips that blocked navigation.

## Trigger

After emit_hz fix reduced spike severity but jank persisted at 72% on a clean cold-boot (ruling out emulator degradation), code inspection revealed HomeFeedPanel.kt:119 Column { items.forEach {} } pattern — and that MainScaffold wrapped it as item { HomeFeedPanel(...) }, defeating virtualization entirely.

## Decision

Converted HomeFeedPanel from @Composable fun → LazyListScope extension function (homeFeedItems) emitting keyed lazy items (header, error, loading, empty, one item per card keyed by stableId). Updated HighlightsTab from single item { HomeFeedPanel() } wrapper to direct homeFeedItems() call with verticalArrangement = spacedBy(10.dp).

## Consequences

- Janky frames dropped from ~72% to ~48%, worst frame skip from 100–379 to 36 (≈10× improvement)
- Feed scrolling smooth; tab navigation registers promptly; unresponsive-freeze behavior eliminated
- Offscreen cards no longer compose or dispatch hydration effects, naturally throttling bursts
- RoomDetailPanel tabs and RoomExplorerPanel shelves were already lazy — no changes needed there

## Open Tail

- Residual 48% jank is per-card image decode/render cost (Coil async, not a composition issue) — polish item for future

## Evidence

- transcript lines 1796-1930

