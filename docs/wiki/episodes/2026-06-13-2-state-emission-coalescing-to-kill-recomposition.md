---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: architecture
status: superseded
subjects:
  - state-flow
  - recomposition
  - highlighter-view-model
supersedes: []
related_claims: []
source_lines:
  - 1304-1328
  - 1438-1494
captured_at: 2026-06-13T13:15:31Z
---

# Episode: State emission coalescing to kill recomposition flood

## Prior State

The core emitted a full HighlighterAppState clone on every resolved op via onState, uncoalesced. The Android ViewModel exposed this as a raw StateFlow, causing whole-tree recompositions per emit.

## Trigger

After adding per-card hydration LaunchedEffects, validation showed 200–322 skipped frames and an ANR. Diagnosis confirmed: dispatch does not block UI (it's try_send into a bounded channel on the actor thread), but the burst of full-snapshot emits + per-card LaunchedEffects that don't check absence caused a recomposition flood.

## Decision

Two-pronged fix: (1a) Changed HighlighterViewModel.state from direct asStateFlow() to a lazy property with .sample(16.milliseconds).stateIn(viewModelScope, SharingStarted.Eagerly, _state.value), coalescing bursts to ~1 recomposition per frame; (1b) Guarded each hydration LaunchedEffect on absence from local caches (profiles.profileFor(pubkey) == null, etc.) before dispatching.

## Consequences

- Jank improved from severe (200–322 skipped frames, ANR) to ~21% janky frames; residual is cold-start class-loading + per-frame GPU/image-decode cost, not recomposition frequency
- The fix is on the exact flow MainActivity collects (viewModel.state), confirmed active
- Core threading constraint preserved: mutations serialized through single actor thread, dispatch safe from any thread

## Open Tail

- Startup 108-frame skip is one-time cold-start cost outside sample's scope
- 99th-percentile GPU spike (~300ms) is per-frame render cost, not emit frequency — cannot be addressed by state coalescing
- Core-side emit_hz rate-limiter at nmp_app.rs:11093 is the next lever if tighter jank reduction is needed

## Evidence

- transcript lines 1304-1328
- transcript lines 1438-1494

