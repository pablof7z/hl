---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: reversal
status: active
subjects:
  - android-parity-audit
  - highlighter-android
supersedes:
  - 2026-06-13-1-android-parity-reframed-surgical-fixes-not
related_claims: []
source_lines:
  - 186-209
captured_at: 2026-06-13T12:25:09Z
---

# Episode: Android parity premise reversal — not a dead shell, a few high-impact defects

## Prior State

Android app was believed to be a 'complete disaster' / non-functional shell requiring extensive rebuild

## Trigger

Full iOS-vs-Android codebase audit cataloged 38 flows and found ~26 WORKING, ~7 PARTIAL, 2 BROKEN, 3 MISSING; the 'complete disaster' experience was produced by a few layered defects, not systemic absence of functionality

## Decision

Reframe the parity problem: Android is a mostly-correct thin renderer over the shared Rust core (same HighlighterNmpApp, same ~140-action contract, same state snapshots). Parity is rendering/IA/navigation work, not new business logic. Focus narrowed to 3 surgical fixes + missing OCR.

## Consequences

- Effort redirected from rebuild to targeted surgical fixes
- Architecture doctrine: both apps are thin renderers over a single Rust state machine — parity = rendering/IA work, not new business logic
- Top-5 critical-fixes list replaced the 'everything is broken' mental model
- OCR capture identified as the only genuinely missing feature area (15 iOS files vs 1 Android stub, no CAMERA permission)

## Open Tail

- Remaining 7 PARTIAL and 3 MISSING flows beyond the top-3 basics
- OCR capture phase still unscheduled

## Evidence

- transcript lines 186-209

