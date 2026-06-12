---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: active
subjects:
  - android-event-bridge
  - nmp-architecture
  - relay-diagnostics
supersedes: []
related_claims: []
source_lines:
  - 2494-2498
captured_at: 2026-06-12T10:50:18Z
---

# Episode: Android EventBridge root cause: setCoreEventCallback was never called

## Prior State

Android never called setCoreEventCallback, so SignerConnected callbacks and relay status deltas were silently dropped — the app appeared to 'never connect' despite the Rust core being functional

## Trigger

Discovery during Android rebuild that no event callback registration existed, mirroring the iOS registerEventBridge() pattern that was present

## Decision

Added Kotlin EventBridge class that registers with the Rust core before Bootstrap dispatch, mirroring iOS's registerEventBridge() call sequence

## Consequences

- All core event callbacks (signer connection, relay status deltas) now reach Android
- This was a durable architectural omission — any new event type the core emits will now flow to Android automatically
- Session credential persistence hooks (onPersistSessionCredential/onClearSessionCredentials) are routed through the same bridge

## Open Tail

*(none)*

## Evidence

- transcript lines 2494-2498
