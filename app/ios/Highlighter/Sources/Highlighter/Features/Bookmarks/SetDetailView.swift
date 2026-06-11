import SwiftUI

struct SetDetailView: View {
    @Environment(HighlighterStore.self) private var app
    let record: BookmarkSetRecord

    private var detail: HighlighterBookmarkCollectionDetailSnapshot {
        app.bookmarks.selectedCollection
    }

    private var activeRecord: BookmarkSetRecord {
        detail.collection ?? record
    }

    private var displayTitle: String {
        activeRecord.title.isEmpty ? (activeRecord.id.isEmpty ? "Collection" : activeRecord.id) : activeRecord.title
    }

    private var curatorName: String {
        let profile = app.profile(pubkeyHex: activeRecord.pubkey)
        if let dn = profile?.displayName, !dn.isEmpty { return dn }
        if let n = profile?.name, !n.isEmpty { return n }
        return String(activeRecord.pubkey.prefix(10))
    }

    private var curatorInitial: String {
        curatorName.first.map { String($0).uppercased() } ?? ""
    }

    var body: some View {
        Group {
            if detail.isLoading {
                ProgressView()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let errorMessage = detail.errorMessage, !errorMessage.isEmpty {
                ContentUnavailableView {
                    Label("Collection unavailable", systemImage: "rectangle.stack")
                } description: {
                    Text(errorMessage)
                }
            } else if detail.articles.isEmpty && !detail.hasNoteItems {
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
        .task(id: "\(record.pubkey):\(record.kind):\(record.id)") {
            app.openBookmarkCollection(record)
        }
        .refreshable {
            app.refreshBookmarkCollection()
        }
        .task(id: activeRecord.pubkey) {
            app.requestProfile(pubkeyHex: activeRecord.pubkey)
        }
    }

    private var curatorHeader: some View {
        HStack(spacing: 10) {
            AuthorAvatar(
                pubkey: activeRecord.pubkey,
                pictureURL: app.profile(pubkeyHex: activeRecord.pubkey)?.picture ?? "",
                displayInitial: curatorInitial,
                size: 32
            )
            VStack(alignment: .leading, spacing: 1) {
                Text("Curated by")
                    .font(.caption2.weight(.medium))
                    .foregroundStyle(Color.highlighterInkMuted)
                    .textCase(.uppercase)
                    .tracking(0.6)
                Text(curatorName)
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

    private var articleList: some View {
        ScrollView {
            LazyVStack(spacing: 0) {
                curatorHeader
                Divider()
                ForEach(detail.articles, id: \.eventId) { article in
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
}
