---
type: noun-entry
slug: bridge-mechanism
name: "Bridge mechanism"
origin: extracted
source_refs:
  - transcript:1638-1644
---

# Bridge mechanism

Proven and in-tree pattern: `KernelEventObserver` + `nmp_ref.register_live_event_tap` / `register_typed_snapshot_projection`, used by kernel domains (reactions, discussions, bookmark_sets, follows) to connect app state to NMP.
