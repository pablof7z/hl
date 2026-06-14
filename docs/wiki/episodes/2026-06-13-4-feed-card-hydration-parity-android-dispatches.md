---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - feed-card-rendering
  - hydration-dispatch
supersedes: []
related_claims: []
source_lines:
  - 604-618
  - 881-960
captured_at: 2026-06-13T12:42:52Z
---

# Episode: Feed card hydration parity — Android dispatches zero enrichment actions

## Prior State

Android feed cards dispatched none of the requestProfile / requestWebMetadata / requestIsbnPreview / article hydration actions that iOS fires from each card type. Result: feed cards showed bare quote text with no resolved author name, avatar, cover image, or article metadata — making a populated feed look unfinished vs iOS.

## Trigger

Opus diagnosis agent compared Android vs iOS render paths line-by-line and identified the missing hydration dispatches as the sole remaining visual parity gap (lines 604-618). Confirmed by validation screenshots showing bare cards with no author/cover.

## Decision

Added LaunchedEffect dispatches per card type in HomeFeedPanel.kt matching iOS .task(id:) semantics: HIGHLIGHTS cards dispatch RequestProfile(lead.pubkey), RequestIsbnPreview(isbn), RequestProfile(articleAuthorPubkey) for 30023 articles, and RequestWebMetadata(url) for web sources; READ cards dispatch RequestProfile(read.pubkey) and RequestProfile(primaryInteractor). Each fires exactly once per unique key value. Wired a new LocalIsbnPreviews CompositionLocal into MainActivity providing state.isbnPreviews from the live snapshot. Reused existing AvatarImage/RemoteImage components and Profiles helpers for rendering.

## Consequences

- Feed cards now display resolved author avatar/name, cover art, and article metadata — approaching iOS visual parity.
- New CompositionLocal LocalIsbnPreviews established as the source-of-truth for ISBN preview data in the Compose tree.
- Test tags added: feed_highlight_card, feed_reading_card, card_cover, card_author.

## Open Tail

- Visual validation of hydrated cards on the rebuilt APK is pending (rebuild + validation pass in progress at session end).

## Evidence

- transcript lines 604-618
- transcript lines 881-960

