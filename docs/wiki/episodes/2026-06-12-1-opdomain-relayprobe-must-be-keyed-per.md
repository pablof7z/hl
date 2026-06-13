---
type: episode-card
date: 2026-06-12
session: f54b4a16-dacb-41e6-b32f-b737d606254f
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/f54b4a16-dacb-41e6-b32f-b737d606254f.jsonl
salience: root-cause
status: active
subjects:
  - oprunner-relay-probe-keying
  - relay-nip11-fetch
supersedes:
  - 2026-06-12-1-keyed-relay-probes-fix-supersession-bug
related_claims: []
source_lines:
  - 1-370
captured_at: 2026-06-12T20:29:27Z
---

# Episode: OpDomain::RelayProbe must be keyed per-URL to avoid mutual cancellation

## Prior State

OpDomain::RelayProbe was an unkeyed singleton slot in OpRunner — any new probe submission aborted the previous in-flight probe, so when the relay list probed every row on appear, only the last relay ever resolved its NIP-11 document.

## Trigger

User screenshot showed only relay.highlighter.com with a name; root-cause analysis revealed the OpRunner domain-abort semantics killed all but one probe.

## Decision

Key RelayProbe by a stable hash of the relay URL (matching the established CommentInteraction/ArticleBookmarkToggle pattern), so different relays probe independently while re-probing the same URL still supersedes.

## Consequences

- All relays now resolve their NIP-11 info concurrently instead of mutually cancelling
- Same-URL re-probe still supersedes via generation bump
- Acceptance test rewritten to assert independent resolution of two relays plus same-URL mid-flight re-probe
- This fix is an interim measure — Highlighter-side relay probing will be deleted once NMP owns NIP-11

## Open Tail

*(none)*

## Evidence

- transcript lines 1-370

