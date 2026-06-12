---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: product
status: active
subjects:
  - share-links
  - deep-link-routing
  - ios
  - android
supersedes: []
related_claims: []
source_lines:
  - 3491-3690
captured_at: 2026-06-12T11:35:05Z
---

# Episode: iOS never consumed its own share links — Android became first, then iOS caught up

## Prior State

iOS minted share links via `highlightShareUrl` but its `App.swift` `onOpenURL` handler only processed nip46 signer callbacks and share-extension handoff. No platform could open a bech32 highlight/profile link and navigate to the correct screen.

## Trigger

The Android agent, implementing deep links, discovered that `HighlighterNmpApp` exposes `decodeNostrEntity` and `resolveNostrEntity` but there was no `OpenHighlight(eventId)` action — the actual routing path is `decode→OpenComments("e", eventId, kind)`. This revealed that iOS was generating links it couldn't consume.

## Decision

Android added two intent-filters (verified App Link for `https://beta.highlighter.com/highlight/` + custom `highlighter://` scheme) and a `HighlighterViewModel.openHighlightDeepLink` that decodes bech32 and dispatches to the correct screen. iOS got a new `ShareLinkRouter.swift` that calls `nmpApp.decodeNostrEntity` and dispatches `OpenComments`/`OpenProfile` accordingly, wired into `App.swift` `onOpenURL` and universal links.

## Consequences

- Android is the first platform to route share links end-to-end (note1, nevent1, nprofile1 all decoded)
- iOS now has the same routing capability via ShareLinkRouter
- naddr deep links remain a TODO on both platforms (no `OpenArticleReader` dispatch mapping yet)
- Android App Links require server-side `assetlinks.json` on beta.highlighter.com to activate autoVerify

## Open Tail

- naddr deep-link routing awaits an `OpenArticleReader` or equivalent dispatch target in the core

## Evidence

- transcript lines 3491-3690
