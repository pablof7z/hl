---
type: episode-card
date: 2026-06-12
session: f54b4a16-dacb-41e6-b32f-b737d606254f
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/f54b4a16-dacb-41e6-b32f-b737d606254f.jsonl
salience: architecture
status: superseded
subjects:
  - nip-11-ownership
  - nmp-platform
  - relay-polish
supersedes:
  - 2026-06-12-1-relayprobe-unkeyed-slot-caused-mutual-abort
related_claims: []
source_lines:
  - 374-464
captured_at: 2026-06-12T18:57:48Z
---

# Episode: NIP-11 ownership shifts from Highlighter app to NMP platform layer

## Prior State

Highlighter's core crate (relay_polish.rs) performed its own one-shot HTTPS GET to fetch and parse NIP-11 documents. The app layer was fully responsible for probing, parsing, and caching NIP-11 data. NMP's pool code explicitly deferred NIP-11 capability handling to later phases.

## Trigger

User directive: 'deploy an opus agent to fix it properly so that highlighter doesn't need to do any heavy lifting — fix it on ~/Work/nostr-multi-platform — highlighter shouldn't have to do any requests itself or parsing or awareness of what nip-11 even is.'

## Decision

NMP (the platform library) will own NIP-11 fetching, parsing, and caching. Highlighter's relay_polish.rs probe code becomes historical. iOS and Android apps should receive relay icon/metadata through NMP's existing relay state pipeline without any app-level awareness of NIP-11.

## Consequences

- Highlighter's probe_nip11 and http_url_for_nip11 in relay_polish.rs will be removed once NMP provides the data
- NMP must expose relay NIP-11 metadata (icon, name) through its existing FFI/observable pipeline so both iOS and Android get it for free
- The keyed RelayProbe fix in Highlighter is an interim patch; the final architecture eliminates app-layer probing entirely

## Open Tail

- NMP implementation of NIP-11 capability fetch/cache not yet started — an Opus agent is to be deployed on the nostr-multi-platform repo
- Need to determine NMP release/contribution workflow (sibling path dependency vs git rev) before Highlighter can consume the new version

## Evidence

- transcript lines 374-464

