---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: active
subjects:
  - home-feed-cards
  - room-tiles
  - card-hydration
supersedes:
  - 2026-06-13-1-feed-emptiness-was-a-data-gap
  - 2026-06-13-5-room-tiles-abbreviated-hex-fallback-and
related_claims: []
source_lines:
  - 866-963
  - 982-1015
captured_at: 2026-06-13T13:20:33Z
---

# Episode: Per-card hydration dispatches for iOS visual parity

## Prior State

Android feed cards displayed bare data — raw quote text, no author avatar/name, no cover art, no ISBN/web previews. Room tiles showed full 64-char hex IDs. iOS resolves all of this via per-card .task(id:) dispatches.

## Trigger

Visual comparison of Android vs iOS screenshots revealed a systemic hydration gap across feed cards and room tiles.

## Decision

Implement per-card LaunchedEffect dispatches matching iOS .task(id:) semantics: RequestProfile (per author pubkey), RequestIsbnPreview (per ISBN), RequestWebMetadata (per URL), wired through CompositionLocal providers. Room tiles switched from AvatarImage to RemoteImage with CoverShape, fallback changed from full hex to abbreviated 8-char + ellipsis, member counts shown as subtitles.

## Consequences

- Feed cards now render resolved author avatars, names, cover images, and source labels
- LocalIsbnPreviews composition local added alongside LocalProfiles and LocalWebMetadata
- Dedupe guards required on all LaunchedEffects to prevent redundant dispatches (checks absence from local cache before firing)
- Initial burst of hydration resolutions creates recomposition pressure (addressed by separate coalescing fix)
- Room tiles still show abbreviated hex for unnamed rooms (minor residual gap)

## Open Tail

- A few author bylines may briefly show hex until their profile resolves (timing-dependent)

## Evidence

- transcript lines 866-963
- transcript lines 982-1015

