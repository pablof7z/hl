import Foundation
import UIKit
import Markdown

/// Converts raw NIP-23 markdown into an `NSAttributedString` suitable for a
/// `UITextView`, plus a mapping of highlight-run ranges back to the
/// `HighlightRecord` they came from.
///
/// Footnote syntax (`[^id]` + `[^id]: …`) is pre-processed out of the body
/// before parsing; references are rendered as superscript, tappable runs with
/// a `highlighter://footnote/<id>` URL. Footnote definitions are rendered as
/// a separate attributed string the reader appends below the body.
///
/// Highlight overlay: any `HighlightRecord` whose `quote` matches a range of
/// flattened body text receives an `.highlighterHighlight` custom attribute
/// holding the event id. The reader uses this to resolve taps without a
/// separate hit-test pass.
enum MarkdownRenderer {
    struct Output: @unchecked Sendable {
        let body: NSAttributedString
        let footnotes: NSAttributedString
        /// Keyed by highlight event id so the reader can resolve a tap back
        /// to the record.
        let highlightsById: [String: HighlightRecord]
        /// Footnote number → character range in `body`, so taps on "^[1]"
        /// back-references in footnotes can flash the inline mark.
        let footnoteAnchors: [Int: NSRange]
    }

    /// Marker attribute the UITextView uses to recognize tapped runs. Value
    /// is the highlight event id.
    static let highlightAttribute = NSAttributedString.Key("highlighterHighlight")
    /// Marker attribute for footnote reference targets. Value is the footnote
    /// number (Int) so we can scroll to `.footnote-<n>` after a tap.
    static let footnoteReferenceAttribute = NSAttributedString.Key("highlighterFootnoteRef")
    /// Marker attribute for footnote back-reference targets in the definition
    /// block. Value is the footnote number (Int) so tapping the back-arrow
    /// can flash the inline reference.
    static let footnoteBackAttribute = NSAttributedString.Key("highlighterFootnoteBack")

    /// Render a full article body. Pure function — safe to call off the main
    /// thread (`UIFont` / `NSParagraphStyle` are thread-safe for construction).
    static func render(
        content: String,
        highlights: [HighlightRecord],
        accent: UIColor,
        tint: UIColor,
        ink: UIColor,
        muted: UIColor,
        bodyPointSize: CGFloat = 18
    ) -> Output {
        let preprocessed = FootnotePreprocessor.extract(content)
        let document = Document(parsing: preprocessed.cleanedMarkdown)

        var walker = BodyWalker(
            accent: accent,
            tint: tint,
            ink: ink,
            muted: muted,
            bodyPointSize: bodyPointSize,
            definitionsById: Dictionary(uniqueKeysWithValues: preprocessed.definitions.map { ($0.id, $0) })
        )
        let body = walker.render(document)

        // Overlay highlights on the flattened body after walking. We do this
        // in a single pass over the event list, finding each quote with
        // `range(of:)`; unmatched highlights are silently dropped (mirrors
        // the web behavior).
        let overlaidBody = body.mutableCopy() as! NSMutableAttributedString
        var highlightsById: [String: HighlightRecord] = [:]
        for highlight in highlights {
            let quote = highlight.quote.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !quote.isEmpty, quote.count >= 4 else { continue }
            let plain = overlaidBody.string
            if let range = plain.range(of: quote) {
                let nsRange = NSRange(range, in: plain)
                // Don't overwrite existing attributes wholesale — layer the
                // highlight attribute + background color.
                overlaidBody.addAttribute(highlightAttribute, value: highlight.eventId, range: nsRange)
                overlaidBody.addAttribute(
                    .backgroundColor,
                    value: tint.withAlphaComponent(0.35),
                    range: nsRange
                )
                highlightsById[highlight.eventId] = highlight
            }
        }

        // Footnote definitions — rendered separately as a smaller attributed
        // string. The reader appends this below the body with a divider.
        let footnotes = renderFootnotes(
            preprocessed.definitions,
            accent: accent,
            ink: ink,
            muted: muted,
            bodyPointSize: bodyPointSize
        )

        return Output(
            body: overlaidBody,
            footnotes: footnotes,
            highlightsById: highlightsById,
            footnoteAnchors: walker.footnoteAnchors
        )
    }

    // MARK: - Footnote block rendering

    private static func renderFootnotes(
        _ defs: [FootnotePreprocessor.Definition],
        accent: UIColor,
        ink: UIColor,
        muted: UIColor,
        bodyPointSize: CGFloat
    ) -> NSAttributedString {
        guard !defs.isEmpty else { return NSAttributedString() }

        let out = NSMutableAttributedString()
        let smallSize = max(14, bodyPointSize - 3)

        for def in defs {
            // Leading number + back-arrow.
            let numberPara = NSMutableParagraphStyle()
            numberPara.paragraphSpacing = 10
            numberPara.lineHeightMultiple = 1.3

            let header = NSMutableAttributedString(
                string: "\(def.number). ",
                attributes: [
                    .font: UIFont.systemFont(ofSize: smallSize, weight: .semibold),
                    .foregroundColor: ink,
                    .paragraphStyle: numberPara
                ]
            )
            out.append(header)

            // Body — parse the definition itself as markdown so nested
            // inlines (emphasis, links, code) render too. Reuse BodyWalker
            // with a smaller point size.
            var inner = BodyWalker(
                accent: accent,
                tint: .clear,
                ink: muted,
                muted: muted,
                bodyPointSize: smallSize,
                definitionsById: [:]
            )
            let innerDoc = Document(parsing: def.markdown)
            let innerString = inner.render(innerDoc).mutableCopy() as! NSMutableAttributedString
            // Strip the trailing newline BodyWalker appends after the last
            // block — we want one newline between definitions, not two.
            if innerString.string.hasSuffix("\n\n") {
                innerString.deleteCharacters(in: NSRange(location: innerString.length - 1, length: 1))
            }
            out.append(innerString)

            // Back-arrow — tappable.
            let back = NSAttributedString(
                string: " ↩",
                attributes: [
                    .font: UIFont.systemFont(ofSize: smallSize),
                    .foregroundColor: accent,
                    footnoteBackAttribute: def.number,
                    .link: URL(string: "highlighter://footnote-back/\(def.number)")!
                ]
            )
            out.append(back)
            out.append(NSAttributedString(string: "\n"))
        }

        return out
    }
}

// MARK: - BodyWalker

/// Walks a `swift-markdown` `Document` and emits an `NSAttributedString`.
/// Mutates as it goes; call `render(_:)` once per document.
private struct BodyWalker {
    let accent: UIColor
    let tint: UIColor
    let ink: UIColor
    let muted: UIColor
    let bodyPointSize: CGFloat
    let definitionsById: [String: FootnotePreprocessor.Definition]

    var footnoteAnchors: [Int: NSRange] = [:]

    // Cached fonts — `UIFontMetrics` scaling is handled at the text-view level.
    private var serif: UIFont { UIFont(descriptor: UIFontDescriptor.preferredFontDescriptor(withTextStyle: .body).withDesign(.serif) ?? UIFontDescriptor.preferredFontDescriptor(withTextStyle: .body), size: bodyPointSize) }
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

    mutating func render(_ document: Document) -> NSAttributedString {
        let out = NSMutableAttributedString()
        for child in document.children {
            out.append(renderBlock(child))
        }
        return out
    }

    // MARK: - Block

    private mutating func renderBlock(_ markup: Markup) -> NSAttributedString {
        switch markup {
        case let heading as Heading:
            return renderHeading(heading)
        case let paragraph as Paragraph:
            let inner = renderInlines(paragraph.inlineChildren)
            let s = NSMutableAttributedString(attributedString: inner)
            s.addAttribute(.paragraphStyle, value: paragraphStyle(), range: NSRange(location: 0, length: s.length))
            s.append(NSAttributedString(string: "\n\n", attributes: [.font: serif]))
            return s
        case let list as UnorderedList:
            return renderList(list, ordered: false)
        case let list as OrderedList:
            return renderList(list, ordered: true)
        case let quote as BlockQuote:
            return renderBlockQuote(quote)
        case let code as CodeBlock:
            return renderCodeBlock(code)
        case is ThematicBreak:
            return NSAttributedString(
                string: "\n———\n\n",
                attributes: [
                    .font: serif,
                    .foregroundColor: muted,
                    .paragraphStyle: centeredParagraphStyle()
                ]
            )
        case let html as HTMLBlock:
            // Render raw HTML as code-block-ish monospaced — we don't parse
            // arbitrary HTML inline. Rare in NIP-23 content.
            return NSAttributedString(
                string: html.rawHTML + "\n\n",
                attributes: [.font: mono, .foregroundColor: muted]
            )
        default:
            // Unknown block: fall through to rendering its children inline.
            let out = NSMutableAttributedString()
            for child in markup.children {
                out.append(renderBlock(child))
            }
            return out
        }
    }

    private mutating func renderHeading(_ heading: Heading) -> NSAttributedString {
        let base = UIFontDescriptor.preferredFontDescriptor(withTextStyle: .body)
            .withDesign(.serif) ?? UIFontDescriptor.preferredFontDescriptor(withTextStyle: .body)
        let pointSize: CGFloat
        switch heading.level {
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

        let inner = renderInlines(heading.inlineChildren)
        let out = NSMutableAttributedString(attributedString: inner)
        out.addAttributes(
            [
                .font: font,
                .foregroundColor: ink,
                .paragraphStyle: para
            ],
            range: NSRange(location: 0, length: out.length)
        )
        out.append(NSAttributedString(string: "\n\n", attributes: [.font: font]))
        return out
    }

    private mutating func renderList(_ list: Markup, ordered: Bool) -> NSAttributedString {
        let out = NSMutableAttributedString()
        var idx = 1
        for child in list.children {
            guard let item = child as? ListItem else { continue }
            let bullet: String = ordered ? "\(idx). " : "•  "
            idx += 1

            let itemBuf = NSMutableAttributedString(
                string: bullet,
                attributes: [.font: serifBold, .foregroundColor: accent]
            )
            for sub in item.children {
                // Inside a list item, paragraphs render as inline lines so
                // the bullet stays on the same visual row.
                if let para = sub as? Paragraph {
                    let inner = renderInlines(para.inlineChildren)
                    itemBuf.append(inner)
                } else {
                    itemBuf.append(renderBlock(sub))
                }
            }
            itemBuf.append(NSAttributedString(string: "\n"))
            // Apply list-friendly paragraph style (indent).
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

    private mutating func renderBlockQuote(_ quote: BlockQuote) -> NSAttributedString {
        let inner = NSMutableAttributedString()
        for child in quote.children {
            inner.append(renderBlock(child))
        }
        let p = NSMutableParagraphStyle()
        p.headIndent = 18
        p.firstLineHeadIndent = 18
        p.paragraphSpacingBefore = 8
        p.paragraphSpacing = 10
        p.lineHeightMultiple = 1.4
        inner.addAttributes(
            [
                .foregroundColor: muted,
                .paragraphStyle: p,
                .font: serifItalic
            ],
            range: NSRange(location: 0, length: inner.length)
        )
        return inner
    }

    private mutating func renderCodeBlock(_ code: CodeBlock) -> NSAttributedString {
        let p = NSMutableParagraphStyle()
        p.paragraphSpacing = 14
        p.paragraphSpacingBefore = 6
        p.lineHeightMultiple = 1.25
        return NSAttributedString(
            string: code.code + "\n",
            attributes: [
                .font: mono,
                .foregroundColor: ink,
                .paragraphStyle: p,
                .backgroundColor: muted.withAlphaComponent(0.08)
            ]
        )
    }

    // MARK: - Inline

    private mutating func renderInlines(_ inlines: LazyMapSequence<MarkupChildren, InlineMarkup>) -> NSAttributedString {
        let out = NSMutableAttributedString()
        for inline in inlines {
            out.append(renderInline(inline))
        }
        return out
    }

    private mutating func renderInline(_ inline: InlineMarkup) -> NSAttributedString {
        switch inline {
        case let text as Markdown.Text:
            return renderPlainText(text.string)
        case let emphasis as Emphasis:
            let inner = renderInlines(emphasis.inlineChildren)
            let out = NSMutableAttributedString(attributedString: inner)
            out.addAttribute(.font, value: serifItalic, range: NSRange(location: 0, length: out.length))
            return out
        case let strong as Strong:
            let inner = renderInlines(strong.inlineChildren)
            let out = NSMutableAttributedString(attributedString: inner)
            out.addAttribute(.font, value: serifBold, range: NSRange(location: 0, length: out.length))
            return out
        case let strike as Strikethrough:
            let inner = renderInlines(strike.inlineChildren)
            let out = NSMutableAttributedString(attributedString: inner)
            out.addAttribute(.strikethroughStyle, value: NSUnderlineStyle.single.rawValue, range: NSRange(location: 0, length: out.length))
            return out
        case let code as InlineCode:
            return NSAttributedString(
                string: code.code,
                attributes: [
                    .font: mono,
                    .backgroundColor: muted.withAlphaComponent(0.15),
                    .foregroundColor: ink
                ]
            )
        case let link as Link:
            let inner = renderInlines(link.inlineChildren)
            let out = NSMutableAttributedString(attributedString: inner)
            if let dest = link.destination, let url = URL(string: dest) {
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
        case let image as Image:
            // Render an image placeholder line. Full inline image loading is
            // v2 — we leave a readable marker so the reader can decide later.
            let alt = image.plainText
            let dest = image.source ?? ""
            let label = alt.isEmpty ? (dest.isEmpty ? "(image)" : dest) : alt
            return NSAttributedString(
                string: "🖼 \(label)\n",
                attributes: [.font: serifItalic, .foregroundColor: muted]
            )
        case is LineBreak:
            return NSAttributedString(string: "\n", attributes: [.font: serif])
        case is SoftBreak:
            return NSAttributedString(string: " ", attributes: [.font: serif])
        default:
            // Unknown inline: flatten via `plainText`.
            return renderPlainText(inline.plainText)
        }
    }

    /// Scan plain text for `[^id]` footnote references and emit them as
    /// superscript, tappable runs. Everything else passes through as serif
    /// body text.
    private mutating func renderPlainText(_ s: String) -> NSAttributedString {
        guard s.contains("[^") else {
            return NSAttributedString(
                string: s,
                attributes: [.font: serif, .foregroundColor: ink]
            )
        }

        let out = NSMutableAttributedString()
        var remainder = s[...]
        while let openRange = remainder.range(of: "[^") {
            // Append text before the marker.
            let before = String(remainder[remainder.startIndex..<openRange.lowerBound])
            if !before.isEmpty {
                out.append(NSAttributedString(
                    string: before,
                    attributes: [.font: serif, .foregroundColor: ink]
                ))
            }
            let afterOpen = remainder[openRange.upperBound...]
            guard let closeRange = afterOpen.range(of: "]") else {
                // Dangling `[^` — keep as literal text.
                let tail = String(remainder[openRange.lowerBound...])
                out.append(NSAttributedString(
                    string: tail,
                    attributes: [.font: serif, .foregroundColor: ink]
                ))
                return out
            }
            let id = String(afterOpen[afterOpen.startIndex..<closeRange.lowerBound])
            let consumedEnd = closeRange.upperBound

            if let def = definitionsById[id] {
                // Superscript numeric marker.
                let marker = "[\(def.number)]"
                let rangeStart = out.length
                let superSize = max(10, bodyPointSize - 6)
                let superFont = UIFont.systemFont(ofSize: superSize, weight: .semibold)
                let attrs: [NSAttributedString.Key: Any] = [
                    .font: superFont,
                    .foregroundColor: accent,
                    .baselineOffset: bodyPointSize * 0.35,
                    MarkdownRenderer.footnoteReferenceAttribute: def.number,
                    .link: URL(string: "highlighter://footnote/\(def.number)")!
                ]
                out.append(NSAttributedString(string: marker, attributes: attrs))
                footnoteAnchors[def.number] = NSRange(location: rangeStart, length: marker.utf16.count)
            } else {
                // Unknown footnote id — keep the raw `[^id]` text so the
                // author sees it's broken rather than silently dropping.
                let literal = "[^\(id)]"
                out.append(NSAttributedString(
                    string: literal,
                    attributes: [.font: serif, .foregroundColor: muted]
                ))
            }

            remainder = afterOpen[consumedEnd...]
        }

        // Trailing text after the final marker.
        let tail = String(remainder)
        if !tail.isEmpty {
            out.append(NSAttributedString(
                string: tail,
                attributes: [.font: serif, .foregroundColor: ink]
            ))
        }
        return out
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
