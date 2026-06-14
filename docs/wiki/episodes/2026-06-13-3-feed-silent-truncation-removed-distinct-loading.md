---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - home-feed-display
  - feed-loading-ux
supersedes:
  - 2026-06-13-4-home-feed-emptiness-root-cause-render
related_claims: []
source_lines:
  - 186-208
  - 469-478
  - 615-616
captured_at: 2026-06-13T12:42:52Z
---

# Episode: Feed silent truncation removed + distinct loading state added

## Prior State

HomeFeedPanel silently capped feed.items at 8 entries via take(8) and showed a single 'Loading highlights' placeholder indistinguishable from a truly empty feed — a syncing/paged feed looked broken.

## Trigger

Gap audit identified this as the #3 critical fix (line 200). User also observed 'shows up completely empty' and pushed back on treating it as merely cosmetic.

## Decision

Removed the take(8) cap entirely. Replaced the single EmptyPanel('Loading highlights') with a distinct loading row (CircularProgressIndicator + 'Syncing highlights…' text, testTag feed_loading). The truly-empty case retains EmptyPanel('No highlights yet') — clearly distinct from the loading state.

## Consequences

- All feed items render once synced (no arbitrary cap).
- Users see an active loading indicator during relay sync instead of a blank screen.
- The loading branch does NOT swallow the populated case — the else branch fires whenever items is non-empty regardless of isLoading.
- Subsequent diagnosis confirmed the render path is correct; emptiness was caused by no follows/incomplete onboarding, not a code bug.

## Open Tail

*(none)*

## Evidence

- transcript lines 186-208
- transcript lines 469-478
- transcript lines 615-616

