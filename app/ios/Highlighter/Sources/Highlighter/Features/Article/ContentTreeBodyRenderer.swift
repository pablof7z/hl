import Foundation
import UIKit

/// Renders the article reading body from the **nmp `content_tree`** (the kernel
/// `KernelArticleReaderSnapshot.contentTreeJson`, decoded into the vendored nmp
/// `ContentTreeWire`) into the SAME `MarkdownRenderer.Output` shape the native
/// select-to-highlight reader consumes.
///
/// Why flatten the nmp tree into an `NSAttributedString` rather than render it
/// with `NostrContentView`? Locked decision #2: the reading body is a NATIVE
/// Swift select-to-highlight / overlay / footnotes interaction layer ON TOP OF
/// nmp `content_tree` rendering. `NostrContentView` uses SwiftUI `Text`
/// concatenation, which has no native text selection. The proven select-to-
/// highlight surface is `ArticleBodyView` (a `UITextView` over an
/// `NSAttributedString` with a custom Edit Menu → `onPublishHighlight`). So the
/// migration keeps that overlay machinery intact and only swaps the BODY
/// SOURCE: bespoke NIP-23 markdown (`MarkdownRenderer.render(content:)`) →
/// nmp `content_tree` (this renderer). The kernel still owns the published
/// kind:9802 highlight (D5); the body is raw `content_tree` emitted by the
/// kernel and rendered Swift-side (D1).
///
/// The output is the identical `MarkdownRenderer.Output` (segments + highlight
/// overlay + footnote anchors) so `ReaderScroll` / `ArticleBodyView` need no
/// change to their consumption contract.
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
    static func render(
        tree: ContentTreeWire,
        highlights: [HighlightRecord],
        accent: UIColor,
        tint: UIColor,
        ink: UIColor,
        muted: UIColor,
        bodyPointSize: CGFloat = 18,
        highlightContent: @Sendable (HighlightRecord) -> HighlightDetailContentProjection,
        profileNames: [String: String] = [:]
    ) -> MarkdownRenderer.Output {
        let walker = TreeWalker(
            tree: tree,
            accent: accent,
            ink: ink,
            muted: muted,
            bodyPointSize: bodyPointSize,
            profileNames: profileNames
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

/// Walks the flat `ContentTreeWire` arena (`roots` → `nodes`) and emits the same
/// `[MarkdownRenderer.BodySegment]` the bespoke `BodyWalker` produced. Block
/// nodes accumulate into a running `NSMutableAttributedString`; standalone
/// images and standalone `nostr:` event refs flush as their own segments so the
/// reader renders them as SwiftUI cards (matching the bespoke path).
private struct TreeWalker {
    let tree: ContentTreeWire
    let accent: UIColor
    let ink: UIColor
    let muted: UIColor
    let bodyPointSize: CGFloat
    let profileNames: [String: String]

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

    func walk() -> [MarkdownRenderer.BodySegment] {
        var segments: [MarkdownRenderer.BodySegment] = []
        var currentText = NSMutableAttributedString()

        func flush() {
            if currentText.length > 0 {
                segments.append(.text(currentText))
                currentText = NSMutableAttributedString()
            }
        }

        for root in tree.roots {
            guard let node = tree.node(at: root) else { continue }
            switch node {
            case .image(let alt, _, let src):
                // Standalone image block → its own segment (SwiftUI card).
                if let src, let url = URL(string: src) {
                    flush()
                    segments.append(.image(url: url, alt: alt))
                } else {
                    currentText.append(renderBlock(node))
                }
            case .media(let urls, let kind) where kind == .image:
                // A media-image block with a single URL → standalone image.
                if let first = urls.first, let url = URL(string: first) {
                    flush()
                    segments.append(.image(url: url, alt: ""))
                } else {
                    currentText.append(renderBlock(node))
                }
            default:
                currentText.append(renderBlock(node))
            }
        }
        flush()
        return segments
    }

    // MARK: - Block rendering

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
        case .blockQuote(let children):
            return renderBlockQuote(children)
        case .codeBlock(_, let body):
            return renderCodeBlock(body)
        case .list(let orderedStart, let items):
            return renderList(orderedStart: orderedStart, items: items)
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

    private func renderList(orderedStart: UInt64?, items: [[UInt32]]) -> NSAttributedString {
        let out = NSMutableAttributedString()
        for (offset, children) in items.enumerated() {
            let bullet: String
            if let orderedStart {
                bullet = "\(orderedStart + UInt64(offset)). "
            } else {
                bullet = "•  "
            }
            let itemBuf = NSMutableAttributedString(
                string: bullet,
                attributes: [.font: serifBold, .foregroundColor: accent]
            )
            itemBuf.append(renderInlines(children))
            itemBuf.append(NSAttributedString(string: "\n"))
            let p = NSMutableParagraphStyle()
            p.headIndent = 24
            p.firstLineHeadIndent = 0
            p.paragraphSpacing = 6
            p.lineHeightMultiple = 1.35
            itemBuf.addAttribute(.paragraphStyle, value: p, range: NSRange(location: 0, length: itemBuf.length))
            out.append(itemBuf)
        }
        out.append(NSAttributedString(string: "\n", attributes: [.font: serif]))
        return out
    }

    private func renderBlockQuote(_ children: [UInt32]) -> NSAttributedString {
        let inner = NSMutableAttributedString(attributedString: renderInlines(children))
        let p = NSMutableParagraphStyle()
        p.headIndent = 18
        p.firstLineHeadIndent = 18
        p.paragraphSpacingBefore = 8
        p.paragraphSpacing = 10
        p.lineHeightMultiple = 1.4
        inner.addAttributes(
            [.foregroundColor: muted, .paragraphStyle: p, .font: serifItalic],
            range: NSRange(location: 0, length: inner.length)
        )
        inner.append(NSAttributedString(string: "\n\n", attributes: [.font: serifItalic]))
        return inner
    }

    private func renderCodeBlock(_ body: String) -> NSAttributedString {
        let p = NSMutableParagraphStyle()
        p.paragraphSpacing = 14
        p.paragraphSpacingBefore = 6
        p.lineHeightMultiple = 1.25
        return NSAttributedString(
            string: body + "\n",
            attributes: [
                .font: mono,
                .foregroundColor: ink,
                .paragraphStyle: p,
                .backgroundColor: muted.withAlphaComponent(0.08)
            ]
        )
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
            // Inline (mid-paragraph) event ref → compact chip label.
            return NSAttributedString(
                string: "[\(shortEntity(uri.primaryId))]",
                attributes: [.font: mono, .foregroundColor: muted]
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
            return NSAttributedString()
        // Block-level nodes should not appear inside an inline reduce; render
        // their flattened children to be safe rather than break concatenation.
        case .paragraph(let children),
             .heading(_, let children),
             .blockQuote(let children):
            return renderInlines(children)
        case .list, .codeBlock, .rule, .media:
            return NSAttributedString()
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
