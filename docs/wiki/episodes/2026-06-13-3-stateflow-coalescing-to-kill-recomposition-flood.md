---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: architecture
status: active
subjects:
  - state-emission
  - recomposition-jank
  - anr
supersedes:
  - 2026-06-13-2-state-emission-coalescing-to-kill-recomposition
related_claims: []
source_lines:
  - 1304-1328
  - 1438-1494
captured_at: 2026-06-13T13:20:33Z
---

# Episode: StateFlow coalescing to kill recomposition flood / ANR

## Prior State

The core actor emits uncoalesced full HighlighterAppState snapshots on every resolved op via onState → StateFlow, causing whole-tree recompositions on every card hydration resolution. Per-card LaunchedEffects also dispatched without checking whether data was already cached, amplifying the burst. This produced 200–322 skipped frames and ANRs.

## Trigger

Diagnosis confirmed block_on_local warnings were on the actor thread (not UI). The real ANR cause was emit frequency: each resolved op triggers a full snapshot, and N simultaneous resolutions → N recompositions in one frame.

## Decision

Apply .sample(16.milliseconds).stateIn(viewModelScope, SharingStarted.Eagerly, _state.value) to the state StateFlow, bounding recomposition to ~1 per frame. Add dedupe guards on all six hydration LaunchedEffect sites checking absence from LocalProfiles/LocalIsbnPreviews/LocalWebMetadata before dispatching. Do NOT wrap onState in Dispatchers.Main (mutations already serialized through the actor).

## Consequences

- Jank reduced from 200–322 skipped frames to ~21% janky frames
- Residual jank is one-time cold-start cost (class loading + first composition) and per-frame GPU/image-decode cost, not recomposition frequency
- The optional next lever — core-side emit_hz rate-limiter at nmp_app.rs:11093 — remains available but deliberately not implemented
- Core threading invariant confirmed: dispatch is safe from any thread (try_send into bounded channel), host must NOT serialize calls itself

## Open Tail

- Core-side emit_hz rate-limiter for tighter frame budgets if needed later
- Startup jank (108 frames) and GPU-heavy frames remain outside this fix's scope

## Evidence

- transcript lines 1304-1328
- transcript lines 1438-1494

