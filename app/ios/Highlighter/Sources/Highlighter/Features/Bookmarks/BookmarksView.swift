import Kingfisher
import SwiftUI

/// Formats a Unix timestamp as a compact relative label (BookmarkCompact style).
/// 0–59 s → "0m", 60–3599 s → "Xm", 1h–23h → "Xh", 1d–6d → "Xd",
/// 1w–3w → "Xw", 4w+ → "Xmo". Returns nil for nil/future timestamps.
private func bookmarkCompactDate(_ seconds: UInt64?) -> String? {
    guard let seconds, seconds > 0 else { return nil }
    let now = UInt64(max(0, Date().timeIntervalSince1970))
    guard now >= seconds else { return nil }
    let delta = now - seconds
    switch delta {
    case 0 ..< 3600:   return "\(delta / 60)m"
    case 3600 ..< 86400:  return "\(delta / 3600)h"
    case 86400 ..< 604800:  return "\(delta / 86400)d"
    case 604800 ..< 2592000: return "\(delta / 604800)w"
    default: return "\(delta / 2592000)mo"
    }
}

struct BookmarksView: View {
    @Environment(HighlighterStore.self) private var app
    /// Phase 7: the kernel owns the Articles pane (bookmarked kind:30023).
    @Environment(HighlighterAppKernel.self) private var kernel
    @Environment(\.dismiss) private var dismiss
    @State private var store = BookmarkStore()
    @State private var filter: BookmarkLibraryFilter = .articles

    var body: some View {
        NavigationStack {
            Group {
                if store.isLoading && store.myArticles.isEmpty && store.myBookmarkSets.isEmpty {
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
                ToolbarItem(placement: .topBarLeading) {
                    scopePicker
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") { dismiss() }
                }
            }
            .navigationDestination(for: ArticleReaderTarget.self) { target in
                ArticleReaderView(target: target)
            }
            .navigationDestination(for: BookmarkSetRecord.self) { rec in
                SetDetailView(record: rec)
            }
        }
        .task {
            guard let bridge = app.eventBridge else { return }
            await store.start(
                core: app.safeCore,
                bridge: bridge,
                kernel: kernel
            )
        }
        .onChange(of: app.bookmarkedArticleAddresses) {
            Task { await store.reload() }
        }
        .onChange(of: kernel.bookmarks) { _, _ in
            store.applyKernelSnapshot()
        }
        .onDisappear { store.stop() }
    }

    private var scopePicker: some View {
        Picker("Scope", selection: $store.scope) {
            Text("Mine").tag(BookmarkLibraryScope.mine)
            Text("Explore").tag(BookmarkLibraryScope.explore)
        }
        .pickerStyle(.segmented)
        .fixedSize()
    }

    private var scrollContent: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                if store.scope == .mine {
                    filterChipRail
                        .padding(.horizontal, 16)
                        .padding(.vertical, 12)
                    Divider()
                    mineContent
                } else {
                    exploreContent
                        .padding(.top, 16)
                }
            }
        }
    }

    private var filterChipRail: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                filterChip(filter: .articles, label: "Articles", icon: "doc.text")
                filterChip(filter: .collections, label: "Collections", icon: "rectangle.stack")
                filterChip(filter: .web, label: "Web", icon: "globe")
            }
        }
        .scrollClipDisabled()
    }

    private func filterChip(filter: BookmarkLibraryFilter, label: String, icon: String) -> some View {
        let isActive = self.filter == filter
        return Button {
            withAnimation(.spring(duration: 0.22)) { self.filter = filter }
        } label: {
            HStack(spacing: 5) {
                Image(systemName: icon)
                    .font(.caption.weight(.semibold))
                Text(label)
                    .font(.subheadline.weight(.medium))
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .foregroundStyle(isActive ? Color.highlighterAccent : Color.highlighterInkMuted)
            .background(.ultraThinMaterial, in: Capsule())
            .overlay(
                Capsule().strokeBorder(
                    isActive ? Color.highlighterAccent.opacity(0.4) : Color.highlighterRule,
                    lineWidth: 1
                )
            )
        }
        .buttonStyle(.plain)
    }

    @ViewBuilder
    private var mineContent: some View {
        switch filter {
        case .articles:
            articlesContent
        case .collections:
            collectionsContent(sets: store.myBookmarkSets + store.myCurationSets)
        case .web:
            webContent
        }
    }

    @ViewBuilder
    private var articlesContent: some View {
        if store.myArticles.isEmpty {
            unavailableState(icon: "bookmark", title: "No bookmarks yet", message: "Save articles from anywhere in Highlighter to find them here.")
        } else {
            LazyVStack(spacing: 0) {
                ForEach(store.myArticles, id: \.address) { article in
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

    @ViewBuilder
    private var webContent: some View {
        if store.myWebBookmarks.isEmpty {
            unavailableState(icon: "globe", title: "No web bookmarks yet", message: "Web pages you bookmark via Nostr will appear here.")
        } else {
            LazyVStack(spacing: 0) {
                ForEach(store.myWebBookmarks, id: \.url) { bookmark in
                    WebBookmarkRow(bookmark: bookmark)
                        .padding(.horizontal, 16)
                        .padding(.vertical, 12)
                    Divider().padding(.leading, 16)
                }
            }
        }
    }

    @ViewBuilder
    private var exploreContent: some View {
        if store.followingCurationSets.isEmpty {
            unavailableState(icon: "rectangle.stack", title: "Nothing to explore", message: "People you follow haven't created any curation sets yet.")
        } else {
            collectionsContent(sets: store.followingCurationSets)
        }
    }

    @ViewBuilder
    private func collectionsContent(sets: [BookmarkSetRecord]) -> some View {
        if sets.isEmpty {
            unavailableState(icon: "rectangle.stack", title: "No collections yet", message: "Create bookmark or curation sets to organise your saved content.")
        } else {
            LazyVStack(spacing: 0) {
                ForEach(sets, id: \.id) { set in
                    NavigationLink(value: set) {
                        CollectionRow(record: set)
                            .padding(.horizontal, 16)
                            .padding(.vertical, 12)
                    }
                    .buttonStyle(.plain)
                    Divider().padding(.leading, 16)
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

// MARK: - Row views

struct BookmarkedArticleRow: View {
    @Environment(HighlighterStore.self) private var app
    let article: ArticleRecord

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            coverImage(imageURL: article.image.isEmpty ? nil : article.image)
                .frame(width: 56, height: 56)
                .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))

            VStack(alignment: .leading, spacing: 4) {
                Text(article.title.isEmpty ? "Untitled" : article.title)
                    .font(.subheadline.weight(.semibold))
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
                    Text(authorDisplay.displayName)
                        .font(.caption2.weight(.medium))
                        .foregroundStyle(Color.highlighterInkMuted)
                    if let date = bookmarkCompactDate(article.publishedAt ?? article.createdAt) {
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
    private func coverImage(imageURL: String?) -> some View {
        if let imageURL, let url = URL(string: imageURL) {
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
                colors: [Color.highlighterAccent.opacity(0.28), Color.highlighterAccent.opacity(0.10)],
                startPoint: .topLeading, endPoint: .bottomTrailing
            )
            Image(systemName: "doc.text")
                .font(.system(size: 20, weight: .medium))
                .foregroundStyle(Color.highlighterInkStrong.opacity(0.4))
        }
    }

    private var authorDisplay: ProfileDisplayProjection {
        let profile = app.profileSnapshots[article.pubkey]
        let name = (profile?.displayName ?? "").isEmpty
            ? ((profile?.name ?? "").isEmpty ? String(article.pubkey.prefix(10)) : profile!.name)
            : profile!.displayName
        return ProfileDisplayProjection(
            displayName: name,
            displayInitial: name.first.map { String($0).uppercased() } ?? "?",
            pictureUrl: profile?.picture ?? ""
        )
    }
}

struct CollectionRow: View {
    @Environment(HighlighterStore.self) private var app
    let record: BookmarkSetRecord

    private static let kindBookmarkSets: UInt32 = 30003

    var body: some View {
        HStack(spacing: 12) {
            ZStack {
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .fill(Color.highlighterAccent.opacity(0.12))
                    .frame(width: 44, height: 44)
                Image(systemName: kindIconSystemName)
                    .font(.system(size: 18, weight: .medium))
                    .foregroundStyle(Color.highlighterAccent)
            }

            VStack(alignment: .leading, spacing: 4) {
                Text(displayTitle)
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(1)

                HStack(spacing: 6) {
                    AuthorAvatar(
                        pubkey: record.pubkey,
                        pictureURL: curatorDisplay.pictureUrl,
                        displayInitial: curatorDisplay.displayInitial,
                        size: 16
                    )
                    Text(curatorDisplay.displayName)
                        .font(.caption2.weight(.medium))
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(1)
                }

                HStack(spacing: 4) {
                    Text(isBookmarkSet ? "Bookmarks" : "Curation")
                        .font(.caption2.weight(.medium))
                        .foregroundStyle(Color.highlighterAccent.opacity(0.8))
                        .padding(.horizontal, 5)
                        .padding(.vertical, 1)
                        .background(Color.highlighterAccent.opacity(0.1), in: Capsule())

                    let itemCount = record.articleAddresses.count + record.noteIds.count
                    if itemCount > 0 {
                        Text("\(itemCount) item\(itemCount == 1 ? "" : "s")")
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
        .task(id: record.pubkey) {
            await app.requestProfile(pubkeyHex: record.pubkey)
        }
    }

    private var isBookmarkSet: Bool { record.kind == CollectionRow.kindBookmarkSets }
    private var kindIconSystemName: String { isBookmarkSet ? "bookmark.fill" : "rectangle.stack.fill" }
    private var displayTitle: String {
        record.title.isEmpty ? (record.id.isEmpty ? "Untitled" : record.id) : record.title
    }

    private var curatorDisplay: ProfileDisplayProjection {
        let profile = app.profileSnapshots[record.pubkey]
        let name = (profile?.displayName ?? "").isEmpty
            ? ((profile?.name ?? "").isEmpty ? String(record.pubkey.prefix(10)) : profile!.name)
            : profile!.displayName
        return ProfileDisplayProjection(
            displayName: name,
            displayInitial: name.first.map { String($0).uppercased() } ?? "?",
            pictureUrl: profile?.picture ?? ""
        )
    }
}

struct WebBookmarkRow: View {
    let bookmark: WebBookmarkRecord

    var body: some View {
        let displayTitle = bookmark.title.isEmpty ? bookmark.url : bookmark.title
        let host = URL(string: bookmark.url)?.host
        let description: String? = bookmark.description.isEmpty ? nil : bookmark.description
        let displayUnixSeconds: UInt64? = bookmark.publishedAt ?? bookmark.createdAt

        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Image(systemName: "globe")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(Color.highlighterAccent)

                if let host {
                    Text(host)
                        .font(.caption2.weight(.medium))
                        .foregroundStyle(Color.highlighterInkMuted)
                }

                Spacer(minLength: 0)

                if let date = bookmarkCompactDate(displayUnixSeconds) {
                    Text(date)
                        .font(.caption2)
                        .foregroundStyle(Color.highlighterInkMuted)
                }
            }

            Text(displayTitle)
                .font(.subheadline.weight(.medium))
                .foregroundStyle(Color.highlighterInkStrong)
                .lineLimit(2)
                .multilineTextAlignment(.leading)

            if let description {
                Text(description)
                    .font(.caption)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(2)
                    .multilineTextAlignment(.leading)
            }

            if !bookmark.topics.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 4) {
                        ForEach(bookmark.topics, id: \.self) { topic in
                            Text("#\(topic)")
                                .font(.caption2.weight(.medium))
                                .foregroundStyle(Color.highlighterAccent.opacity(0.8))
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Color.highlighterAccent.opacity(0.1), in: Capsule())
                        }
                    }
                }
                .scrollClipDisabled()
            }
        }
    }
}
