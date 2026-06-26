import Foundation
import Testing
@testable import Highlighter

/// Unit tests for `ContentTreeFootnotes.scan` — the content-tree footnote
/// definition scanner. All fixtures are decoded via the same
/// `ContentTreeBodyRenderer.decodeTree(json:)` helper the nested-block tests
/// use, so the JSON format stays consistent.
///
/// GFM footnotes survive as literal text in CommonMark content trees:
///   • A definition paragraph looks like `[^id]: body` in its text node.
///   • An inline reference looks like `[^id]` inside a body paragraph.
/// `ContentTreeFootnotes.scan` recovers definitions by pattern-matching root
/// paragraphs and returns both the ordered list and the set of root indices
/// to skip during body rendering.
struct ContentTreeFootnotesTests {

    // MARK: - Harness

    private func scan(_ json: String) -> (definitions: [FootnotePreprocessor.Definition], definitionRootIndices: Set<UInt32>) {
        let tree = ContentTreeBodyRenderer.decodeTree(json: json)
        #expect(tree != nil, "fixture must decode")
        return ContentTreeFootnotes.scan(tree: tree!)
    }

    // MARK: - Nominal: multiple definitions in source order

    @Test func liftsDefinitionParagraphsInSourceOrder() {
        // Two definition paragraphs — expect them numbered 1, 2 in root order.
        let json = """
        {
          "roots": [1, 3],
          "nodes": [
            {"kind": "text", "text": "[^a]: First footnote."},
            {"kind": "paragraph", "children": [0]},
            {"kind": "text", "text": "[^b]: Second footnote."},
            {"kind": "paragraph", "children": [2]}
          ]
        }
        """
        let result = scan(json)
        #expect(result.definitions.count == 2)
        #expect(result.definitions[0].id == "a")
        #expect(result.definitions[0].number == 1)
        #expect(result.definitions[0].markdown == "First footnote.")
        #expect(result.definitions[1].id == "b")
        #expect(result.definitions[1].number == 2)
        #expect(result.definitions[1].markdown == "Second footnote.")
        #expect(result.definitionRootIndices.contains(1))
        #expect(result.definitionRootIndices.contains(3))
    }

    // MARK: - Duplicate id keeps first definition

    @Test func duplicateIdKeepsFirstDefinition() {
        // Second paragraph has the same id: only the first is kept.
        let json = """
        {
          "roots": [1, 3],
          "nodes": [
            {"kind": "text", "text": "[^a]: First."},
            {"kind": "paragraph", "children": [0]},
            {"kind": "text", "text": "[^a]: Duplicate."},
            {"kind": "paragraph", "children": [2]}
          ]
        }
        """
        let result = scan(json)
        #expect(result.definitions.count == 1)
        #expect(result.definitions[0].id == "a")
        #expect(result.definitions[0].markdown == "First.")
        // Only root 1 (first definition) is flagged; root 3 is NOT.
        #expect(result.definitionRootIndices.contains(1))
        #expect(!result.definitionRootIndices.contains(3))
    }

    // MARK: - Inline reference is NOT a definition

    @Test func paragraphWithoutDefinitionPrefixIsNotADefinition() {
        // A paragraph containing an inline reference "see [^1] above" must
        // NOT be mistaken for a definition — the pattern requires the paragraph
        // text to START with `[^id]:`.
        let json = """
        {
          "roots": [1],
          "nodes": [
            {"kind": "text", "text": "see [^1] above"},
            {"kind": "paragraph", "children": [0]}
          ]
        }
        """
        let result = scan(json)
        #expect(result.definitions.isEmpty)
        #expect(result.definitionRootIndices.isEmpty)
    }

    // MARK: - No footnotes at all

    @Test func noFootnotesYieldsEmptyResult() {
        let json = """
        {
          "roots": [1],
          "nodes": [
            {"kind": "text", "text": "Simple paragraph without footnotes."},
            {"kind": "paragraph", "children": [0]}
          ]
        }
        """
        let result = scan(json)
        #expect(result.definitions.isEmpty)
        #expect(result.definitionRootIndices.isEmpty)
    }
}
