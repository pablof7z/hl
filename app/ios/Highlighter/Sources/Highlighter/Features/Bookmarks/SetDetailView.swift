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
    }

    private var articleList: some View {
        ScrollView {
            LazyVStack(spacing: 0) {
                ForEach(articles, id: \.eventId) { article in
                    NavigationLink(value: ArticleReaderTarget(pubkey: article.pubkey, dTag: article.identifier, seed: article)) {
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

        var loaded: [ArticleRecord] = []
        for address in record.articleAddresses {
            let parts = address.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false)
            guard parts.count == 3 else { continue }
            let pubkey = String(parts[1])
            let dTag = String(parts[2])
            guard !pubkey.isEmpty, !dTag.isEmpty else { continue }
            if let article = try? await app.safeCore.getArticle(pubkeyHex: pubkey, dTag: dTag) {
                loaded.append(article)
            }
        }
        articles = loaded.sorted {
            ($0.publishedAt ?? $0.createdAt ?? 0) > ($1.publishedAt ?? $1.createdAt ?? 0)
        }
    }
}
