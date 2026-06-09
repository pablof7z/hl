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
        app.core.getHighlightBookRoute(
            externalReference: lead.highlight.externalReference,
            artifactAddress: lead.highlight.artifactAddress
        ).value?.isbn
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            resourceHeader
            if showHighlightersStrip {
                highlightersStrip
            }
            highlightsBody
        }
        .padding(.vertical, 18)
        .task(id: lead.highlight.pubkey) {
            await app.requestProfile(pubkeyHex: lead.highlight.pubkey)
        }
        .task(id: lead.highlight.artifactAddress + lead.highlight.externalReference) {
            await resolveSource()
        }
        .task(id: webMetadataURL) {
            if let url = webMetadataURL {
                await app.requestWebMetadata(url: url)
            }
        }
    }

    // MARK: - Resource header

    private var resourceHeader: some View {
        HStack(alignment: .top, spacing: 12) {
            resourceCover
                .frame(width: 44, height: 44)
                .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))

            VStack(alignment: .leading, spacing: 3) {
                Text(resourceTitle)
                    .font(.headline.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(2)
                    .multilineTextAlignment(.leading)
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: .infinity, alignment: .leading)

                resourceSubtitleRow
            }
        }
    }

    private var resourceSubtitleRow: some View {
        HStack(spacing: 4) {
            let author = resourceAuthorOrDomain
            let time = resourceTimeLabel
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
    private var resourceCover: some View {
        if let urlString = resourceCoverURL,
           !urlString.isEmpty,
           let url = URL(string: urlString) {
            Color.clear
                .overlay(
                    KFImage(url)
                        .placeholder { coverFallback }
                        .fade(duration: 0.15)
                        .resizable()
                        .scaledToFill()
                )
                .clipped()
        } else {
            coverFallback
        }
    }

    private var coverFallback: some View {
        ZStack {
            LinearGradient(
                colors: [
                    Color.highlighterAccent.opacity(0.30),
                    Color.highlighterAccent.opacity(0.12),
                ],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
            Image(systemName: kindIconName)
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(Color.highlighterInkStrong.opacity(0.55))
        }
    }

    // MARK: - Highlighters strip (only when 2+ unique highlighters)

    private var highlightersStrip: some View {
        HStack(spacing: 8) {
            HStack(spacing: -6) {
                ForEach(uniqueHighlighters.prefix(3), id: \.highlight.pubkey) { h in
                    let highlighter = profileDisplay(for: h.highlight.pubkey)

                    AuthorAvatar(
                        pubkey: h.highlight.pubkey,
                        pictureURL: highlighter.pictureUrl,
                        displayInitial: highlighter.displayInitial,
                        size: 20
                    )
                    .overlay(
                        Circle().stroke(Color.highlighterPaperTint, lineWidth: 1.5)
                    )
                    .task(id: h.highlight.pubkey) {
                        await app.requestProfile(pubkeyHex: h.highlight.pubkey)
                    }
                }
                if uniqueHighlighters.count > 3 {
                    ZStack {
                        Circle()
                            .fill(Color.highlighterPaper)
                            .overlay(Circle().stroke(Color.highlighterRule, lineWidth: 0.5))
                        Text("+\(uniqueHighlighters.count - 3)")
                            .font(.system(size: 8, weight: .bold))
                            .foregroundStyle(Color.highlighterInkMuted)
                    }
                    .frame(width: 20, height: 20)
                    .overlay(Circle().stroke(Color.highlighterPaperTint, lineWidth: 1.5))
                }
            }

            Text(highlightersLabel)
                .font(.caption)
                .foregroundStyle(Color.highlighterInkMuted)
                .lineLimit(1)
                .truncationMode(.tail)
        }
    }

    private var highlightersLabel: AttributedString {
        let names = uniqueHighlighters.map { profileDisplay(for: $0.highlight.pubkey).displayName }
        var out = AttributedString("Highlighted by ")
        out.foregroundColor = Color.highlighterInkMuted

        switch names.count {
        case 0:
            return out
        case 1:
            return out + boldName(names[0])
        case 2:
            return out + boldName(names[0]) + plain(" and ") + boldName(names[1])
        default:
            // First two by name, then "+N others"
            let lead = boldName(names[0]) + plain(", ") + boldName(names[1])
            let othersCount = names.count - 2
            return out + lead + plain(" and ") + boldName("\(othersCount) others")
        }
    }

    private func boldName(_ name: String) -> AttributedString {
        var s = AttributedString(name)
        s.font = .caption.weight(.semibold)
        s.foregroundColor = Color.highlighterInkStrong
        return s
    }

    private func plain(_ str: String) -> AttributedString {
        var s = AttributedString(str)
        s.foregroundColor = Color.highlighterInkMuted
        return s
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
        VStack(alignment: .leading, spacing: 12) {
            highlighterByline(for: h)

            if let pageURL = pageImageURL(for: h.highlight) {
                pageHighlight(h, pageURL: pageURL)
            } else {
                textHighlight(h)
            }
        }
    }

    /// Text-only treatment: accent rail + serif italic pull-quote + note.
    private func textHighlight(_ h: HydratedHighlight) -> some View {
        HStack(alignment: .top, spacing: 14) {
            Rectangle()
                .fill(Color.highlighterAccent)
                .frame(width: 3)
                .clipShape(RoundedRectangle(cornerRadius: 1.5))

            VStack(alignment: .leading, spacing: 8) {
                Text(h.highlight.quote.trimmingCharacters(in: .whitespacesAndNewlines))
                    .font(.system(size: 18, design: .default).italic())
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineSpacing(4)
                    .lineLimit(8)
                    .truncationMode(.tail)
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: .infinity, alignment: .leading)

                if !h.highlight.note.isEmpty {
                    Text(h.highlight.note)
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
    private func pageHighlight(_ h: HydratedHighlight, pageURL: URL) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HighlightPageImage(url: pageURL, treatment: .feature)

            VStack(alignment: .leading, spacing: 6) {
                Text(h.highlight.quote.trimmingCharacters(in: .whitespacesAndNewlines))
                    .font(.system(size: 18, design: .default).italic())
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineSpacing(4)
                    .lineLimit(8)
                    .truncationMode(.tail)
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: .infinity, alignment: .leading)

                if !h.highlight.note.isEmpty {
                    Text(h.highlight.note)
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

    private func pageImageURL(for highlight: HighlightRecord) -> URL? {
        let raw = highlight.imageUrl.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !raw.isEmpty else { return nil }
        return URL(string: raw)
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

    // MARK: - Derived: artifact kind

    /// Canonical artifact kind for header rendering. Rust owns source and
    /// reference interpretation; Swift only maps the enum to visual treatment.
    private var artifactKind: HighlightSourceKind {
        app.core.getHighlightSourceKind(
            previewSource: lead.artifact?.preview.source ?? "",
            externalReference: lead.highlight.externalReference,
            artifactAddress: lead.highlight.artifactAddress,
            sourceUrl: lead.highlight.sourceUrl
        )
    }

    private var kindIconName: String {
        switch artifactKind {
        case .article: return "doc.text"
        case .web:     return "globe"
        case .podcast: return "waveform"
        case .book:    return "book.closed"
        case .video:   return "play.rectangle"
        case .paper:   return "doc.richtext"
        case .unknown: return "quote.bubble"
        }
    }

    // MARK: - Derived: resource fields

    private var resourceCoverURL: String? {
        if let img = lead.artifact?.preview.image, !img.isEmpty { return img }
        if artifactKind == .book, let img = bookPreview?.image, !img.isEmpty { return img }
        if artifactKind == .article, let img = sourceArticle?.image, !img.isEmpty { return img }
        if artifactKind == .web, let m = webMetadata {
            if !m.image.isEmpty { return m.image }
            if !m.favicon.isEmpty { return m.favicon }
        }
        return nil
    }

    private var resourceAuthorOrDomain: String {
        switch artifactKind {
        case .article:
            return articleAuthorDisplayName
        case .podcast:
            let show = lead.artifact?.preview.podcastShowTitle ?? ""
            if !show.isEmpty { return show }
            return lead.artifact?.preview.author ?? ""
        case .book:
            return lead.artifact?.preview.author ?? bookPreview?.author ?? ""
        case .web:
            if let m = webMetadata {
                if !m.siteName.isEmpty { return m.siteName }
                if !m.author.isEmpty { return m.author }
            }
            if let domain = lead.artifact?.preview.domain, !domain.isEmpty {
                return domain
            }
            return urlHost ?? ""
        case .video, .paper:
            return lead.artifact?.preview.author ?? (lead.artifact?.preview.domain ?? "")
        case .unknown:
            return urlHost ?? ""
        }
    }

    private var resourceTitle: String {
        switch artifactKind {
        case .article:
            if let t = sourceArticle?.title, !t.isEmpty { return t }
            if let t = lead.artifact?.preview.title, !t.isEmpty { return t }
            return "Untitled"
        case .podcast, .video, .paper:
            if let t = lead.artifact?.preview.title, !t.isEmpty { return t }
            return "Untitled"
        case .book:
            if let t = lead.artifact?.preview.title, !t.isEmpty { return t }
            if let t = bookPreview?.title, !t.isEmpty { return t }
            return "Untitled"
        case .web:
            if let m = webMetadata, !m.title.isEmpty { return m.title }
            if let t = lead.artifact?.preview.title, !t.isEmpty { return t }
            return urlHost ?? "Web page"
        case .unknown:
            if let t = lead.artifact?.preview.title, !t.isEmpty { return t }
            return urlHost ?? "Highlight"
        }
    }

    private var resourceTimeLabel: String? {
        switch artifactKind {
        case .article:
            guard let mins = articleReadMinutes else { return nil }
            return "\(mins) min"
        case .podcast:
            guard let secs = lead.artifact?.preview.durationSeconds, secs > 0 else { return nil }
            return formatDuration(seconds: Int(secs))
        default: return nil
        }
    }

    private var urlHost: String? {
        let raw = lead.highlight.sourceUrl.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !raw.isEmpty, let url = URL(string: raw), let host = url.host else { return nil }
        return host
    }

    /// Source URL the OG/favicon fetcher should hit. Only populated for
    /// the web kind — article/podcast/book branches own their own
    /// hydration path. Prefers the artifact's normalized URL (when a
    /// kind:11 share exists) over the raw highlight `sourceUrl` so the
    /// cache key matches what Rust would store.
    private var webMetadataURL: String? {
        guard artifactKind == .web else { return nil }
        if let u = lead.artifact?.preview.url, !u.isEmpty { return u }
        let raw = lead.highlight.sourceUrl.trimmingCharacters(in: .whitespacesAndNewlines)
        return raw.isEmpty ? nil : raw
    }

    /// Cached enrichment for the web URL (if any). Returns nil for
    /// non-web kinds. The cache key is whatever URL was passed to
    /// `requestWebMetadata` — Rust canonicalizes it, but stores the entry
    /// under the canonical key, so we reach in with the canonical URL too.
    /// In practice the artifact preview URL is already canonical (built
    /// by `normalize_artifact_url`), so this lookup is a direct hit.
    private var webMetadata: WebMetadata? {
        guard let url = webMetadataURL else { return nil }
        return app.webMetadataCache[url]
    }

    // MARK: - Derived: profile / article resolution

    /// Profile-resolved display name for a NIP-23 article author. The resource
    /// subtitle keeps the prior behavior of falling back to the artifact label,
    /// not a pubkey, when the profile is unresolved.
    private var articleAuthorDisplayName: String {
        let pubkey = articleAuthorPubkey ?? ""
        return app.safeCore.projectProfileDisplayWithLabel(
            input: ProfileDisplayWithLabelProjectionInput(
                pubkey: "",
                profile: pubkey.isEmpty ? nil : app.profileSnapshots[pubkey],
                labelFallback: lead.artifact?.preview.author ?? "",
                pubkeyFallback: .pubkey10,
                emptyFallback: ""
            )
        ).displayName
    }

    private var articleAuthorPubkey: String? {
        if let pubkey = sourceArticle?.pubkey, !pubkey.isEmpty { return pubkey }
        if let pubkey = sourceArticleAuthorPubkey, !pubkey.isEmpty { return pubkey }
        return nil
    }

    private var articleReadMinutes: Int? {
        guard let content = sourceArticle?.content, !content.isEmpty else { return nil }
        let words = content.split(whereSeparator: { $0.isWhitespace }).count
        guard words > 60 else { return nil }
        return max(1, words / 240)
    }

    private func formatDuration(seconds: Int) -> String {
        let h = seconds / 3600
        let m = (seconds % 3600) / 60
        if h > 0 { return "\(h)h \(m)m" }
        return "\(m)m"
    }

    // MARK: - Derived: highlighters

    private var uniqueHighlighters: [HydratedHighlight] {
        var seen = Set<String>()
        var out: [HydratedHighlight] = []
        for h in items {
            if seen.insert(h.highlight.pubkey).inserted {
                out.append(h)
            }
        }
        return out
    }

    private var showHighlightersStrip: Bool {
        items.count >= 2 && uniqueHighlighters.count >= 2
    }

    // MARK: - Derived: profile helpers

    private func profileDisplay(for pubkey: String) -> ProfileDisplayProjection {
        app.safeCore.projectProfileDisplay(
            input: ProfileDisplayProjectionInput(
                pubkey: pubkey,
                profile: app.profileSnapshots[pubkey],
                fallback: .pubkey10
            )
        )
    }

    private func relativeDate(_ seconds: UInt64?) -> String? {
        app.safeCore.projectRelativeTimeLabel(
            input: RelativeTimeLabelInput(
                unixSeconds: seconds,
                style: .compact
            )
        ).label
    }

    private func resolveSource() async {
        sourceArticle = nil
        sourceArticleAuthorPubkey = nil

        if let isbn = isbnFromLead {
            await app.requestIsbnPreview(isbn: isbn)
            return
        }

        let addr = lead.highlight.artifactAddress.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !addr.isEmpty else { return }

        let outcome = await app.safeCore.getArticleByAddress(address: addr)
        sourceArticle = outcome.error.isEmpty ? outcome.value : nil
        if let pubkey = sourceArticle?.pubkey, !pubkey.isEmpty {
            sourceArticleAuthorPubkey = pubkey
            await app.requestProfile(pubkeyHex: pubkey)
            return
        }
        let authorOutcome = await app.safeCore.getArticleAddressAuthor(address: addr)
        if authorOutcome.error.isEmpty, let pubkey = authorOutcome.value, !pubkey.isEmpty {
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

            if let pageURL = pageImageURL {
                VStack(alignment: .leading, spacing: 8) {
                    HighlightPageImage(url: pageURL, treatment: .card)
                    quoteBlock
                }
                .padding(12)
            } else {
                HStack(alignment: .top, spacing: 10) {
                    Rectangle()
                        .fill(Color.highlighterAccent)
                        .frame(width: 3)
                        .clipShape(RoundedRectangle(cornerRadius: 1.5))

                    quoteBlock
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

    private var quoteBlock: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(highlight.highlight.quote.trimmingCharacters(in: .whitespacesAndNewlines))
                .font(.system(size: 14, design: .default).italic())
                .foregroundStyle(Color.highlighterInkStrong)
                .lineSpacing(3)
                .lineLimit(6)
                .truncationMode(.tail)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)

            if !highlight.highlight.note.isEmpty {
                Text(highlight.highlight.note)
                    .font(.caption)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(2)
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    private var pageImageURL: URL? {
        let raw = highlight.highlight.imageUrl.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !raw.isEmpty else { return nil }
        return URL(string: raw)
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
        app.safeCore.projectProfileDisplay(
            input: ProfileDisplayProjectionInput(
                pubkey: highlight.highlight.pubkey,
                profile: app.profileSnapshots[highlight.highlight.pubkey],
                fallback: .pubkey10
            )
        )
    }

    private var relative: String? {
        app.safeCore.projectRelativeTimeLabel(
            input: RelativeTimeLabelInput(
                unixSeconds: highlight.highlight.createdAt,
                style: .compact
            )
        ).label
    }
}
