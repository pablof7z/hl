import Foundation
import Testing
import UIKit
@testable import Highlighter

/// Tests for the footnote rendering path in `ContentTreeBodyRenderer`.
///
/// After B2 wiring: definition paragraphs are removed from the body,
/// inline `[^id]` references are replaced with superscript runs that carry
/// `MarkdownRenderer.footnoteReferenceAttribute`, and the footnote block is
/// populated in `output.footnotes`.
///
/// The last test (`articleWithoutFootnotesReturnsEmptyFootnoteBlock`) acts as
/// a regression guard so the existing `ContentTreeBodyRendererNestedTests`
/// behaviour is unchanged when no footnotes are present.
struct ContentTreeBodyRendererFootnoteTests {

    // MARK: - Harness (mirror ContentTreeBodyRendererNestedTests)

    private func render(_ json: String) -> MarkdownRenderer.Output {
        let tree = ContentTreeBodyRenderer.decodeTree(json: json)
        #expect(tree != nil, "fixture must decode")
        return ContentTreeBodyRenderer.render(
            tree: tree!,
            highlights: [],
            accent: .systemBlue,
            tint: .systemYellow,
            ink: .label,
            muted: .secondaryLabel,
            highlightContent: { _ in
                HighlightDetailContentProjection(
                    quoteText: "", noteText: nil, pageImageUrl: nil, shareMessage: ""
                )
            }
        )
    }

    /// Concatenate plain text of every `.text` segment (separator \u{1} so
    /// cross-segment matches don't produce false positives).
    private func allText(_ output: MarkdownRenderer.Output) -> String {
        output.segments.compactMap { seg -> String? in
            if case .text(let a) = seg { return a.string } else { return nil }
        }.joined(separator: "\u{1}")
    }

    // MARK: - Inline reference renders superscript + anchor

    @Test func inlineReferenceRendersSuperscriptAnchorWithFootnoteAttribute() {
        // Body paragraph: "Body text[^1]."
        // Definition paragraph: "[^1]: The footnote definition."
        let json = """
        {
          "roots": [1, 3],
          "nodes": [
            {"kind": "text", "text": "Body text[^1]."},
            {"kind": "paragraph", "children": [0]},
            {"kind": "text", "text": "[^1]: The footnote definition."},
            {"kind": "paragraph", "children": [2]}
          ]
        }
        """
        let output = render(json)

        // Anchor must be registered for footnote 1.
        #expect(output.footnoteAnchors[1] != nil)

        // The last text segment must carry the superscript attribute at the
        // reference location with value == 1.
        let textSegments = output.segments.compactMap { seg -> NSAttributedString? in
            if case .text(let a) = seg { return a } else { return nil }
        }
        #expect(!textSegments.isEmpty)
        let lastText = textSegments.last!
        var foundRefAttr = false
        lastText.enumerateAttribute(
            MarkdownRenderer.footnoteReferenceAttribute,
            in: NSRange(location: 0, length: lastText.length)
        ) { value, _, _ in
            if let number = value as? Int, number == 1 {
                foundRefAttr = true
            }
        }
        #expect(foundRefAttr)
    }

    // MARK: - Definition paragraph removed from body, rendered in footnote block

    @Test func definitionParagraphIsRemovedFromBodyAndRenderedInFootnoteBlock() {
        // Body paragraph: "Body text."
        // Definition paragraph: "[^1]: footnote body."
        let json = """
        {
          "roots": [1, 3],
          "nodes": [
            {"kind": "text", "text": "Body text."},
            {"kind": "paragraph", "children": [0]},
            {"kind": "text", "text": "[^1]: footnote body."},
            {"kind": "paragraph", "children": [2]}
          ]
        }
        """
        let output = render(json)
        let bodyText = allText(output)

        // Definition must NOT appear verbatim in the body.
        #expect(!bodyText.contains("[^1]:"))
        #expect(!bodyText.contains("footnote body"))

        // Must appear in the footnote block.
        #expect(output.footnotes.length > 0)
        #expect(output.footnotes.string.contains("footnote body"))

        // Back-arrow ↩ must be present in the footnote block.
        #expect(output.footnotes.string.contains("↩"))
    }

    // MARK: - Multiple footnotes numbered and anchored in order

    @Test func multipleFootnotesNumberAndAnchorInOrder() {
        // Body paragraph references both [^a] and [^b].
        // Two definition paragraphs in source order.
        let json = """
        {
          "roots": [1, 3, 5],
          "nodes": [
            {"kind": "text", "text": "First[^a] and second[^b]."},
            {"kind": "paragraph", "children": [0]},
            {"kind": "text", "text": "[^a]: Alpha."},
            {"kind": "paragraph", "children": [2]},
            {"kind": "text", "text": "[^b]: Beta."},
            {"kind": "paragraph", "children": [4]}
          ]
        }
        """
        let output = render(json)
        #expect(output.footnoteAnchors[1] != nil)
        #expect(output.footnoteAnchors[2] != nil)
        // Both definition bodies must appear in the footnote block.
        #expect(output.footnotes.string.contains("Alpha"))
        #expect(output.footnotes.string.contains("Beta"))
    }

    // MARK: - Unmatched reference stays literal

    @Test func unmatchedReferenceStaysLiteral() {
        // [^99] has no matching definition — must render as literal text "[^99]".
        let json = """
        {
          "roots": [1],
          "nodes": [
            {"kind": "text", "text": "Reference to [^99] with no definition."},
            {"kind": "paragraph", "children": [0]}
          ]
        }
        """
        let output = render(json)
        let bodyText = allText(output)
        #expect(bodyText.contains("[^99]"))
        #expect(output.footnoteAnchors[99] == nil)
    }

    // MARK: - No footnotes → empty block (regression guard for nested tests)

    @Test func articleWithoutFootnotesReturnsEmptyFootnoteBlock() {
        // Plain prose — no [^id] patterns at all. Guards that the existing
        // ContentTreeBodyRendererNestedTests segment behaviour is identical:
        // footnotes.length==0, no extra anchors.
        let json = """
        {
          "roots": [1],
          "nodes": [
            {"kind": "text", "text": "Plain paragraph without footnotes."},
            {"kind": "paragraph", "children": [0]}
          ]
        }
        """
        let output = render(json)
        #expect(output.footnotes.length == 0)
        #expect(output.footnoteAnchors.isEmpty)
        let bodyText = allText(output)
        #expect(bodyText.contains("Plain paragraph"))
    }
}
