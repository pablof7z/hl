import Foundation

/// Scans an nmp `ContentTreeWire` arena for GFM-style footnote definitions
/// that survive CommonMark parsing as literal text inside paragraph nodes.
///
/// In CommonMark (no GFM footnote extension), a definition line
/// `[^id]: body` is not parsed as a footnote — it remains a plain paragraph
/// whose text node contains the literal string `[^id]: body`. This scanner
/// recovers those definitions so `ContentTreeBodyRenderer` can:
///   1. Skip the definition paragraphs when rendering the body (they must not
///      appear twice).
///   2. Build a real footnote block from the collected definitions via
///      `MarkdownRenderer.renderFootnotes`.
enum ContentTreeFootnotes {

    /// Scan the arena for root-level footnote definition paragraphs.
    ///
    /// A root paragraph qualifies iff its flattened plain text starts with the
    /// pattern `[^<id>]: <body>` where `<id>` contains no whitespace. The
    /// pattern is identical to `FootnotePreprocessor.parseDefinitionHeader`.
    ///
    /// Duplicate ids: first definition wins; the second is NOT flagged so it
    /// renders as ordinary body prose rather than being silently dropped.
    ///
    /// - Returns:
    ///   - `definitions`: Ordered `FootnotePreprocessor.Definition` values,
    ///     numbered from 1 in source order.
    ///   - `definitionRootIndices`: The `UInt32` root indices that were
    ///     identified as definitions and should be excluded from body rendering.
    static func scan(
        tree: ContentTreeWire
    ) -> (definitions: [FootnotePreprocessor.Definition], definitionRootIndices: Set<UInt32>) {
        var seenIds: Set<String> = []
        var definitions: [FootnotePreprocessor.Definition] = []
        var definitionRootIndices: Set<UInt32> = []

        for rootIdx in tree.roots {
            guard let node = tree.node(at: rootIdx) else { continue }
            guard case .paragraph(let children) = node else { continue }

            // Flatten all text content from the paragraph's inline children.
            let flatText = children
                .compactMap { tree.node(at: $0) }
                .map { plainText(of: $0, in: tree) }
                .joined()

            guard let parsed = parseDefinitionHeader(flatText) else { continue }
            guard !seenIds.contains(parsed.id) else { continue }

            seenIds.insert(parsed.id)
            definitions.append(FootnotePreprocessor.Definition(
                id: parsed.id,
                number: definitions.count + 1,
                markdown: parsed.firstLine
            ))
            definitionRootIndices.insert(rootIdx)
        }

        return (definitions, definitionRootIndices)
    }

    // MARK: - Private helpers

    /// Recursively collect the plain string content of a wire node, visiting
    /// only `.text` leaves (and transparent containers like emphasis / strong /
    /// link). Used to reconstruct the verbatim GFM `[^id]: body` string from
    /// the content tree's inline model.
    private static func plainText(of node: NostrWireNode, in tree: ContentTreeWire) -> String {
        switch node {
        case .text(let value):
            return value
        case .emphasis(let children),
             .strong(let children),
             .blockQuote(let children),
             .paragraph(let children):
            return children
                .compactMap { tree.node(at: $0) }
                .map { plainText(of: $0, in: tree) }
                .joined()
        case .link(let children, _):
            return children
                .compactMap { tree.node(at: $0) }
                .map { plainText(of: $0, in: tree) }
                .joined()
        default:
            return ""
        }
    }

    /// Parse `[^id]: first-line-text`. Mirrors `FootnotePreprocessor`'s
    /// private `parseDefinitionHeader(_:)` so the same matching rules apply
    /// to content-tree text nodes.
    ///
    /// Returns `nil` if the text does not start with the definition pattern.
    private static func parseDefinitionHeader(
        _ text: String
    ) -> (id: String, firstLine: String)? {
        guard text.hasPrefix("[^") else { return nil }
        let afterOpen = text.dropFirst(2)
        guard let closeRange = afterOpen.range(of: "]:") else { return nil }
        let id = String(afterOpen[..<closeRange.lowerBound])
        guard !id.isEmpty, !id.contains(where: \.isWhitespace) else { return nil }
        var rest = String(afterOpen[closeRange.upperBound...])
        if rest.first == " " { rest.removeFirst() }
        return (id, rest)
    }
}
