---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: reversal
status: active
subjects:
  - android-app
  - feature-parity
  - nmp-adoption
supersedes: []
related_claims: []
source_lines:
  - 110-133
  - 135-137
  - 4447-4465
captured_at: 2026-06-12T15:08:36Z
---

# Episode: Android promoted from single-file reference implementation to production app

## Prior State

Android was a 3,300-line single-file MainActivity.kt described as a 'skeleton' (commit 8f317ba) later expanded (commit 60282b9). It had feature coverage but no structural polish — no multi-file architecture, no session persistence, no CI, no brand identity, no podcast player UI, only arm64-v8a builds.

## Trigger

User asked whether Android was working and at feature parity with iOS. Assessment revealed it was a 'complete single-file reference' — capability parity without production quality. A session-scoped stop hook set the condition: 'professionalize, productize, improve the android app as a real app.'

## Decision

Full restructuring: multi-file architecture (RootScene, destinations, shared components), session persistence (survives force-stop, verified on-device), Coil image loading throughout, ACTION_SEND share-in, deep links, comment/chat avatars, edit profile, curation menu, Media3 podcast player with mini-player, relay management UI, dark mode, brand launcher icon, CI workflow, unit tests.

## Consequences

- Android is now a real multi-file Compose app, not a reference sketch
- Session persistence works (force-stop/relaunch verified)
- CI workflow exists (.github/workflows for core lint + tests)
- Deep architectural issue found during testing: actor blocking bug (separate arc)
- Remaining gaps: waveform/transcript views (iOS-only), Android strings still hardcoded, multi-ABI builds not configured, App Links need assetlinks.json deployment

## Open Tail

- App Links / universal links need server-side assetlinks.json
- Multi-ABI builds not yet configured
- Waveform and transcript views remain iOS-only

## Evidence

- transcript lines 110-133
- transcript lines 135-137
- transcript lines 4447-4465
