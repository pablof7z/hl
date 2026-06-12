---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: product
status: active
subjects:
  - deep-links
  - share-links
  - android
  - ios
  - bech32-routing
supersedes: []
related_claims: []
source_lines:
  - 3491-3686
captured_at: 2026-06-12T11:17:28Z
---

# Episode: Share link routing — Android first platform to consume bech32 links end-to-end

## Prior State

Neither iOS nor Android routed inbound share links (https://beta.highlighter.com/highlight/{token} or highlighter:// scheme) to actual content views. iOS's App.swift onOpenURL only handled nip46 signer callbacks and share-extension handoff — the share links the app itself mints were never consumed by any platform.

## Trigger

Android professionalization required deep link support; investigation of the core API revealed `decodeNostrEntity` and `OpenComments`/`OpenProfile` actions existed but no platform shell consumed them. This also exposed the iOS gap.

## Decision

Android added manifest intent-filters for verified App Links and custom-scheme fallbacks, plus ViewModel routing via decodeNostrEntity → OpenComments(«e», eventId, kind) for events and OpenProfile for profiles. iOS gained a new ShareLinkRouter.swift with the same decode-and-dispatch logic, wired into App.swift's onOpenURL and universal-links handler.

## Consequences

- Android is the first platform with end-to-end share link consumption (verified live on emulator)
- iOS now has parity for share link routing
- naddr (parameterised replaceable event) deep links are still a TODO — no single dispatch target exists in the core yet
- App Links autoVerify requires server-side assetlinks.json on beta.highlighter.com before Android verified links activate

## Open Tail

- Host /.well-known/assetlinks.json on beta.highlighter.com for Android autoVerify
- Add OpenArticleReader mapping for naddr once the share surface mints addressable links

## Evidence

- transcript lines 3491-3686
