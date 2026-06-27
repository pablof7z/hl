---
type: noun-entry
slug: nmp-backed-kernel-domains-vs-legacy-nostrdb-lane
name: "NMP-backed kernel domains (vs legacy nostrdb lane)"
origin: extracted
source_refs:
  - transcript:1633-1643
---

# NMP-backed kernel domains (vs legacy nostrdb lane)

set of Rust domains (`highlight_feed.rs`, `articles_feed.rs`, `profiles.rs`, `feedback.rs`, `relays.rs`, `relay_diagnostics.rs`) that serve all live product reads via `KernelEventObserver` + NMP's typed projections and mailbox cache; the authority superseding hl's direct nostrdb queries
