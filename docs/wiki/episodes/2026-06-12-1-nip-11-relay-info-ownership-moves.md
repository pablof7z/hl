---
type: episode-card
date: 2026-06-12
session: f54b4a16-dacb-41e6-b32f-b737d606254f
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/f54b4a16-dacb-41e6-b32f-b737d606254f.jsonl
salience: architecture
status: active
subjects:
  - nip11-in-nmp
  - relay-info-ownership
  - relay-diagnostics-surface
supersedes:
  - 2026-06-12-2-nip-11-relay-info-ownership-migrates
related_claims: []
source_lines:
  - 1-4
  - 366-372
  - 374-374
  - 474-481
  - 489-501
  - 799-805
captured_at: 2026-06-12T20:37:49Z
---

# Episode: NIP-11 relay info ownership moves from app layer to platform

## Prior State

NMP explicitly deferred NIP-11 support to 'phases C/D' in its pool code. Highlighter did its own HTTP probing and JSON parsing in relay_polish.rs, with a ProbeNetworkRelayNip11 action dispatched through the app store. This app-layer probe was broken by an unkeyed OpDomain::RelayProbe slot — concurrent probes for different relays aborted each other, so only one relay's NIP-11 ever resolved per render cycle.

## Trigger

User directive (line 374): 'deploy an opus agent to fix it properly so that highlighter doesn't need to do any heavy lifting — highlighter shouldn't have to do any requests itself or parsing or awareness of what nip-11 even is.' Preceded by diagnosis (lines 366-372) that the existing app-layer approach had a supersession bug and was architecturally wrong: apps should not own protocol fetch logic.

## Decision

NIP-11 relay information is now first-class in NMP. A new nmp-nip11 protocol crate fetches on relay connect (RelayConnectedHook substrate seam), caches with a 5-minute per-URL TTL, and surfaces name/icon/capabilities through the existing relay_diagnostics projection that apps already consume. An on-demand probe API exists for the add-relay preview flow. Apps get relay info with zero HTTP, JSON, or NIP-11 awareness. ADR-0051 documents this.

## Consequences

- Highlighter will delete relay_polish.rs and the ProbeNetworkRelayNip11 action plumbing, mapping from the diagnostics info field instead.
- The intermediate OpDomain keying fix (RelayProbe keyed by URL hash) will also be removed since the app-side probe disappears entirely.
- Both iOS and Android receive relay info for free through the shared FFI/diagnostics projection — no platform-specific work needed.
- New RelayInfoDoc type is gated out of the codegen-schema JSON schema (schemars(skip)) because iOS consumes it through the diagnostics projection, not the flat KernelTypes.generated.swift mirror.
- The nmp-nip11 crate is registered in the release manifest as a public crate; post-merge file-size baseline refresh needed per repository precedent.

## Open Tail

- Highlighter integration not yet done: delete relay_polish.rs, remove ProbeNetworkRelayNip11, map diagnostics info into the existing network.nip11 projection, regenerate FFI bindings.
- relay.highlighter.com has no icon in its NIP-11 document — the monogram fallback will continue for that relay.

## Evidence

- transcript lines 1-4
- transcript lines 366-372
- transcript lines 374-374
- transcript lines 474-481
- transcript lines 489-501
- transcript lines 799-805

