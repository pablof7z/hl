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
            await commentsStore.start(
                scope: commentsScope,
                core: app.safeCore
            )
        }
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
                shareTarget = .highlight(highlight, core: app.safeCore)
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
        let projection = app.safeCore.projectCommentToolbar(
            input: CommentToolbarProjectionInput(records: commentsStore.records)
        )

        return Button {
            showComments = true
        } label: {
            HStack(spacing: 5) {
                Image(systemName: "bubble.left")
                    .font(.system(size: 20, weight: .medium))
                if projection.showsCount {
                    Text(projection.countLabel)
                        .font(.system(size: 14, weight: .semibold, design: .rounded))
                        .monospacedDigit()
                }
            }
            .foregroundStyle(Color.highlighterInkStrong)
        }
        .accessibilityLabel(projection.accessibilityLabel)
    }

    private func actionIcon(systemName: String) -> some View {
        Image(systemName: systemName)
            .font(.system(size: 20, weight: .medium))
            .foregroundStyle(Color.highlighterInkStrong)
    }

    // MARK: - Comments scope

    private var commentsScope: CommentScope? {
        let snapshot = app.safeCore.getHighlightCommentScope(eventIdHex: highlight.eventId)
        return snapshot.attach ? snapshot.scope : nil
    }

    /// Public web URL that the share sheet hands to other apps. The
    /// route at `/highlight/<nevent>` on `beta.highlighter.com` is
    /// server-rendered with full Open Graph + Twitter Card meta so the
    /// link unfurls into a social card built around the quote.
    private func refreshShareURL() async {
        let snapshot = await app.safeCore.getHighlightShareUrlSnapshot(
            eventIdHex: highlight.eventId,
            authorPubkeyHex: highlight.pubkey
        )
        guard snapshot.ready, let url = snapshot.shareUrl else {
            shareURL = nil
            return
        }
        shareURL = URL(string: url)
    }

    // MARK: - Resource projection

    private var resourceProjection: HighlightDetailResourceProjection {
        app.safeCore.projectHighlightDetailResource(
            input: HighlightDetailResourceProjectionInput(item: item)
        )
    }

    private var contentProjection: HighlightDetailContentProjection {
        app.safeCore.projectHighlightDetailContent(
            input: HighlightDetailContentProjectionInput(highlight: highlight)
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
        app.safeCore.projectProfileDisplay(
            input: ProfileDisplayProjectionInput(
                pubkey: highlight.pubkey,
                profile: app.profileSnapshots[highlight.pubkey],
                fallback: .pubkey10
            )
        )
    }

    private func relativeDate(_ seconds: UInt64?) -> String? {
        app.safeCore.projectRelativeTimeLabel(
            input: RelativeTimeLabelInput(
                unixSeconds: seconds,
                style: .ago
            )
        ).label
    }
}
