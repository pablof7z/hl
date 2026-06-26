import Foundation
import Testing
@testable import Highlighter

/// Unit tests for `ArticleReaderSelection.project` — the pure paragraph-scan
/// logic extracted from `ArticleBodyView.Coordinator.selectionText`.
///
/// Tests run against the pure function, not the UIKit `UITextView`, so they
/// are synchronous and have no UI dependency.
struct ArticleReaderSelectionTests {

    // MARK: - Nominal path

    @Test func projectsTrimmedQuoteAndSurroundingParagraphContext() {
        // "Para one.\n\nThe quote here lives.\n\nPara three."
        //  0         9 10               (positions)
        //            ^--- double-newline paragraph break
        // "The " starts at 11, "quote here" at 15, length 10.
        let fullText = "Para one.\n\nThe quote here lives.\n\nPara three."
        let range = NSRange(location: 15, length: 10)
        let result = ArticleReaderSelection.project(fullText: fullText, selectedRange: range)
        #expect(result.hasQuote)
        #expect(result.quote == "quote here")
        #expect(result.context == "The quote here lives.")
    }

    // MARK: - Context equals quote → clear context

    @Test func contextClearedWhenItEqualsTheQuote() {
        // Selecting the entire paragraph means context == quote → context must
        // be cleared to avoid duplicating the text in the highlight payload.
        let fullText = "Entire paragraph."
        let range = NSRange(location: 0, length: fullText.count)
        let result = ArticleReaderSelection.project(fullText: fullText, selectedRange: range)
        #expect(result.hasQuote)
        #expect(result.quote == "Entire paragraph.")
        #expect(result.context == "")
    }

    // MARK: - Paragraph boundary stops at double-newline

    @Test func contextStopsAtDoubleNewlineParagraphBreak() {
        // "Before.\n\nThis quote here.\n\nAfter."
        //  B=0..6, \n=7, \n=8, T=9, h=10, i=11, s=12, ' '=13,
        //  q=14, u=15, o=16, t=17, e=18, ' '=19, h=20, e=21, r=22, e=23,
        //  .=24, \n=25, \n=26, A=27...
        let fullText = "Before.\n\nThis quote here.\n\nAfter."
        let range = NSRange(location: 14, length: 10) // "quote here"
        let result = ArticleReaderSelection.project(fullText: fullText, selectedRange: range)
        #expect(result.hasQuote)
        #expect(result.quote == "quote here")
        #expect(result.context == "This quote here.")
        #expect(!result.context.contains("Before"))
        #expect(!result.context.contains("After"))
    }

    // MARK: - Empty selection

    @Test func emptySelectionHasNoQuote() {
        let fullText = "Some text here."
        let range = NSRange(location: 5, length: 0)
        let result = ArticleReaderSelection.project(fullText: fullText, selectedRange: range)
        #expect(!result.hasQuote)
        #expect(result.quote.isEmpty)
        #expect(result.context.isEmpty)
    }

    // MARK: - Whitespace trimming

    @Test func leadingTrailingWhitespaceTrimmed() {
        // The full text has surrounding whitespace; the trimmed quote must not.
        let fullText = "  quote  "
        let range = NSRange(location: 0, length: fullText.count)
        let result = ArticleReaderSelection.project(fullText: fullText, selectedRange: range)
        #expect(result.hasQuote)
        #expect(result.quote == "quote")
    }
}
