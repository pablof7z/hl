---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: architecture
status: active
subjects:
  - nmp-architecture
  - web-app
  - native-platforms
supersedes: []
related_claims: []
source_lines:
  - 124-133
captured_at: 2026-06-12T08:19:17Z
---

# Episode: NMP Fully Adopted on Native Platforms; Web Intentionally Excluded

## Prior State

Uncertainty whether NMP (Native Model Pattern) was fully adopted throughout the codebase

## Trigger

User question: 'is nmp already fully adopted throughout?'

## Decision

NMP is at 100% adoption where it applies: Rust core owns all business logic and state, iOS dispatches via nmpApp.dispatch, Android built NMP-native from start. The web app (SvelteKit) uses NDK directly and has no NMP integration — this is a deliberate architectural boundary, not unfinished migration

## Consequences

- Only transient device-local state (PodcastPlayerStore for AVPlayer, CaptureStore for local OCR) remains in Swift-side stores
- Web app duplicates logic that the Rust core already owns, creating a consistency risk
- Decision needed on whether to bring web into NMP via a WASM build of the core

## Open Tail

- Should the web app eventually consume the Rust core via WASM, or remain independently implemented?
- Is the web's NDK-direct approach acceptable as a permanent boundary?

## Evidence

- transcript lines 124-133
