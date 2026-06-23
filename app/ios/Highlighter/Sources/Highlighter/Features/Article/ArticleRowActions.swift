import SwiftUI

/// Attaches Bookmark + Share-to-community actions to any article row.
///
/// - `.swipeActions` fires when the row lives inside a `List`. Leading edge:
///   Bookmark (accent). Trailing edge: Share.
/// - `.contextMenu` fires on long-press regardless of container (works in
///   `LazyVStack` too), so rows that aren't inside a `List` still expose the
///   same affordances via long-press.
///
/// Usage:
///
///     NavigationLink(value: target) {
///         ArticleCardView(article: article)
///     }
///     .articleRowActions(article: article)
extension View {
    func articleRowActions(article: ArticleRecord) -> some View {
        modifier(ArticleRowActionsModifier(article: article))
    }
}

private struct ArticleRowActionsModifier: ViewModifier {
    @Environment(HighlighterStore.self) private var app
    let article: ArticleRecord
    @State private var shareTarget: ShareToCommunityTarget?

    private var address: String {
        article.address
    }

    private var isBookmarked: Bool {
        app.isBookmarked(articleAddress: address)
    }

    private var bookmarkChrome: (swipeTitle: String, menuTitle: String, systemImage: String) {
        isBookmarked
            ? (swipeTitle: "Remove", menuTitle: "Remove Bookmark", systemImage: "bookmark.slash")
            : (swipeTitle: "Bookmark", menuTitle: "Bookmark", systemImage: "bookmark")
    }

    func body(content: Content) -> some View {
        content
            .swipeActions(edge: .leading, allowsFullSwipe: true) {
                Button {
                    Task { await app.toggleBookmark(articleAddress: address) }
                } label: {
                    Label(
                        bookmarkChrome.swipeTitle,
                        systemImage: bookmarkChrome.systemImage
                    )
                }
                .tint(Color.highlighterAccent)
            }
            .swipeActions(edge: .trailing, allowsFullSwipe: false) {
                Button {
                    shareTarget = ShareToCommunityTarget.article(article, core: app.core)
                } label: {
                    Label("Share", systemImage: "square.and.arrow.up")
                }
                .tint(.blue)
            }
            .contextMenu {
                Button {
                    Task { await app.toggleBookmark(articleAddress: address) }
                } label: {
                    Label(
                        bookmarkChrome.menuTitle,
                        systemImage: bookmarkChrome.systemImage
                    )
                }
                Button {
                    shareTarget = ShareToCommunityTarget.article(article, core: app.core)
                } label: {
                    Label("Share to community", systemImage: "square.and.arrow.up")
                }
            }
            .sheet(item: $shareTarget) { target in
                ShareToCommunitySheet(target: target)
                    .presentationDetents([.medium, .large])
            }
    }
}
