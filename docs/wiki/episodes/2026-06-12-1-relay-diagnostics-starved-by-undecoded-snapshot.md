---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: superseded
subjects:
  - relay-diagnostics
  - nmp-snapshot
  - cross-platform
supersedes:
  - 2026-06-12-1-relay-diagnostics-never-reached-native-platforms
related_claims: []
source_lines:
  - 3479-3482
captured_at: 2026-06-12T13:29:48Z
---

# Episode: Relay diagnostics starved by undecoded snapshot frames

## Prior State

Relay connection status never updated on either iOS or Android; settings UI showed no live status for relays.

## Trigger

While building the Android relay management UI, discovered the NMP kernel's built-in relay-diagnostics projection only travels inside emitted snapshot frames, which highlighter-core never decoded — the data existed but was silently dropped.

## Decision

Wired `nmp_app_set_update_callback` → frame decode → diagnostics state, so the reconciler receives and surfaces relay status updates.

## Consequences

- Both platforms now show relay Online/Offline status dots in Settings → Network
- The same undecoded-frame pattern may apply to other kernel projections — worth auditing all snapshot fields for similar starvation

## Open Tail

- Audit other NMP snapshot fields for silent data loss

## Evidence

- transcript lines 3479-3482
