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
  - lazy-virtualization
supersedes: []
related_claims: []
source_lines:
  - 1743-1931
captured_at: 2026-06-13T16:48:47Z
---

# Episode: Feed jank root cause: eager Column defeated LazyColumn virtualization

## Prior State

Jank was attributed to network I/O blocking the main thread and/or an excessively high emit_hz rate; earlier fix (emit_hz reduction + trailing-emit rate limiter) only shrank frame-skip spikes without curing the jank. Unresponsiveness was suspected to be emulator degradation from marathon test sessions.

## Trigger

Cold-boot test on a fresh emulator confirmed navigation worked (ruling out emulator exhaustion) but jank persisted at 72% janky frames; code inspection then revealed HomeFeedPanel was hosted as a single `item { HomeFeedPanel(...) }` inside MainScaffold's LazyColumn — all ~140 cards composed eagerly inside one giant lazy item, completely defeating virtualization.

## Decision

Rewrote HomeFeedPanel from a @Composable returning a Column into a LazyListScope extension function (`homeFeedItems`) emitting each card as its own keyed lazy item (`items(feed.items, key = { it.stableId })`); updated MainScaffold's HighlightsTab to call the extension instead of wrapping it in a single `item {}`.

## Consequences

- Janky frames dropped from ~72% to 48%; worst frame skip fell from 100–379 to 36 frames (~10× improvement)
- Navigation-blocking multi-second freezes eliminated; scrolling and tab taps register promptly
- Residual 48% jank is per-card image-decode/composition cost (polish tier), not the all-at-once realization that was the dominant cause
- Eager Column/forEach rendering is now a known first-class performance anti-pattern for this codebase

## Open Tail

- Per-card jank (image decode, hydration reads on visible cards) remains at ~48% — future work: image-load tuning and lighter per-card composition

## Evidence

- transcript lines 1743-1931

