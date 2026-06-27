---
title: Article Reader
slug: article-reader
topic: article-reader
summary: Article reading progress is calculated as a scroll-position fraction (contentOffset / scrollableHeight, clamped [0,1])
tags:
  - capture
volatility: warm
confidence: medium
created: 2026-06-26
updated: 2026-06-27
verified: 2026-06-26
compiled-from: conversation
sources:
  - session:9ae03596-fa74-4208-88c6-a90bd3b176e4
---

# Article Reader

## Reading Progress

Article reading progress is calculated as a scroll-position fraction (contentOffset / scrollableHeight, clamped [0,1]). This is tracked via onScrollGeometryChange and implemented in ReadingProgress.fraction(contentOffsetY, contentHeight, viewportHeight). The progress is displayed as a thin (3pt) animated progress bar overlay at the top of the article scroll view, colored `Color.highlighterAccent`, appearing only when content is rendered, with transitions smoothed by `.animation(.linear(0.08s))`.

<!-- citations: [^9ae03-e5ca1] [^9ae03-038fc] [^9ae03-9d1e8] -->
## Article Text Selection

Article text selection is extracted via the pure function ArticleReaderSelection.project(fullText:selectedRange:), which returns a trimmed quote, surrounding paragraph context, and a hasQuote flag, enabling unit testing of the paragraph-scan logic in isolation. The function is covered by 5 unit tests.

<!-- citations: [^9ae03-c122d] [^9ae03-e33ce] -->
## Article Footnotes

Article footnotes are recovered from the content_tree CommonMark by scanning for `<!-- [^id] -->: body` definition paragraphs. They are integrated into ContentTreeBodyRenderer and rendered as a footnote block with superscript reference links and ↩ backlinks. <!-- [^9ae03-4cf86] -->
