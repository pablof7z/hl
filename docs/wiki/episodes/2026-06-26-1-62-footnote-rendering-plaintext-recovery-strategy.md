---
type: episode-card
date: 2026-06-26
session: 9ae03596-fa74-4208-88c6-a90bd3b176e4
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/9ae03596-fa74-4208-88c6-a90bd3b176e4.jsonl
salience: root-cause
status: active
subjects:
  - footnotes
  - article-renderer
  - content-tree-renderer
supersedes: []
related_claims: []
source_lines:
  - 502-516
  - 562-597
captured_at: 2026-06-26T11:53:52Z
---

# Episode: #62 footnote rendering: plaintext recovery strategy identified

## Prior State

Issue #62 framed footnotes as unported features from the bespoke renderer. Implicit assumption: footnote support required porting from or implementing NMP ContentView support.

## Trigger

Planner investigated actual code state of main (HEAD 7014eece) and discovered issue description was based on an outdated commit; analyzed the actual rendering architecture in the current codebase.

## Decision

Identified that ContentTreeBodyRenderer hard-returns empty footnotes because content_tree (CommonMark) has no footnote node type, but plaintext references `[^id]` and definition lines survive as literal text in paragraphs. Adopted plaintext-recovery strategy: scan paragraph text in ContentTreeBodyRenderer, extract definitions, reuse existing MarkdownRenderer.renderFootnotes machinery to emit the block, without NMP changes.

## Consequences

- Footnote support can be restored entirely in the iOS shell without upstream NMP API changes or timeline dependencies
- Reuses existing MarkdownRenderer footnote machinery (Output structure, footnoteAnchors, ArticleBodyView tap routing), minimizing new code surface
- Scope confined to hl iOS Features/Article/ files; no core or NMP modifications required
- Implementation becomes unblocked from NMP rendering pipeline dependencies
- Issue #62 can be fully resolved in parallel with #63 in this session

## Open Tail

- Scroll-to-footnote navigation (jumping from reference to definition location) deferred as optional refinement; footnote visibility and tappability land in #62, scroll optimization left for follow-up

## Evidence

- transcript lines 502-516
- transcript lines 562-597

## Conversation

- Cleaned transcript (verbatim user words, abbreviated agent replies): [`transcripts/2026-06-26-1-62-footnote-rendering-plaintext-recovery-strategy.json`](transcripts/2026-06-26-1-62-footnote-rendering-plaintext-recovery-strategy.json)
- Raw transcript (verbatim user words, full agent replies): [`transcripts/raw/2026-06-26-1-62-footnote-rendering-plaintext-recovery-strategy.json`](transcripts/raw/2026-06-26-1-62-footnote-rendering-plaintext-recovery-strategy.json)
