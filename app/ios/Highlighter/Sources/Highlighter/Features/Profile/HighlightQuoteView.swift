import SwiftUI

/// A single highlight rendered as a blockquote. Two treatments:
///   - With a NIP-92 page image: the photo is the centerpiece, with the
///     quote as a serif pull-quote below. No accent rail, no card chrome —
///     let the page breathe.
///   - Without an image: accent-colored left bar, pale background, serif
///     quote body, source hint underneath.
struct HighlightQuoteView: View {
    let highlight: HighlightRecord

    var body: some View {
        if let pageURL = pageImageURL {
            pageBody(pageURL: pageURL)
        } else {
            textBody
        }
    }

    private func pageBody(pageURL: URL) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HighlightPageImage(url: pageURL, treatment: .feature)

            VStack(alignment: .leading, spacing: 8) {
                Text(highlight.quote)
                    .font(.system(.body, design: .serif).italic())
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineSpacing(4)
                    .fixedSize(horizontal: false, vertical: true)
                    .multilineTextAlignment(.leading)

                if !highlight.note.isEmpty {
                    Text(highlight.note)
                        .font(.footnote)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .fixedSize(horizontal: false, vertical: true)
                }

                if let source = sourceHint {
                    Text(source)
                        .font(.caption)
                        .foregroundStyle(Color.highlighterAccent)
                        .lineLimit(1)
                }
            }
            .padding(.horizontal, 4)
        }
    }

    private var textBody: some View {
        HStack(alignment: .top, spacing: 0) {
            Rectangle()
                .fill(Color.highlighterAccent)
                .frame(width: 3)

            VStack(alignment: .leading, spacing: 10) {
                Text(highlight.quote)
                    .font(.system(.body, design: .serif))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .fixedSize(horizontal: false, vertical: true)
                    .multilineTextAlignment(.leading)

                if !highlight.note.isEmpty {
                    Text(highlight.note)
                        .font(.footnote)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .fixedSize(horizontal: false, vertical: true)
                }

                if let source = sourceHint {
                    Text(source)
                        .font(.caption)
                        .foregroundStyle(Color.highlighterAccent)
                        .lineLimit(1)
                }
            }
            .padding(14)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(Color.highlighterTintPale.opacity(0.5))
        .clipShape(RoundedRectangle(cornerRadius: 10))
    }

    private var pageImageURL: URL? {
        let raw = highlight.imageUrl.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !raw.isEmpty else { return nil }
        return URL(string: raw)
    }

    private var sourceHint: String? {
        if !highlight.sourceUrl.isEmpty {
            return URL(string: highlight.sourceUrl)?.host ?? highlight.sourceUrl
        }
        if !highlight.artifactAddress.isEmpty {
            // NIP-33 address like `30023:<pubkey>:<d>` — keep a short prefix.
            let parts = highlight.artifactAddress.split(separator: ":")
            return parts.first.map { "Article · \($0)" }
        }
        if !highlight.eventReference.isEmpty {
            return "Note"
        }
        return nil
    }
}
