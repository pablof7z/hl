---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: product
status: active
subjects:
  - android-session-persistence
  - authentication
supersedes: []
related_claims: []
source_lines:
  - 2478-2498
captured_at: 2026-06-12T10:50:18Z
---

# Episode: Encrypted session persistence on Android

## Prior State

Android had no session persistence — every cold launch required full re-login

## Trigger

Professionalization directive included a mandatory kill-and-relaunch acceptance test; force-stop/relaunch would lose the user's session

## Decision

Implemented SessionStore.kt using EncryptedSharedPreferences with corrupt-keystore self-heal, persisting HighlighterSessionCredential (Nsec/BunkerUri). Bootstrap restores stored credential via SignInNsec(persist=false, clearStoredOnFailure=true) or PairBunker, mirroring iOS dispatchStoredCredential

## Consequences

- Force-stop/relaunch acceptance test passed — app goes straight to logged-in Highlights tab
- Corrupt keystore scenario is self-healing rather than fatal
- Fresh UI logins use persist=true, stored-credential restores use persist=false with failure cleanup

## Open Tail

*(none)*

## Evidence

- transcript lines 2478-2498
