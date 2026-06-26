import SwiftUI

struct SetDetailView: View {
    @Environment(HighlighterStore.self) private var app
    let record: BookmarkSetRecord

    @State private var articles: [ArticleRecord] = []
    @State private var displayTitle = ""
    @State private var isCollectionEmpty = false
    @State private var isLoading = false

    /// Canonical web share URL for this set, resolved in Rust. `nil` for
    /// non-30004 sets (the FFI rejects them) so the toolbar Share item only
    /// appears for curation sets (#63).
    private var shareURL: URL? {
        let snapshot = curationSetShareUrlSnapshot(coordinate: record.setCoordinate)
        guard snapshot.error.isEmpty, !snapshot.url.isEmpty else { return nil }
        return URL(string: snapshot.url)
    }

    var body: some View {
        Group {
            if isLoading {
                ProgressView()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if isCollectionEmpty {
                ContentUnavailableView {
                    Label("Empty Collection", systemImage: "rectangle.stack")
                } description: {
                    Text("No items have been added to this collection yet.")
                }
            } else {
                articleList
            }
        }
        .navigationTitle(displayTitle)
        .navigationBarTitleDisplayMode(.large)
        .toolbar {
            if let url = shareURL {
                ToolbarItem(placement: .topBarTrailing) {
                    ShareLink(item: url, subject: Text(displayTitle)) {
                        Image(systemName: "square.and.arrow.up")
                    }
                    .accessibilityLabel("Share collection")
                }
            }
        }
        .task { await loadArticles() }
        .task(id: record.pubkey) {
            await app.requestProfile(pubkeyHex: record.pubkey)
        }
    }

    @ViewBuilder
    private var curatorHeader: some View {
        let curator = curatorDisplay

        HStack(spacing: 10) {
            AuthorAvatar(
                pubkey: record.pubkey,
                pictureURL: curator.pictureUrl,
                displayInitial: curator.displayInitial,
                size: 32
            )
            VStack(alignment: .leading, spacing: 1) {
                Text("Curated by")
                    .font(.caption2.weight(.medium))
                    .foregroundStyle(Color.highlighterInkMuted)
                    .textCase(.uppercase)
                    .tracking(0.6)
                Text(curator.displayName)
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(1)
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .background(Color.highlighterAccent.opacity(0.06))
    }

    private var curatorDisplay: ProfileDisplayProjection {
        let profile = app.profileSnapshots[record.pubkey]
        let pubkey = record.pubkey
        let displayName: String = {
            if let dn = profile?.displayName, !dn.isEmpty { return dn }
            if let n = profile?.name, !n.isEmpty { return n }
            return String(pubkey.prefix(10))
        }()
        let displayInitial: String = {
            if let dn = profile?.displayName, !dn.isEmpty { return String(dn.prefix(1)) }
            if let n = profile?.name, !n.isEmpty { return String(n.prefix(1)) }
            return String(pubkey.prefix(1))
        }()
        return ProfileDisplayProjection(
            displayName: displayName,
            displayInitial: displayInitial,
            pictureUrl: profile?.picture ?? ""
        )
    }

    private var articleList: some View {
        ScrollView {
            LazyVStack(spacing: 0) {
                curatorHeader
                Divider()
                ForEach(articles, id: \.eventId) { article in
                    NavigationLink(value: ArticleReaderTarget(article: article, seed: article)) {
                        BookmarkedArticleRow(article: article)
                            .padding(.horizontal, 16)
                            .padding(.vertical, 12)
                    }
                    .buttonStyle(.plain)
                    Divider().padding(.leading, 84)
                }
            }
        }
    }

    private func loadArticles() async {
        // Kernel does not have a getBookmarkSetDetailSnapshot projection yet.
        // Degrade gracefully: show an empty set rather than calling bespoke core.
        articles = []
        isCollectionEmpty = true
    }
}
