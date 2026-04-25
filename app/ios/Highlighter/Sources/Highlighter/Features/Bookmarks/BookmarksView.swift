import Kingfisher
import SwiftUI

enum BookmarkFilter: CaseIterable, Identifiable {
    case all, articles, notes, web

    var id: Self { self }

    var label: String {
        switch self {
        case .all:      return "All"
        case .articles: return "Articles"
        case .notes:    return "Notes"
        case .web:      return "Web"
        }
    }

    var icon: String {
        switch self {
        case .all:      return "square.grid.2x2"
        case .articles: return "doc.text"
        case .notes:    return "note.text"
        case .web:      return "globe"
        }
    }
}

struct BookmarksView: View {
    @Environment(HighlighterStore.self) private var app
    @Environment(\.dismiss) private var dismiss
    @State private var store = BookmarkStore()
    @State private var filter: BookmarkFilter = .all

    var body: some View {
        NavigationStack {
            Group {
                if store.isLoading {
                    ProgressView()
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    scrollContent
                }
            }
            .background(Color.highlighterPaper.ignoresSafeArea())
            .navigationTitle("Bookmarks")
            .navigationBarTitleDisplayMode(.large)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") { dismiss() }
                }
            }
            .navigationDestination(for: ArticleReaderTarget.self) { target in
                ArticleReaderView(target: target)
            }
        }
        .task(id: app.bookmarkedArticleAddresses) {
            await store.load(addresses: app.bookmarkedArticleAddresses, core: app.safeCore)
        }
    }

    private var scrollContent: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                filterChipRail
                    .padding(.horizontal, 16)
                    .padding(.vertical, 12)

                Divider()

                switch filter {
                case .all, .articles:
                    articlesContent
                case .notes:
                    unavailableState(
                        icon: "note.text",
                        title: "No notes saved",
                        message: "Bookmarked Nostr notes will appear here."
                    )
                case .web:
                    unavailableState(
                        icon: "globe",
                        title: "No web pages saved",
                        message: "Bookmarked links will appear here."
                    )
                }
            }
        }
    }

    private var filterChipRail: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(BookmarkFilter.allCases) { item in
                    chip(for: item)
                }
            }
        }
        .scrollClipDisabled()
    }

    private func chip(for item: BookmarkFilter) -> some View {
        let isActive = filter == item
        let articleCount = store.articles.count
        let showCount = (item == .all || item == .articles) && articleCount > 0

        return Button {
            withAnimation(.spring(duration: 0.22)) { filter = item }
        } label: {
            HStack(spacing: 5) {
                Image(systemName: item.icon)
                    .font(.caption.weight(.semibold))
                Text(item.label)
                    .font(.subheadline.weight(.medium))
                if showCount {
                    Text("\(articleCount)")
                        .font(.caption.weight(.bold))
                        .padding(.horizontal, 5)
                        .padding(.vertical, 1)
                        .background(
                            isActive ? Color.highlighterAccent.opacity(0.2) : Color.highlighterRule,
                            in: Capsule()
                        )
                }
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .foregroundStyle(isActive ? Color.highlighterAccent : Color.highlighterInkMuted)
            .background(.ultraThinMaterial, in: Capsule())
            .overlay(
                Capsule()
                    .strokeBorder(
                        isActive ? Color.highlighterAccent.opacity(0.4) : Color.highlighterRule,
                        lineWidth: 1
                    )
            )
        }
        .buttonStyle(.plain)
    }

    @ViewBuilder
    private var articlesContent: some View {
        if store.articles.isEmpty {
            unavailableState(
                icon: "bookmark",
                title: "No bookmarks yet",
                message: "Save articles from anywhere in Highlighter to find them here."
            )
        } else {
            LazyVStack(spacing: 0) {
                ForEach(store.articles, id: \.eventId) { article in
                    NavigationLink(value: ArticleReaderTarget(pubkey: article.pubkey, dTag: article.identifier, seed: article)) {
                        BookmarkedArticleRow(article: article)
                            .padding(.horizontal, 16)
                            .padding(.vertical, 12)
                    }
                    .buttonStyle(.plain)

                    Divider()
                        .padding(.leading, 84)
                }
            }
        }
    }

    private func unavailableState(icon: String, title: String, message: String) -> some View {
        ContentUnavailableView {
            Label(title, systemImage: icon)
        } description: {
            Text(message)
        }
        .padding(.top, 40)
    }
}

private struct BookmarkedArticleRow: View {
    @Environment(HighlighterStore.self) private var app
    let article: ArticleRecord

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            coverImage
                .frame(width: 56, height: 56)
                .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))

            VStack(alignment: .leading, spacing: 4) {
                Text(article.title.isEmpty ? "Untitled" : article.title)
                    .font(.system(.subheadline, design: .serif).weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(2)
                    .multilineTextAlignment(.leading)

                if !article.summary.isEmpty {
                    Text(article.summary)
                        .font(.caption)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(2)
                        .multilineTextAlignment(.leading)
                }

                HStack(spacing: 4) {
                    Text(authorName)
                        .font(.caption2.weight(.medium))
                        .foregroundStyle(Color.highlighterInkMuted)
                    if let date = relativeDate {
                        Text("·")
                            .font(.caption2)
                            .foregroundStyle(Color.highlighterInkMuted)
                        Text(date)
                            .font(.caption2)
                            .foregroundStyle(Color.highlighterInkMuted)
                    }
                }
            }

            Spacer(minLength: 0)

            Image(systemName: "chevron.right")
                .font(.caption.weight(.semibold))
                .foregroundStyle(Color.highlighterInkMuted.opacity(0.5))
        }
        .task(id: article.pubkey) {
            await app.requestProfile(pubkeyHex: article.pubkey)
        }
    }

    @ViewBuilder
    private var coverImage: some View {
        if !article.image.isEmpty, let url = URL(string: article.image) {
            KFImage(url)
                .placeholder { coverFallback }
                .fade(duration: 0.15)
                .resizable()
                .scaledToFill()
        } else {
            coverFallback
        }
    }

    private var coverFallback: some View {
        ZStack {
            LinearGradient(
                colors: [
                    Color.highlighterAccent.opacity(0.28),
                    Color.highlighterAccent.opacity(0.10),
                ],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
            Image(systemName: "doc.text")
                .font(.system(size: 20, weight: .medium))
                .foregroundStyle(Color.highlighterInkStrong.opacity(0.4))
        }
    }

    private var authorName: String {
        let profile = app.profileCache[article.pubkey]
        if let dn = profile?.displayName, !dn.isEmpty { return dn }
        if let n = profile?.name, !n.isEmpty { return n }
        return String(article.pubkey.prefix(10))
    }

    private var relativeDate: String? {
        let seconds = article.publishedAt ?? article.createdAt
        guard let s = seconds, s > 0 else { return nil }
        let delta = Date().timeIntervalSince1970 - TimeInterval(s)
        guard delta >= 0 else { return nil }
        switch delta {
        case ..<3600:              return "\(Int(delta / 60))m"
        case ..<86400:             return "\(Int(delta / 3600))h"
        case ..<(86400 * 7):       return "\(Int(delta / 86400))d"
        case ..<(86400 * 30):      return "\(Int(delta / (86400 * 7)))w"
        default:                   return "\(Int(delta / (86400 * 30)))mo"
        }
    }
}
