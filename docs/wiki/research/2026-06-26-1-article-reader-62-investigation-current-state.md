---
type: research-record
date: 2026-06-26
session: 9ae03596-fa74-4208-88c6-a90bd3b176e4
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/9ae03596-fa74-4208-88c6-a90bd3b176e4.jsonl
source_lines: 497-643
agent_attribution: ab194b7564f5844ac
has_preregistered_criteria: true
has_method: true
has_structured_report: true
characterization: "Article Reader #62 investigation: current-state evaluation vs. issue claims with findings and risk assessment"
captured_at: 2026-06-26T11:52:04Z
---

Article Reader #62 investigation: current-state evaluation vs. issue claims with findings and risk assessment

---

User: [Agent task result: Agent "Plan #62 article reader features" finished]
I have everything I need. Here is the plan.

---

# Implementation Plan — Issue #62: Article body renderer (selection, footnotes, reading progress)

## 0. Critical correction to the issue's framing (verified against `main`)

The issue assumes "the article reader now uses `NostrContentView` for body rendering" and that selection/footnotes/overlay were lost. That describes commit `26c39c09` ("cut article body to NMP ContentTreeWire + NostrContentView, gate #1695"), which **deleted `ArticleBodyView.swift`**. That commit is **NOT an ancestor of current `main`** (`git merge-base --is-ancestor 26c39c09 HEAD` → false). 

What `main` (HEAD `7014eece`) actually ships is the **hybrid** from `b4234881` / `4f61dd4b` (#22): the body renders from the kernel `content_tree` via `ContentTreeBodyRenderer`, which flattens prose into `NSAttributedString` segments that are displayed in the bespoke `ArticleBodyView` (a `UITextView`). So the real state of the three features is:

| Feature | Actual state on `main` | Gap |
|---|---|---|
| **Text selection** | **Works.** `ArticleBodyView` (`UITextView`, `isSelectable`) with a custom Edit Menu ("Highlight" / "Highlight with note") → `onPublishHighlight` / `onRequestNote`. | None functional. Selection-projection logic is untested + un-extracted. |
| **Footnotes** | **Broken via the live path.** `ArticleBodyView` still has all the tap/anchor/back-link machinery, and `ReaderScroll.withFootnotes(...)` still appends a footnote block — but `ContentTreeBodyRenderer.render(...)` hard-returns `footnotes: NSAttributedString()` / `footnoteAnchors: [:]` (see its lines 110-120). The `content_tree` has no footnote node kind, and `ArticleRecord(kernelSnapshot:).content = ""`, so the old `MarkdownRenderer`/`FootnotePreprocessor` markdown path no longer runs for the body. Net: footnotes never render. | Recover footnotes from `content_tree` text in the hl shell. |
| **Reading progress overlay** | **Fully absent.** No scroll-offset tracking anywhere in `ArticleReaderView`. The `ScrollAnchor` state is set on footnote taps but **never consumed** (no `ScrollViewReader`/`scrollTo`). | Build it. |

This is good news: text selection needs only test coverage; footnotes need an extraction pass; reading progress is the one genuinely new surface. The work stays entirely in the hl iOS shell (`Features/Article/`) — no NMP edits (NMP `NostrWireNode` has no footnote concept and must not be changed).

Key files (all absolute):
- `/Users/pablofernandez/Work/hl/app/ios/Highlighter/Sources/Highlighter/Features/Article/ContentTreeBodyRenderer.swift` — the live body renderer (footnote gap is here).
- `/Users/pablofernandez/Work/hl/app/ios/Highlighter/Sources/Highlighter/Features/Article/ArticleReaderView.swift` — `ReaderScroll` (ScrollView, `withFootnotes`, `bodySegments`), the place for the progress overlay + scroll wiring.
- `/Users/pablofernandez/Work/hl/app/ios/Highlighter/Sources/Highlighter/Features/Article/ArticleBodyView.swift` — `UITextView` wrapper; `Coordinator.selectionText` (selection projection) + footnote tap routing.
- `/Users/pablofernandez/Work/hl/app/ios/Highlighter/Sources/Highlighter/Features/Article/MarkdownRenderer.swift` — owns `Output`, the footnote attribute keys, and `renderFootnotes` (currently `private`).
- `/Users/pablofernandez/Work/hl/app/ios/Highlighter/Sources/Highlighter/Features/Article/FootnotePreprocessor.swift` — `Definition` type + GFM footnote parsing (reusable).

## 1. Build & test commands (verified)

- Project is **xcodegen-driven** (`app/ios/Highlighter/project.yml` is the source of truth; **no** `.xcworkspace`, no Podfile despite the stale `app/AGENTS.md` text). Scheme: **`Highlighter`**. Test target: **`HighlighterTests`** (`bundle.unit-test`, depends on the app target).
- Build runs a **pre-build script** that builds the Rust static lib + Swift bindings: `app/core/scripts/generate-swift-bindings.sh` (wired as `preBuildScripts` in `project.yml`, `basedOnDependencyAnalysis: false`, so it runs every build). You do **not** invoke it manually for a normal build — `xcodebuild` triggers it. Run it standalone only if bindings look stale: `bash /Users/pablofernandez/Work/hl/app/core/scripts/generate-swift-bindings.sh`.
- If you add new test files, regenerate the project first so they're in the target: `cd /Users/pablofernandez/Work/hl/app/ios/Highlighter && xcodegen generate`.
- The CI simulator exists locally as **`iPhone 16 ci`** (`xcrun simctl list` confirms it). Test command:

```
cd /Users/pablofernandez/Work/hl/app/ios/Highlighter && \
xcodebuild test -project Highlighter.xcodeproj -scheme Highlighter \
  -sdk iphonesimulator -destination 'platform=iOS Simulator,name=iPhone 16 ci'
```

(Plain build for the view-only parts: same minus `test`, `-sdk iphonesimulator` build.) To run one suite while iterating, append e.g. `-only-testing:HighlighterTests/ContentTreeBodyRendererFootnoteTests`.

Test conventions (from `ContentTreeBodyRendererNestedTests.swift`, `ArticleMarkdownRendererTests.swift`): **Swift Testing** (`import Testing`, `struct …Tests { @Test func … { #expect(...) } }`), `@testable import Highlighter`, fixtures as inline `content_tree` JSON strings decoded with `ContentTreeBodyRenderer.decodeTree(json:)`. Follow this exactly — do not introduce XCTest.

---

## 2. Feature A — Text selection (extract + cover with tests first)

Selection already works; the TDD value is making the **selection-context projection** a pure, tested function (it currently lives inline in `ArticleBodyView.Coordinator.selectionText`, lines 176-215, untestable because it needs a live `UITextView`).

**Tests first** — new file `…/HighlighterTests/ArticleReaderSelectionTests.swift`:
- `projectsTrimmedQuoteAndSurroundingParagraphContext` — full text `"Para one.\n\nThe quote here lives.\n\nPara three."`, selected range covering `"quote here"`; expect `quote == "quote here"`, `context == "The quote here lives."`, `hasQuote == true`.
- `contextClearedWhenItEqualsTheQuote` — select an entire single-paragraph body; expect `context == ""` (mirrors the existing D1 rule at lines 209-214).
- `contextStopsAtDoubleNewlineParagraphBreak` — selection near a `\n\n` boundary does not bleed into the adjacent paragraph.
- `emptySelectionHasNoQuote` — zero-length range → `hasQuote == false`, empty quote/context.
- `leadingTrailingWhitespaceTrimmed`.

**Implementation:** add `enum ArticleReaderSelection { static func project(fullText: String, selectedRange: NSRange) -> (quote: String, context: String, hasQuote: Bool) }` (new file `…/Features/Article/ArticleReaderSelection.swift`) holding the exact paragraph-scan logic moved verbatim from `Coordinator.selectionText`. Then make `Coordinator.selectionText` call `ArticleReaderSelection.project(fullText: tv.text, selectedRange: tv.selectedRange)`. Pure refactor, zero behavior change.

**Share/highlight integration (already wired, document it, don't rebuild):** Edit-Menu action → `parent.onPublishHighlight(quote, context)` / `parent.onRequestNote(...)` (ArticleBodyView lines 150-174) → `ReaderScroll` closures (ArticleReaderView 134-146) → `ArticleReaderView.publish(quote:context:note:)` (159) → `ArticleReaderStore.publishHighlight` (154) → `kernel.app.dispatch(.publishHighlight(content:sourceReference:…))`. The note path raises `NoteComposerSheet` via `pendingHighlight`. The toolbar "share to community" is a separate flow (`ShareToCommunitySheet`). No change needed.

---

## 3. Feature B — Footnotes (the real renderer gap; strongest TDD surface)

The `content_tree` is CommonMark, which does not model `[^id]` footnotes, so both the reference tokens (`Body text[^1].`) and the definition lines (`[^1]: the note`) survive as **literal `.text` content** inside `paragraph` nodes. Recover them in the hl shell, reusing the machinery `ArticleBodyView` + `MarkdownRenderer` already have.

### B1. Pure extraction function (test-first)

Add `enum ContentTreeFootnotes` (new file `…/Features/Article/ContentTreeFootnotes.swift`) with:
`static func scan(tree: ContentTreeWire) -> (definitions: [FootnotePreprocessor.Definition], definitionRootIndices: Set<UInt32>)`
- Walk `tree.roots`; for each root `paragraph`, flatten its child `.text` plain string; if it matches `^\s*\[\^(<id>)\]:\s*(.*)$` treat the **whole paragraph** as a definition (id, body markdown = remainder, number = source order, dedup first-wins — reuse the exact rules in `FootnotePreprocessor.parseDefinitionHeader`). Record the paragraph's root node index in `definitionRootIndices` so the walker can skip it.
- Reuse the existing `FootnotePreprocessor.Definition` struct (id/number/markdown) so downstream rendering stays shared.

**Tests** — new file `…/HighlighterTests/ContentTreeFootnotesTests.swift` (Swift Testing, JSON fixtures like the nested tests):
- `liftsDefinitionParagraphsInSourceOrder` — two `[^a]:` / `[^b]:` paragraphs → numbers 1,2, correct markdown bodies, both root indices flagged.
- `duplicateIdKeepsFirstDefinition`.
- `paragraphWithoutDefinitionPrefixIsNotADefinition` — `"see [^1] above"` is a reference, not a definition; not flagged.
- `noFootnotesYieldsEmptyResult` — regression so plain articles are untouched.

### B2. Wire extraction into `ContentTreeBodyRenderer`

In `ContentTreeBodyRenderer.render(...)`:
1. Call `ContentTreeFootnotes.scan(tree:)` up front. Pass `definitions` (as `[id: Definition]`) and `definitionRootIndices` into `TreeWalker`.
2. In `TreeWalker.walk()`, skip roots whose index is in `definitionRootIndices` (don't emit them as prose).
3. In `TreeWalker.renderInlineNode` `.text` case (line 612), port `MarkdownRenderer.renderPlainText`'s `[^id]` scanner: when an id resolves to a known definition, emit a superscript run (`UIFont` ~`bodyPointSize-6`, `.baselineOffset`, `MarkdownRenderer.footnoteReferenceAttribute = number`, `.link = highlighter://footnote/<n>`) and record `footnoteAnchors[number] = NSRange(...)`. Unknown ids stay literal. The walker needs a mutable `footnoteAnchors` accumulator (it's currently a `struct` with non-mutating methods — make `walk()`/emit paths thread it through, mirroring `BodyWalker.footnoteAnchors`).
4. Replace the hard-coded empty return (lines 114-119): build the footnotes `NSAttributedString` from the collected `definitions` and return real `footnoteAnchors`.

For the footnotes block, **reuse `MarkdownRenderer.renderFootnotes`** — change it from `private static` to `static` (internal) so `ContentTreeBodyRenderer` can call it with `[FootnotePreprocessor.Definition]`. It already renders the number, parses the definition markdown via `BodyWalker`, and appends the tappable `↩` back-arrow with `footnoteBackAttribute` + `highlighter://footnote-back/<n>` — exactly what `ArticleBodyView.handleTap` (lines 236-243) and `withFootnotes` (ArticleReaderView 381-401) already consume. No view changes needed for footnotes to appear once `Output.footnotes`/`footnoteAnchors` are populated.

**Renderer-level tests** — new file `…/HighlighterTests/ContentTreeBodyRendererFootnoteTests.swift` (reuse the harness shape from `ContentTreeBodyRendererNestedTests`):
- `inlineReferenceRendersSuperscriptAnchorWithFootnoteAttribute` — body text contains `"[1]"`; walking attributes at that range yields `MarkdownRenderer.footnoteReferenceAttribute == 1`; `output.footnoteAnchors[1] != nil`.
- `definitionParagraphIsRemovedFromBodyAndRenderedInFootnoteBlock` — concatenated `.text` segments do NOT contain the definition body; `output.footnotes.length > 0` and its string contains the definition text + `"↩"`.
- `multipleFootnotesNumberAndAnchorInOrder`.
- `unmatchedReferenceStaysLiteral` — `[^99]` with no def → `"[^99]"` literal, `footnoteAnchors[99] == nil`.
- `articleWithoutFootnotesReturnsEmptyBlock` — guards the existing `ContentTreeBodyRendererNestedTests` (no defs → `footnotes.length == 0`, identical segment behavior).

### B3. (Optional, build/manual-verify) footnote scroll-to

`scrollAnchor` is dead state today. Tapping a footnote already calls `onFootnoteTap` → sets `scrollAnchor`, but nothing scrolls. Full scroll-to-definition is hard because body+footnotes live inside one non-scrolling `UITextView` embedded in the outer SwiftUI `ScrollView`, so it requires translating the `footnoteAnchors` NSRange to a rect in the text view and scrolling the outer ScrollView. Recommend: land B1/B2 (makes footnotes visible + tappable) in this PR; treat scroll-to as a follow-up or a best-effort `ScrollViewReader` jump to a `.id("footnotes")` block (gives "jump to notes section", not exact line). Mark it manual-verify only — not unit-testable.

---

## 4. Feature C — Reading-progress overlay

Allowed as native presentation-only state per `app/AGENTS.md` ("scroll position while a view is alive" is an accepted exception) — no Rust/NMP involvement.

### C1. Pure fraction function (test-first)

Add `enum ReadingProgress` (new file `…/Features/Article/ReadingProgress.swift`):
`static func fraction(contentOffsetY: CGFloat, contentHeight: CGFloat, viewportHeight: CGFloat) -> Double` — `let scrollable = contentHeight - viewportHeight; guard scrollable > 0 else { return 0 }; return min(1, max(0, Double(contentOffsetY / scrollable)))`. Optionally `static func percentLabel(_ fraction: Double) -> String`.

**Tests** — new file `…/HighlighterTests/ReadingProgressTests.swift`:
- `zeroAtTop`, `oneAtBottom`, `midpointIsHalf`, `clampsNegativeOffsetToZero`, `clampsOverscrollToOne`, `contentShorterThanViewportReturnsZero`.

### C2. View wiring (build/manual-verify)

In `ReaderScroll.body` (`ScrollView` at ArticleReaderView line 256): deployment target is iOS 26.1, so use the modern API — attach `.onScrollGeometryChange(for: Double.self) { geo in ReadingProgress.fraction(contentOffsetY: geo.contentOffset.y, contentHeight: geo.contentSize.height, viewportHeight: geo.containerSize.height) } action: { _, newValue in progress = newValue }` with a new `@State private var progress: Double = 0`. Render a thin top progress bar via `.overlay(alignment: .top)` (a `GeometryReader`-width `Capsule`/`Rectangle` scaled by `progress`, `Color.highlighterAccent`) or reuse the existing bottom-`safeAreaInset` slot pattern. Keep it inside `ReaderScroll` so it only shows once `rendered != nil`.

This part is view code (not unit-testable); the testable contract is `ReadingProgress.fraction`.

---

## 5. Sequencing, risks, what's testable

**Land order (single PR, TDD):**
1. **A (selection projection extract + tests)** — pure refactor, zero behavior change, safest, fastest green. Establishes the pattern.
2. **B (footnotes)** — the real user-visible gap, fully self-contained in `ContentTreeBodyRenderer` + reused `MarkdownRenderer.renderFootnotes`, strongest unit coverage.
3. **C (reading progress)** — pure `fraction` tested, then view wiring last (most likely to need a simulator to eyeball).

**Risks / watch-items:**
- **Regression on `ContentTreeBodyRendererNestedTests`**: the footnote change touches `TreeWalker`. Guard with `articleWithoutFootnotesReturnsEmptyBlock` and confirm those 4 nested tests still pass — behavior must be identical when no definitions exist.
- **`MarkdownRenderer.renderFootnotes` visibility change** (`private`→internal) is the only edit to that file's surface; it re-parses definition markdown via `BodyWalker`, so footnote bodies keep emphasis/links/code. Verify the definition body you pass is the post-`[^id]:` remainder.
- **`TreeWalker` is a `struct` with non-mutating emit methods**; threading a mutable `footnoteAnchors` through `emitBlock`/`renderInline*` requires either `inout` accumulation (like `SegmentEmitter`) or making the walker a `class`/`mutating`. Mirror how `BodyWalker` in `MarkdownRenderer` already holds `var footnoteAnchors`.
- **Footnote anchor ranges** are per-segment; `withFootnotes` only appends the block to the **last** text segment and passes `output.footnoteAnchors` only for that segment (ArticleReaderView 339-349). Ensure references and the footnote block end up in the same final `.text` segment, or document that anchors are relative to that last segment (which is what `ArticleBodyView` already assumes).
- **Genuinely not unit-testable (build/simulator-verify only):** the `UITextView` Edit Menu interaction, footnote tap → visual flash/scroll, and the progress-bar overlay rendering. Everything else (selection projection, footnote extraction + attribute emission on the `Output`, progress fraction) is pure and must be covered by Swift Testing.
- **Don't touch** anything under `Sources/Highlighter/Vendor/nmp/` (NMP). All work is in `Features/Article/` + `HighlighterTests/`.
- After adding the 5 new files (3 sources + 4 test files), run `xcodegen generate` before `xcodebuild test` so they join the targets.

### Critical Files for Implementation
- /Users/pablofernandez/Work/hl/app/ios/Highlighter/Sources/Highlighter/Features/Article/ContentTreeBodyRenderer.swift
- /Users/pablofernandez/Work/hl/app/ios/Highlighter/Sources/Highlighter/Features/Article/MarkdownRenderer.swift
- /Users/pablofernandez/Work/hl/app/ios/Highlighter/Sources/Highlighter/Features/Article/ArticleReaderView.swift
- /Users/pablofernandez/Work/hl/app/ios/Highlighter/Sources/Highlighter/Features/Article/ArticleBodyView.swift
- /Users/pablofernandez/Work/hl/app/ios/Highlighter/Sources/Highlighter/Features/Article/FootnotePreprocessor.swift

