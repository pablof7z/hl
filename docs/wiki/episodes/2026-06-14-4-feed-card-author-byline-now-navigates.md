---
type: episode-card
date: 2026-06-14
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: active
subjects:
  - home-feed-panel
  - navigation-dispatch
  - card-author-byline
supersedes:
  - 2026-06-14-5-feed-card-author-byline-dispatches-openprofile
related_claims: []
source_lines:
  - 3825-3895
captured_at: 2026-06-14T09:19:44Z
---

# Episode: Feed card author-byline now navigates to profile instead of falling through to article reader

## Prior State

Tapping a feed card's author byline (card_author Row) fell through to the card's own click handler and opened the article reader/detail screen. The byline had a testTag but no independent clickable action, so taps propagated to the parent.

## Trigger

Maestro validation flows 27-profile and 28-follow both failed: tapping card_author opened article_reader instead of profile_screen. UIAutomator dump confirmed article_reader present with no profile_screen node. iOS navigates to the profile from the author byline.

## Decision

Added independent .clickable { dispatch(HighlighterAppAction.OpenProfile(pubkey)) } to the card_author Row in both HighlightFeedCard (lead.pubkey) and ReadingFeedCard (read.pubkey). The child Row's clickable consumes the tap event per Compose propagation semantics, so the parent card's reader action no longer fires on byline taps.

## Consequences

- Author byline taps now navigate to the correct profile (iOS parity)
- Follow button renders and works on non-own-profile views (flow 28-follow passes)
- Rest of card remains independently clickable for article/room navigation
- Validated via Maestro flows 27-profile and 28-follow (both PASS)

## Open Tail

*(none)*

## Evidence

- transcript lines 3825-3895

