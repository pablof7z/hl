---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: product
status: active
subjects:
  - android-app
  - nmp-android
supersedes: []
related_claims: []
source_lines:
  - 110-134
  - 4447-4466
captured_at: 2026-06-12T16:31:21Z
---

# Episode: Android app rebuilt from single-file reference to production quality

## Prior State

Android was a 3,300-line single MainActivity.kt reference implementation with no navigation structure, no session persistence, no CI, no multi-ABI builds, and no on-device verification — described as a "skeleton" despite covering all 16 feature areas through the shared Rust core.

## Trigger

Stop hook directive: "professionalize, productize, improve the android app as a real app"; on-device testing confirmed the app built but revealed multiple functional gaps.

## Decision

Full rebuild into production-quality Jetpack Compose app: welcome → login/create-account → onboarding → 3 tabs + settings/profile/capture/podcast destinations; session persistence (survives force-stop, verified on-device); event bridge; Coil images; ACTION_SEND share-in; deep links; comment/chat avatars; edit profile; curation menu; Media3 podcast player with mini-player; relay management UI; dark mode; brand launcher icon; CI workflow; unit tests.

## Consequences

- Android now feature-complete with iOS through the shared HighlighterNmpApp Rust facade
- Session persistence verified on-device (force-stop recovery)
- CI workflow added (.github/workflows/android.yml)
- Still needs structural file split, multi-ABI builds, and strings internationalization
- Waveform/transcript views remain iOS-only

## Open Tail

- Android strings still hardcoded (deferred to avoid conflicting with parallel work)
- App Links/universal links need assetlinks.json/AASA deployed server-side
- Beta NIP-05 API returns 404 (server-side route missing)

## Evidence

- transcript lines 110-134
- transcript lines 4447-4466
