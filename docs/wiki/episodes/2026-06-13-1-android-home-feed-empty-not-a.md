---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: superseded
subjects:
  - android-home-feed
  - render-path-diagnosis
supersedes: []
related_claims: []
source_lines:
  - 602-631
  - 715-757
captured_at: 2026-06-13T12:58:25Z
---

# Episode: Android home feed empty — not a render bug, data starvation

## Prior State

Android home feed appeared empty; assumed to be a Compose/mapping bug in the render path

## Trigger

Opus diagnosis agent compared Android render path byte-for-byte against iOS — mapping, enum/field names, state plumbing, and NMP bridges all correct; emulator had zero stored credentials and zero core logs, confirming no authenticated session

## Decision

The feed render path is correct; emptiness is caused by data/auth starvation (no logged-in account, no follows, onboarding not completed), not a Compose or binding bug. No render-path code changes needed for population.

## Consequences

- Seeded test account with 16 followed highlighters and 115 confirmed kind:9802 events immediately produced 143 visible highlight cards
- Eliminated mapping/enum/list-composition as root causes; future feed-empty reports should first check auth/sync state
- Loading state ('Syncing highlights…') retained — it is correct and does not mask the populated case

## Open Tail

- Frame-skip jank observed during refresh (block_on_local budget warnings) — not ANR but performance concern

## Evidence

- transcript lines 602-631
- transcript lines 715-757

