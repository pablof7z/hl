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

    /// Inline ISBN extraction — mirrors `book_route_for_highlight` in highlights.rs.
    /// Checks `externalReference` first, then `artifactAddress`.
    private var isbnFromLead: String? {
        let ext = lead.highlight.externalReference.trimmingCharacters(in: .whitespaces)
        if ext.lowercased().hasPrefix("isbn:") {
            let isbn = String(ext.dropFirst(5)).trimmingCharacters(in: .whitespaces)
            return isbn.isEmpty ? nil : isbn
        }
        let addr = lead.highlight.artifactAddress.trimmingCharacters(in: .whitespaces)
        if addr.lowercased().hasPrefix("isbn:") {
            let isbn = String(addr.dropFirst(5)).trimmingCharacters(in: .whitespaces)
            return isbn.isEmpty ? nil : isbn
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

    /// Inline port of `highlight_feed_content_projection` (highlights.rs).
    private func highlightFeedContent(for highlight: HighlightRecord) -> HighlightFeedContentProjection {
        let quoteText = highlight.quote.trimmingCharacters(in: .whitespaces)
        let noteRaw = highlight.note.trimmingCharacters(in: .whitespaces)
        let imgRaw = highlight.imageUrl.trimmingCharacters(in: .whitespaces)
        return HighlightFeedContentProjection(
            quoteText: quoteText,
            noteText: noteRaw.isEmpty ? nil : noteRaw,
            pageImageUrl: imgRaw.isEmpty ? nil : imgRaw
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

    /// Inline port of `highlight_resource_header_projection` (highlights.rs).
    private var resourceProjection: HighlightResourceHeaderProjection {
        let preview = lead.artifact?.preview
        let sourceKind = highlightSourceKind(preview: preview)
        let urlHost = urlHost(rawUrl: lead.highlight.sourceUrl)
        let isbn = isbnFromLead
        let articleAddr = articleAddressForHighlight(lead.highlight.artifactAddress)
        let authorPubkey = articleAuthorPubkey(
            sourceArticle: sourceArticle,
            resolved: sourceArticleAuthorPubkey ?? ""
        )
        let authorProfile = app.profileSnapshots[authorPubkey]
        let webMeta: WebMetadata? = {
            guard let url = webMetadataUrl(sourceKind: sourceKind, preview: preview) else { return nil }
            return app.webMetadataCache[url]
        }()

        return HighlightResourceHeaderProjection(
            sourceKind: sourceKind,
            iconSystemName: sourceKindIcon(sourceKind),
            title: resourceTitle(
                sourceKind: sourceKind,
                preview: preview,
                sourceArticle: sourceArticle,
                bookPreview: bookPreview,
                webMetadata: webMeta,
                urlHost: urlHost
            ),
            authorOrDomain: resourceAuthorOrDomain(
                sourceKind: sourceKind,
                preview: preview,
                bookPreview: bookPreview,
                webMetadata: webMeta,
                urlHost: urlHost,
                articleAuthorPubkey: authorPubkey,
                articleAuthorProfile: authorProfile
            ),
            timeLabel: resourceTimeLabel(
                sourceKind: sourceKind,
                preview: preview,
                sourceArticle: sourceArticle
            ),
            coverUrl: resourceCoverUrl(
                sourceKind: sourceKind,
                preview: preview,
                sourceArticle: sourceArticle,
                bookPreview: bookPreview,
                webMetadata: webMeta
            ),
            bookIsbn: isbn,
            articleAddress: articleAddr,
            articleAuthorPubkey: authorPubkey,
            webMetadataUrl: webMetadataUrl(sourceKind: sourceKind, preview: preview)
        )
    }

    // MARK: - Resource projection helpers (inline ports of highlights.rs functions)

    private func highlightSourceKind(preview: ArtifactPreview?) -> HighlightSourceKind {
        let src = (preview?.source ?? "").trimmingCharacters(in: .whitespaces).lowercased()
        switch src {
        case "article": return .article
        case "web": return .web
        case "podcast": return .podcast
        case "book": return .book
        case "video": return .video
        case "paper": return .paper
        case "": break
        default: return .unknown
        }
        let ext = lead.highlight.externalReference.trimmingCharacters(in: .whitespaces).lowercased()
        let addr = lead.highlight.artifactAddress.trimmingCharacters(in: .whitespaces)
        if ext.hasPrefix("isbn:") { return .book }
        if isArticleAddress(addr) { return .article }
        if addr.lowercased().hasPrefix("isbn:") { return .book }
        if !lead.highlight.sourceUrl.trimmingCharacters(in: .whitespaces).isEmpty { return .web }
        return .unknown
    }

    private func sourceKindIcon(_ kind: HighlightSourceKind) -> String {
        switch kind {
        case .article: return "doc.text"
        case .web: return "globe"
        case .podcast: return "waveform"
        case .book: return "book.closed"
        case .video: return "play.rectangle"
        case .paper: return "doc.richtext"
        case .unknown: return "quote.bubble"
        }
    }

    private func isArticleAddress(_ address: String) -> Bool {
        // kind:30023 coordinate starts with "30023:"
        address.hasPrefix("30023:")
    }

    private func articleAddressForHighlight(_ artifactAddress: String) -> String? {
        let trimmed = artifactAddress.trimmingCharacters(in: .whitespaces)
        return isArticleAddress(trimmed) ? trimmed : nil
    }

    private func articleAuthorPubkey(sourceArticle: ArticleRecord?, resolved: String) -> String {
        if let pubkey = sourceArticle?.pubkey, !pubkey.isEmpty { return pubkey }
        let trimmed = resolved.trimmingCharacters(in: .whitespaces)
        return trimmed.isEmpty ? "" : trimmed
    }

    private func webMetadataUrl(sourceKind: HighlightSourceKind, preview: ArtifactPreview?) -> String? {
        guard sourceKind == .web else { return nil }
        if let url = preview?.url, !url.isEmpty { return url }
        let raw = lead.highlight.sourceUrl.trimmingCharacters(in: .whitespaces)
        return raw.isEmpty ? nil : raw
    }

    private func urlHost(rawUrl: String) -> String? {
        let trimmed = rawUrl.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty, let components = URLComponents(string: trimmed) else { return nil }
        return components.host
    }

    private func resourceTitle(
        sourceKind: HighlightSourceKind,
        preview: ArtifactPreview?,
        sourceArticle: ArticleRecord?,
        bookPreview: ArtifactPreview?,
        webMetadata: WebMetadata?,
        urlHost: String?
    ) -> String {
        func ne(_ s: String?) -> String? { s.flatMap { $0.isEmpty ? nil : $0 } }
        switch sourceKind {
        case .article:
            return ne(sourceArticle?.title)
                ?? ne(preview?.title)
                ?? "Untitled"
        case .podcast, .video, .paper:
            return ne(preview?.title) ?? "Untitled"
        case .book:
            return ne(preview?.title) ?? ne(bookPreview?.title) ?? "Untitled"
        case .web:
            return ne(webMetadata?.title)
                ?? ne(preview?.title)
                ?? urlHost
                ?? "Web page"
        case .unknown:
            return ne(preview?.title) ?? urlHost ?? "Highlight"
        }
    }

    private func resourceAuthorOrDomain(
        sourceKind: HighlightSourceKind,
        preview: ArtifactPreview?,
        bookPreview: ArtifactPreview?,
        webMetadata: WebMetadata?,
        urlHost: String?,
        articleAuthorPubkey: String,
        articleAuthorProfile: ProfileMetadata?
    ) -> String {
        func ne(_ s: String?) -> String? { s.flatMap { $0.isEmpty ? nil : $0 } }
        switch sourceKind {
        case .article:
            // Profile display with label fallback to preview author
            let fallback = preview?.author ?? ""
            if articleAuthorPubkey.isEmpty {
                return ne(fallback) ?? ""
            }
            return profileDisplayName(pubkey: articleAuthorPubkey, profile: articleAuthorProfile, fallback: fallback)
        case .podcast:
            return ne(preview?.podcastShowTitle)
                ?? ne(preview?.author)
                ?? ""
        case .book:
            return ne(preview?.author) ?? ne(bookPreview?.author) ?? ""
        case .web:
            return ne(webMetadata?.siteName)
                ?? ne(webMetadata?.author)
                ?? ne(preview?.domain)
                ?? urlHost
                ?? ""
        case .video, .paper:
            return ne(preview?.author) ?? ne(preview?.domain) ?? ""
        case .unknown:
            return urlHost ?? ""
        }
    }

    private func resourceTimeLabel(
        sourceKind: HighlightSourceKind,
        preview: ArtifactPreview?,
        sourceArticle: ArticleRecord?
    ) -> String? {
        switch sourceKind {
        case .article:
            guard let content = sourceArticle?.content, !content.isEmpty else { return nil }
            let words = content.split(separator: " ").count
            guard words > 60 else { return nil }
            let minutes = max(1, words / 240)
            return "\(minutes) min"
        case .podcast:
            guard let secs = preview?.durationSeconds, secs > 0 else { return nil }
            let hours = secs / 3600
            let minutes = (secs % 3600) / 60
            return hours > 0 ? "\(hours)h \(minutes)m" : "\(minutes)m"
        default:
            return nil
        }
    }

    private func resourceCoverUrl(
        sourceKind: HighlightSourceKind,
        preview: ArtifactPreview?,
        sourceArticle: ArticleRecord?,
        bookPreview: ArtifactPreview?,
        webMetadata: WebMetadata?
    ) -> String? {
        func ne(_ s: String?) -> String? { s.flatMap { $0.isEmpty ? nil : $0 } }
        if let img = ne(preview?.image) { return img }
        switch sourceKind {
        case .book:
            if let img = ne(bookPreview?.image) { return img }
        case .article:
            if let img = ne(sourceArticle?.image) { return img }
        case .web:
            if let img = ne(webMetadata?.image) { return img }
            if let fav = ne(webMetadata?.favicon) { return fav }
        default:
            break
        }
        return nil
    }

    // MARK: - Derived: group projection

    /// Inline port of `highlight_group_card_projection` (highlights.rs).
    private var groupProjection: HighlightGroupCardProjection {
        // Collect unique pubkeys in first-seen order
        var seen = Set<String>()
        var uniquePubkeys: [String] = []
        for item in items {
            if seen.insert(item.highlight.pubkey).inserted {
                uniquePubkeys.append(item.highlight.pubkey)
            }
        }
        let showStrip = items.count >= 2 && uniquePubkeys.count >= 2
        guard showStrip else {
            return HighlightGroupCardProjection(
                showHighlightersStrip: false,
                visibleHighlighters: [],
                overflowCount: 0,
                highlightersLabelSegments: []
            )
        }
        let highlighters: [HighlightGroupHighlighterProjection] = uniquePubkeys.map { pubkey in
            let profile = app.profileSnapshots[pubkey]
            let name = profileDisplayName(pubkey: pubkey, profile: profile, fallback: nil)
            return HighlightGroupHighlighterProjection(
                pubkey: pubkey,
                displayName: name,
                displayInitial: String(name.prefix(1)),
                pictureUrl: profile?.picture ?? ""
            )
        }
        let visible = Array(highlighters.prefix(3))
        let overflow = max(0, highlighters.count - 3)
        return HighlightGroupCardProjection(
            showHighlightersStrip: true,
            visibleHighlighters: visible,
            overflowCount: UInt32(overflow),
            highlightersLabelSegments: highlightersLabelSegments(highlighters)
        )
    }

    /// Inline port of `highlighters_label_segments` (highlights.rs).
    private func highlightersLabelSegments(
        _ highlighters: [HighlightGroupHighlighterProjection]
    ) -> [HighlightGroupLabelSegment] {
        var out: [HighlightGroupLabelSegment] = [
            HighlightGroupLabelSegment(text: "Highlighted by ", emphasized: false)
        ]
        switch highlighters.count {
        case 0:
            break
        case 1:
            out.append(HighlightGroupLabelSegment(text: highlighters[0].displayName, emphasized: true))
        case 2:
            out.append(HighlightGroupLabelSegment(text: highlighters[0].displayName, emphasized: true))
            out.append(HighlightGroupLabelSegment(text: " and ", emphasized: false))
            out.append(HighlightGroupLabelSegment(text: highlighters[1].displayName, emphasized: true))
        default:
            out.append(HighlightGroupLabelSegment(text: highlighters[0].displayName, emphasized: true))
            out.append(HighlightGroupLabelSegment(text: ", ", emphasized: false))
            out.append(HighlightGroupLabelSegment(text: highlighters[1].displayName, emphasized: true))
            out.append(HighlightGroupLabelSegment(text: " and ", emphasized: false))
            out.append(HighlightGroupLabelSegment(text: "\(highlighters.count - 2) others", emphasized: true))
        }
        return out
    }

    // MARK: - Derived: profile helpers

    /// Inline port of `profile_display_projection` — mirrors CommentRow.authorDisplay pattern.
    private func profileDisplay(for pubkey: String) -> ProfileDisplayProjection {
        let profile = app.profileSnapshots[pubkey]
        let name = profileDisplayName(pubkey: pubkey, profile: profile, fallback: nil)
        return ProfileDisplayProjection(
            displayName: name,
            displayInitial: String(name.prefix(1)),
            pictureUrl: profile?.picture ?? ""
        )
    }

    /// Common name-resolution logic: displayName → name → pubkey prefix.
    private func profileDisplayName(pubkey: String, profile: ProfileMetadata?, fallback: String?) -> String {
        if let d = profile?.displayName, !d.isEmpty { return d }
        if let n = profile?.name, !n.isEmpty { return n }
        if let f = fallback, !f.isEmpty { return f }
        return String(pubkey.prefix(8))
    }

    /// Inline relative-time label using Foundation's RelativeDateTimeFormatter.
    private func relativeDate(_ seconds: UInt64?) -> String? {
        guard let seconds, seconds > 0 else { return nil }
        let date = Date(timeIntervalSince1970: TimeInterval(seconds))
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        formatter.dateTimeStyle = .numeric
        return formatter.localizedString(for: date, relativeTo: Date())
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

        sourceArticle = await app.core.getArticleByAddress(address: addr)
        if let pubkey = sourceArticle?.pubkey, !pubkey.isEmpty {
            sourceArticleAuthorPubkey = pubkey
            await app.requestProfile(pubkeyHex: pubkey)
            return
        }
        if let pubkey = await app.core.getArticleAddressAuthor(address: addr), !pubkey.isEmpty {
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
        let content = highlightFeedContent(for: highlight.highlight)

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

    /// Inline port of `profile_display_projection` — mirrors CommentRow.authorDisplay pattern.
    private var authorDisplay: ProfileDisplayProjection {
        let pubkey = highlight.highlight.pubkey
        let profile = app.profileSnapshots[pubkey]
        let name: String = {
            if let d = profile?.displayName, !d.isEmpty { return d }
            if let n = profile?.name, !n.isEmpty { return n }
            return String(pubkey.prefix(8))
        }()
        return ProfileDisplayProjection(
            displayName: name,
            displayInitial: String(name.prefix(1)),
            pictureUrl: profile?.picture ?? ""
        )
    }

    /// Inline relative-time label using Foundation's RelativeDateTimeFormatter.
    private var relative: String? {
        guard let s = highlight.highlight.createdAt, s > 0 else { return nil }
        let date = Date(timeIntervalSince1970: TimeInterval(s))
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        formatter.dateTimeStyle = .numeric
        return formatter.localizedString(for: date, relativeTo: Date())
    }

    /// Inline port of `highlight_feed_content_projection` (highlights.rs).
    private func highlightFeedContent(for highlight: HighlightRecord) -> HighlightFeedContentProjection {
        let quoteText = highlight.quote.trimmingCharacters(in: .whitespaces)
        let noteRaw = highlight.note.trimmingCharacters(in: .whitespaces)
        let imgRaw = highlight.imageUrl.trimmingCharacters(in: .whitespaces)
        return HighlightFeedContentProjection(
            quoteText: quoteText,
            noteText: noteRaw.isEmpty ? nil : noteRaw,
            pageImageUrl: imgRaw.isEmpty ? nil : imgRaw
        )
    }
}
