import Foundation
import UIKit

/// Renders the article reading body from the **nmp `content_tree`** (the kernel
/// `KernelArticleReaderSnapshot.contentTreeJson`, decoded into the vendored nmp
/// `ContentTreeWire`) into the SAME `MarkdownRenderer.Output` shape the native
/// select-to-highlight reader consumes.
///
/// Locked decision #2: the reading body is a NATIVE Swift
/// select-to-highlight / overlay layer ON TOP OF nmp `content_tree` rendering.
/// `NostrContentView` uses SwiftUI `Text` concatenation, which has no native
/// text selection. The proven select-to-highlight surface is `ArticleBodyView`
/// (a `UITextView` over an `NSAttributedString` with a custom Edit Menu →
/// `onPublishHighlight`). So the body is a **hybrid**: prose blocks
/// (paragraph / heading / list / block-quote / rule / code) flatten into
/// selectable `NSAttributedString` text segments, while rich non-text blocks
/// (images, video / audio media, `nostr:` entity refs, placeholders) flush as
/// their own segments that the reader renders with the same SwiftUI surfaces
/// `NostrContentView` uses — composed in document order. This gives full
/// `content_tree` rendering fidelity (#22 HIGH) without losing selection on the
/// prose.
///
/// Fidelity contract vs. `NostrContentView` (every wire-node variant is
/// rendered, never silently dropped):
///   • text / emphasis / strong / inlineCode / link / url / hashtag / softBreak
///     / hardBreak / emoji / invoice → selectable prose runs.
///   • heading / paragraph / blockQuote / list / rule → selectable prose blocks.
///   • codeBlock → selectable prose block WITH its language header preserved.
///   • image (standalone) AND every URL of a `media(.image)` block → one
///     `.image` segment each (ALL images, not just the first).
///   • media(.video) / media(.audio) → `.media` segment → reader renders the
///     native `NostrContentView` video player / audio row.
///   • mention (npub / nprofile) → inline tappable `@name` run in the prose.
///   • eventRef (note / nevent / naddr) standalone → `.nostrEntity` segment →
///     reader renders the resolving `NostrEntityCard`. Inline (mid-paragraph)
///     event refs render as a compact chip run.
///   • placeholder → `.placeholder` segment → reader renders the chip.
///
/// The output is the identical `MarkdownRenderer.Output` (segments + highlight
/// overlay) so `ReaderScroll` / `ArticleBodyView` keep their consumption
/// contract; the reader's `bodySegments` switch grows the rich-block cases.
enum ContentTreeBodyRenderer {
    /// Decode `contentTreeJson` (the kernel snapshot field) into a
    /// `ContentTreeWire`. Returns `nil` for the empty cold-start window or a
    /// decode failure (D6 — Swift shows nothing until the document loads, same
    /// as the bespoke empty-body window).
    static func decodeTree(json: String) -> ContentTreeWire? {
        guard !json.isEmpty, let data = json.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(ContentTreeWire.self, from: data)
    }

    /// Flatten a decoded nmp `content_tree` into the native reader body. Pure
    /// function — safe to call off the main thread (`UIFont` / `NSParagraphStyle`
    /// are thread-safe for construction), matching `MarkdownRenderer.render`.
    ///
    /// `highlights` are overlaid by quote-text match (same strategy as the
    /// bespoke path) so existing kind:9802 highlights paint as overlay marks on
    /// the rendered body. Mentions/profile entities resolve via `profileNames`.
    ///
    /// `resolveEntity` converts a wire entity URI into the app's resolving
    /// `NostrEntityRef` (the kernel `standaloneNostrEntity` decode); standalone
    /// event refs become `.nostrEntity` card segments. The closure is
    /// `nonisolated` on `SafeHighlighterCore`, so it is safe to call here off
    /// the main actor. When it returns `nil` (undecodable URI) the ref falls
    /// back to a visible inline chip — never to empty.
    static func render(
        tree: ContentTreeWire,
        highlights: [HighlightRecord],
        accent: UIColor,
        tint: UIColor,
        ink: UIColor,
        muted: UIColor,
        bodyPointSize: CGFloat = 18,
        highlightContent: @Sendable (HighlightRecord) -> HighlightDetailContentProjection,
        profileNames: [String: String] = [:],
        resolveEntity: (@Sendable (String) -> NostrEntityRef?)? = nil
    ) -> MarkdownRenderer.Output {
        let walker = TreeWalker(
            tree: tree,
            accent: accent,
            ink: ink,
            muted: muted,
            bodyPointSize: bodyPointSize,
            profileNames: profileNames,
            resolveEntity: resolveEntity
        )
        let rawSegments = walker.walk()

        // Overlay highlights on each text segment — identical strategy to
        // `MarkdownRenderer.render`: a highlight is applied to whichever segment
        // contains the matching flattened text run; unmatched highlights drop.
        var highlightsById: [String: HighlightRecord] = [:]
        let segments: [MarkdownRenderer.BodySegment] = rawSegments.map { segment in
            guard case .text(let attrStr) = segment else { return segment }
            let mutable = attrStr.mutableCopy() as! NSMutableAttributedString
            for highlight in highlights {
                let quote = highlightContent(highlight).quoteText
                guard !quote.isEmpty, quote.count >= 4 else { continue }
                let plain = mutable.string
                if let range = plain.range(of: quote) {
                    let nsRange = NSRange(range, in: plain)
                    mutable.addAttribute(MarkdownRenderer.highlightAttribute, value: highlight.eventId, range: nsRange)
                    mutable.addAttribute(.backgroundColor, value: tint.withAlphaComponent(0.35), range: nsRange)
                    highlightsById[highlight.eventId] = highlight
                }
            }
            return .text(mutable)
        }

        // The nmp `content_tree` (CommonMark) does not model `[^id]` footnotes —
        // those were a bespoke pre-processor concern outside the kernel data
        // model. So the migrated body carries no footnote block / anchors; the
        // reader's footnote affordance degrades to a no-op (decision #2: support
        // footnotes only if the data model has them — it does not).
        return MarkdownRenderer.Output(
            segments: segments,
            footnotes: NSAttributedString(),
            highlightsById: highlightsById,
            footnoteAnchors: [:]
        )
    }
}

// MARK: - Tree walker

/// Walks the flat `ContentTreeWire` arena (`roots` → `nodes`) and emits the
/// `[MarkdownRenderer.BodySegment]` hybrid the reader consumes. Prose blocks
/// accumulate into a running selectable `NSMutableAttributedString`; rich
/// non-text blocks (images, video/audio media, standalone `nostr:` event refs,
/// placeholders) flush as their own segments — preserving document order so the
/// reader composes them in place.
private struct TreeWalker {
    let tree: ContentTreeWire
    let accent: UIColor
    let ink: UIColor
    let muted: UIColor
    let bodyPointSize: CGFloat
    let profileNames: [String: String]
    let resolveEntity: (@Sendable (String) -> NostrEntityRef?)?

    // Cached fonts — mirror the bespoke `BodyWalker` serif styling so the
    // migrated body reads identically.
    private var serif: UIFont {
        UIFont(
            descriptor: UIFontDescriptor.preferredFontDescriptor(withTextStyle: .body).withDesign(.serif)
                ?? UIFontDescriptor.preferredFontDescriptor(withTextStyle: .body),
            size: bodyPointSize
        )
    }
    private var serifItalic: UIFont {
        let d = UIFontDescriptor.preferredFontDescriptor(withTextStyle: .body)
            .withDesign(.serif)?
            .withSymbolicTraits(.traitItalic)
            ?? UIFontDescriptor.preferredFontDescriptor(withTextStyle: .body)
        return UIFont(descriptor: d, size: bodyPointSize)
    }
    private var serifBold: UIFont {
        let d = UIFontDescriptor.preferredFontDescriptor(withTextStyle: .body)
            .withDesign(.serif)?
            .withSymbolicTraits(.traitBold)
            ?? UIFontDescriptor.preferredFontDescriptor(withTextStyle: .body)
        return UIFont(descriptor: d, size: bodyPointSize)
    }
    private var mono: UIFont { UIFont.monospacedSystemFont(ofSize: bodyPointSize - 2, weight: .regular) }
    private var monoSmall: UIFont { UIFont.monospacedSystemFont(ofSize: max(11, bodyPointSize - 6), weight: .semibold) }

    func walk() -> [MarkdownRenderer.BodySegment] {
        // The emitter is the single recursive surface. Block containers
        // (blockQuote / list-item / any future container) recurse through
        // `emitBlock`, which is what makes the renderer fully block-aware at
        // arbitrary depth (#22). Prose blocks accumulate into a running
        // selectable `NSMutableAttributedString`; rich blocks (image / media /
        // standalone event-ref card / placeholder) FLUSH the text and append
        // their own segment in document order — they cannot live inside an
        // `NSAttributedString`, so a depth-recursive append-into-text scheme
        // would drop them (the original bug). Driving the recursion from the
        // segment layer preserves document order across nested rich blocks.
        var emitter = SegmentEmitter()
        for root in tree.roots {
            guard let node = tree.node(at: root) else { continue }
            emitBlock(node, into: &emitter, indent: 0)
        }
        emitter.flush()
        return emitter.segments
    }

    /// Running accumulator for the hybrid output: a selectable prose buffer plus
    /// the flushed rich-block segments, kept in document order.
    private struct SegmentEmitter {
        var segments: [MarkdownRenderer.BodySegment] = []
        var currentText = NSMutableAttributedString()

        /// Append prose (heading / paragraph / code block / rule / list rows /
        /// quote prose) to the running selectable buffer.
        mutating func appendProse(_ attr: NSAttributedString) {
            currentText.append(attr)
        }

        /// Flush the running prose buffer, then append a rich segment so it
        /// interleaves in document order with the surrounding prose.
        mutating func appendRich(_ segment: MarkdownRenderer.BodySegment) {
            flush()
            segments.append(segment)
        }

        mutating func flush() {
            if currentText.length > 0 {
                segments.append(.text(currentText))
                currentText = NSMutableAttributedString()
            }
        }
    }

    // MARK: - Recursive block emission

    /// Recursively emit ANY block node into the segment stream. This is the
    /// convergence point: every block variant renders at every depth, whether
    /// it sits at a root, inside a block-quote, or inside a list item.
    ///
    /// `indent` is the nesting depth (in container levels) used to indent prose
    /// produced beneath block-quotes / list items so nested structure reads
    /// visually. Rich segments are depth-agnostic (the reader renders them as
    /// full-width slices) but still flush in correct document order.
    private func emitBlock(_ node: NostrWireNode, into emitter: inout SegmentEmitter, indent: CGFloat) {
        switch node {
        // — Containers: recurse into their block children. —
        case .blockQuote(let children):
            for child in children {
                guard let childNode = tree.node(at: child) else { continue }
                emitBlock(childNode, into: &emitter, indent: indent + 1, quote: true)
            }
        case .list(let orderedStart, let items):
            for (offset, itemChildren) in items.enumerated() {
                let bullet: String
                if let orderedStart {
                    bullet = "\(orderedStart + UInt64(offset)). "
                } else {
                    bullet = "•  "
                }
                emitListItem(bullet: bullet, children: itemChildren, into: &emitter, indent: indent + 1)
            }

        // — Rich blocks: their own segment (flushes prose first). —
        case .image(let alt, _, let src):
            if let src, let url = URL(string: src) {
                emitter.appendRich(.image(url: url, alt: alt))
            } else {
                emitter.appendProse(indentProse(renderBlock(node), indent: indent))
            }
        case .media(let urls, let kind):
            switch kind {
            case .image:
                // Emit EVERY image URL as its own segment (not just the first).
                let parsed = urls.compactMap(URL.init(string:))
                if parsed.isEmpty {
                    emitter.appendProse(indentProse(renderBlock(node), indent: indent))
                } else {
                    for url in parsed { emitter.appendRich(.image(url: url, alt: "")) }
                }
            case .video, .audio:
                emitter.appendRich(.media(urls: urls, kind: kind))
            }
        case .eventRef(let uri):
            if let ref = resolveEntity?(uri.uri) {
                emitter.appendRich(.nostrEntity(ref))
            } else {
                let s = NSMutableAttributedString(attributedString: renderInlineNode(nodeOrSelf: node))
                s.append(NSAttributedString(string: "\n\n", attributes: [.font: serif]))
                emitter.appendProse(indentProse(s, indent: indent))
            }
        case .placeholder(let reason):
            emitter.appendRich(.placeholder(reason: reason))

        // — Paragraph: a lone event-ref paragraph promotes to a card. —
        case .paragraph(let children):
            if children.count == 1,
               let only = tree.node(at: children[0]),
               case .eventRef(let uri) = only,
               let ref = resolveEntity?(uri.uri) {
                emitter.appendRich(.nostrEntity(ref))
            } else {
                emitter.appendProse(indentProse(renderBlock(node), indent: indent))
            }

        // — Prose blocks + inline-as-block: straight into the prose buffer. —
        default:
            emitter.appendProse(indentProse(renderBlock(node), indent: indent))
        }
    }

    /// Block-quote-flavoured recursion. A quote's block children render with the
    /// quote's muted-italic styling AND recurse (so a nested list / code block /
    /// rule / image inside a quote is no longer dropped). Rich children still
    /// flush as their own segments.
    private func emitBlock(_ node: NostrWireNode, into emitter: inout SegmentEmitter, indent: CGFloat, quote: Bool) {
        guard quote else { emitBlock(node, into: &emitter, indent: indent); return }
        switch node {
        case .blockQuote, .list, .image, .media, .eventRef, .placeholder:
            // Containers / rich blocks: defer to the canonical emitter so
            // nesting + rich-segment flushing behave identically inside quotes.
            emitBlock(node, into: &emitter, indent: indent)
        case .paragraph(let children):
            if children.count == 1,
               let only = tree.node(at: children[0]),
               case .eventRef(let uri) = only,
               let ref = resolveEntity?(uri.uri) {
                emitter.appendRich(.nostrEntity(ref))
            } else {
                emitter.appendProse(quoteStyled(renderInlines(children), indent: indent))
            }
        case .heading(_, let children):
            // A heading inside a quote keeps the quote's muted prose styling.
            emitter.appendProse(quoteStyled(renderInlines(children), indent: indent))
        default:
            // Code block / rule / inline-as-block: render via the block path,
            // indented to the quote depth.
            emitter.appendProse(indentProse(renderBlock(node), indent: indent))
        }
    }

    /// Emit one list item's block children. The item's leading text (raw inline
    /// children, plus the inline content of a leading paragraph) renders as the
    /// bullet row; any further BLOCK child (nested list / code block / rule /
    /// block-quote / image / media / extra paragraphs) recurses through
    /// `emitBlock`, indented one further level — so nested blocks inside a list
    /// item are never dropped.
    private func emitListItem(
        bullet: String,
        children: [UInt32],
        into emitter: inout SegmentEmitter,
        indent: CGFloat
    ) {
        // Split the item's children into a leading inline run (the bullet's own
        // text) and trailing block children (which recurse). CommonMark list
        // items are "loose" — children may be paragraphs or raw inlines.
        var bulletInline: [UInt32] = []
        var blocks: [NostrWireNode] = []
        var sawBlock = false
        for child in children {
            guard let childNode = tree.node(at: child) else { continue }
            if isBlockNode(childNode) {
                // A leading paragraph (before any other block) folds its inline
                // content into the bullet row so the bullet isn't empty above
                // its own text. Anything after that recurses as a nested block.
                if !sawBlock, case .paragraph(let pChildren) = childNode,
                   !isStandaloneEventRefParagraph(pChildren) {
                    bulletInline.append(contentsOf: pChildren)
                } else {
                    blocks.append(childNode)
                    sawBlock = true
                }
            } else {
                if sawBlock {
                    // A stray inline after a block — wrap it so it isn't lost.
                    blocks.append(.paragraph(children: [child]))
                } else {
                    bulletInline.append(child)
                }
            }
        }

        // The bullet row: bullet glyph + the item's inline content.
        let itemBuf = NSMutableAttributedString(
            string: bullet,
            attributes: [.font: serifBold, .foregroundColor: accent]
        )
        itemBuf.append(renderInlines(bulletInline))
        itemBuf.append(NSAttributedString(string: "\n"))
        let p = NSMutableParagraphStyle()
        p.headIndent = 24 + indent * 16
        p.firstLineHeadIndent = max(0, (indent - 1) * 16)
        p.paragraphSpacing = 6
        p.lineHeightMultiple = 1.35
        itemBuf.addAttribute(.paragraphStyle, value: p, range: NSRange(location: 0, length: itemBuf.length))
        emitter.appendProse(itemBuf)

        // Recurse into the item's remaining block children at one deeper indent.
        for block in blocks {
            emitBlock(block, into: &emitter, indent: indent)
        }
    }

    /// True if a paragraph's children are exactly one resolvable event-ref —
    /// such a paragraph promotes to a standalone card, not bullet text.
    private func isStandaloneEventRefParagraph(_ children: [UInt32]) -> Bool {
        guard children.count == 1, let only = tree.node(at: children[0]),
              case .eventRef = only else { return false }
        return true
    }

    /// True for any container/leaf BLOCK-level node (vs. an inline node).
    private func isBlockNode(_ node: NostrWireNode) -> Bool {
        switch node {
        case .paragraph, .heading, .blockQuote, .codeBlock, .list, .rule,
             .image, .media, .placeholder:
            return true
        case .eventRef:
            // An event-ref is block-ish only when it can resolve to a card;
            // otherwise it renders inline as a chip.
            return false
        case .text, .mention, .hashtag, .url, .emoji, .invoice, .emphasis,
             .strong, .inlineCode, .link, .softBreak, .hardBreak:
            return false
        }
    }

    /// Indent prose produced beneath a container by `indent` levels.
    private func indentProse(_ attr: NSAttributedString, indent: CGFloat) -> NSAttributedString {
        guard indent > 0, attr.length > 0 else { return attr }
        let out = NSMutableAttributedString(attributedString: attr)
        out.enumerateAttribute(.paragraphStyle, in: NSRange(location: 0, length: out.length)) { value, range, _ in
            let base = (value as? NSParagraphStyle).flatMap { $0.mutableCopy() as? NSMutableParagraphStyle }
                ?? NSMutableParagraphStyle()
            base.headIndent += indent * 16
            base.firstLineHeadIndent += indent * 16
            out.addAttribute(.paragraphStyle, value: base, range: range)
        }
        return out
    }

    /// Quote-styled prose (muted italic, indented) used for block-quote prose
    /// children at any depth.
    private func quoteStyled(_ inner: NSAttributedString, indent: CGFloat) -> NSAttributedString {
        let out = NSMutableAttributedString(attributedString: inner)
        let p = NSMutableParagraphStyle()
        p.headIndent = 18 + indent * 16
        p.firstLineHeadIndent = 18 + indent * 16
        p.paragraphSpacingBefore = 8
        p.paragraphSpacing = 10
        p.lineHeightMultiple = 1.4
        out.addAttributes(
            [.foregroundColor: muted, .paragraphStyle: p, .font: serifItalic],
            range: NSRange(location: 0, length: out.length)
        )
        out.append(NSAttributedString(string: "\n\n", attributes: [.font: serifItalic]))
        return out
    }

    // MARK: - Block rendering (leaf / prose blocks)

    private func renderBlock(_ node: NostrWireNode) -> NSAttributedString {
        switch node {
        case .heading(let level, let children):
            return renderHeading(level: level, children: children)
        case .paragraph(let children):
            let inner = renderInlines(children)
            let s = NSMutableAttributedString(attributedString: inner)
            s.addAttribute(.paragraphStyle, value: paragraphStyle(), range: NSRange(location: 0, length: s.length))
            s.append(NSAttributedString(string: "\n\n", attributes: [.font: serif]))
            return s
        case .blockQuote, .list:
            // Containers are normally intercepted by `emitBlock` (which keeps
            // rich nested children as their own segments). This path is only a
            // defensive flatten for a container reached as a non-root block; it
            // recurses through the SAME emitter so no nested block is dropped,
            // then concatenates the result (rich segments become visible chips).
            return flattenContainer(node)
        case .codeBlock(let info, let body):
            return renderCodeBlock(info: info, body: body)
        case .rule:
            return NSAttributedString(
                string: "\n———\n\n",
                attributes: [
                    .font: serif,
                    .foregroundColor: muted,
                    .paragraphStyle: centeredParagraphStyle()
                ]
            )
        case .image(let alt, _, _):
            // Inline image inside mixed content — render the alt text.
            return NSAttributedString(
                string: alt.isEmpty ? "[image]" : "[\(alt)]",
                attributes: [.font: serifItalic, .foregroundColor: muted]
            )
        case .media(let urls, _):
            // Non-image media inside mixed content — render the first URL plain.
            let label = urls.first ?? ""
            return NSAttributedString(
                string: label,
                attributes: [.font: serif, .foregroundColor: accent]
            )
        // Inline-level nodes that can appear directly under roots collapse to
        // their inline rendering wrapped as a paragraph.
        case .text, .mention, .eventRef, .hashtag, .url, .emoji, .invoice,
             .emphasis, .strong, .inlineCode, .link, .softBreak, .hardBreak,
             .placeholder:
            let s = NSMutableAttributedString(attributedString: renderInlineNode(nodeOrSelf: node))
            s.append(NSAttributedString(string: "\n\n", attributes: [.font: serif]))
            return s
        }
    }

    private func renderHeading(level: UInt8, children: [UInt32]) -> NSAttributedString {
        let base = UIFontDescriptor.preferredFontDescriptor(withTextStyle: .body)
            .withDesign(.serif) ?? UIFontDescriptor.preferredFontDescriptor(withTextStyle: .body)
        let pointSize: CGFloat
        switch level {
        case 1: pointSize = bodyPointSize + 14
        case 2: pointSize = bodyPointSize + 10
        case 3: pointSize = bodyPointSize + 6
        case 4: pointSize = bodyPointSize + 3
        default: pointSize = bodyPointSize + 1
        }
        let bold = base.withSymbolicTraits(.traitBold) ?? base
        let font = UIFont(descriptor: bold, size: pointSize)

        let para = NSMutableParagraphStyle()
        para.paragraphSpacing = 10
        para.paragraphSpacingBefore = 18
        para.lineHeightMultiple = 1.1

        let inner = renderInlines(children)
        let out = NSMutableAttributedString(attributedString: inner)
        out.addAttributes(
            [.font: font, .foregroundColor: ink, .paragraphStyle: para],
            range: NSRange(location: 0, length: out.length)
        )
        out.append(NSAttributedString(string: "\n\n", attributes: [.font: font]))
        return out
    }

    /// Flatten a container (block-quote / list) to an attributed string via the
    /// SAME recursive emitter used at the top level, so nested blocks are never
    /// dropped here either. Any rich child (image / media / card / placeholder)
    /// that would normally be its own segment is rendered as a visible chip so
    /// it survives the attributed-string-only context.
    private func flattenContainer(_ node: NostrWireNode) -> NSAttributedString {
        var emitter = SegmentEmitter()
        emitBlock(node, into: &emitter, indent: 0)
        emitter.flush()
        let out = NSMutableAttributedString()
        for segment in emitter.segments {
            switch segment {
            case .text(let attr):
                out.append(attr)
            case .image(_, let alt):
                out.append(NSAttributedString(
                    string: (alt.isEmpty ? "[image]" : "[\(alt)]") + "\n",
                    attributes: [.font: serifItalic, .foregroundColor: muted]
                ))
            case .media(let urls, let kind):
                out.append(NSAttributedString(
                    string: "[\(kind.rawValue.lowercased()): \(urls.first ?? "")]\n",
                    attributes: [.font: serif, .foregroundColor: accent]
                ))
            case .nostrEntity:
                out.append(NSAttributedString(
                    string: "[embedded note]\n",
                    attributes: [.font: mono, .foregroundColor: accent]
                ))
            case .placeholder:
                out.append(NSAttributedString(
                    string: "[content unavailable]\n",
                    attributes: [.font: serifItalic, .foregroundColor: muted]
                ))
            }
        }
        return out
    }

    /// Code block stays selectable prose so users can copy code. The language
    /// (`info` string, e.g. `swift`) is preserved as a small monospace header
    /// row above the body — the prior path dropped it entirely (#22).
    private func renderCodeBlock(info: String?, body: String) -> NSAttributedString {
        let out = NSMutableAttributedString()
        if let info, !info.isEmpty {
            let headerPara = NSMutableParagraphStyle()
            headerPara.paragraphSpacingBefore = 6
            headerPara.lineHeightMultiple = 1.1
            out.append(NSAttributedString(
                string: info + "\n",
                attributes: [
                    .font: monoSmall,
                    .foregroundColor: muted,
                    .paragraphStyle: headerPara,
                    .backgroundColor: muted.withAlphaComponent(0.08)
                ]
            ))
        }
        let p = NSMutableParagraphStyle()
        p.paragraphSpacing = 14
        p.paragraphSpacingBefore = (info?.isEmpty ?? true) ? 6 : 0
        p.lineHeightMultiple = 1.25
        out.append(NSAttributedString(
            string: body + "\n",
            attributes: [
                .font: mono,
                .foregroundColor: ink,
                .paragraphStyle: p,
                .backgroundColor: muted.withAlphaComponent(0.08)
            ]
        ))
        return out
    }

    // MARK: - Inline rendering

    private func renderInlines(_ indices: [UInt32]) -> NSAttributedString {
        let out = NSMutableAttributedString()
        for idx in indices {
            out.append(renderInline(idx))
        }
        return out
    }

    private func renderInline(_ index: UInt32) -> NSAttributedString {
        guard let node = tree.node(at: index) else { return NSAttributedString() }
        return renderInlineNode(nodeOrSelf: node)
    }

    private func renderInlineNode(nodeOrSelf node: NostrWireNode) -> NSAttributedString {
        switch node {
        case .text(let value):
            return NSAttributedString(string: value, attributes: [.font: serif, .foregroundColor: ink])
        case .softBreak:
            return NSAttributedString(string: " ", attributes: [.font: serif])
        case .hardBreak:
            return NSAttributedString(string: "\n", attributes: [.font: serif])
        case .emphasis(let children):
            let out = NSMutableAttributedString(attributedString: renderInlines(children))
            out.addAttribute(.font, value: serifItalic, range: NSRange(location: 0, length: out.length))
            return out
        case .strong(let children):
            let out = NSMutableAttributedString(attributedString: renderInlines(children))
            out.addAttribute(.font, value: serifBold, range: NSRange(location: 0, length: out.length))
            return out
        case .inlineCode(let value):
            return NSAttributedString(
                string: value,
                attributes: [.font: mono, .backgroundColor: muted.withAlphaComponent(0.15), .foregroundColor: ink]
            )
        case .link(let children, let href):
            let out = NSMutableAttributedString(attributedString: renderInlines(children))
            if let href, let url = URL(string: href) {
                out.addAttributes(
                    [
                        .link: url,
                        .foregroundColor: accent,
                        .underlineStyle: NSUnderlineStyle.single.rawValue,
                        .underlineColor: accent.withAlphaComponent(0.4)
                    ],
                    range: NSRange(location: 0, length: out.length)
                )
            }
            return out
        case .url(let value):
            let out = NSMutableAttributedString(
                string: value,
                attributes: [.font: serif, .foregroundColor: accent]
            )
            if let url = URL(string: value) {
                out.addAttributes(
                    [.link: url, .underlineStyle: NSUnderlineStyle.single.rawValue],
                    range: NSRange(location: 0, length: out.length)
                )
            }
            return out
        case .hashtag(let tag):
            return NSAttributedString(
                string: "#\(tag)",
                attributes: [.font: serifBold, .foregroundColor: accent]
            )
        case .mention(let uri):
            // Render an inline profile mention as a tappable `@name` run routed
            // to the existing `highlighter://profile/<pubkey>` handler in
            // `ArticleBodyView` (preserves the bespoke profile-tap behaviour).
            let pubkey = uri.primaryId
            let label = profileNames[pubkey].map { $0.hasPrefix("@") ? $0 : "@\($0)" }
                ?? "@\(NostrContentView.defaultMentionLabel(uri))"
            var attrs: [NSAttributedString.Key: Any] = [.font: serifBold, .foregroundColor: accent]
            if !pubkey.isEmpty, let url = URL(string: "highlighter://profile/\(pubkey)") {
                attrs[.link] = url
            }
            return NSAttributedString(string: label, attributes: attrs)
        case .eventRef(let uri):
            // Inline (mid-paragraph) event ref → compact chip label. Standalone
            // event refs are promoted to resolving cards by `walk()`.
            return NSAttributedString(
                string: "↩ \(shortEntity(uri.primaryId))",
                attributes: [.font: mono, .foregroundColor: accent]
            )
        case .emoji(let shortcode, _):
            return NSAttributedString(string: ":\(shortcode):", attributes: [.font: serif, .foregroundColor: ink])
        case .invoice:
            return NSAttributedString(string: "⚡ invoice", attributes: [.font: serif, .foregroundColor: accent])
        case .image(let alt, _, _):
            return NSAttributedString(
                string: alt.isEmpty ? "[image]" : "[\(alt)]",
                attributes: [.font: serifItalic, .foregroundColor: muted]
            )
        case .placeholder:
            // A placeholder reached inline (shouldn't normally happen — they are
            // promoted to their own segment by `walk()`). Render a visible chip
            // label rather than collapsing to empty.
            return NSAttributedString(
                string: "[content unavailable]",
                attributes: [.font: serifItalic, .foregroundColor: muted]
            )
        // Block-level nodes should not appear inside an inline reduce (block
        // children of containers go through the block path `emitBlock`). If one
        // is ever reached inline, render a visible representation rather than
        // collapsing to empty — no legal node silently disappears (#22).
        case .paragraph(let children),
             .heading(_, let children),
             .blockQuote(let children):
            return renderInlines(children)
        case .list(_, let items):
            // Flatten nested-list items to comma-joined inline text.
            let out = NSMutableAttributedString()
            for (i, item) in items.enumerated() {
                if i > 0 { out.append(NSAttributedString(string: "  •  ", attributes: [.font: serif, .foregroundColor: accent])) }
                out.append(renderInlines(item))
            }
            return out
        case .codeBlock(let info, let body):
            let label = (info?.isEmpty == false) ? "\(info!): " : ""
            return NSAttributedString(
                string: label + body,
                attributes: [.font: mono, .foregroundColor: ink, .backgroundColor: muted.withAlphaComponent(0.15)]
            )
        case .rule:
            return NSAttributedString(string: " — ", attributes: [.font: serif, .foregroundColor: muted])
        case .media(let urls, let kind):
            return NSAttributedString(
                string: "[\(kind.rawValue.lowercased()): \(urls.first ?? "")]",
                attributes: [.font: serif, .foregroundColor: accent]
            )
        }
    }

    private func shortEntity(_ value: String) -> String {
        guard value.count > 12 else { return value }
        return "\(value.prefix(8))…\(value.suffix(4))"
    }

    // MARK: - Paragraph styles

    private func paragraphStyle() -> NSParagraphStyle {
        let p = NSMutableParagraphStyle()
        p.paragraphSpacing = 4
        p.lineHeightMultiple = 1.45
        return p
    }

    private func centeredParagraphStyle() -> NSParagraphStyle {
        let p = NSMutableParagraphStyle()
        p.alignment = .center
        p.paragraphSpacing = 12
        p.paragraphSpacingBefore = 12
        return p
    }
}
