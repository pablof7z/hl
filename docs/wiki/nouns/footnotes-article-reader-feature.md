---
type: noun-entry
slug: footnotes-article-reader-feature
name: "footnotes (article reader feature)"
origin: extracted
source_refs:
  - transcript:509-514
---

# footnotes (article reader feature)

Broken via live path — ContentTreeBodyRenderer hard-returns empty footnotes (NSAttributedString()) and empty anchors dict; content_tree has no footnote node kind, so old MarkdownRenderer/FootnotePreprocessor path never runs
