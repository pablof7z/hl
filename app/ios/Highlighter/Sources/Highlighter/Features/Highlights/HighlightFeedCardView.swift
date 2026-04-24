import Kingfisher
import SwiftUI

/// Editorial pull-quote card for the Highlights home feed. Three visual
/// priorities, top → bottom:
///  1. The quote (serif, hero treatment, with a terracotta accent rail).
///  2. The highlighter's note, when present (serif italic, muted).
///  3. Source — a read-strip (cover + title + read time) when the highlight
///     points at a resolved NIP-23 article; otherwise a compact "FROM …" line.
///
/// The highlighter's byline floats above the quote as a small social line.
/// Tap targets are split: byline → highlighter profile, source line →
/// article reader. The card as a whole opens the article on tap (wired by
/// the parent `NavigationLink`).
struct HighlightFeedCardView: View {
    @Environment(HighlighterStore.self) private var app

    let item: HydratedHighlight

    @State private var sourceArticle: ArticleRecord?

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            highlighterByline
            quoteBlock
            if !item.highlight.note.isEmpty {
                noteBlock
            }
            sourceBlock
        }
        .padding(.vertical, 28)
        .contentShape(Rectangle())
        .task(id: item.highlight.pubkey) {
            await app.requestProfile(pubkeyHex: item.highlight.pubkey)
        }
        .task(id: item.highlight.artifactAddress) {
            await resolveSource()
        }
    }

    // MARK: - Highlighter byline (top)

    private var highlighterByline: some View {
        NavigationLink(value: ProfileDestination.pubkey(item.highlight.pubkey)) {
            HStack(spacing: 8) {
                AuthorAvatar(
                    pubkey: item.highlight.pubkey,
                    pictureURL: app.profileCache[item.highlight.pubkey]?.picture ?? "",
                    displayInitial: highlighterInitial,
                    size: 22
                )
                Text(highlighterDisplayName)
                    .font(.footnote.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(1)
                if let rel = relativeDate {
                    Text("·")
                        .foregroundStyle(Color.highlighterInkMuted)
                    Text(rel)
                        .font(.footnote)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(1)
                }
                Spacer(minLength: 0)
            }
        }
        .buttonStyle(.plain)
    }

    // MARK: - Quote + note

    private var quoteBlock: some View {
        HStack(alignment: .top, spacing: 16) {
            Rectangle()
                .fill(Color.highlighterAccent)
                .frame(width: 3)
                .clipShape(RoundedRectangle(cornerRadius: 1.5))

            Text(trimmedQuote)
                .font(.system(size: 22, design: .serif).italic())
                .foregroundStyle(Color.highlighterInkStrong)
                .lineSpacing(5)
                .lineLimit(12)
                .truncationMode(.tail)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var noteBlock: some View {
        Text(item.highlight.note)
            .font(.system(.subheadline, design: .serif))
            .foregroundStyle(Color.highlighterInkMuted)
            .lineSpacing(2)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.leading, 19) // align with quote body (3pt bar + 16pt gap)
    }

    // MARK: - Source block (read-strip for articles, compact line otherwise)

    @ViewBuilder
    private var sourceBlock: some View {
        if let article = sourceArticle {
            readStrip(for: article)
                .padding(.leading, 19)
                .padding(.top, 6)
        } else if let attributed = sourceAttributed {
            Text(attributed)
                .font(.caption)
                .lineLimit(2)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.leading, 19)
                .padding(.top, 4)
        }
    }

    /// Compact read-affordance rendered under a highlight whose source is a
    /// resolved NIP-23 article. Cover thumb + title + author · read-time +
    /// a terracotta "Read →" tail. Purely visual — the parent
    /// `NavigationLink` owns the tap.
    @ViewBuilder
    private func readStrip(for article: ArticleRecord) -> some View {
        HStack(spacing: 10) {
            readStripThumb(imageURLString: article.image)
                .frame(width: 38, height: 38)
                .clipShape(RoundedRectangle(cornerRadius: 4, style: .continuous))

            VStack(alignment: .leading, spacing: 2) {
                Text(article.title.isEmpty ? "Untitled" : article.title)
                    .font(.system(.footnote, design: .serif).weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(1)
                    .truncationMode(.tail)

                Text(readStripMeta(for: article))
                    .font(.caption2)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            Text("Read →")
                .font(.caption.weight(.semibold))
                .foregroundStyle(Color.highlighterAccent)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(Color.highlighterPaper)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .stroke(Color.highlighterRule, lineWidth: 1)
        )
    }

    @ViewBuilder
    private func readStripThumb(imageURLString: String) -> some View {
        let url = URL(string: imageURLString.trimmingCharacters(in: .whitespacesAndNewlines))
        if let url, !imageURLString.isEmpty {
            KFImage(url)
                .placeholder { readStripThumbPlaceholder }
                .fade(duration: 0.15)
                .resizable()
                .scaledToFill()
        } else {
            readStripThumbPlaceholder
        }
    }

    private var readStripThumbPlaceholder: some View {
        LinearGradient(
            colors: [Color.highlighterRule.opacity(0.7), Color.highlighterRule.opacity(0.35)],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )
        .overlay(
            Image(systemName: "doc.text")
                .font(.caption2)
                .foregroundStyle(Color.highlighterInkMuted.opacity(0.7))
        )
    }

    /// "Lyn Alden · 12 min" — author (if resolved) joined with a rough read
    /// estimate (240 wpm, matches `ReadingFeedCardView`). Falls back to just
    /// the author or just the read time when either is missing.
    private func readStripMeta(for article: ArticleRecord) -> String {
        var bits: [String] = []
        if let authorName = sourceAuthorDisplayName, !authorName.isEmpty {
            bits.append(authorName)
        }
        if let mins = readTimeMinutes(for: article) {
            bits.append("\(mins) min read")
        }
        return bits.joined(separator: " · ")
    }

    private func readTimeMinutes(for article: ArticleRecord) -> Int? {
        let words = article.content.split(whereSeparator: { $0.isWhitespace }).count
        guard words > 60 else { return nil }
        return max(1, words / 240)
    }

    // MARK: - Derived

    private var trimmedQuote: String {
        item.highlight.quote.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var highlighterDisplayName: String {
        let profile = app.profileCache[item.highlight.pubkey]
        if let dn = profile?.displayName, !dn.isEmpty { return dn }
        if let n = profile?.name, !n.isEmpty { return n }
        return String(item.highlight.pubkey.prefix(10))
    }

    private var highlighterInitial: String {
        highlighterDisplayName.first.map { String($0).uppercased() } ?? "?"
    }

    /// Short relative time — "2h", "3d", "just now". Avoids the trailing
    /// "ago" that feels notification-y in an editorial context.
    private var relativeDate: String? {
        guard let seconds = item.highlight.createdAt, seconds > 0 else { return nil }
        let now = Date().timeIntervalSince1970
        let delta = now - TimeInterval(seconds)
        guard delta >= 0 else { return nil }
        switch delta {
        case ..<60: return "just now"
        case ..<3600: return "\(Int(delta / 60))m"
        case ..<86400: return "\(Int(delta / 3600))h"
        case ..<(86400 * 7): return "\(Int(delta / 86400))d"
        case ..<(86400 * 30): return "\(Int(delta / (86400 * 7)))w"
        default: return "\(Int(delta / (86400 * 30)))mo"
        }
    }

    /// "FROM TITLE · Author" with FROM in small-caps muted, the title in
    /// near-ink, and the author in muted. Uses AttributedString so we get
    /// per-run styling in a single layout-sensible Text.
    private var sourceAttributed: AttributedString? {
        var title: String?
        if let article = sourceArticle {
            title = article.title.isEmpty ? "Untitled" : article.title
        } else if !item.highlight.sourceUrl.isEmpty,
                  let url = URL(string: item.highlight.sourceUrl),
                  let host = url.host {
            title = host
        }
        guard let title else { return nil }

        var out = AttributedString("FROM  ")
        out.font = .caption2.weight(.semibold)
        out.foregroundColor = Color.highlighterInkMuted
        out.kern = 1.2

        var titleRun = AttributedString(title)
        titleRun.font = .caption.weight(.medium)
        titleRun.foregroundColor = Color.highlighterInkStrong
        out.append(titleRun)

        if let authorName = sourceAuthorDisplayName, !authorName.isEmpty {
            var sep = AttributedString("  ·  ")
            sep.font = .caption
            sep.foregroundColor = Color.highlighterInkMuted
            out.append(sep)
            var by = AttributedString(authorName)
            by.font = .caption
            by.foregroundColor = Color.highlighterInkMuted
            out.append(by)
        }
        return out
    }

    private var sourceArtifactPubkey: String? {
        let addr = item.highlight.artifactAddress.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !addr.isEmpty else { return nil }
        let parts = addr.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false)
        guard parts.count == 3, parts[0] == "30023" else { return nil }
        let pubkey = String(parts[1])
        return pubkey.isEmpty ? nil : pubkey
    }

    private var sourceAuthorDisplayName: String? {
        guard let pubkey = sourceArtifactPubkey else { return nil }
        let profile = app.profileCache[pubkey]
        if let dn = profile?.displayName, !dn.isEmpty { return dn }
        if let n = profile?.name, !n.isEmpty { return n }
        return nil
    }

    /// For NIP-23 article highlights we carry `30023:<pubkey>:<d>` in
    /// `artifact_address`. Parse it and fetch the article (for the title) +
    /// the original author's profile (for the byline). Silently no-ops for
    /// non-article highlights — those fall back to the `sourceUrl` path.
    private func resolveSource() async {
        sourceArticle = nil

        let addr = item.highlight.artifactAddress.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !addr.isEmpty else { return }
        let parts = addr.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false)
        guard parts.count == 3, parts[0] == "30023" else { return }
        let pubkey = String(parts[1])
        let dTag = String(parts[2])
        guard !pubkey.isEmpty, !dTag.isEmpty else { return }

        sourceArticle = try? await app.safeCore.getArticle(pubkeyHex: pubkey, dTag: dTag)
        await app.requestProfile(pubkeyHex: pubkey)
    }
}
