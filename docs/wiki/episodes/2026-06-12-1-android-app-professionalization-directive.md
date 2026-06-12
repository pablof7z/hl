---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: architecture
status: active
subjects:
  - android-app
  - nmp-architecture
  - app-structure
supersedes: []
related_claims: []
source_lines:
  - 60-70
  - 112-133
  - 135-139
  - 236-266
captured_at: 2026-06-12T08:00:36Z
---

# Episode: Android app professionalization directive

## Prior State

Android app existed as a single 3,317-line MainActivity.kt — described in commit history as a 'skeleton' but actually a complete feature-mapped reference implementation sharing the NMP Rust core with iOS. Not structured for production use: monolithic file, no podcast mini-player UI, arm64-v8a only, no CI, no verified on-device testing.

## Trigger

Stop hook activated with condition 'professionalize, productize, improve the android app as a real app' — an explicit directive to elevate Android from reference implementation to production-quality app.

## Decision

Begin modular restructuring of the Android app: split the monolithic MainActivity.kt into proper composable and architectural modules, address missing podcast player UI, add multi-ABI builds, establish CI, and verify on-device functionality.

## Consequences

- The entire 3,300-line MainActivity.kt must be decomposed into organized files matching iOS's 105-file structure
- Podcast playback needs a dedicated mini-player composable to match iOS UX
- Build must expand beyond arm64-v8a to support broader device coverage
- CI pipeline must be created (only iOS Xcode Cloud scripts exist currently)
- Web app remains intentionally outside NMP, duplicating Rust core logic — requires explicit architectural decision on WASM integration

## Open Tail

- File split plan and execution not yet complete
- No CI configuration written yet
- Emulator/on-device testing still pending in a parallel session
- Web app's NMP exclusion status remains an unresolved architectural question

## Evidence

- transcript lines 60-70
- transcript lines 112-133
- transcript lines 135-139
- transcript lines 236-266
