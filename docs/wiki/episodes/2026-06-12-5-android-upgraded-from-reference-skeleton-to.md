---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: reversal
status: active
subjects:
  - android
  - app-maturity
  - platform-parity
supersedes: []
related_claims: []
source_lines:
  - 58-133
  - 135-140
  - 3699-3733
captured_at: 2026-06-12T14:10:16Z
---

# Episode: Android Upgraded from Reference Skeleton to Production App

## Prior State

Android was a single 3,317-line MainActivity.kt reference implementation described as a 'skeleton' — feature-mapped through the shared Rust core but with no structural separation, no session persistence, no CI, no on-device verification, and multiple missing feature surfaces (no podcast player, no edit profile, no curation, no deep links, no brand icon).

## Trigger

Explicit directive to 'professionalize, productize, improve the Android app as a real app' — assessment confirmed Android built but was not production-viable.

## Decision

Full rebuild into a multi-file production app: proper package structure (ui/auth, ui/profile, ui/bookmarks, ui/rooms, ui/player, etc.), all 16 iOS feature areas implemented through NMP, session persistence (survives force-stop), deep links, edit profile, curation menu, Media3 podcast player, relay management UI, dark mode, brand launcher icon, CI workflow, unit tests.

## Consequences

- Android is now a real app requiring ongoing maintenance rather than a disposable reference
- Feature parity with iOS through shared HighlighterNmpApp facade is now genuine
- Single-file reference implementation is historical — all future Android work targets the new structure

## Open Tail

- Multi-ABI builds not yet configured (only arm64-v8a)
- Android strings still hardcoded (not localized)
- Waveform/transcript views remain iOS-only

## Evidence

- transcript lines 58-133
- transcript lines 135-140
- transcript lines 3699-3733
