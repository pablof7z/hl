---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: reversal
status: superseded
subjects:
  - android-parity-audit
  - app-state-assessment
supersedes: []
related_claims: []
source_lines:
  - 186-209
captured_at: 2026-06-13T12:01:56Z
---

# Episode: Android is not a dead shell — premise corrected

## Prior State

Android client was believed to be a non-functional shell requiring massive rebuilding; user described it as a 'complete disaster'

## Trigger

Opus audit cataloged 38 iOS flows and mapped them against Android: ~26 WORKING, ~7 PARTIAL, 2 BROKEN, 3 MISSING

## Decision

Reclassified Android as a mostly-working app with a few high-impact defects layered on correct core wiring (HighlighterViewModel: listenForUpdates + setCoreEventCallback armed pre-login, StateFlow dispatch intact)

## Consequences

- Approach shifted from 'rebuild' to 'surgical fixes' targeting the top 3 broken/partial items
- Both apps confirmed as thin renderers over the same Rust state machine (~140-action contract); parity is rendering/IA/navigation work, not new business logic
- 8-phase dependency-ordered roadmap produced instead of a ground-up rewrite

## Open Tail

- OCR capture (Phase 4) remains genuinely MISSING — 15 iOS files, 1 Android stub, no CAMERA permission

## Evidence

- transcript lines 186-209

