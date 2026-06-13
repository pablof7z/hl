---
type: episode-card
date: 2026-06-12
session: f54b4a16-dacb-41e6-b32f-b737d606254f
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/f54b4a16-dacb-41e6-b32f-b737d606254f.jsonl
salience: architecture
status: superseded
subjects:
  - nip11-ownership
  - nmp-nip11-crate
  - relay-diagnostics-projection
  - relay-info-doc
supersedes:
  - 2026-06-12-2-nip-11-ownership-moves-from-highlighter
related_claims: []
source_lines:
  - 374-1100
captured_at: 2026-06-12T20:29:27Z
---

# Episode: NIP-11 relay info ownership migrates from app to platform

## Prior State

NMP explicitly deferred NIP-11 support to 'phases C/D' (comments in pool/types.rs); Highlighter app did its own HTTPS GET probing and JSON parsing in relay_polish.rs, requiring app-level awareness of NIP-11 HTTP mechanics.

## Trigger

User directive: 'deploy an opus agent to fix it properly so that highlighter doesn't need to do any heavy lifting — fix it on nostr-multi-platform — highlighter shouldn't have to do any requests itself or parsing or awareness of what nip-11 even is'

## Decision

NIP-11 relay info is now a first-class NMP protocol crate (nmp-nip11): fetched automatically on relay connect via a RelayConnectedHook seam, cached with 5-minute TTL, surfaced as an `info` child object on each row of the existing relay_diagnostics projection. Apps consume relay name/icon/capabilities as a field on diagnostics rows — zero HTTP, zero JSON parsing, zero NIP-11 awareness at the app layer. On-demand probe API also provided for the add-relay preview flow. ADR-0051 (renumbered from 0049 collision) documents this.

## Consequences

- Highlighter will delete relay_polish.rs and ProbeNetworkRelayNip11 action plumbing; map row.info into the existing network.nip11 projection
- iOS and Android both receive relay info through the same generated FFI core — no platform-specific work needed
- RelayInfoDoc is excluded from the flat Swift KernelTypes mirror (schemars(skip)) because iOS consumes it through the diagnostics projection's serde JSON and FlatBuffers sidecar, not the flat type mirror
- nmp-nip11 added as a public crate in the release manifest; new RelayConnectedHook substrate seam in nmp-core
- Previous 'phases C/D' deferral comments in pool/types.rs superseded by ADR-0051
- Post-merge follow-up: Highlighter integration (delete probe code, map from diagnostics info field), and owner-side file-size-baseline refresh per established precedent

## Open Tail

- Highlighter-side integration not yet done — awaiting NMP merge and release cut
- File-size baseline needs owner-side refresh post-merge (same pattern as PR #1196)

## Evidence

- transcript lines 374-1100

