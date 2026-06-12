---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: reversal
status: active
subjects:
  - android-app
  - productization
supersedes: []
related_claims: []
source_lines:
  - 58-70
  - 110-133
  - 135-137
  - 662-676
captured_at: 2026-06-12T08:19:17Z
---

# Episode: Android: Reference Skeleton → Production Track

## Prior State

Android app was committed as a 'skeleton' (8f317ba) — a single 3,317-line MainActivity.kt with no CI, no proper icons, no release build, no back navigation, and no structural separation

## Trigger

Session-scoped stop hook with condition: 'professionalize, productize, improve the android app as a real app' (line 135), plus assessment confirming Android was feature-complete through the Rust core but structurally a reference implementation

## Decision

Android app is upgraded to production track: modularized into 23 files, given proper adaptive icons, dark theme support, Android system back-navigation wired to NMP Close actions, release build with ProGuard, CI via GitHub Actions, and gradle performance tuning

## Consequences

- Android app now has architectural parity with iOS's 105-file Swift structure
- CI pipeline will catch regressions on push
- Release APK shrinks from 37MB debug to 29MB release
- System back button now dispatches correct NMP Close actions rather than being unhandled
- Ongoing work remains: podcast mini-player UI, multi-ABI builds, device verification

## Open Tail

- Podcast playback lacks dedicated mini-player UI compared to iOS
- Only arm64-v8a ABI built; x86_64 and others missing
- No on-device testing confirmed yet

## Evidence

- transcript lines 58-70
- transcript lines 110-133
- transcript lines 135-137
- transcript lines 662-676
