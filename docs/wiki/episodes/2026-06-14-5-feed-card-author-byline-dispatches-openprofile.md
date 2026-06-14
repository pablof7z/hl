---
type: episode-card
date: 2026-06-14
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - home-feed-panel
  - navigation
  - android-parity
supersedes: []
related_claims: []
source_lines:
  - 3814-3895
captured_at: 2026-06-14T08:39:07Z
---

# Episode: Feed card author byline dispatches OpenProfile instead of parent action

## Prior State

Tapping an author byline on a feed highlight/reading card fell through to the parent card's click handler and dispatched OpenArticleReader — the card_author Row had a testTag but no independent click handler

## Trigger

Maestro validation flows 27 (profile) and 28 (follow) revealed that tapping card_author opens the article reader instead of the profile screen; iOS navigates to profile from author byline

## Decision

Added .clickable { dispatch(HighlighterAppAction.OpenProfile(pubkey)) } to the card_author Row on both HighlightFeedCard and ReadingFeedCard; Compose click propagation means the child clickable consumes the tap, so the parent card's click still works for its own action

## Consequences

- Tapping an author byline now navigates to that user's profile (iOS parity)
- Parent card tap is unaffected — the child clickable intercepts the tap event
- Profile and follow Maestro flows now pass

## Open Tail

*(none)*

## Evidence

- transcript lines 3814-3895

