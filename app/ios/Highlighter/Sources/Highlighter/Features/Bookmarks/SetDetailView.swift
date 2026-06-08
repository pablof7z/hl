import SwiftUI

struct SetDetailView: View {
    @Environment(HighlighterStore.self) private var app
    let record: BookmarkSetRecord

    @State private var articles: [ArticleRecord] = []
    @State private var isLoading = false

    private var displayTitle: String {
        record.title.isEmpty ? (record.id.isEmpty ? "Collection" : record.id) : record.title
    }

    var body: some View {
        Group {
            if isLoading {
                ProgressView()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if articles.isEmpty && record.noteIds.isEmpty {
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
        app.safeCore.projectProfileDisplay(
            input: ProfileDisplayProjectionInput(
                pubkey: record.pubkey,
                profile: app.profileSnapshots[record.pubkey],
                fallback: .pubkey10
            )
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
        isLoading = true
        defer { isLoading = false }

        let outcome = await app.safeCore.getBookmarkSetArticles(record: record)
        articles = outcome.error.isEmpty ? outcome.values : []
    }
}
