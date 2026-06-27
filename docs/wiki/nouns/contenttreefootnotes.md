---
type: noun-entry
slug: contenttreefootnotes
name: "ContentTreeFootnotes"
origin: extracted
source_refs:
  - transcript:568-574
---

# ContentTreeFootnotes

Rust enum that scans a content_tree for footnote definition paragraphs (matching pattern `^\s*\[^id\]:\s*body`) and returns paired definitions plus root indices to exclude from body rendering
