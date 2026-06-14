---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: reversal
status: superseded
subjects:
  - android-parity
  - app-assessment
supersedes:
  - 2026-06-13-1-android-is-not-a-dead-shell
related_claims: []
source_lines:
  - 186-209
captured_at: 2026-06-13T12:09:15Z
---

# Episode: Android parity reframed: surgical fixes, not a rebuild

## Prior State

Android app was characterized as a 'complete disaster' — implied broad dysfunction requiring extensive rework

## Trigger

Opus audit of both codebases cataloged 38 flows: ~26 WORKING, ~7 PARTIAL, 2 BROKEN, 3 MISSING. Both apps are thin renderers over the same Rust state machine; most iOS features already exist in the core as actions + snapshot fields

## Decision

Reframed from 'rebuild a dead shell' to 'surgical fixes on a mostly-working app' — focus on 5 high-impact defects rather than broad rewrite

## Consequences

- Approach changed to targeted bug fixes (CloseRoom onDispose, CreateRoom IA, feed truncation) plus OCR as the only genuinely missing feature
- Architecture doctrine confirmed: parity is rendering/IA/navigation work, not new business logic
- 8-phase dependency-ordered roadmap produced instead of open-ended rewrite

## Open Tail

- OCR capture (Phase 4) is the largest genuinely missing feature

## Evidence

- transcript lines 186-209

