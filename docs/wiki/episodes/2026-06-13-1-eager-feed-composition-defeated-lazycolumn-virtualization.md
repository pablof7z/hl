---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: active
subjects:
  - home-feed-rendering
  - android-jank
supersedes:
  - 2026-06-13-1-feed-jank-root-cause-eager-column
related_claims: []
source_lines:
  - 1787-1845
  - 1856-1930
captured_at: 2026-06-13T16:54:53Z
---

# Episode: Eager feed composition defeated LazyColumn virtualization

## Prior State

HomeFeedPanel was a @Composable returning a Column with items.forEach, hosted inside LazyColumn as a single item { } — so all 140+ cards composed eagerly as one giant lazy item, giving zero virtualization benefit and causing 72% janky frames with 100–379 frame skips.

## Trigger

On-device gfx profiling on a clean emulator confirmed 72.56% janky frames, 220 slow draw commands, and repeated 'Skipped N frames' in logcat — ruling out emulator exhaustion as cause.

## Decision

Rewrote HomeFeedPanel from a @Composable Column into a LazyListScope extension function (homeFeedItems), emitting each card as its own keyed lazy item (items(feed.items, key = { it.stableId })), plus separate keyed items for header/error/loading/empty states. Updated MainScaffold HighlightsTab to call the extension directly instead of wrapping in a single item {}.

## Consequences

- Janky frames dropped from ~72% to 48%, worst frame skip from 100–379 to 36 (~10× improvement)
- Scrolling and navigation became responsive; multi-second freezes eliminated
- Residual 48% jank attributed to per-card image-decode (polish layer), not eager composition
- RoomDetailPanel and RoomExplorerPanel were already lazy — no change needed there

## Open Tail

- Remaining image-decode jank (48% → lower) would require per-card image-load tuning and lighter composition

## Evidence

- transcript lines 1787-1845
- transcript lines 1856-1930

