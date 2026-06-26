import Foundation

/// Pure paragraph-context extractor for the article text-selection highlight
/// flow. Extracted verbatim from `ArticleBodyView.Coordinator.selectionText`
/// so the logic can be unit-tested without a `UITextView`.
///
/// The coordinator now calls `ArticleReaderSelection.project` and forwards
/// the result — zero behaviour change in production.
enum ArticleReaderSelection {

    /// Project a selected range within `fullText` into the trimmed quote, its
    /// surrounding paragraph context, and a flag indicating whether a quote was
    /// actually selected.
    ///
    /// - Parameters:
    ///   - fullText: The entire attributed text body as a plain string.
    ///   - selectedRange: The `UITextView.selectedRange` at the moment the user
    ///     tapped **Highlight** / **Highlight with note**.
    /// - Returns:
    ///   - `quote`: The selected text, trimmed of leading/trailing whitespace.
    ///   - `context`: The surrounding paragraph trimmed of whitespace. Empty
    ///     when it equals `quote` (i.e. the whole paragraph was selected).
    ///   - `hasQuote`: `true` iff `quote` is non-empty after trimming.
    static func project(
        fullText: String,
        selectedRange: NSRange
    ) -> (quote: String, context: String, hasQuote: Bool) {
        guard selectedRange.length > 0 else { return ("", "", false) }
        guard let textRange = Range(selectedRange, in: fullText) else { return ("", "", false) }
        let quote = String(fullText[textRange])

        // Context: the paragraph the selection starts in. Find the paragraph
        // bounds by scanning for double-newlines on either side.
        let full = fullText as NSString
        var start = selectedRange.location
        var end = selectedRange.location + selectedRange.length

        while start > 0 {
            let prior = full.substring(with: NSRange(location: start - 1, length: 1))
            if prior == "\n" {
                // Stop one step before a double-newline paragraph break.
                if start >= 2, full.substring(with: NSRange(location: start - 2, length: 1)) == "\n" {
                    break
                }
            }
            start -= 1
        }

        while end < full.length {
            if end + 1 < full.length,
               full.substring(with: NSRange(location: end, length: 1)) == "\n",
               full.substring(with: NSRange(location: end + 1, length: 1)) == "\n" {
                break
            }
            end += 1
        }

        let paragraphRange = NSRange(location: start, length: max(0, end - start))
        let paragraph = full.substring(with: paragraphRange)

        let trimmedQuote = quote.trimmingCharacters(in: .whitespaces)
        let trimmedContext = paragraph.trimmingCharacters(in: .whitespaces)
        let context = trimmedContext == trimmedQuote ? "" : trimmedContext
        return (trimmedQuote, context, !trimmedQuote.isEmpty)
    }
}
