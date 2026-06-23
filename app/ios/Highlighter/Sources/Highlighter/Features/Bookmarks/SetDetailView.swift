import SwiftUI

struct SetDetailView: View {
    @Environment(HighlighterStore.self) private var app
    let record: BookmarkSetRecord

    @State private var articles: [ArticleRecord] = []
    @State private var displayTitle = ""
    @State private var isCollectionEmpty = false
    @State private var isLoading = false

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
        {
            let profile = app.profileSnapshots[record.pubkey]
            let name = (profile?.displayName ?? "").isEmpty
                ? ((profile?.name ?? "").isEmpty ? String(record.pubkey.prefix(10)) : profile!.name)
                : profile!.displayName
            return ProfileDisplayProjection(
                displayName: name,
                displayInitial: name.first.map { String($0).uppercased() } ?? "?",
                pictureUrl: profile?.picture ?? ""
            )
        }()
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
        isLoading = true
        defer { isLoading = false }

        let snapshot = await app.safeCore.getBookmarkSetDetailSnapshot(record: record)
        displayTitle = snapshot.displayTitle
        articles = snapshot.articles
        isCollectionEmpty = snapshot.isEmpty
    }
}
