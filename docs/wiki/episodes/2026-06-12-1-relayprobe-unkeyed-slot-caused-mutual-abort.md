---
type: episode-card
date: 2026-06-12
session: f54b4a16-dacb-41e6-b32f-b737d606254f
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/f54b4a16-dacb-41e6-b32f-b737d606254f.jsonl
salience: root-cause
status: superseded
subjects:
  - op-domain-relay-probe
  - nip-11-icon-display
supersedes: []
related_claims: []
source_lines:
  - 1-370
captured_at: 2026-06-12T18:57:48Z
---

# Episode: RelayProbe unkeyed slot caused mutual abort of concurrent NIP-11 probes

## Prior State

OpDomain::RelayProbe was an unkeyed enum variant, so all relay NIP-11 probes shared a single in-flight slot. Submitting a new probe aborted the previous one, meaning only the last-relay-probed could ever resolve. The UI showed at most one relay's icon/name.

## Trigger

User screenshot showed only relay.highlighter.com resolving its NIP-11 name while other relays (damus, purplepag.es, primal) — which all have icons — displayed only monogram fallbacks. Curl verification confirmed the NIP-11 documents contain icons.

## Decision

RelayProbe is now keyed by a stable hash of the relay URL (OpDomain::RelayProbe { url_key: u64 }), matching the existing pattern used by CommentInteraction and ArticleBookmarkToggle. Different relays probe independently; re-probing the same URL still supersedes.

## Consequences

- All relays in the list can resolve their NIP-11 data concurrently
- The acceptance test was rewritten from asserting 'first probe must not resolve' to asserting 'both relays resolve independently, including same-URL re-probe mid-flight'
- Relay icons from NIP-11 will now appear for all relays that publish them

## Open Tail

*(none)*

## Evidence

- transcript lines 1-370

