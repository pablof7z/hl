---
type: episode-card
date: 2026-06-12
session: f54b4a16-dacb-41e6-b32f-b737d606254f
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/f54b4a16-dacb-41e6-b32f-b737d606254f.jsonl
salience: root-cause
status: superseded
subjects:
  - op-domain-keying
  - relay-probe
supersedes: []
related_claims: []
source_lines:
  - 107-372
captured_at: 2026-06-12T19:51:57Z
---

# Episode: Keyed relay probes fix supersession bug

## Prior State

OpDomain::RelayProbe was a single unkeyed slot — all relay NIP-11 probes shared one in-flight entry, so each new probe aborted the previous one. Only the last relay to be probed could ever resolve its NIP-11 document, which matched the observed symptom: only relay.highlighter.com (which has no icon) ever populated.

## Trigger

User asked why relay icons weren't showing; investigation confirmed damus, purplepag.es, and primal all serve NIP-11 icons but their probes were being aborted by the unkeyed slot.

## Decision

RelayProbe is now keyed by a stable hash of the relay URL (the same pattern CommentInteraction and ArticleBookmarkToggle already use). Different relays probe independently; re-probing the same URL still supersedes the prior in-flight request.

## Consequences

- Acceptance test rewritten from asserting 'first probe must NOT resolve' to asserting 'both relays resolve independently, including same-URL re-probe mid-flight'
- All 8 acceptance tests pass
- This fix is temporary — the follow-on architecture change moves NIP-11 fetching entirely out of Highlighter

## Open Tail

*(none)*

## Evidence

- transcript lines 107-372

