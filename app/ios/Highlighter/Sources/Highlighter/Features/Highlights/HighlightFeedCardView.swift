import SwiftUI

/// Editorial pull-quote card for the Highlights home feed. Three visual
/// priorities, top → bottom:
///  1. The quote (serif, hero treatment, with a terracotta accent rail).
///  2. The highlighter's note, when present (serif italic, muted).
///  3. Source attribution — "From [title] by [author]" — at the bottom.
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
            sourceLine
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

    // MARK: - Source line

    @ViewBuilder
    private var sourceLine: some View {
        if let attributed = sourceAttributed {
            Text(attributed)
                .font(.caption)
                .lineLimit(2)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.leading, 19)
                .padding(.top, 4)
        }
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
