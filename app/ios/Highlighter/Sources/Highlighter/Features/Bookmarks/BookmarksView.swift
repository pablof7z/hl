import Kingfisher
import SwiftUI

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
        let projection = libraryProjection

        return Picker("Scope", selection: $store.scope) {
            ForEach(projection.scopeOptions, id: \.scope) { option in
                Text(option.label).tag(option.scope)
            }
        }
        .pickerStyle(.segmented)
        .fixedSize()
    }

    private var scrollContent: some View {
        let projection = libraryProjection

        return ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                if store.scope == .mine {
                    filterChipRail(chips: projection.filterChips)
                        .padding(.horizontal, 16)
                        .padding(.vertical, 12)
                    Divider()
                    mineContent(projection: projection)
                } else {
                    exploreContent(projection: projection)
                        .padding(.top, 16)
                }
            }
        }
    }

    private func filterChipRail(chips: [BookmarkLibraryFilterChipProjection]) -> some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(chips, id: \.filter) { item in
                    chip(for: item)
                }
            }
        }
        .scrollClipDisabled()
    }

    private func chip(for item: BookmarkLibraryFilterChipProjection) -> some View {
        let isActive = filter == item.filter
        return Button {
            withAnimation(.spring(duration: 0.22)) { filter = item.filter }
        } label: {
            HStack(spacing: 5) {
                Image(systemName: item.iconSystemName)
                    .font(.caption.weight(.semibold))
                Text(item.label)
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
    private func mineContent(projection: BookmarkLibraryProjection) -> some View {
        switch projection.selectedPane {
        case .articles:
            articlesContent(projection: projection)
        case .collections:
            collectionsContent(
                sets: store.myBookmarkSets + store.myCurationSets,
                projection: projection
            )
        case .web:
            webContent(projection: projection)
        case .explore:
            EmptyView()
        }
    }

    @ViewBuilder
    private func articlesContent(projection: BookmarkLibraryProjection) -> some View {
        if projection.isEmpty {
            unavailableState(projection)
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
    private func webContent(projection: BookmarkLibraryProjection) -> some View {
        if projection.isEmpty {
            unavailableState(projection)
        } else {
            LazyVStack(spacing: 0) {
                ForEach(store.myWebBookmarks, id: \.url) { bookmark in
                    WebBookmarkRowView(bookmark: bookmark)
                        .padding(.horizontal, 16)
                        .padding(.vertical, 12)
                    Divider().padding(.leading, 16)
                }
            }
        }
    }

    @ViewBuilder
    private func exploreContent(projection: BookmarkLibraryProjection) -> some View {
        if projection.isEmpty {
            unavailableState(projection)
        } else {
            collectionsContent(
                sets: store.followingCurationSets,
                projection: projection
            )
        }
    }

    @ViewBuilder
    private func collectionsContent(sets: [BookmarkSetRecord], projection: BookmarkLibraryProjection) -> some View {
        if projection.isEmpty {
            unavailableState(projection)
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

    private func unavailableState(_ projection: BookmarkLibraryProjection) -> some View {
        ContentUnavailableView {
            Label(projection.emptyTitle, systemImage: projection.emptyIconSystemName)
        } description: {
            Text(projection.emptyMessage)
        }
        .padding(.top, 40)
    }

    private var libraryProjection: BookmarkLibraryProjection {
        app.safeCore.projectBookmarkLibrary(
            input: BookmarkLibraryProjectionInput(
                scope: store.scope,
                selectedFilter: filter,
                articleCount: UInt64(store.myArticles.count),
                collectionCount: UInt64(store.myBookmarkSets.count + store.myCurationSets.count),
                webBookmarkCount: UInt64(store.myWebBookmarks.count),
                exploreCount: UInt64(store.followingCurationSets.count)
            )
        )
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
                Text(article.title)
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
                    Text(authorDisplayName)
                        .font(.caption2.weight(.medium))
                        .foregroundStyle(Color.highlighterInkMuted)
                    if let date = relativeDate(article.publishedAt ?? article.createdAt) {
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

    /// Phase 7: inline display name — no safeCore round-trip.
    /// Falls back: displayName → name → first 8 hex chars of pubkey.
    private var authorDisplayName: String {
        let profile = app.profileSnapshots[article.pubkey]
        if let dn = profile?.displayName, !dn.isEmpty { return dn }
        if let n = profile?.name, !n.isEmpty { return n }
        return article.pubkey.isEmpty ? "" : String(article.pubkey.prefix(8))
    }

    private func relativeDate(_ seconds: UInt64?) -> String? {
        guard let seconds else { return nil }
        let date = Date(timeIntervalSince1970: TimeInterval(seconds))
        return RelativeDateTimeFormatter().localizedString(for: date, relativeTo: Date())
    }
}

struct CollectionRow: View {
    @Environment(HighlighterStore.self) private var app
    let record: BookmarkSetRecord

    var body: some View {
        let curator = curatorDisplay
        let projection = rowProjection

        HStack(spacing: 12) {
            ZStack {
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .fill(Color.highlighterAccent.opacity(0.12))
                    .frame(width: 44, height: 44)
                Image(systemName: projection.kindIconSystemName)
                    .font(.system(size: 18, weight: .medium))
                    .foregroundStyle(Color.highlighterAccent)
            }

            VStack(alignment: .leading, spacing: 4) {
                Text(projection.displayTitle)
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(1)

                HStack(spacing: 6) {
                    AuthorAvatar(
                        pubkey: record.pubkey,
                        pictureURL: curator.pictureUrl,
                        displayInitial: curator.displayInitial,
                        size: 16
                    )
                    Text(curator.displayName)
                        .font(.caption2.weight(.medium))
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(1)
                }

                HStack(spacing: 4) {
                    Text(projection.kindLabel)
                        .font(.caption2.weight(.medium))
                        .foregroundStyle(Color.highlighterAccent.opacity(0.8))
                        .padding(.horizontal, 5)
                        .padding(.vertical, 1)
                        .background(Color.highlighterAccent.opacity(0.1), in: Capsule())

                    if let itemCountLabel = projection.itemCountLabel {
                        Text(itemCountLabel)
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

    private var curatorDisplay: ProfileDisplayProjection {
        ProfileDisplayProjection.from(pubkey: record.pubkey, profile: app.profileSnapshots[record.pubkey])
    }

    private var rowProjection: BookmarkSetRowProjection {
        app.safeCore.projectBookmarkSetRow(
            input: BookmarkSetRowProjectionInput(record: record)
        )
    }
}

struct WebBookmarkRowView: View {
    @Environment(HighlighterStore.self) private var app
    let bookmark: WebBookmarkRecord

    var body: some View {
        let projection = rowProjection

        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Image(systemName: "globe")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(Color.highlighterAccent)

                if let host = projection.host {
                    Text(host)
                        .font(.caption2.weight(.medium))
                        .foregroundStyle(Color.highlighterInkMuted)
                }

                Spacer(minLength: 0)

                if let date = relativeDate(projection.displayUnixSeconds) {
                    Text(date)
                        .font(.caption2)
                        .foregroundStyle(Color.highlighterInkMuted)
                }
            }

            Text(projection.displayTitle)
                .font(.subheadline.weight(.medium))
                .foregroundStyle(Color.highlighterInkStrong)
                .lineLimit(2)
                .multilineTextAlignment(.leading)

            if let description = projection.description {
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

    private var rowProjection: WebBookmarkRowProjection {
        app.safeCore.projectWebBookmarkRow(
            input: WebBookmarkRowProjectionInput(bookmark: bookmark)
        )
    }

    private func relativeDate(_ seconds: UInt64?) -> String? {
        guard let seconds else { return nil }
        let date = Date(timeIntervalSince1970: TimeInterval(seconds))
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: date, relativeTo: Date())
    }
}
