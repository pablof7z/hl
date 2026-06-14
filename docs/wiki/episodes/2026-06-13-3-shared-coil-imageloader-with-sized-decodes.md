---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: architecture
status: superseded
subjects:
  - image-loading
  - coil-configuration
  - android-feed
supersedes: []
related_claims: []
source_lines:
  - 3370-3431
  - 3444-3482
captured_at: 2026-06-13T18:37:41Z
---

# Episode: Shared Coil ImageLoader with sized decodes replaces per-request loading

## Prior State

No shared ImageLoader existed; each AsyncImage request created its own decoder with no memory or disk cache. Every avatar/cover was decoded at full source resolution regardless of the rendered slot size. Crossfade(true) was set per-request, causing per-frame redraw storms during flings.

## Trigger

48% janky frames measured during feed scrolling, caused by repeated full-resolution decodes of the same images and crossfade redraws on every newly-loaded bitmap during flings.

## Decision

Introduced HighlighterApplication implementing SingletonImageLoader.Factory with: 25% memory cache (strong-reference LRU), 100MB disk cache, crossfade at loader level (removing per-request crossfade), sized decode requests for all fixed-size cover/avatar slots (40–56 dp), allowRgb565(true) for opaque thumbnails, allowHardware(true) for hardware bitmaps, and bitmapFactoryMaxParallelism(8) to prevent decode serialization during fast flings.

## Consequences

- Same avatar/cover bitmap is decoded once then served from memory cache on recomposition or scroll
- Fixed-size slots (avatars, covers, thumbnails) decode at slot resolution instead of full source resolution
- RGB_565 halves decode bytes for opaque thumbnails
- Hardware bitmaps skip the CPU→GPU upload copy
- No crossfade animation — images appear when ready, placeholders still render via surfaceVariant background

## Open Tail

- Crossfade removal and RGB_565 are UX/quality tradeoffs not yet committed — pending controlled jank measurement before deciding to ship
- Measured jank was inconclusive (48%→62% in uncontrolled comparison); agent recommends restoring crossfade if data doesn't justify removal

## Evidence

- transcript lines 3370-3431
- transcript lines 3444-3482

