---
type: episode-card
date: 2026-06-14
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - image-loading
  - coil-configuration
  - android-perf
supersedes:
  - 2026-06-13-3-shared-coil-imageloader-with-sized-decodes
related_claims: []
source_lines:
  - 3369-3534
captured_at: 2026-06-14T08:09:37Z
---

# Episode: Shared Coil ImageLoader with sized decodes

## Prior State

No shared ImageLoader — each AsyncImage request decoded at full resolution, no memory or disk cache, crossfade set per-request; avatar/cover bitmaps re-decoded on every scroll and recomposition

## Trigger

Residual 48% janky-frame rate after LazyColumn fix; root cause analysis showed per-card full-resolution image decoding and re-decoding on scroll as the dominant remaining cost

## Decision

Created HighlighterApplication implementing SingletonImageLoader.Factory with MemoryCache (25% heap), DiskCache (100MB), allowHardware(true), bitmapFactoryMaxParallelism(8), and crossfade(true) at the loader level. Added targetSize parameter to RemoteImage and AvatarImage for sized decodes at slot resolution (44/48/56dp); crossfade and ARGB_8888 preserved for UX quality

## Consequences

- Avatars and covers decode at slot resolution instead of full-res — roughly 4–10x fewer bytes per decode
- Shared memory cache prevents re-decoding on scroll/recomposition
- Crossfade preserved (was briefly removed, restored as UX win outweighs unmeasurable emulator-jank gain)
- RGB_565 considered and reverted — full ARGB_8888 quality kept for thumbnails
- Jank percentage on emulator GPU (swiftshader) remained unmeasurable at ~62%/4950ms GPU times, confirming emulator is not a valid jank measurement environment

## Open Tail

- Real-device jank measurement needed to quantify actual improvement

## Evidence

- transcript lines 3369-3534

