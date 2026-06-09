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
        app.safeCore.getHighlightBookRoute(
            externalReference: lead.highlight.externalReference,
            artifactAddress: lead.highlight.artifactAddress
        )?.isbn
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
        app.safeCore.projectHighlightFeedContent(
            input: HighlightFeedContentProjectionInput(highlight: highlight)
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
        let base = app.safeCore.projectHighlightResourceHeader(
            input: resourceProjectionInput(webMetadata: nil)
        )
        guard let url = base.webMetadataUrl,
              let metadata = app.webMetadataCache[url] else {
            return base
        }
        return app.safeCore.projectHighlightResourceHeader(
            input: resourceProjectionInput(webMetadata: metadata)
        )
    }

    private func resourceProjectionInput(webMetadata: WebMetadata?) -> HighlightResourceHeaderProjectionInput {
        HighlightResourceHeaderProjectionInput(
            lead: lead,
            sourceArticle: sourceArticle,
            sourceArticleAuthorPubkey: sourceArticleAuthorPubkey ?? "",
            articleAuthorProfiles: articleAuthorProfileCandidates,
            bookPreview: bookPreview,
            webMetadata: webMetadata
        )
    }

    private var articleAuthorProfileCandidates: [HighlightResourceAuthorProfile] {
        var candidates: [HighlightResourceAuthorProfile] = []
        if let pubkey = sourceArticle?.pubkey, !pubkey.isEmpty {
            candidates.append(
                HighlightResourceAuthorProfile(
                    pubkey: pubkey,
                    profile: app.profileSnapshots[pubkey]
                )
            )
        }
        if let pubkey = sourceArticleAuthorPubkey,
           !pubkey.isEmpty,
           !candidates.contains(where: { $0.pubkey == pubkey }) {
            candidates.append(
                HighlightResourceAuthorProfile(
                    pubkey: pubkey,
                    profile: app.profileSnapshots[pubkey]
                )
            )
        }
        return candidates
    }

    // MARK: - Derived: group projection

    private var groupProjection: HighlightGroupCardProjection {
        app.safeCore.projectHighlightGroupCard(
            input: HighlightGroupCardProjectionInput(
                items: items,
                highlighterProfiles: items.map { h in
                    HighlightGroupHighlighterProfile(
                        pubkey: h.highlight.pubkey,
                        profile: app.profileSnapshots[h.highlight.pubkey]
                    )
                }
            )
        )
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

        let outcome = await app.safeCore.getArticleByAddress(address: addr)
        sourceArticle = outcome.error.isEmpty ? outcome.value : nil
        if let pubkey = sourceArticle?.pubkey, !pubkey.isEmpty {
            sourceArticleAuthorPubkey = pubkey
            await app.requestProfile(pubkeyHex: pubkey)
            return
        }
        if let pubkey = await app.safeCore.getArticleAddressAuthor(address: addr), !pubkey.isEmpty {
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
        let content = app.safeCore.projectHighlightFeedContent(
            input: HighlightFeedContentProjectionInput(highlight: highlight.highlight)
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
