---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: superseded
subjects:
  - feed-hydration
  - home-feed-panel
  - card-rendering
supersedes:
  - 2026-06-13-1-android-home-feed-empty-not-a
  - 2026-06-13-2-home-feed-cards-ios-parity-hydration
related_claims: []
source_lines:
  - 761-766
  - 881-962
  - 982-1015
captured_at: 2026-06-13T13:15:31Z
---

# Episode: Feed emptiness was a data gap, not a render bug — hydration parity added

## Prior State

The Android home feed appeared empty; the prevailing assumption was that the render code was broken.

## Trigger

Seeded test account with 16 follows and 115 highlights showed 143 real cards after completing onboarding — the render path worked fine, but cards showed bare quotes with no author avatar/name/cover art, unlike iOS.

## Decision

The feed was never broken — it lacked data (no follows) and lacked per-card hydration dispatches. Added LaunchedEffect-based RequestProfile/RequestWebMetadata/RequestIsbnPreview dispatches in HomeFeedPanel, a new LocalIsbnPreviews composition local wired in MainActivity, and previewForIsbn helper in Profiles.kt. Room tiles similarly upgraded with RemoteImage covers and memberSubtitle fallbacks instead of raw hex IDs.

## Consequences

- Feed cards now render author avatars, names, cover art, and ISBN/web previews at iOS parity (validated with screenshots)
- Room tiles show covers and member counts; unnamed rooms show truncated hex (8 chars + '…') instead of full 64-char IDs
- Per-card hydration dispatches triggered a recomposition flood (200–322 skipped frames), requiring a separate architectural fix
- Three benign lookup-failure paths surfaced as persistent 'not found' toasts, requiring a separate core-side fix

## Open Tail

- A few feed author bylines still show hex until their profile resolves (timing-dependent, minor)
- Optional core-side emit_hz rate-limiter at nmp_app.rs:11093 identified as next lever for further jank reduction

## Evidence

- transcript lines 761-766
- transcript lines 881-962
- transcript lines 982-1015

