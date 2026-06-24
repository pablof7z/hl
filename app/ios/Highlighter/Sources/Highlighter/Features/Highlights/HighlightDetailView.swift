import Kingfisher
import SwiftUI

/// Navigation payload for the highlight-centric detail view. Wraps the
/// hydrated highlight so the destination has full context without a
/// round-trip; Hashable so the parent NavigationStack can dispatch via
/// `.navigationDestination(for:)`.
struct HighlightDetailTarget: Hashable {
    let item: HydratedHighlight
}

/// Detail screen that puts a single highlight at the centerpiece. The
/// underlying artifact (article / book / web / podcast) is reduced to a
/// compact, tappable header at the top — a one-tap escape hatch into the
/// reader views the feed previously navigated to directly. The action
/// bar surfaces:
///   - bookmark (source article only; existing kind:10003/30004 flow)
///   - comments (NIP-22, scoped to the kind:9802 highlight event)
///   - share (system share sheet — `https://beta.highlighter.com/highlight/<nevent>`
///     URL that the SvelteKit web app server-renders into a social card)
///   - add to room (kind:16 repost into one of the user's NIP-29 rooms)
struct HighlightDetailView: View {
    @Environment(HighlighterStore.self) private var app
    @Environment(HighlighterAppKernel.self) private var kernel

    let item: HydratedHighlight

    @State private var commentsStore = CommentsStore()
    @State private var commentsStarted = false
    @State private var showComments = false
    @State private var shareTarget: ShareToCommunityTarget?
    @State private var shareURL: URL?

    private var highlight: HighlightRecord { item.highlight }

    var body: some View {
        let content = contentProjection

        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                resourceHeader
                bylineRow
                quoteBlock(content)
                if let note = content.noteText {
                    noteBlock(note)
                }
                actionBar(content)
                    .padding(.top, 4)
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 24)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(Color.highlighterPaper.ignoresSafeArea())
        .navigationTitle("Highlight")
        .navigationBarTitleDisplayMode(.inline)
        .navigationDestination(isPresented: $showComments) {
            if let commentsScope {
                CommentsView(
                    scope: commentsScope,
                    artifactAuthorPubkey: highlight.pubkey,
                    artifactHeader: nil,
                    store: commentsStore
                )
            }
        }
        .sheet(item: $shareTarget) { target in
            ShareToCommunitySheet(target: target)
                .presentationDetents([.medium, .large])
        }
        .task {
            guard !commentsStarted, let commentsScope else { return }
            commentsStarted = true
            await commentsStore.start(scope: commentsScope, kernel: kernel)
        }
        .onChange(of: commentsScope.map { kernel.commentThreads[$0.rootTagValue] }) { _, _ in
            commentsStore.applyKernelSnapshot()
        }
        .onDisappear { commentsStore.stop() }
        .task(id: highlight.eventId) {
            await refreshShareURL()
        }
        .task(id: highlight.pubkey) {
            await app.requestProfile(pubkeyHex: highlight.pubkey)
        }
    }

    // MARK: - Resource header (tappable → opens artifact view)

    @ViewBuilder
    private var resourceHeader: some View {
        let resource = resourceProjection

        if let route = resource.articleRoute {
            NavigationLink(value: ArticleReaderTarget(route: route)) {
                resourceHeaderCard(resource, showsChevron: true)
            }
                .buttonStyle(.plain)
        } else if let catalogId = resource.bookCatalogId {
            NavigationLink(value: BookTarget(catalogId: catalogId)) {
                resourceHeaderCard(resource, showsChevron: true)
            }
                .buttonStyle(.plain)
        } else if let t = webReaderTarget(resource) {
            NavigationLink(value: t) {
                resourceHeaderCard(resource, showsChevron: true)
            }
                .buttonStyle(.plain)
        } else {
            resourceHeaderCard(resource, showsChevron: false)
        }
    }

    private func resourceHeaderCard(
        _ resource: HighlightDetailResourceProjection,
        showsChevron: Bool
    ) -> some View {
        HStack(alignment: .center, spacing: 12) {
            resourceCover(resource)
                .frame(width: 40, height: 40)
                .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))

            VStack(alignment: .leading, spacing: 2) {
                Text(resource.kindLabel.uppercased())
                    .font(.caption2.weight(.bold))
                    .tracking(0.6)
                    .foregroundStyle(Color.highlighterInkMuted)
                Text(resource.title)
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(2)
                    .multilineTextAlignment(.leading)
                if !resource.author.isEmpty {
                    Text(resource.author)
                        .font(.caption)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(1)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            if showsChevron {
                Image(systemName: "chevron.right")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkMuted)
            }
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(Color.highlighterPaperTint)
        )
    }

    @ViewBuilder
    private func resourceCover(_ resource: HighlightDetailResourceProjection) -> some View {
        if let urlString = resource.coverUrl,
           !urlString.isEmpty,
           let url = URL(string: urlString) {
            KFImage(url)
                .placeholder { coverFallback(resource) }
                .fade(duration: 0.15)
                .resizable()
                .scaledToFill()
        } else {
            coverFallback(resource)
        }
    }

    private func coverFallback(_ resource: HighlightDetailResourceProjection) -> some View {
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
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(Color.highlighterInkStrong.opacity(0.55))
        }
    }

    // MARK: - Highlighter byline

    @ViewBuilder
    private var bylineRow: some View {
        let highlighter = highlighterDisplay

        NavigationLink(value: ProfileDestination.pubkey(highlight.pubkey)) {
            HStack(spacing: 10) {
                AuthorAvatar(
                    pubkey: highlight.pubkey,
                    pictureURL: highlighter.pictureUrl,
                    displayInitial: highlighter.displayInitial,
                    size: 32
                )
                VStack(alignment: .leading, spacing: 2) {
                    Text(highlighter.displayName)
                        .font(.subheadline.weight(.semibold))
                        .foregroundStyle(Color.highlighterInkStrong)
                        .lineLimit(1)
                    if let rel = relativeDate(highlight.createdAt) {
                        Text(rel)
                            .font(.caption)
                            .foregroundStyle(Color.highlighterInkMuted)
                    }
                }
                Spacer(minLength: 0)
            }
        }
        .buttonStyle(.plain)
    }

    // MARK: - Quote

    @ViewBuilder
    private func quoteBlock(_ content: HighlightDetailContentProjection) -> some View {
        if let pageURL = pageImageURL(content) {
            VStack(alignment: .leading, spacing: 14) {
                HighlightPageImage(url: pageURL, treatment: .feature)
                quoteText(content)
            }
        } else {
            HStack(alignment: .top, spacing: 14) {
                Rectangle()
                    .fill(Color.highlighterAccent)
                    .frame(width: 3)
                    .clipShape(RoundedRectangle(cornerRadius: 1.5))
                quoteText(content)
            }
        }
    }

    private func quoteText(_ content: HighlightDetailContentProjection) -> some View {
        Text(content.quoteText)
            .font(.system(size: 21, design: .default).italic())
            .foregroundStyle(Color.highlighterInkStrong)
            .lineSpacing(5)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
            .textSelection(.enabled)
    }

    private func noteBlock(_ note: String) -> some View {
        return VStack(alignment: .leading, spacing: 6) {
            Text("NOTE")
                .font(.caption2.weight(.bold))
                .tracking(0.6)
                .foregroundStyle(Color.highlighterInkMuted)
            Text(note)
                .font(.system(.body, design: .default))
                .foregroundStyle(Color.highlighterInkStrong)
                .lineSpacing(3)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
                .textSelection(.enabled)
        }
        .padding(.leading, 17)
    }

    // MARK: - Action bar

    private func actionBar(_ content: HighlightDetailContentProjection) -> some View {
        return HStack(spacing: 22) {
            if let articleAddress = articleAddressForBookmark {
                BookmarkMenuButton(articleAddress: articleAddress)
                    .font(.system(size: 20, weight: .medium))
            }

            commentsButton

            if let url = shareURL {
                ShareLink(
                    item: url,
                    subject: Text("Highlight"),
                    message: Text(content.shareMessage)
                ) {
                    actionIcon(systemName: "square.and.arrow.up")
                }
                .accessibilityLabel("Share highlight")
            }

            Button {
                shareTarget = .highlight(highlight)
            } label: {
                actionIcon(systemName: "rectangle.stack.badge.plus")
            }
            .disabled(app.joinedCommunities.isEmpty)
            .accessibilityLabel("Add to room")

            Spacer(minLength: 0)
        }
        .padding(.vertical, 12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .overlay(alignment: .top) {
            Rectangle()
                .fill(Color.highlighterRule.opacity(0.5))
                .frame(height: 1)
        }
    }

    private var commentsButton: some View {
        let count = commentsStore.records.count
        let countLabel = count > 99 ? "99+" : "\(count)"
        let a11yLabel = count == 0 ? "Comments" : "\(count) comment\(count == 1 ? "" : "s")"

        return Button {
            showComments = true
        } label: {
            HStack(spacing: 5) {
                Image(systemName: "bubble.left")
                    .font(.system(size: 20, weight: .medium))
                if count > 0 {
                    Text(countLabel)
                        .font(.system(size: 14, weight: .semibold, design: .rounded))
                        .monospacedDigit()
                }
            }
            .foregroundStyle(Color.highlighterInkStrong)
        }
        .accessibilityLabel(a11yLabel)
    }

    private func actionIcon(systemName: String) -> some View {
        Image(systemName: systemName)
            .font(.system(size: 20, weight: .medium))
            .foregroundStyle(Color.highlighterInkStrong)
    }

    // MARK: - Comments scope

    private var commentsScope: CommentScope? {
        // D1: highlight comment scope is always event-id + kind 9802.
        CommentScope(rootTagName: "E", rootTagValue: highlight.eventId, rootKind: 9802)
    }

    /// Public web URL that the share sheet hands to other apps. The
    /// route at `/highlight/<nevent>` on `beta.highlighter.com` is
    /// server-rendered with full Open Graph + Twitter Card meta so the
    /// link unfurls into a social card built around the quote.
    private func refreshShareURL() async {
        // Stub: use the hex event ID URL until the kernel provides NIP-19
        // nevent encoding (bech32 + TLV). The server resolves by hex too.
        shareURL = URL(string: "https://beta.highlighter.com/highlight/\(highlight.eventId)")
    }

    // MARK: - Resource projection

    private var resourceProjection: HighlightDetailResourceProjection {
        // D1: inline highlight_detail_resource_projection.
        let preview = item.artifact?.preview
        let artAddr = highlight.artifactAddress.trimmingCharacters(in: .whitespaces)

        let kind = Self.highlightSourceKind(
            previewSource: preview?.source ?? "",
            externalReference: highlight.externalReference,
            artifactAddress: artAddr,
            sourceUrl: highlight.sourceUrl
        )

        // URL host used as title/author fallback for web highlights.
        let urlHost = Self.urlHostFromString(highlight.sourceUrl)

        // Article route: valid "30023:pk:d" address only.
        let articleRoute: ArticleReaderRoute? = Self.parseArticleRoute(artAddr)

        // Book catalog id: "isbn:..." from externalReference or artifactAddress.
        let bookCatalogId: String? = Self.bookCatalogId(
            externalReference: highlight.externalReference,
            artifactAddress: artAddr
        )

        // Web URL: http/https only.
        let webUrl: String? = {
            let t = highlight.sourceUrl.trimmingCharacters(in: .whitespaces)
            guard !t.isEmpty,
                  let url = URL(string: t),
                  url.scheme == "http" || url.scheme == "https" else { return nil }
            return t
        }()

        let title: String = {
            if let t = preview?.title, !t.isEmpty { return t }
            if let h = urlHost { return h }
            return "Untitled"
        }()

        let author: String = {
            if let a = preview?.author, !a.isEmpty { return a }
            if let d = preview?.domain, !d.isEmpty { return d }
            if let h = urlHost { return h }
            return ""
        }()

        let coverUrl: String? = preview.flatMap { !$0.image.isEmpty ? $0.image : nil }

        return HighlightDetailResourceProjection(
            sourceKind: kind,
            kindLabel: Self.highlightKindLabel(kind),
            iconSystemName: Self.highlightSourceKindIconName(kind),
            title: title,
            author: author,
            coverUrl: coverUrl,
            articleRoute: articleRoute,
            bookCatalogId: bookCatalogId,
            webUrl: webUrl
        )
    }

    // MARK: - Highlight source kind helpers (D1 inlined from Rust highlights.rs)

    private static func highlightSourceKind(
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
        if parseArticleRoute(artifactAddress) != nil { return .article }
        if artifactAddress.trimmingCharacters(in: .whitespaces).lowercased().hasPrefix("isbn:") {
            return .book
        }
        if !sourceUrl.trimmingCharacters(in: .whitespaces).isEmpty { return .web }
        return .unknown
    }

    private static func parseArticleRoute(_ address: String) -> ArticleReaderRoute? {
        let t = address.trimmingCharacters(in: .whitespaces)
        guard !t.isEmpty else { return nil }
        let parts = t.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false)
        guard parts.count == 3,
              parts[0] == "30023" else { return nil }
        let pubkey = String(parts[1]).trimmingCharacters(in: .whitespaces)
        let dTag   = String(parts[2]).trimmingCharacters(in: .whitespaces)
        guard !pubkey.isEmpty, !dTag.isEmpty else { return nil }
        return ArticleReaderRoute(address: t, pubkey: pubkey, dTag: dTag)
    }

    private static func bookCatalogId(externalReference: String, artifactAddress: String) -> String? {
        for ref in [externalReference, artifactAddress] {
            let t = ref.trimmingCharacters(in: .whitespaces)
            guard t.lowercased().hasPrefix("isbn:") else { continue }
            let isbn = String(t.dropFirst(5)).trimmingCharacters(in: .whitespaces)
            if !isbn.isEmpty { return "isbn:\(isbn)" }
        }
        return nil
    }

    private static func urlHostFromString(_ rawUrl: String) -> String? {
        let t = rawUrl.trimmingCharacters(in: .whitespaces)
        guard !t.isEmpty else { return nil }
        return URL(string: t)?.host
    }

    private static func highlightKindLabel(_ kind: HighlightSourceKind) -> String {
        switch kind {
        case .article: return "Article"
        case .book:    return "Book"
        case .podcast: return "Podcast"
        case .web:     return "Web"
        case .video:   return "Video"
        case .paper:   return "Paper"
        case .unknown: return "Source"
        }
    }

    private static func highlightSourceKindIconName(_ kind: HighlightSourceKind) -> String {
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

    private var contentProjection: HighlightDetailContentProjection {
        let trimmedImage = highlight.imageUrl.trimmingCharacters(in: .whitespaces)
        let quoteText = highlight.quote.trimmingCharacters(in: .whitespaces)
        return HighlightDetailContentProjection(
            quoteText: quoteText,
            noteText: highlight.note.trimmingCharacters(in: .whitespaces).isEmpty ? nil : highlight.note,
            pageImageUrl: trimmedImage.isEmpty ? nil : trimmedImage,
            shareMessage: quoteText
        )
    }

    /// Article a-tag we can bookmark. Only NIP-23 articles are bookmarkable
    /// today; Rust owns the address interpretation and returns the route.
    private var articleAddressForBookmark: String? {
        resourceProjection.articleRoute?.address
    }

    private func webReaderTarget(_ resource: HighlightDetailResourceProjection) -> WebReaderTarget? {
        guard let raw = resource.webUrl, let url = URL(string: raw) else { return nil }
        return WebReaderTarget(url: url, highlightQuote: highlight.quote)
    }

    // MARK: - Resource metadata

    private func pageImageURL(_ content: HighlightDetailContentProjection) -> URL? {
        guard let raw = content.pageImageUrl else { return nil }
        return URL(string: raw)
    }

    // MARK: - Profile helpers

    private var highlighterDisplay: ProfileDisplayProjection {
        let profile = app.profileSnapshots[highlight.pubkey]
        let name = (profile?.displayName ?? "").isEmpty
            ? ((profile?.name ?? "").isEmpty ? String(highlight.pubkey.prefix(10)) : profile!.name)
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
}
