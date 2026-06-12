---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: reversal
status: active
subjects:
  - android-architecture
  - nmp-android
  - app-structure
supersedes:
  - 2026-06-12-1-android-monolith-nmp-aligned-modular-architecture
related_claims: []
source_lines:
  - 135-140
captured_at: 2026-06-12T13:29:48Z
---

# Episode: Android app rebuilt from single-file skeleton to production

## Prior State

Android was a 3,317-line single-file `MainActivity.kt` — a reference implementation with all UI in one file, no session persistence, no proper navigation, no CI, and no on-device verification.

## Trigger

User directive to 'professionalize, productize, improve the android app as a real app' — a Stop hook condition requiring the app to be production-quality.

## Decision

Complete structural rebuild: split into proper composable packages (auth, comments, rooms, bookmarks, settings, profile), added `HighlighterViewModel` exposing reconciled NMP state as `StateFlow`, session persistence via credential storage, deep links, share-intent handling, edit profile, curation menu, Media3 podcast player, relay management UI, Material theming with brand colors, and CI workflow.

## Consequences

- Android is now a structured multi-file app with feature-scoped composables instead of a monolith
- Session persistence survives force-stop (verified on-device)
- Feature parity with iOS through the shared Rust core, with structural differences (single ViewModel vs 105 Swift files)
- Only arm64-v8a ABI built; multi-ABI and device testing still needed for production

## Open Tail

- Multi-ABI builds (x86, arm64-v8a for emulators)
- Android CI pipeline (currently only iOS Xcode Cloud exists)
- Strings still hardcoded (not internationalized)
- Waveform/transcript views remain iOS-only

## Evidence

- transcript lines 135-140
