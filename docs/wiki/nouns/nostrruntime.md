---
type: noun-entry
slug: nostrruntime
name: "NostrRuntime"
origin: extracted
source_refs:
  - transcript:1645-1649
---

# NostrRuntime

legacy Rust module providing direct nostrdb/nostr-sdk client access for read and write; vestigial for product reads (all already replaced by NMP-backed kernel domains); still instantiated at app startup solely for `data_dir()` (feeds OnboardingStore/PodcastPositionStore) and `install_diagnostics_callback()`
