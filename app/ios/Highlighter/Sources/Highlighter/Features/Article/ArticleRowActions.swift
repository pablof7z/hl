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

    private var bookmarkChrome: ArticleBookmarkChromeProjection {
        isBookmarked
            ? ArticleBookmarkChromeProjection(
                toolbarSystemImage: "bookmark.fill", usesAccentColor: true,
                accessibilityLabel: "Remove bookmark", swipeTitle: "Remove",
                menuTitle: "Remove bookmark", actionSystemImage: "bookmark.slash")
            : ArticleBookmarkChromeProjection(
                toolbarSystemImage: "bookmark", usesAccentColor: false,
                accessibilityLabel: "Bookmark article", swipeTitle: "Bookmark",
                menuTitle: "Bookmark", actionSystemImage: "bookmark")
    }

    func body(content: Content) -> some View {
        content
            .swipeActions(edge: .leading, allowsFullSwipe: true) {
                Button {
                    Task { await app.toggleBookmark(articleAddress: address) }
                } label: {
                    Label(
                        bookmarkChrome.swipeTitle,
                        systemImage: bookmarkChrome.actionSystemImage
                    )
                }
                .tint(Color.highlighterAccent)
            }
            .swipeActions(edge: .trailing, allowsFullSwipe: false) {
                Button {
                    shareTarget = ShareToCommunityTarget.article(article, core: app.safeCore)
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
                        systemImage: bookmarkChrome.actionSystemImage
                    )
                }
                Button {
                    shareTarget = ShareToCommunityTarget.article(article, core: app.safeCore)
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
