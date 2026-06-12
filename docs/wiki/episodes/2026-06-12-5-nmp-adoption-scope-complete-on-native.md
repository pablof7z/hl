---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: architecture
status: active
subjects:
  - nmp-app-facade
  - web-architecture
supersedes: []
related_claims: []
source_lines:
  - 124-133
captured_at: 2026-06-12T08:31:54Z
---

# Episode: NMP adoption scope: complete on native, web deliberately outside

## Prior State

Uncertainty about whether NMP (Native Model Pattern — Rust app facade, fire-and-forget actions, reconciled state snapshots) was fully adopted across all surfaces.

## Trigger

User question: 'is nmp already fully adopted throughout?'

## Decision

Assessment confirmed: NMP is 100% adopted where it applies — Rust core owns all business logic, iOS completed migration (deleted 7 feature-scoped stores), Android was built NMP-native. The web app (SvelteKit, uses NDK directly) is intentionally outside NMP.

## Consequences

- Web app duplicates logic that the Rust core owns for native platforms
- Architecture boundary needs an explicit decision: keep web on NDK, or bring it into NMP via WASM build of the core
- iOS transient stores (PodcastPlayerStore for AVPlayer, CaptureStore for local OCR) are the only Swift-side state, and legitimately don't belong in Rust

## Open Tail

- Web-on-NMP strategy remains an open decision (WASM vs. current NDK approach)

## Evidence

- transcript lines 124-133
