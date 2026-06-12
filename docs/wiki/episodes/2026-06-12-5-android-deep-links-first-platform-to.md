---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: product
status: active
subjects:
  - android-deep-links
  - share-links
  - app-links
supersedes: []
related_claims: []
source_lines:
  - 3490-3538
captured_at: 2026-06-12T13:54:23Z
---

# Episode: Android deep links — first platform to consume share links end-to-end

## Prior State

Android had no deep link handling; shared highlighter URLs could only open in a browser, not the app

## Trigger

Professionalization directive to make Android a real app

## Decision

Added two manifest intent-filters (verified autoVerify App Link for https://beta.highlighter.com/highlight/ plus highlighter:// highlight/{token} custom-scheme fallback); MainActivity parses ACTION_VIEW in onCreate and onNewIntent, extracts bech32 token via MutableStateFlow → LaunchedEffect; ViewModel calls decodeNostrEntity then dispatches OpenComments/OpenProfile

## Consequences

- Android is the first shell to consume share links end-to-end
- Malformed nevents are cleanly rejected by the core with no crash
- Custom scheme fallback works without server-side verification

## Open Tail

- assetlinks.json deployment needed on beta.highlighter.com for autoVerify
- naddr deep links TODO (no single dispatch target in core)

## Evidence

- transcript lines 3490-3538
