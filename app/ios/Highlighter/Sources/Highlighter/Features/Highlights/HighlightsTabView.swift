import SwiftUI

/// Home tab — a single chronological stream mixing highlights from people
/// the user follows (and their rooms) with articles surfaced by the same
/// social graph. Friend-highlighted articles use the pull-quote card (the
/// friend's voice carries an inline read-strip); articles surfaced by other
/// signals fall through to the existing `ReadingFeedCardView`.
struct HighlightsTabView: View {
    @Environment(HighlighterStore.self) private var app
    @State private var shareTarget: ShareToCommunityTarget?
    @State private var capturePresented: Bool = false

    var body: some View {
        NavigationStack {
            content(feed: app.homeFeed)
            .navigationTitle("Highlights")
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        capturePresented = true
                    } label: {
                        Image(systemName: "plus")
                    }
                    .accessibilityLabel("Capture highlight")
                }
            }
            .navigationDestination(for: HighlightDetailTarget.self) { target in
                HighlightDetailView(item: target.item)
            }
            .navigationDestination(for: ArticleReaderTarget.self) { target in
                ArticleReaderView(target: target)
            }
            .navigationDestination(for: WebReaderTarget.self) { target in
                WebReaderView(target: target)
            }
            .navigationDestination(for: BookTarget.self) { target in
                BookView(catalogId: target.catalogId)
            }
            .navigationDestination(for: ProfileDestination.self) { destination in
                if case .pubkey(let pk) = destination {
                    ProfileView(pubkey: pk)
                }
            }
            .globalUserToolbar()
        }
        .sheet(item: $shareTarget) { target in
            ShareToCommunitySheet(target: target)
                .presentationDetents([.medium, .large])
        }
        .captureFlow(isPresented: $capturePresented)
        .task {
            app.openHomeFeed()
        }
        .refreshable {
            app.refreshHomeFeed()
        }
        .onDisappear {
            app.closeHomeFeed()
        }
    }

    @ViewBuilder
    private func content(feed: HighlighterHomeFeedSnapshot) -> some View {
        if feed.isLoading && feed.items.isEmpty {
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if feed.items.isEmpty {
            emptyState
        } else {
            feedList(feed: feed)
        }
    }

    private func feedList(feed: HighlighterHomeFeedSnapshot) -> some View {
        ScrollView {
            LazyVStack(spacing: 0) {
                ForEach(Array(feed.items.enumerated()), id: \.element.stableId) { index, item in
                    row(for: item)
                    if index < feed.items.count - 1 {
                        Rectangle()
                            .fill(Color.highlighterRule)
                            .frame(height: 1)
                    }
                }
            }
            .padding(.horizontal, 12)
        }
        .background(Color.highlighterPaper.ignoresSafeArea())
    }

    private var emptyState: some View {
        VStack(alignment: .leading, spacing: 14) {
            Rectangle()
                .fill(Color.highlighterAccent.opacity(0.6))
                .frame(width: 3, height: 28)
                .clipShape(RoundedRectangle(cornerRadius: 1.5))
            Text("No highlights yet")
                .font(.system(.title2, design: .default).weight(.semibold))
                .foregroundStyle(Color.highlighterInkStrong)
            Text("Quotes surfaced by people you follow, your rooms, and articles from your network will land here.")
                .font(.system(.subheadline, design: .default))
                .foregroundStyle(Color.highlighterInkMuted)
                .lineSpacing(3)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: 360, alignment: .leading)
        .padding(.horizontal, 32)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
        .background(Color.highlighterPaper.ignoresSafeArea())
    }

    @ViewBuilder
    private func row(for item: HighlighterHomeFeedItem) -> some View {
        switch item {
        case .highlights(stableId: _, sortKey: _, highlights: let hs, highlightCount: _):
            highlightRow(hs)
        case .read(stableId: _, sortKey: _, item: let r):
            readRow(r)
        }
    }

    // MARK: - Highlight row

    @ViewBuilder
    private func highlightRow(_ items: [HydratedHighlight]) -> some View {
        let lead = items[0]
        NavigationLink(value: HighlightDetailTarget(item: lead)) {
            HighlightFeedCardView(items: items)
        }
        .buttonStyle(.plain)
        .contextMenu { highlightContextMenu(lead) }
    }

    @ViewBuilder
    private func highlightContextMenu(_ item: HydratedHighlight) -> some View {
        Button {
            shareTarget = .highlight(item.highlight)
        } label: {
            Label("Share quote to room", systemImage: "quote.bubble")
        }
        if let target = shareTargetForHighlight(item) {
            Button {
                shareTarget = target
            } label: {
                Label("Share article to room", systemImage: "doc.text")
            }
        }
        Button {
            UIPasteboard.general.string = item.highlight.quote
        } label: {
            Label("Copy quote", systemImage: "doc.on.doc")
        }
    }

    // MARK: - Article (read-only surfacing) row

    private func readRow(_ item: ReadingFeedItem) -> some View {
        NavigationLink(value: ArticleReaderTarget(
            pubkey: item.article.pubkey,
            dTag: item.article.identifier,
            seed: item.article
        )) {
            ReadingFeedCardView(item: item)
        }
        .buttonStyle(.plain)
        .contextMenu {
            Button {
                shareTarget = .article(item.article)
            } label: {
                Label("Share to community", systemImage: "square.and.arrow.up")
            }
        }
    }

    /// Share-to-community target. Only supported for NIP-23 article
    /// highlights today — we reshare the source article, not the quote.
    private func shareTargetForHighlight(_ item: HydratedHighlight) -> ShareToCommunityTarget? {
        if let existing = item.artifact {
            return .artifact(existing)
        }
        let addr = item.highlight.artifactAddress.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !addr.isEmpty else { return nil }
        let parts = addr.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false)
        guard parts.count == 3, parts[0] == "30023" else { return nil }
        let dTag = String(parts[2])
        let preview = ArtifactPreview(
            id: dTag,
            url: "",
            title: "",
            author: "",
            image: "",
            description: "",
            source: "article",
            domain: "",
            catalogId: "",
            catalogKind: "",
            podcastGuid: "",
            podcastItemGuid: "",
            podcastShowTitle: "",
            audioUrl: "",
            audioPreviewUrl: "",
            transcriptUrl: "",
            feedUrl: "",
            publishedAt: "",
            durationSeconds: nil,
            referenceTagName: "a",
            referenceTagValue: addr,
            referenceKind: "30023",
            highlightTagName: "a",
            highlightTagValue: addr,
            highlightReferenceKey: "a:\(addr)",
            chapters: []
        )
        return ShareToCommunityTarget(
            kind: .artifactShare(preview: preview),
            displayTitle: "Article",
            displaySubtitle: item.highlight.quote,
            imageURL: nil
        )
    }
}

private extension HighlighterHomeFeedItem {
    var stableId: String {
        switch self {
        case .highlights(stableId: let stableId, sortKey: _, highlights: _, highlightCount: _):
            stableId
        case .read(stableId: let stableId, sortKey: _, item: _):
            stableId
        }
    }
}
