import Kingfisher
import SwiftUI

/// Universal grouped-highlight module used by both the Highlights tab and
/// the Room home. The shape is uniform: a tinted rounded module that
/// joins the resource header (top) to the highlight content (below) as
/// one coherent block.
///
/// Three rendering rules, encoded by `items.count` and the number of
/// distinct highlighter pubkeys:
///   1 highlight                → resource header → byline + pull-quote
///   2+ highlights, 1 highlighter → resource header → reel of cards (no strip)
///   2+ highlights, 2+ highlighters → resource header → "Highlighted by …" → reel
///
/// The resource header adapts per artifact kind (article, web, podcast,
/// book) — the rest of the layout is shared.
struct HighlightFeedCardView: View {
    @Environment(HighlighterStore.self) private var app

    let items: [HydratedHighlight]

    /// The lead item drives the resource header and tasks. All items in
    /// the array share the same source (grouping invariant), so any of
    /// them resolves to the same artifact metadata.
    private var lead: HydratedHighlight { items[0] }

    @State private var sourceArticle: ArticleRecord?
    @State private var sourceArticleAuthorPubkey: String?

    private var bookPreview: ArtifactPreview? {
        guard let isbn = isbnFromLead else { return nil }
        return app.isbnPreviewCache[isbn]
    }

    private var isbnFromLead: String? {
        for candidate in [lead.highlight.externalReference, lead.highlight.artifactAddress] {
            let t = candidate.trimmingCharacters(in: .whitespaces)
            if t.hasPrefix("isbn:") {
                let isbn = String(t.dropFirst(5)).trimmingCharacters(in: .whitespaces)
                if !isbn.isEmpty { return isbn }
            }
        }
        return nil
    }

    var body: some View {
        let groupProjection = groupProjection
        let resourceProjection = resourceProjection

        VStack(alignment: .leading, spacing: 14) {
            resourceHeader(resourceProjection)
            if groupProjection.showHighlightersStrip {
                highlightersStrip(groupProjection)
            }
            highlightsBody
        }
        .padding(.vertical, 18)
        .task(id: lead.highlight.pubkey) {
            await app.requestProfile(pubkeyHex: lead.highlight.pubkey)
        }
        .task(id: resourceSourceTaskId(resourceProjection)) {
            await resolveSource(resourceProjection)
        }
        .task(id: resourceProjection.articleAuthorPubkey) {
            if !resourceProjection.articleAuthorPubkey.isEmpty {
                await app.requestProfile(pubkeyHex: resourceProjection.articleAuthorPubkey)
            }
        }
        .task(id: resourceProjection.webMetadataUrl) {
            if let url = resourceProjection.webMetadataUrl {
                await app.requestWebMetadata(url: url)
            }
        }
    }

    // MARK: - Resource header

    private func resourceHeader(_ resource: HighlightResourceHeaderProjection) -> some View {
        HStack(alignment: .top, spacing: 12) {
            resourceCover(resource)
                .frame(width: 44, height: 44)
                .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))

            VStack(alignment: .leading, spacing: 3) {
                Text(resource.title)
                    .font(.headline.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(2)
                    .multilineTextAlignment(.leading)
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: .infinity, alignment: .leading)

                resourceSubtitleRow(resource)
            }
        }
    }

    private func resourceSubtitleRow(_ resource: HighlightResourceHeaderProjection) -> some View {
        HStack(spacing: 4) {
            let author = resource.authorOrDomain
            let time = resource.timeLabel
            if !author.isEmpty {
                Text(author.uppercased())
                    .font(.caption2.weight(.bold))
                    .tracking(0.6)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(1)
            }
            if let time, !author.isEmpty {
                Text("·")
                    .font(.caption2)
                    .foregroundStyle(Color.highlighterInkMuted)
                Text(time)
                    .font(.caption2)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(1)
            } else if let time {
                Text(time)
                    .font(.caption2)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(1)
            }
            Spacer(minLength: 0)
        }
    }

    @ViewBuilder
    private func resourceCover(_ resource: HighlightResourceHeaderProjection) -> some View {
        if let urlString = resource.coverUrl,
           !urlString.isEmpty,
           let url = URL(string: urlString) {
            Color.clear
                .overlay(
                    KFImage(url)
                        .placeholder { coverFallback(resource) }
                        .fade(duration: 0.15)
                        .resizable()
                        .scaledToFill()
                )
                .clipped()
        } else {
            coverFallback(resource)
        }
    }

    private func coverFallback(_ resource: HighlightResourceHeaderProjection) -> some View {
        ZStack {
            LinearGradient(
                colors: [
                    Color.highlighterAccent.opacity(0.30),
                    Color.highlighterAccent.opacity(0.12),
                ],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
            Image(systemName: resource.iconSystemName)
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(Color.highlighterInkStrong.opacity(0.55))
        }
    }

    // MARK: - Highlighters strip (only when 2+ unique highlighters)

    private func highlightersStrip(_ projection: HighlightGroupCardProjection) -> some View {
        HStack(spacing: 8) {
            HStack(spacing: -6) {
                ForEach(projection.visibleHighlighters, id: \.pubkey) { highlighter in
                    AuthorAvatar(
                        pubkey: highlighter.pubkey,
                        pictureURL: highlighter.pictureUrl,
                        displayInitial: highlighter.displayInitial,
                        size: 20
                    )
                    .overlay(
                        Circle().stroke(Color.highlighterPaperTint, lineWidth: 1.5)
                    )
                    .task(id: highlighter.pubkey) {
                        await app.requestProfile(pubkeyHex: highlighter.pubkey)
                    }
                }
                if projection.overflowCount > 0 {
                    ZStack {
                        Circle()
                            .fill(Color.highlighterPaper)
                            .overlay(Circle().stroke(Color.highlighterRule, lineWidth: 0.5))
                        Text("+\(projection.overflowCount)")
                            .font(.system(size: 8, weight: .bold))
                            .foregroundStyle(Color.highlighterInkMuted)
                    }
                    .frame(width: 20, height: 20)
                    .overlay(Circle().stroke(Color.highlighterPaperTint, lineWidth: 1.5))
                }
            }

            Text(highlightersLabel(projection.highlightersLabelSegments))
                .font(.caption)
                .foregroundStyle(Color.highlighterInkMuted)
                .lineLimit(1)
                .truncationMode(.tail)
        }
    }

    private func highlightersLabel(_ segments: [HighlightGroupLabelSegment]) -> AttributedString {
        var out = AttributedString()
        for segment in segments {
            var text = AttributedString(segment.text)
            if segment.emphasized {
                text.font = .caption.weight(.semibold)
                text.foregroundColor = Color.highlighterInkStrong
            } else {
                text.foregroundColor = Color.highlighterInkMuted
            }
            out += text
        }
        return out
    }

    // MARK: - Highlight body (single inline OR reel)

    @ViewBuilder
    private var highlightsBody: some View {
        if items.count == 1 {
            singleHighlight(lead)
        } else {
            reel
        }
    }

    private func singleHighlight(_ h: HydratedHighlight) -> some View {
        let content = highlightFeedContent(for: h.highlight)

        return VStack(alignment: .leading, spacing: 12) {
            highlighterByline(for: h)

            if let pageImage = content.pageImageUrl, let pageURL = URL(string: pageImage) {
                pageHighlight(content: content, pageURL: pageURL)
            } else {
                textHighlight(content)
            }
        }
    }

    /// Text-only treatment: accent rail + serif italic pull-quote + note.
    private func textHighlight(_ content: HighlightFeedContentProjection) -> some View {
        return HStack(alignment: .top, spacing: 14) {
            Rectangle()
                .fill(Color.highlighterAccent)
                .frame(width: 3)
                .clipShape(RoundedRectangle(cornerRadius: 1.5))

            VStack(alignment: .leading, spacing: 8) {
                Text(content.quoteText)
                    .font(.system(size: 18, design: .default).italic())
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineSpacing(4)
                    .lineLimit(8)
                    .truncationMode(.tail)
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: .infinity, alignment: .leading)

                if let note = content.noteText {
                    Text(note)
                        .font(.system(.subheadline, design: .default))
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineSpacing(2)
                        .fixedSize(horizontal: false, vertical: true)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
    }

    /// Page-photo treatment: the scan is the centerpiece, with the quote as
    /// a serif pull-quote underneath. No accent rail — let the image breathe.
    private func pageHighlight(
        content: HighlightFeedContentProjection,
        pageURL: URL
    ) -> some View {
        return VStack(alignment: .leading, spacing: 12) {
            HighlightPageImage(url: pageURL, treatment: .feature)

            VStack(alignment: .leading, spacing: 6) {
                Text(content.quoteText)
                    .font(.system(size: 18, design: .default).italic())
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineSpacing(4)
                    .lineLimit(8)
                    .truncationMode(.tail)
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: .infinity, alignment: .leading)

                if let note = content.noteText {
                    Text(note)
                        .font(.system(.subheadline, design: .default))
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineSpacing(2)
                        .fixedSize(horizontal: false, vertical: true)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            .padding(.horizontal, 4)
        }
    }

    private func highlightFeedContent(for highlight: HighlightRecord) -> HighlightFeedContentProjection {
        let trimmedImage = highlight.imageUrl.trimmingCharacters(in: .whitespaces)
        return HighlightFeedContentProjection(
            quoteText: highlight.quote.trimmingCharacters(in: .whitespaces),
            noteText: highlight.note.isEmpty ? nil : highlight.note,
            pageImageUrl: trimmedImage.isEmpty ? nil : trimmedImage
        )
    }

    private var reel: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(alignment: .top, spacing: 10) {
                ForEach(items, id: \.highlight.eventId) { h in
                    HighlightQuoteCard(highlight: h)
                }
                Color.clear.frame(width: 4)
            }
        }
    }

    @ViewBuilder
    private func highlighterByline(for h: HydratedHighlight) -> some View {
        let highlighter = profileDisplay(for: h.highlight.pubkey)

        HStack(spacing: 8) {
            AuthorAvatar(
                pubkey: h.highlight.pubkey,
                pictureURL: highlighter.pictureUrl,
                displayInitial: highlighter.displayInitial,
                size: 22
            )
            Text(highlighter.displayName)
                .font(.footnote.weight(.semibold))
                .foregroundStyle(Color.highlighterInkStrong)
                .lineLimit(1)
            if let rel = relativeDate(h.highlight.createdAt) {
                Text("·").foregroundStyle(Color.highlighterInkMuted)
                Text(rel)
                    .font(.footnote)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(1)
            }
            Spacer(minLength: 0)
        }
        .task(id: h.highlight.pubkey) {
            await app.requestProfile(pubkeyHex: h.highlight.pubkey)
        }
    }

    // MARK: - Derived: resource projection

    private var resourceProjection: HighlightResourceHeaderProjection {
        // D1: inline highlight_resource_header_projection. Web metadata is read from the
        // cache in a single pass — SwiftUI re-renders when the cache updates, so the
        // two-pass Rust pattern is naturally replaced by observation.
        let preview = lead.artifact?.preview
        let h = lead.highlight

        let kind = Self.hlSourceKind(
            previewSource: preview?.source ?? "",
            externalReference: h.externalReference,
            artifactAddress: h.artifactAddress,
            sourceUrl: h.sourceUrl
        )

        let urlHost: String? = {
            let t = h.sourceUrl.trimmingCharacters(in: .whitespaces)
            return t.isEmpty ? nil : URL(string: t)?.host
        }()

        // Book ISBN (bare, without "isbn:" prefix; used by requestIsbnPreview / isbnPreviewCache)
        let bookIsbn: String? = {
            for ref in [h.externalReference, h.artifactAddress] {
                let t = ref.trimmingCharacters(in: .whitespaces)
                guard t.lowercased().hasPrefix("isbn:") else { continue }
                let isbn = String(t.dropFirst(5)).trimmingCharacters(in: .whitespaces)
                if !isbn.isEmpty { return isbn }
            }
            return nil
        }()

        // Article address: valid "30023:pk:d" trimmed string, or nil.
        let articleAddress: String? = {
            let t = h.artifactAddress.trimmingCharacters(in: .whitespaces)
            guard !t.isEmpty else { return nil }
            let parts = t.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false)
            guard parts.count == 3, parts[0] == "30023",
                  !String(parts[1]).trimmingCharacters(in: .whitespaces).isEmpty,
                  !String(parts[2]).trimmingCharacters(in: .whitespaces).isEmpty else { return nil }
            return t
        }()

        // Article author pubkey: prefer sourceArticle.pubkey, then sourceArticleAuthorPubkey.
        let artAuthorPk: String = {
            if let pk = sourceArticle?.pubkey, !pk.isEmpty { return pk }
            return sourceArticleAuthorPubkey ?? ""
        }()

        // Web metadata URL (nil for non-web kinds) and cached metadata.
        let webMetadataUrl: String? = {
            guard kind == .web else { return nil }
            if let url = preview?.url, !url.isEmpty { return url }
            let t = h.sourceUrl.trimmingCharacters(in: .whitespaces)
            return t.isEmpty ? nil : t
        }()
        let webMetadata: WebMetadata? = webMetadataUrl.flatMap { app.webMetadataCache[$0] }

        let title: String = {
            switch kind {
            case .article:
                if let t = sourceArticle?.title, !t.isEmpty { return t }
                if let t = preview?.title, !t.isEmpty { return t }
                return "Untitled"
            case .podcast, .video, .paper:
                if let t = preview?.title, !t.isEmpty { return t }
                return "Untitled"
            case .book:
                if let t = preview?.title, !t.isEmpty { return t }
                if let t = bookPreview?.title, !t.isEmpty { return t }
                return "Untitled"
            case .web:
                if let t = webMetadata?.title, !t.isEmpty { return t }
                if let t = preview?.title, !t.isEmpty { return t }
                return urlHost ?? "Web page"
            case .unknown:
                if let t = preview?.title, !t.isEmpty { return t }
                return urlHost ?? "Highlight"
            }
        }()

        let authorOrDomain: String = {
            switch kind {
            case .article:
                let profile = artAuthorPk.isEmpty ? nil : app.profileSnapshots[artAuthorPk]
                if let dn = profile?.displayName, !dn.isEmpty { return dn }
                if let n  = profile?.name, !n.isEmpty { return n }
                return preview?.author ?? ""
            case .podcast:
                if let s = preview?.podcastShowTitle, !s.isEmpty { return s }
                return preview?.author ?? ""
            case .book:
                // Mirror Rust: if preview exists, use preview.author even if empty.
                if let prev = preview { return prev.author }
                return bookPreview?.author ?? ""
            case .web:
                if let s = webMetadata?.siteName, !s.isEmpty { return s }
                if let a = webMetadata?.author, !a.isEmpty { return a }
                if let d = preview?.domain, !d.isEmpty { return d }
                return urlHost ?? ""
            case .video, .paper:
                if let a = preview?.author, !a.isEmpty { return a }
                return preview?.domain ?? ""
            case .unknown:
                return urlHost ?? ""
            }
        }()

        let timeLabel: String? = {
            switch kind {
            case .article:
                guard let content = sourceArticle?.content, !content.isEmpty else { return nil }
                let words = content.split(whereSeparator: \.isWhitespace).count
                guard words > 60 else { return nil }
                return "\(max(1, words / 240)) min"
            case .podcast:
                guard let secs = preview?.durationSeconds, secs > 0 else { return nil }
                let hrs = secs / 3600, mins = (secs % 3600) / 60
                return hrs > 0 ? "\(hrs)h \(mins)m" : "\(mins)m"
            default:
                return nil
            }
        }()

        let coverUrl: String? = {
            if let img = preview?.image, !img.isEmpty { return img }
            if kind == .book, let img = bookPreview?.image, !img.isEmpty { return img }
            if kind == .article, let img = sourceArticle?.image, !img.isEmpty { return img }
            if kind == .web {
                if let img = webMetadata?.image, !img.isEmpty { return img }
                if let fav = webMetadata?.favicon, !fav.isEmpty { return fav }
            }
            return nil
        }()

        return HighlightResourceHeaderProjection(
            sourceKind: kind,
            iconSystemName: Self.hlSourceKindIconName(kind),
            title: title,
            authorOrDomain: authorOrDomain,
            timeLabel: timeLabel,
            coverUrl: coverUrl,
            bookIsbn: bookIsbn,
            articleAddress: articleAddress,
            articleAuthorPubkey: artAuthorPk,
            webMetadataUrl: webMetadataUrl
        )
    }

    // MARK: - Highlight source kind helpers (D1 inlined from Rust highlights.rs)

    private static func hlSourceKind(
        previewSource: String,
        externalReference: String,
        artifactAddress: String,
        sourceUrl: String
    ) -> HighlightSourceKind {
        switch previewSource.trimmingCharacters(in: .whitespaces).lowercased() {
        case "article": return .article
        case "web":     return .web
        case "podcast": return .podcast
        case "book":    return .book
        case "video":   return .video
        case "paper":   return .paper
        case "":        break
        default:        return .unknown
        }
        if externalReference.trimmingCharacters(in: .whitespaces).lowercased().hasPrefix("isbn:") {
            return .book
        }
        let addr = artifactAddress.trimmingCharacters(in: .whitespaces)
        if !addr.isEmpty {
            let parts = addr.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false)
            if parts.count == 3, parts[0] == "30023",
               !String(parts[1]).trimmingCharacters(in: .whitespaces).isEmpty,
               !String(parts[2]).trimmingCharacters(in: .whitespaces).isEmpty {
                return .article
            }
            if addr.lowercased().hasPrefix("isbn:") { return .book }
        }
        if !sourceUrl.trimmingCharacters(in: .whitespaces).isEmpty { return .web }
        return .unknown
    }

    private static func hlSourceKindIconName(_ kind: HighlightSourceKind) -> String {
        switch kind {
        case .article: return "doc.text"
        case .web:     return "globe"
        case .podcast: return "waveform"
        case .book:    return "book.closed"
        case .video:   return "play.rectangle"
        case .paper:   return "doc.richtext"
        case .unknown: return "quote.bubble"
        }
    }

    // MARK: - Derived: group projection

    private var groupProjection: HighlightGroupCardProjection {
        var seen = Set<String>()
        var uniquePubkeys: [String] = []
        for h in items {
            if seen.insert(h.highlight.pubkey).inserted {
                uniquePubkeys.append(h.highlight.pubkey)
            }
        }
        let show = items.count >= 2 && uniquePubkeys.count >= 2
        guard show else {
            return HighlightGroupCardProjection(showHighlightersStrip: false, visibleHighlighters: [], overflowCount: 0, highlightersLabelSegments: [])
        }
        let highlighters = uniquePubkeys.map { pubkey -> HighlightGroupHighlighterProjection in
            let profile = app.profileSnapshots[pubkey]
            let name = (profile?.displayName ?? "").isEmpty
                ? ((profile?.name ?? "").isEmpty ? String(pubkey.prefix(10)) : profile!.name)
                : profile!.displayName
            return HighlightGroupHighlighterProjection(
                pubkey: pubkey,
                displayName: name,
                displayInitial: name.first.map { String($0).uppercased() } ?? "?",
                pictureUrl: profile?.picture ?? ""
            )
        }
        let overflow = max(0, highlighters.count - 3)
        var segments: [HighlightGroupLabelSegment] = [HighlightGroupLabelSegment(text: "Highlighted by ", emphasized: false)]
        switch highlighters.count {
        case 0: break
        case 1:
            segments.append(HighlightGroupLabelSegment(text: highlighters[0].displayName, emphasized: true))
        case 2:
            segments += [
                HighlightGroupLabelSegment(text: highlighters[0].displayName, emphasized: true),
                HighlightGroupLabelSegment(text: " and ", emphasized: false),
                HighlightGroupLabelSegment(text: highlighters[1].displayName, emphasized: true),
            ]
        default:
            segments += [
                HighlightGroupLabelSegment(text: highlighters[0].displayName, emphasized: true),
                HighlightGroupLabelSegment(text: ", ", emphasized: false),
                HighlightGroupLabelSegment(text: highlighters[1].displayName, emphasized: true),
                HighlightGroupLabelSegment(text: " and ", emphasized: false),
                HighlightGroupLabelSegment(text: "\(highlighters.count - 2) others", emphasized: false),
            ]
        }
        return HighlightGroupCardProjection(
            showHighlightersStrip: true,
            visibleHighlighters: Array(highlighters.prefix(3)),
            overflowCount: UInt32(overflow),
            highlightersLabelSegments: segments
        )
    }

    // MARK: - Derived: profile helpers

    private func profileDisplay(for pubkey: String) -> ProfileDisplayProjection {
        let profile = app.profileSnapshots[pubkey]
        let name = (profile?.displayName ?? "").isEmpty
            ? ((profile?.name ?? "").isEmpty ? String(pubkey.prefix(10)) : profile!.name)
            : profile!.displayName
        return ProfileDisplayProjection(
            displayName: name,
            displayInitial: name.first.map { String($0).uppercased() } ?? "?",
            pictureUrl: profile?.picture ?? ""
        )
    }

    private func relativeDate(_ seconds: UInt64?) -> String? {
        guard let seconds, seconds > 0 else { return nil }
        let now = UInt64(max(0, Date().timeIntervalSince1970))
        guard now >= seconds else { return nil }
        let delta = now - seconds
        if delta < 60 { return "just now" }
        switch delta {
        case 60 ..< 3600:   return "\(delta / 60)m"
        case 3600 ..< 86400:  return "\(delta / 3600)h"
        case 86400 ..< 604800:  return "\(delta / 86400)d"
        case 604800 ..< 2592000: return "\(delta / 604800)w"
        default: return "\(delta / 2592000)mo"
        }
    }

    private func resourceSourceTaskId(_ resource: HighlightResourceHeaderProjection) -> String {
        "\(lead.highlight.eventId)|\(resource.bookIsbn ?? "")|\(resource.articleAddress ?? "")"
    }

    private func resolveSource(_ resource: HighlightResourceHeaderProjection) async {
        sourceArticle = nil
        sourceArticleAuthorPubkey = nil

        if let isbn = resource.bookIsbn {
            await app.requestIsbnPreview(isbn: isbn)
            return
        }

        guard let addr = resource.articleAddress else { return }

        // Pure parse of "30023:pubkeyHex:d-tag" — the author pubkey is the
        // second segment. Replaces the nostrdb article lookup (D1 inline).
        let parts = addr.split(separator: ":", maxSplits: 2).map(String.init)
        if parts.count >= 3, !parts[1].isEmpty {
            let pubkey = parts[1]
            sourceArticleAuthorPubkey = pubkey
            await app.requestProfile(pubkeyHex: pubkey)
        }
    }
}

// MARK: - Single quote card (used inside the reel)

/// One quote inside the horizontal reel of a multi-highlight module.
/// Shows the highlighter byline at the top, the quote with the accent
/// rail below, and the optional note. Width is fixed so the reel paces
/// consistently.
private struct HighlightQuoteCard: View {
    @Environment(HighlighterStore.self) private var app

    let highlight: HydratedHighlight

    var body: some View {
        let h = highlight.highlight
        let trimmedImage = h.imageUrl.trimmingCharacters(in: .whitespaces)
        let content = HighlightFeedContentProjection(
            quoteText: h.quote.trimmingCharacters(in: .whitespaces),
            noteText: h.note.isEmpty ? nil : h.note,
            pageImageUrl: trimmedImage.isEmpty ? nil : trimmedImage
        )

        VStack(alignment: .leading, spacing: 0) {
            byline
                .padding(12)
                .padding(.bottom, 10)
                .overlay(alignment: .bottom) {
                    Rectangle()
                        .fill(Color.highlighterRule.opacity(0.5))
                        .frame(height: 1)
                        .padding(.horizontal, 12)
                }

            if let pageImage = content.pageImageUrl, let pageURL = URL(string: pageImage) {
                VStack(alignment: .leading, spacing: 8) {
                    HighlightPageImage(url: pageURL, treatment: .card)
                    quoteBlock(content)
                }
                .padding(12)
            } else {
                HStack(alignment: .top, spacing: 10) {
                    Rectangle()
                        .fill(Color.highlighterAccent)
                        .frame(width: 3)
                        .clipShape(RoundedRectangle(cornerRadius: 1.5))

                    quoteBlock(content)
                }
                .padding(12)
            }
        }
        .frame(width: 240, alignment: .topLeading)
        .background(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(Color.highlighterPaper)
        )
        .task(id: highlight.highlight.pubkey) {
            await app.requestProfile(pubkeyHex: highlight.highlight.pubkey)
        }
    }

    private func quoteBlock(_ content: HighlightFeedContentProjection) -> some View {
        return VStack(alignment: .leading, spacing: 6) {
            Text(content.quoteText)
                .font(.system(size: 14, design: .default).italic())
                .foregroundStyle(Color.highlighterInkStrong)
                .lineSpacing(3)
                .lineLimit(6)
                .truncationMode(.tail)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)

            if let note = content.noteText {
                Text(note)
                    .font(.caption)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(2)
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    @ViewBuilder
    private var byline: some View {
        let highlighter = authorDisplay

        HStack(spacing: 7) {
            AuthorAvatar(
                pubkey: highlight.highlight.pubkey,
                pictureURL: highlighter.pictureUrl,
                displayInitial: highlighter.displayInitial,
                size: 22
            )
            Text(highlighter.displayName)
                .font(.caption.weight(.semibold))
                .foregroundStyle(Color.highlighterInkStrong)
                .lineLimit(1)
            if let rel = relative {
                Text("·")
                    .font(.caption2)
                    .foregroundStyle(Color.highlighterInkMuted)
                Text(rel)
                    .font(.caption2)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(1)
            }
            Spacer(minLength: 0)
        }
    }

    private var authorDisplay: ProfileDisplayProjection {
        let pubkey = highlight.highlight.pubkey
        let profile = app.profileSnapshots[pubkey]
        let name = (profile?.displayName ?? "").isEmpty
            ? ((profile?.name ?? "").isEmpty ? String(pubkey.prefix(10)) : profile!.name)
            : profile!.displayName
        return ProfileDisplayProjection(
            displayName: name,
            displayInitial: name.first.map { String($0).uppercased() } ?? "?",
            pictureUrl: profile?.picture ?? ""
        )
    }

    private var relative: String? {
        guard let seconds = highlight.highlight.createdAt, seconds > 0 else { return nil }
        let now = UInt64(max(0, Date().timeIntervalSince1970))
        guard now >= seconds else { return nil }
        let delta = now - seconds
        if delta < 60 { return "just now" }
        switch delta {
        case 60 ..< 3600:   return "\(delta / 60)m"
        case 3600 ..< 86400:  return "\(delta / 3600)h"
        case 86400 ..< 604800:  return "\(delta / 86400)d"
        case 604800 ..< 2592000: return "\(delta / 604800)w"
        default: return "\(delta / 2592000)mo"
        }
    }
}
