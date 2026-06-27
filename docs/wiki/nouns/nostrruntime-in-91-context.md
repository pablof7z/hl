---
type: noun-entry
slug: nostrruntime-in-91-context
name: "NostrRuntime (in #91 context)"
origin: extracted
source_refs:
  - transcript:1645-1653
---

# NostrRuntime (in #91 context)

legacy Rust struct for writes only; every function taking &NostrRuntime is a write op (publish/set/upsert) with zero product callers; survives only for non-read jobs (data_dir, diagnostics callback)
