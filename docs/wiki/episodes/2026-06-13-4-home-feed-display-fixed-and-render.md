---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: superseded
subjects:
  - home-feed
  - feed-display
supersedes:
  - 2026-06-13-3-empty-feed-is-data-starvation-not
related_claims: []
source_lines:
  - 186-208
  - 469-478
  - 602-621
captured_at: 2026-06-13T12:09:15Z
---

# Episode: Home feed display fixed and render path verified correct

## Prior State

HomeFeedPanel silently capped items at 8 with take(8) and had no loading-vs-empty distinction, making a syncing feed look broken. Feed emptiness was assumed to be a Compose rendering/mapping bug requiring rendering fixes

## Trigger

Audit identified take(8) truncation and missing loading state. Subsequent Opus line-by-line diagnosis verified the render path, enum/field names, state plumbing, and bridges are byte-for-byte correct — emptiness is data starvation (no logged-in account), not a Compose bug

## Decision

Removed take(8) cap; added distinct 'Syncing highlights…' loading state separate from 'No highlights yet'. No further rendering fixes needed. Real parity gap identified: Android cards don't dispatch hydration actions that iOS fires (requestProfile/requestWebMetadata/requestIsbnPreview/article)

## Consequences

- Full feed visible when data exists; loading and empty states are distinguishable
- Feed investigation redirected from 'fix Compose rendering' to 'ensure data flow + add card hydration dispatches'
- Cards will render bare quote/title immediately but lack resolved author/cover/title until hydration dispatches are added

## Open Tail

- Card hydration dispatches need to be added for iOS parity (requestProfile/requestWebMetadata/requestIsbnPreview/article)
- Logged-in validation still needed to confirm feed populates with real data

## Evidence

- transcript lines 186-208
- transcript lines 469-478
- transcript lines 602-621

