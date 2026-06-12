---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: reversal
status: active
subjects:
  - android-mainactivity
  - android-mvvm-restructure
supersedes:
  - 2026-06-12-1-android-monolith-refactored-to-modular-nmp
related_claims: []
source_lines:
  - 56-133
  - 135-136
  - 5579-5582
captured_at: 2026-06-12T18:07:35Z
---

# Episode: Android app restructuring from single-file reference to production architecture

## Prior State

Android app was a single 3,317-line MainActivity.kt covering all 16 feature areas (auth, capture, articles, highlights, bookmarks, communities, comments, chat, profile, search, share, feedback, settings, What's New). Only arm64-v8a built, no Android CI, no on-device test verification. Described as 'complete single-file reference implementation, not a polished app.'

## Trigger

Assessment revealed structural inadequacy: parity of capability but not of maintainability; goal hook directed 'professionalize, productize, improve the android app as a real app.'

## Decision

Restructure Android app from monolithic single file into proper multi-file MVVM architecture with separate ViewModels and screens, targeting production quality. Core NMP architecture hardened (OpRunner) as prerequisite for reliable cross-platform behavior.

## Consequences

- Android debug APK builds successfully against restructured core (39 MB, arm64-v8a)
- iOS build also compiles against updated core with zero warnings
- Podcast mini-player UI still missing on Android
- No Android CI pipeline yet; no multi-ABI builds; no on-device test verification

## Open Tail

- Multi-ABI builds (x86, arm64-v8a, others) needed for emulator and broad device support
- Android CI pipeline needed
- On-device testing and verification still unconfirmed from repo alone

## Evidence

- transcript lines 56-133
- transcript lines 135-136
- transcript lines 5579-5582
