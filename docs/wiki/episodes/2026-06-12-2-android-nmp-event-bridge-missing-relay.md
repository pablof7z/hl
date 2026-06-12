---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: active
subjects:
  - android-nmp-integration
  - relay-connection
  - session-persistence
supersedes: []
related_claims: []
source_lines:
  - 1858-1870
  - 2100-2140
  - 2219-2222
captured_at: 2026-06-12T09:08:45Z
---

# Episode: Android NMP event bridge missing — relay state and login deltas silently dropped

## Prior State

Android ViewModel only wired 1 of 3 required NMP channels: it listened for state snapshots via HighlighterAppReconciler.onState() but never called setCoreEventCallback() (which iOS registers before bootstrap). Session credential callbacks (onPersistSessionCredential, onClearSessionCredentials) were empty stubs. Relay sockets opened but the app never heard about it.

## Trigger

Diagnosis during Android restructuring: network sockets showed established TLS connections (Cloudflare + Hetzner, plausibly relays), yet UI showed connection state 'Unknown' forever. Root cause traced to missing event bridge registration and empty session persistence stubs.

## Decision

Wire all three NMP channels on Android exactly as iOS does: (1) state reconciliation via onState, (2) core event callback via setCoreEventCallback before bootstrap, (3) session credential persistence via onPersistSessionCredential/onClearSessionCredentials backed by Android EncryptedSharedPreferences.

## Consequences

- Relay connection state now correctly transitions Unknown → Connecting → Online
- NIP-46 nostrconnect:// login completion now delivers the SignerConnected delta to the UI instead of silently dropping it
- Session survives app restart — kill-and-relaunch stays logged in
- All future live data deltas (bookmarks, highlights, messages) are now received on Android

## Open Tail

- Android session persistence implementation still needs on-emulator verification

## Evidence

- transcript lines 1858-1870
- transcript lines 2100-2140
- transcript lines 2219-2222
