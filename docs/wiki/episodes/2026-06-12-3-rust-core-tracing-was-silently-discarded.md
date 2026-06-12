---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: architecture
status: active
subjects:
  - rust-core-logging
  - observability
  - android-logcat
supersedes: []
related_claims: []
source_lines:
  - 2289-2300
  - 2347-2360
  - 2445-2457
captured_at: 2026-06-12T09:08:45Z
---

# Episode: Rust core tracing was silently discarded on both platforms — platform logging now wired

## Prior State

The Rust core used the `tracing` crate extensively (warnings for NIP-46 failures, relay errors, etc.) but no tracing subscriber was ever installed on either platform. All diagnostic output was silently dropped — zero observability in production or debugging.

## Trigger

While diagnosing the Android relay connection issue, logcat showed zero Rust-level output despite the core running and making network connections. The gap was obvious: no logger bridge existed.

## Decision

Added a new `logging.rs` module to highlighter-core exposing `init_platform_logging()`. On Android this routes `tracing` events to logcat via android_logger; on iOS to stderr/Xcode console via os_log. The iOS side is wired into HighlighterStore.init(); the Android rebuild agent was told to wire it into Application.onCreate().

## Consequences

- All future Rust core warnings/errors are visible in platform-native log viewers
- Android debugging no longer requires network-level packet inspection to see if relays connected
- The `tracing` crate is now a reliable observability channel, not dead code

## Open Tail

- Android wiring depends on the rebuild agent completing

## Evidence

- transcript lines 2289-2300
- transcript lines 2347-2360
- transcript lines 2445-2457
