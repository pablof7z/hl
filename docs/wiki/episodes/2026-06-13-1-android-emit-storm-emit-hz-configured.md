---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: superseded
subjects:
  - android-emit-storm
  - nmp-app-emit-hz
  - viewmodel-onstate
supersedes: []
related_claims: []
source_lines:
  - 1619-1674
captured_at: 2026-06-13T16:12:24Z
---

# Episode: Android emit storm — emit_hz configured but never implemented + onState OS calls on every emit

## Prior State

emit_hz=30 was configured in HighlighterViewModel and passed to spawn_actor, presumed to rate-limit state emissions to 30Hz. onState() called syncNetworkCallback (registerNetworkCallback/unregisterNetworkCallback) unconditionally on every state emission regardless of whether wifiOnlyEnabled changed.

## Trigger

Jank investigation revealed the actor loop called emit() unconditionally on every OpResolved — emit_hz was only referenced in a dead tracing::debug on actor exit. onState made expensive OS calls hundreds of times per second from the actor thread during initial feed load bursts of N concurrent op resolutions.

## Decision

Implemented recv_timeout-based rate limiter with trailing-emit guarantee in nmp_app.rs (op_emit_interval = 1000ms/emit_hz.clamp(1,120), dirty flag + timeout flush). Added change guard on wifiOnlyEnabled in HighlighterViewModel.onState so syncNetworkCallback only fires when the value actually changes.

## Consequences

- Reduced state emission frequency from unbounded to ≤30Hz with trailing-emit correctness guarantee
- Eliminated hundreds of spurious OS network-callback registrations per second
- Combined with LazyColumn fix, reduced worst frame skip from 379 to 36
- Residual jank (48%) is per-card image-decode cost, not emit frequency

## Open Tail

- Optional core-side emit_hz rate-limiter at nmp_app.rs:11093 was explicitly deferred as lower-priority — available lever if further coalescing is wanted
- Memoized joinedRoomIds (derivedStateOf) and Log.isLoggable guard also added as minor perf wins in same pass

## Evidence

- transcript lines 1619-1674

