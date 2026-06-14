---
type: episode-card
date: 2026-06-14
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - image-loading
  - coil-cache
  - android-perf
supersedes:
  - 2026-06-14-4-shared-coil-imageloader-with-sized-decodes
related_claims: []
source_lines:
  - 3369-3534
captured_at: 2026-06-14T08:39:07Z
---

# Episode: Image loading: shared Coil loader with sized decodes

## Prior State

No shared Coil ImageLoader — each AsyncImage request decoded at full resolution regardless of display slot size, no memory or disk cache, crossfade set per-request causing per-frame redraw storms during flings

## Trigger

48% janky frames measured during feed scrolling; investigation showed full-res decodes and crossfade redraws as main cost sources; subsequent attempt to remove crossfade and add RGB_565 measured 62% janky but was deemed an emulator artifact (swiftshader GPU showing 4950ms 90th-percentile GPU times)

## Decision

Introduced HighlighterApplication implementing SingletonImageLoader.Factory with 25% memory cache, 100MB disk cache, hardware bitmaps enabled, parallel decode ceiling of 8; added targetSize/.size(px) at 11 fixed-size avatar/cover call sites; kept crossfade at ImageLoader level (restored after initial removal); reverted RGB_565 (kept ARGB_8888 for quality)

## Consequences

- Each thumbnail decoded at its actual rendered slot size (44–56dp) instead of full resolution
- Shared memory/disk cache prevents re-decoding the same image on scroll or recomposition
- Crossfade preserved as a UX feature — removed per-request, set globally at ImageLoader level
- Emulator jank percentage unreliable for A/B measurement (software GPU); real-device testing needed for residual jank assessment

## Open Tail

- Residual jank percentage needs real-device measurement to validate
- 2 pull-to-refresh gestures still present (may conflict with event-driven design principle)

## Evidence

- transcript lines 3369-3534

