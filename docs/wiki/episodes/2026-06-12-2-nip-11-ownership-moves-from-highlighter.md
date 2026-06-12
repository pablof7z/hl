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
  - relay-diagnostics-surface
supersedes:
  - 2026-06-12-2-nip-11-ownership-shifts-from-highlighter
related_claims: []
source_lines:
  - 374-892
captured_at: 2026-06-12T19:51:57Z
---

# Episode: NIP-11 ownership moves from Highlighter to NMP

## Prior State

Highlighter core owned NIP-11 fetching directly via relay_polish.rs (one-shot HTTPS GET, manual JSON parsing into Nip11Document). NMP's pool code explicitly deferred 'NIP-11 capability map' to later phases. Apps had to know what NIP-11 was, issue their own HTTP requests, and handle parsing.

## Trigger

User directive: 'deploy an opus agent to fix it properly so that highlighter doesn't need to do any heavy lifting — fix it on ~/Work/nostr-multi-platform — make it work easily on the highlighter apps (ios and android should get it without any extra work) — highlighter shouldn't have to do any requests itself or parsing or awareness of what nip-11 even is.'

## Decision

NIP-11 is now first-class in NMP via a new nmp-nip11 protocol crate. Relay info fetches automatically on connect (RelayConnectedHook seam with 5-minute per-URL TTL gate), lands as an `info` object on each row of the existing `relay_diagnostics` projection. On-demand probe API also provided for add-relay preview flows. Highlighter will delete relay_polish.rs and ProbeNetworkRelayNip11 action plumbing, mapping from the diagnostics `info` field instead.

## Consequences

- New crate nmp-nip11 (bounded ureq GET off-thread, 64 KiB cap, 10s timeout) mirrors nmp-nip57's LNURL fetcher pattern
- ADR-0049 written; supersedes old 'phases C/D' deferral comments in pool/types.rs and relay-lifecycle docs
- RelayConnectedHook substrate seam added — generic fan-on-connect mechanism reusable by future protocol crates
- RelayDiagnosticsRow gains `info: Option<RelayDiagnosticsInfo>` with name, description, icon, pubkey, contact, software, version, supported_nips, and auth/payment flags
- C-ABI probe (nmp_app_probe_relay_info) follows the borrowed-during-callback model conformant with v0.6.0 free-string retirement
- PR #1195 opened, rebased onto v0.6.0 master, merge-ready; iOS and Android both get relay info through the same generated core with zero extra integration work
- Highlighter follow-up: delete probe_nip11/Nip11Document, map from diagnostics info field, regenerate FFI bindings

## Open Tail

- Swift codegen CI check failing — RelayInfoDoc missing JsonSchema derive under codegen-schema feature; agent is fixing
- File-size CI check flaked (base ref unavailable after force-push); re-push will retrigger
- Merge and release cut pending CI green; then Highlighter integration (delete relay_polish.rs, remap diagnostics `info` into existing `network.nip11` projection)

## Evidence

- transcript lines 374-892

