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
  - ios-navigation
supersedes: []
related_claims: []
source_lines:
  - 3540-3689
captured_at: 2026-06-12T14:10:16Z
---

# Episode: iOS Could Not Open Its Own Share Links

## Prior State

iOS minted nevent/nprofile/naddr share links but onOpenURL only handled nip46 + share-extension handoff — the app generated share URLs it could not route when tapped.

## Trigger

During Android deep-link implementation, discovered iOS had no routing for the share link types it created. Android now routes them; iOS was the parity gap.

## Decision

Created ShareLinkRouter.swift using decodeNostrEntity/resolveNostrEntity from the Rust core, added universal-link entitlement and onOpenURL handling for nevent/nprofile/naddr URLs.

## Consequences

- iOS can now open its own share links for events, profiles, and articles
- Deep-link parity achieved between iOS and Android

## Open Tail

- naddr deep links still have a TODO dispatch point (no single route in core)
- App Links activation requires server-side assetlinks.json on beta.highlighter.com

## Evidence

- transcript lines 3540-3689
