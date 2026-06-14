---
type: episode-card
date: 2026-06-14
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: architecture
status: active
subjects:
  - coil-image-loading
  - remote-image
  - avatar-image
  - highlighter-application
supersedes:
  - 2026-06-14-3-image-loading-shared-coil-loader-with
related_claims: []
source_lines:
  - 3370-3534
captured_at: 2026-06-14T09:19:44Z
---

# Episode: Adopt shared Coil ImageLoader with sized decodes; preserve crossfade and image quality

## Prior State

Each image request was independent with no shared cache, no sized decodes (full-resolution bitmaps decoded regardless of slot size), crossfade configured per-request, and no memory/disk cache — causing redundant re-decoding of avatars/covers on every scroll and recomposition.

## Trigger

48% janky frames measured during feed scrolling; earlier 379-frame worst-case freezes (since fixed by LazyColumn). A tuning pass attempted to cut jank further by removing crossfade and switching to RGB_565, but the measurement was unreliable (4950ms GPU times = emulator swiftshader, not the app).

## Decision

Adopt a shared SingletonImageLoader.Factory (MemoryCache 25% of heap, DiskCache 100MB, allowHardware(true), BitmapFactoryMaxParallelism(8)) with sized decode requests for all fixed-size cover/avatar slots (44/48/56dp). RESTORE crossfade (moved to ImageLoader level) and REVERT RGB_565 — no UX/quality regression for unmeasurable gains on an emulator GPU.

## Consequences

- Sized decode requests (.size(px)) for 11 fixed-size call sites; variable-size images (banners, podcast art) remain unbounded
- Shared memory cache prevents re-decoding same bitmap on scroll/recomposition
- Crossfade preserved at ImageLoader level (removed from per-request sites)
- ARGB_8888 quality preserved (RGB_565 reverted)
- Emulator jank measurements are unreliable for fine-grained tuning due to swiftshader GPU; real-device measurement needed for further jank work
- Image loading is now centralized through HighlighterApplication (AndroidManifest android:name='.HighlighterApplication')

## Open Tail

- Residual jank (image decode during fast fling) needs a real device to measure honestly

## Evidence

- transcript lines 3370-3534

