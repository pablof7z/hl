---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: active
subjects:
  - android-relay-connection
  - nmp-android
  - android-logging
supersedes: []
related_claims: []
source_lines:
  - 1854-1860
  - 2022-2050
captured_at: 2026-06-12T08:57:33Z
---

# Episode: Android relay connection failure: Rust core never initializes Android logger

## Prior State

Android app should connect to Nostr relays via the NMP runtime and show online/syncing state

## Trigger

User reported 'it doesn't even connect to relays'; investigation found TLS connections to plausible relay IPs exist but the app shows 'Process system not responding' and no Rust log output reaches logcat

## Decision

Diagnosed root cause: the Rust core never initializes an Android logger, so all diagnostic output is silently dropped. The connection failure itself is still under investigation — sockets exist but the UI doesn't reflect connected state.

## Consequences

- Android app is non-functional for real use without relay connectivity
- No observability into Rust core behavior on Android
- The NMP reconciler callback chain may also have issues (state not flowing to UI)

## Open Tail

- Must wire Android logcat bridge for Rust core
- Must diagnose why reconciler state doesn't reflect connected status in UI
- The restructuring of the single-column UI into proper navigation must also fix the auth/bootstrap flow

## Evidence

- transcript lines 1854-1860
- transcript lines 2022-2050
