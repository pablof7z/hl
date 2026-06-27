---
type: noun-entry
slug: bridge-shape-for-65-web-nmp-migration
name: "bridge shape (for #65 web NMP migration)"
origin: extracted
source_refs:
  - transcript:3873-3875
---

# bridge shape (for #65 web NMP migration)

WASM-in-worker architecture via nmp-browser-runtime + vendored runtime-web; main thread spawns Worker that boots the WASM runtime, drives it with FlatBuffers messages, receives UpdateFrame bytes back.
