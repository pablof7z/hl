import Foundation
import Testing
import UIKit
@testable import Highlighter

/// Regression guard for the #22 HIGH: nested BLOCK nodes inside a block-quote or
/// list item were silently dropped because the renderer sent container children
/// through the inline path, whose fallback returned EMPTY for `.list` /
/// `.codeBlock` / `.rule` / `.media`. The renderer is now fully block-aware and
/// recursive: every block variant renders at every depth, prose stays selectable
/// in `.text` segments, and rich blocks (image / media / card / placeholder)
/// flush as their own segments in document order.
///
/// These tests build deep-nesting `ContentTreeWire` fixtures and assert over the
/// `MarkdownRenderer.BodySegment` tree the renderer produces — counts/types per
/// nested block — so a future regression that drops a nested block fails here.
struct ContentTreeBodyRendererNestedTests {

    // MARK: - Harness

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
            },
            // Resolve every nostr URI to a card so standalone event-refs become
            // `.nostrEntity` segments.
            resolveEntity: { uri in .event(eventIdHex: uri, relays: [], authorHintHex: nil, kindHint: nil) }
        )
    }

    /// Concatenate the plain text of every `.text` segment.
    private func allText(_ output: MarkdownRenderer.Output) -> String {
        output.segments.compactMap { segment -> String? in
            if case .text(let attr) = segment { return attr.string }
            return nil
        }.joined(separator: "\u{1}")
    }

    private func imageSegments(_ output: MarkdownRenderer.Output) -> [(URL, String)] {
        output.segments.compactMap { segment in
            if case .image(let url, let alt) = segment { return (url, alt) }
            return nil
        }
    }

    private func mediaSegments(_ output: MarkdownRenderer.Output) -> [(urls: [String], kind: NostrMediaKind)] {
        output.segments.compactMap { segment in
            if case .media(let urls, let kind) = segment { return (urls, kind) }
            return nil
        }
    }

    private func entitySegmentCount(_ output: MarkdownRenderer.Output) -> Int {
        output.segments.filter { if case .nostrEntity = $0 { return true }; return false }.count
    }

    private func placeholderSegmentCount(_ output: MarkdownRenderer.Output) -> Int {
        output.segments.filter { if case .placeholder = $0 { return true }; return false }.count
    }

    // MARK: - Deep nesting: block-quote → nested list → {code block, rule, image}

    /// arena:
    ///   0 text "in quote"
    ///   1 paragraph[0]
    ///   2 text "before code"
    ///   3 code_block(swift, "let x = 1")
    ///   4 rule
    ///   5 image(alt="diagram", src=...)
    ///   6 list[[2],[3],[4],[5]]   ← nested list w/ code + rule + image items
    ///   7 block_quote[1, 6]        ← quote containing prose para + nested list
    @Test func blockQuoteContainingNestedListWithCodeRuleImageRendersAll() {
        let json = """
        {
          "roots": [7],
          "nodes": [
            {"kind":"text","text":"in quote"},
            {"kind":"paragraph","children":[0]},
            {"kind":"text","text":"before code"},
            {"kind":"code_block","info":"swift","body":"let x = 1"},
            {"kind":"rule"},
            {"kind":"image","alt":"diagram","title":null,"src":"https://example.com/d.png"},
            {"kind":"list","ordered_start":null,"items":[[2],[3],[4],[5]]},
            {"kind":"block_quote","children":[1,6]}
          ]
        }
        """
        let output = render(json)
        let text = allText(output)

        // Quote prose survived.
        #expect(text.contains("in quote"))
        // Nested list bullet text survived.
        #expect(text.contains("before code"))
        // Nested CODE BLOCK survived WITH its language header (was dropped).
        #expect(text.contains("swift"))
        #expect(text.contains("let x = 1"))
        // Nested RULE survived (rendered as the divider glyph, was dropped).
        #expect(text.contains("———"))
        // Nested IMAGE inside the nested list inside the quote became its own
        // image segment (was dropped).
        let images = imageSegments(output)
        #expect(images.count == 1)
        #expect(images.first?.0.absoluteString == "https://example.com/d.png")
        #expect(images.first?.1 == "diagram")
    }

    // MARK: - Deep nesting: list items containing {block-quote, nested list, media}

    /// arena:
    ///   0 text "quoted in item"
    ///   1 paragraph[0]
    ///   2 block_quote[1]            ← item-1 child block
    ///   3 text "inner item"
    ///   4 list[[3]]                 ← item-2 child nested list
    ///   5 media(video)              ← item-3 child media
    ///   6 list[[2],[4],[5]]         ← outer list: 3 items, each a block child
    @Test func listItemsContainingBlockQuoteNestedListMediaRenderAll() {
        let json = """
        {
          "roots": [6],
          "nodes": [
            {"kind":"text","text":"quoted in item"},
            {"kind":"paragraph","children":[0]},
            {"kind":"block_quote","children":[1]},
            {"kind":"text","text":"inner item"},
            {"kind":"list","ordered_start":null,"items":[[3]]},
            {"kind":"media","urls":["https://example.com/v.mp4"],"media_kind":"Video"},
            {"kind":"list","ordered_start":null,"items":[[2],[4],[5]]}
          ]
        }
        """
        let output = render(json)
        let text = allText(output)

        // Block-quote nested inside a list item survived.
        #expect(text.contains("quoted in item"))
        // Nested list nested inside a list item survived.
        #expect(text.contains("inner item"))
        // Media (video) nested inside a list item became its own media segment.
        let media = mediaSegments(output)
        #expect(media.count == 1)
        #expect(media.first?.kind == .video)
        #expect(media.first?.urls == ["https://example.com/v.mp4"])
    }

    // MARK: - Every rich variant nested inside a quote becomes a segment

    /// A block-quote containing a multi-image media block, a standalone
    /// event-ref, and a placeholder — all three rich variants must flush as
    /// their own segments from inside the quote (none dropped).
    @Test func blockQuoteWithMultiImageEventRefPlaceholderFlushesAllRichSegments() {
        let json = """
        {
          "roots": [5],
          "nodes": [
            {"kind":"media","urls":["https://e.com/1.png","https://e.com/2.png"],"media_kind":"Image"},
            {"kind":"event_ref","uri":{"uri":"nostr:nevent1abc","kind":"event","primary_id":"abc","relays":[]}},
            {"kind":"paragraph","children":[1]},
            {"kind":"placeholder","reason":"unresolved_uri"},
            {"kind":"text","text":"tail"},
            {"kind":"block_quote","children":[0,2,3,4]}
          ]
        }
        """
        let output = render(json)

        // Multi-image media → one image segment PER url (both, not just first).
        #expect(imageSegments(output).count == 2)
        // Standalone event-ref paragraph inside the quote → entity card segment.
        #expect(entitySegmentCount(output) == 1)
        // Placeholder inside the quote → placeholder segment.
        #expect(placeholderSegmentCount(output) == 1)
        // Trailing prose still selectable.
        #expect(allText(output).contains("tail"))
    }

    // MARK: - Document order is preserved across nested rich blocks

    /// A quote with text, then an image, then more text must yield segments in
    /// order: text("head") , image , text("tail").
    @Test func nestedRichBlockPreservesDocumentOrder() {
        let json = """
        {
          "roots": [5],
          "nodes": [
            {"kind":"text","text":"head"},
            {"kind":"paragraph","children":[0]},
            {"kind":"image","alt":"","title":null,"src":"https://e.com/x.png"},
            {"kind":"text","text":"tail"},
            {"kind":"paragraph","children":[3]},
            {"kind":"block_quote","children":[1,2,4]}
          ]
        }
        """
        let output = render(json)
        // Find positions.
        var sawHead = false
        var sawImageAfterHead = false
        var sawTailAfterImage = false
        for segment in output.segments {
            switch segment {
            case .text(let attr):
                if attr.string.contains("head") { sawHead = true }
                if attr.string.contains("tail") && sawImageAfterHead { sawTailAfterImage = true }
            case .image:
                if sawHead { sawImageAfterHead = true }
            default:
                break
            }
        }
        #expect(sawHead)
        #expect(sawImageAfterHead)
        #expect(sawTailAfterImage)
    }
}
