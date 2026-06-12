---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: root-cause
status: active
subjects:
  - platform-logging
  - observability
  - cross-platform
supersedes: []
related_claims: []
source_lines:
  - 2445-2459
  - 2497-2498
captured_at: 2026-06-12T10:50:18Z
---

# Episode: Platform logging root cause: Rust tracing output dropped on both platforms

## Prior State

The Rust core's tracing output was silently discarded on both Android and iOS because no subscriber was ever installed — core events like relay config application were invisible

## Trigger

Discovered during session observability work that logcat showed no core output despite successful initialization

## Decision

Added initPlatformLogging() UniFFI export that routes tracing to logcat on Android (tag 'highlighter-core') and stderr/Xcode console on iOS, wired as first call in both HighlighterStore.init (iOS) and HighlighterViewModel init (Android)

## Consequences

- Core diagnostic messages (relay config, connection events) now visible on both platforms
- Future debugging of core behavior is significantly easier
- This was another cross-platform gap fixed for both surfaces simultaneously

## Open Tail

*(none)*

## Evidence

- transcript lines 2445-2459
- transcript lines 2497-2498
