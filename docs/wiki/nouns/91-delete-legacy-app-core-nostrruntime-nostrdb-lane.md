---
type: noun-entry
slug: 91-delete-legacy-app-core-nostrruntime-nostrdb-lane
name: "#91 (Delete legacy app-core NostrRuntime/nostrdb lane)"
origin: extracted
source_refs:
  - transcript:1626-1668
---

# #91 (Delete legacy app-core NostrRuntime/nostrdb lane)

fully actionable (not upstream-NMP-blocked); requires removal of dead nostrdb read+publish functions across 5 feature modules (highlights, artifacts, profile, feedback, relays) and decoupling `HighlighterCore` from `NostrRuntime` instantiation while preserving its FFI surface for iOS onboarding/diagnostics
