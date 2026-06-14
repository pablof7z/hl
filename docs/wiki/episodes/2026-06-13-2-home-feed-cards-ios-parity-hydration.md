---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - android-home-feed
  - card-hydration
  - feed-truncation
supersedes:
  - 2026-06-13-3-feed-silent-truncation-removed-distinct-loading
  - 2026-06-13-4-feed-card-hydration-parity-android-dispatches
related_claims: []
source_lines:
  - 469-478
  - 881-961
captured_at: 2026-06-13T12:58:25Z
---

# Episode: Home feed cards — iOS parity hydration and uncapped display

## Prior State

Android feed cards showed bare quote text with no author avatar/name, cover art, or metadata; the feed was also silently capped at 8 items via .take(8) and had no distinct loading state

## Trigger

Diagnosis identified Android dispatches none of the requestProfile/requestWebMetadata/requestIsbnPreview/article actions that iOS fires per card; the .take(8) cap and missing loading indicator were found in the parity audit

## Decision

Removed .take(8) cap; added a distinct 'Syncing highlights…' loading state; added LaunchedEffect-driven hydration dispatches per card (requestProfile, requestWebMetadata, requestIsbnPreview, article) keyed on unique identifiers; wired state.profiles/webMetadata/isbnPreviews through CompositionLocalProviders; reused existing AvatarImage/RemoteImage components

## Consequences

- Feed cards now render author avatars, display names, cover images, and source metadata matching iOS
- All feed items display (no silent truncation)
- Loading state visually distinct from empty state
- New CompositionLocal LocalIsbnPreviews added alongside existing LocalProfiles and LocalWebMetadata

## Open Tail

- Some author bylines briefly show hex until profile resolves over the network (timing, not a bug)

## Evidence

- transcript lines 469-478
- transcript lines 881-961

