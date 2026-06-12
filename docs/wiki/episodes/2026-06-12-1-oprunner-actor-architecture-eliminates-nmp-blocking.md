---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: architecture
status: active
subjects:
  - nmp-actor-oprunner
  - nmp-app-rs
  - actor-blocking-fix
supersedes:
  - 2026-06-12-1-oprunner-pattern-adopted-to-eliminate-actor
related_claims: []
source_lines:
  - 44-52
  - 124-133
  - 135-136
  - 4786-4790
  - 4841-4862
  - 5022-5060
  - 5156-5165
  - 5602-5673
  - 5674-5893
captured_at: 2026-06-12T18:07:35Z
---

# Episode: OpRunner actor architecture eliminates NMP blocking

## Prior State

The NMP actor thread could block on network I/O (inline .await / block_on for relay operations, sign-in, publishes), causing UI stalls. No generation-based supersession, no bounded deadlines, no per-domain busy flags, no cancellation on logout. Timeout fallbacks used a generic op_timed_out() helper that produced generation=0 outcomes with empty payloads — stale resolutions could fire persist side-effects and busy flags would never clear on timeout.

## Trigger

Session goal to professionalize the app; root-cause analysis identified actor-blocking as the core liveness problem. A rewritten auth-supersession test exposed that generation=0 timeout outcomes let superseded nsec sign-ins fire credential-persist callbacks, confirming a systemic defect across all 46 submit_op sites.

## Decision

Adopt OpRunner architecture: all network work dispatched off-actor via submit_op with per-domain generation counters, bounded deadlines (30s network, 6s relay-probe, 5s relay-import), and abort-on-cancel. Actor thread only does Class-D local work via block_on_local. Timeout outcomes carry live generations and real payloads (not placeholder gen-0). Per-domain busy flags set+emitted before submit and cleared in apply_op_outcome. handle_core_delta converted to sync fn, making the non-blocking invariant structural. CI lint gate (lint-actor-blocking.sh) prevents regressions.

## Consequences

- All 46 production submit_op sites migrated to inline timeout outcomes; generic op_timed_out() deleted
- Auth supersession verified: last-wins semantics with stale-resolution drop (test auth_supersession_nsec_then_bunker_drops_stale_resolution)
- Logout cancels all in-flight ops via ops.cancel_all() with generation bumps
- Adversarial review verdict: SHIP, zero blockers; two should-fix UX gaps (JoinRoom and CurationWrite had no busy flags — fixed post-review)
- Dead InFlightOp.started field and uninformative 'core' trace tag cleaned up
- Aborted-mid-wait NMP ops now prune stale waiters via Drop guard
- Design doc status updated to Implemented; zero TODO/FIXME/HACK introduced

## Open Tail

- nsec login spawn_blocking side-effects cannot be cancelled by abort (acknowledged pre-existing, documented in supersession test)
- clear_network_action_error does not clear is_saving on success path (latent — currently masked by refresh_network_settings)
- lint-actor-blocking.sh is line-based substring matching; multi-line runtime.block_on( would slip past
- CommentInteraction and ArticleBookmarkToggle submit without busy flags — defensible as optimistic-UI but not enumerated in design §4.3

## Evidence

- transcript lines 44-52
- transcript lines 124-133
- transcript lines 135-136
- transcript lines 4786-4790
- transcript lines 4841-4862
- transcript lines 5022-5060
- transcript lines 5156-5165
- transcript lines 5602-5673
- transcript lines 5674-5893
