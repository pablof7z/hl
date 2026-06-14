---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: superseded
subjects:
  - home-feed-empty-diagnosis
  - feed-render-path
supersedes: []
related_claims: []
source_lines:
  - 602-621
captured_at: 2026-06-13T12:01:56Z
---

# Episode: Empty feed is data starvation, not a render bug

## Prior State

Feed showing empty was assumed to be a Compose/mapping bug or the loading-state branch swallowing populated data

## Trigger

Opus root-cause investigation compared Android render path byte-for-byte against iOS: enum mapping, field names, state plumbing, and bridges all symmetric; emulator was in logged-out state with zero core activity

## Decision

Render path is correct and symmetric with iOS; empty feed is data/auth starvation (no stored credential, no relay sync), not a rendering failure. The loading state is correct and should be kept.

## Consequences

- No changes needed to VM, bridges, SessionStore, or loading/empty logic
- Real parity gap identified: Android cards don't dispatch requestProfile/requestWebMetadata/requestIsbnPreview/article hydration that iOS fires — populated cards will show bare quote without resolved author/cover
- Decisive confirmation requires login with an account that follows real highlighters (seeded account created with 115 verified kind:9802 events from 16 highlighters)

## Open Tail

- Login validation with seeded account in progress — will confirm whether populated feed renders cards
- Card hydration dispatches (requestProfile etc.) are a follow-on fix not yet implemented

## Evidence

- transcript lines 602-621

