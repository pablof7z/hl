---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: reversal
status: active
subjects:
  - ios-share-links
  - deep-links
  - onopenurl
supersedes: []
related_claims: []
source_lines:
  - 3540-3689
captured_at: 2026-06-12T13:54:23Z
---

# Episode: iOS share links minted but never consumed

## Prior State

iOS App.swift onOpenURL only handled NIP-46 signer callbacks and share-extension handoff; links minted by highlight_share_url (nevent/nprofile/naddr) could not be opened back into the app

## Trigger

Android deep-link implementation revealed that iOS had no inbound routing for the share links it was already generating

## Decision

Added ShareLinkRouter.swift decoding nevent→OpenComments, nprofile→OpenProfile, naddr (TODO); wired into onOpenURL + universal-links entitlement in project.yml

## Consequences

- Android is no longer the only platform that can consume its own share links
- naddr deep links remain a TODO on both platforms (no single dispatch target in core)
- iOS build+tests green with the new router

## Open Tail

- naddr routing needs a core dispatch action
- AASA file deployment on beta.highlighter.com

## Evidence

- transcript lines 3540-3689
