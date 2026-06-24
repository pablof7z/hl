import SwiftUI

/// The search destination. Tap the liquid-glass search button in any tab's
/// toolbar to land here.
///
/// Layout follows Apple's search-fields HIG for a dedicated discovery
/// destination:
///
/// - The field starts unfocused so the editorial empty state can breathe.
/// - Suggested terms and recent searches sit above a curated browse section
///   (so an empty query isn't a dead screen).
/// - As the user types, results appear in sections — Highlights, Articles,
///   Communities, People — each with a "See all" row that drills into a
///   kind-specific sub-screen.
/// - NIP-50 relay results fade into the Articles section as the relays
///   reply; there's no separate "web results" bucket to make the user cross
///   between local and remote.
struct SearchView: View {
    @Environment(HighlighterStore.self) private var app
    @Environment(HighlighterAppKernel.self) private var kernel

    @State private var store: SearchStore?
    @FocusState private var focusedField: Bool
    @State private var recentQueries: [String] = []
    @State private var directArticleTarget: ArticleReaderTarget?
    @State private var directArticleActive = false
    @State private var directProfilePubkey: String?
    @State private var directProfileActive = false
    @State private var directEntity: NostrEntityRef?
    @State private var directEntityActive = false
    @State private var directGroupId: String?
    @State private var directGroupActive = false

    var body: some View {
        NavigationStack {
            ZStack {
                Color.highlighterPaper.ignoresSafeArea()
                if let store {
                    content(store: store)
                } else {
                    Color.clear
                }
            }
            .navigationTitle("Search")
            .navigationBarTitleDisplayMode(.large)
            .searchable(
                text: Binding(
                    get: { store?.query ?? "" },
                    set: { new in store?.query = new }
                ),
                placement: .navigationBarDrawer(displayMode: .always),
                prompt: Text("Quotes, essays, people, rooms")
            )
            .searchFocused($focusedField)
            .onSubmit(of: .search) {
                commitRecentQuery()
            }
            .navigationDestination(for: ArticleReaderTarget.self) { target in
                ArticleReaderView(target: target)
            }
            .navigationDestination(for: WebReaderTarget.self) { target in
                WebReaderView(target: target)
            }
            .navigationDestination(for: ProfileDestination.self) { destination in
                if case .pubkey(let pk) = destination {
                    ProfileView(pubkey: pk)
                }
            }
            .navigationDestination(for: String.self) { groupId in
                RoomHomeView(groupId: groupId)
            }
            .navigationDestination(for: SearchSeeAllTarget.self) { target in
                if let store {
                    SearchSeeAllView(target: target, store: store)
                }
            }
            .navigationDestination(isPresented: $directArticleActive) {
                if let directArticleTarget {
                    ArticleReaderView(target: directArticleTarget)
                }
            }
            .navigationDestination(isPresented: $directProfileActive) {
                if let directProfilePubkey {
                    ProfileView(pubkey: directProfilePubkey)
                }
            }
            .navigationDestination(isPresented: $directEntityActive) {
                if let directEntity {
                    NostrEntityReaderView(entity: directEntity)
                }
            }
            .navigationDestination(isPresented: $directGroupActive) {
                if let directGroupId {
                    RoomHomeView(groupId: directGroupId)
                }
            }
            .onChange(of: store?.directNavigation) { _, navigation in
                handleDirectNavigation(navigation)
            }
            // Omnibox resolver outcome (#1865) → route through the store.
            .onChange(of: kernel.search?.omnibox) { _, outcome in
                store?.applyOmniboxOutcome(outcome)
            }
            .globalUserToolbar()
        }
        .task {
            if store == nil {
                let s = SearchStore(
                    safeCore: app.safeCore,
                    eventBridge: app.eventBridge,
                    kernel: kernel
                )
                store = s
                await s.start()
            }
            store?.disarmOmnibox()
            kernel.openSearch()
            let snapshot = await app.safeCore.getSearchChromeSnapshot()
            recentQueries = snapshot.recentQueries
        }
        .onDisappear {
            store?.stop()
            kernel.closeSearch()
        }
    }

    // MARK: - Content switcher

    @ViewBuilder
    private func content(store: SearchStore) -> some View {
        if store.secretRejected {
            secretRejectedNotice
        } else if store.hasQuery {
            results(store: store)
        } else {
            emptyState(store: store)
        }
    }

    /// Safe inline notice shown when the omnibox resolver (#1865) classifies the
    /// input as a secret key. The secret is NEVER echoed.
    private var secretRejectedNotice: some View {
        VStack(alignment: .leading, spacing: 10) {
            Rectangle()
                .fill(Color.highlighterAccent.opacity(0.6))
                .frame(width: 3, height: 24)
                .clipShape(RoundedRectangle(cornerRadius: 1.5))
            Text("Secret keys aren't accepted")
                .font(.system(.title3, design: .default).weight(.semibold))
                .foregroundStyle(Color.highlighterInkStrong)
            Text("That looks like a private key (nsec). For your safety it's never searched, stored, or shown. Clear the field and paste a public reference, NIP-05 address, or search terms instead.")
                .font(.footnote)
                .foregroundStyle(Color.highlighterInkMuted)
                .lineSpacing(3)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .padding(.horizontal, 20)
        .padding(.top, 36)
    }

    // MARK: - Empty (discovery) state

    private func emptyState(store: SearchStore) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 28) {
                if !recentQueries.isEmpty {
                    recentSection
                }
                suggestedSection(store: store)
                browseRoomsSection
                browseHighlightsPreviewSection(store: store)
                browseRelaysFootnote(store: store)
            }
            .padding(.horizontal, 20)
            .padding(.top, 8)
            .padding(.bottom, 40)
        }
        .scrollDismissesKeyboard(.interactively)
    }

    private var recentSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .firstTextBaseline) {
                SectionKicker(text: "Recent")
                Spacer()
                Button("Clear") {
                    clearRecentQueries()
                }
                .font(.caption.weight(.semibold))
                .foregroundStyle(Color.highlighterInkMuted)
            }
            VStack(spacing: 0) {
                ForEach(Array(recentQueries.enumerated()), id: \.element) { index, q in
                    Button {
                        store?.submit(q)
                    } label: {
                        HStack(spacing: 12) {
                            Image(systemName: "clock")
                                .font(.footnote)
                                .foregroundStyle(Color.highlighterInkMuted)
                            Text(q)
                                .font(.callout)
                                .foregroundStyle(Color.highlighterInkStrong)
                            Spacer()
                            Image(systemName: "arrow.up.left")
                                .font(.caption)
                                .foregroundStyle(Color.highlighterInkMuted.opacity(0.8))
                        }
                        .padding(.vertical, 10)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)

                    if index < recentQueries.count - 1 {
                        Rectangle()
                            .fill(Color.highlighterRule)
                            .frame(height: 0.5)
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func suggestedSection(store: SearchStore) -> some View {
        let suggestions = suggestedQueries()
        if !suggestions.isEmpty {
            VStack(alignment: .leading, spacing: 12) {
                SectionKicker(text: "Try")
                FlowLayout(spacing: 10, runSpacing: 10) {
                    ForEach(suggestions, id: \.self) { term in
                        Button {
                            store.submit(term)
                        } label: {
                            Text(term)
                                .font(.callout.weight(.medium))
                                .foregroundStyle(Color.highlighterInkStrong)
                                .padding(.horizontal, 14)
                                .padding(.vertical, 9)
                                .background {
                                    Capsule()
                                        .fill(Color.highlighterTintPale)
                                }
                                .overlay {
                                    Capsule()
                                        .stroke(Color.highlighterRule, lineWidth: 0.5)
                                }
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
    }

    private var browseRoomsSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            SectionKicker(text: "Your rooms")
            if app.joinedCommunities.isEmpty {
                Text("Rooms you join appear here.")
                    .font(.footnote)
                    .foregroundStyle(Color.highlighterInkMuted)
            } else {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 12) {
                        ForEach(app.joinedCommunities.prefix(12)) { room in
                            NavigationLink(value: room.id) {
                                RoomMiniTile(room: room)
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .padding(.horizontal, 2)
                }
                .scrollClipDisabled()
            }
        }
    }

    @ViewBuilder
    private func browseHighlightsPreviewSection(store: SearchStore) -> some View {
        SectionKicker(text: "The library")
        Text("Your nostrdb cache holds every highlight, article, community, and profile you've ever loaded. Search finds them instantly. Anything not yet on your device — searched across your configured search relays.")
            .font(.system(.subheadline, design: .default))
            .foregroundStyle(Color.highlighterInkMuted)
            .lineSpacing(4)
            .padding(.top, -4)
    }

    @ViewBuilder
    private func browseRelaysFootnote(store: SearchStore) -> some View {
        if !store.searchRelays.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                SectionKicker(text: "Search relays")
                ForEach(store.searchRelays, id: \.self) { url in
                    HStack(spacing: 10) {
                        Circle()
                            .fill(Color.highlighterAccent.opacity(0.7))
                            .frame(width: 5, height: 5)
                        Text(displayRelay(url))
                            .font(.footnote.monospaced())
                            .foregroundStyle(Color.highlighterInkMuted)
                    }
                }
            }
            .padding(.top, 8)
        }
    }

    // MARK: - Results state

    private func results(store: SearchStore) -> some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 28) {
                directOpenHint(store: store)
                if store.isLocalLoading && allEmpty(store: store) {
                    loadingSkeleton
                } else if allEmpty(store: store) && !store.isRelayLoading {
                    noResults(store: store)
                } else {
                    highlightsResultsSection(store: store)
                    articlesResultsSection(store: store)
                    communitiesResultsSection(store: store)
                    peopleResultsSection(store: store)
                    if store.isRelayLoading {
                        relayLoadingHint
                    }
                }
            }
            .padding(.horizontal, 20)
            .padding(.top, 12)
            .padding(.bottom, 40)
        }
        .scrollDismissesKeyboard(.interactively)
    }

    private var loadingSkeleton: some View {
        let trailingPadding: [CGFloat] = [96, 148, 64]

        return VStack(alignment: .leading, spacing: 16) {
            ForEach(Array(trailingPadding.enumerated()), id: \.offset) { _, padding in
                RoundedRectangle(cornerRadius: 4)
                    .fill(Color.highlighterRule.opacity(0.5))
                    .frame(height: 14)
                    .frame(maxWidth: .infinity)
                    .padding(.trailing, padding)
            }
        }
        .padding(.vertical, 20)
    }

    private func noResults(store: SearchStore) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Rectangle()
                .fill(Color.highlighterAccent.opacity(0.6))
                .frame(width: 3, height: 24)
                .clipShape(RoundedRectangle(cornerRadius: 1.5))
            Text("Nothing yet for \u{201C}\(store.query)\u{201D}")
                .font(.system(.title3, design: .default).weight(.semibold))
                .foregroundStyle(Color.highlighterInkStrong)
            Text("Relay search is still running in the background — results may arrive in a moment.")
                .font(.footnote)
                .foregroundStyle(Color.highlighterInkMuted)
        }
        .padding(.top, 36)
    }

    private var relayLoadingHint: some View {
        HStack(spacing: 10) {
            ProgressView()
                .controlSize(.small)
            Text("Searching the web…")
                .font(.footnote)
                .foregroundStyle(Color.highlighterInkMuted)
        }
        .padding(.top, 8)
    }

    @ViewBuilder
    private func directOpenHint(store: SearchStore) -> some View {
        if let message = store.directOpenMessage {
            HStack(spacing: 10) {
                ProgressView()
                    .controlSize(.small)
                Text(message)
                    .font(.footnote)
                    .foregroundStyle(Color.highlighterInkMuted)
            }
            .padding(.top, 8)
        }
    }

    // MARK: - Sections

    @ViewBuilder
    private func highlightsResultsSection(store: SearchStore) -> some View {
        if !store.highlights.isEmpty {
            SearchSectionHeader(
                title: "Highlights",
                count: store.highlights.count,
                target: store.highlights.count > 4
                    ? .highlights(query: store.query) : nil
            )
            VStack(spacing: 0) {
                ForEach(Array(store.highlights.prefix(4).enumerated()), id: \.element.eventId) { idx, highlight in
                    highlightRow(highlight)
                    if idx < min(store.highlights.count, 4) - 1 {
                        Rectangle()
                            .fill(Color.highlighterRule)
                            .frame(height: 0.5)
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func articlesResultsSection(store: SearchStore) -> some View {
        if !store.articles.isEmpty {
            SearchSectionHeader(
                title: "Articles",
                count: store.articles.count,
                target: store.articles.count > 4
                    ? .articles(query: store.query) : nil
            )
            VStack(spacing: 0) {
                ForEach(Array(store.articles.prefix(4).enumerated()), id: \.element.eventId) { idx, article in
                    articleRow(article)
                    if idx < min(store.articles.count, 4) - 1 {
                        Rectangle()
                            .fill(Color.highlighterRule)
                            .frame(height: 0.5)
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func communitiesResultsSection(store: SearchStore) -> some View {
        if !store.communities.isEmpty {
            SearchSectionHeader(
                title: "Communities",
                count: store.communities.count,
                target: store.communities.count > 3
                    ? .communities(query: store.query) : nil
            )
            VStack(spacing: 0) {
                ForEach(Array(store.communities.prefix(3).enumerated()), id: \.element.id) { idx, c in
                    NavigationLink(value: c.id) {
                        SearchCommunityRow(community: c)
                    }
                    .buttonStyle(.plain)
                    if idx < min(store.communities.count, 3) - 1 {
                        Rectangle()
                            .fill(Color.highlighterRule)
                            .frame(height: 0.5)
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func peopleResultsSection(store: SearchStore) -> some View {
        if !store.profiles.isEmpty {
            SearchSectionHeader(
                title: "People",
                count: store.profiles.count,
                target: store.profiles.count > 5
                    ? .people(query: store.query) : nil
            )
            VStack(spacing: 0) {
                ForEach(Array(store.profiles.prefix(5).enumerated()), id: \.element.pubkey) { idx, profile in
                    NavigationLink(value: ProfileDestination.pubkey(profile.pubkey)) {
                        SearchProfileRow(profile: profile)
                    }
                    .buttonStyle(.plain)
                    if idx < min(store.profiles.count, 5) - 1 {
                        Rectangle()
                            .fill(Color.highlighterRule)
                            .frame(height: 0.5)
                    }
                }
            }
        }
    }

    // MARK: - Rows (dispatch to shared components)

    @ViewBuilder
    private func highlightRow(_ h: HighlightRecord) -> some View {
        let route = articleReaderRoute(for: h.artifactAddress)
        let pageImageUrl = validPageImageUrl(h.imageUrl)
        if let route {
            NavigationLink(value: ArticleReaderTarget(route: route)) {
                SearchHighlightRow(
                    highlight: h,
                    query: store?.query ?? "",
                    pageImageUrl: pageImageUrl
                )
            }
            .buttonStyle(.plain)
        } else {
            SearchHighlightRow(
                highlight: h,
                query: store?.query ?? "",
                pageImageUrl: pageImageUrl
            )
        }
    }

    /// Parse a NIP-33 article address (`30023:<pubkey>:<d>`) into a reader route.
    private func articleReaderRoute(for address: String) -> ArticleReaderRoute? {
        let addr = address.trimmingCharacters(in: .whitespaces)
        let parts = addr.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false).map(String.init)
        guard parts.count == 3, parts[0] == "30023" else { return nil }
        let pubkey = parts[1].trimmingCharacters(in: .whitespaces)
        let dTag   = parts[2].trimmingCharacters(in: .whitespaces)
        guard !pubkey.isEmpty, !dTag.isEmpty else { return nil }
        return ArticleReaderRoute(address: "30023:\(pubkey):\(dTag)", pubkey: pubkey, dTag: dTag)
    }

    /// Return the URL string if it is a valid http/https URL, otherwise nil.
    private func validPageImageUrl(_ value: String) -> String? {
        let trimmed = value.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty,
              let url = URL(string: trimmed),
              url.scheme == "http" || url.scheme == "https"
        else { return nil }
        return trimmed
    }

    @ViewBuilder
    private func articleRow(_ a: ArticleRecord) -> some View {
        NavigationLink(value: ArticleReaderTarget(article: a)) {
            ArticleCardView(article: a)
        }
        .buttonStyle(.plain)
        .articleRowActions(article: a)
    }

    // MARK: - Helpers

    private func commitRecentQuery() {
        let searchQuery = (store?.query ?? "").trimmingCharacters(in: .whitespaces)
        guard !searchQuery.isEmpty else { return }
        var queries = UserDefaults.standard.stringArray(forKey: "hl.search.recent_queries") ?? []
        queries.removeAll { $0 == searchQuery }
        queries.insert(searchQuery, at: 0)
        if queries.count > 10 { queries = Array(queries.prefix(10)) }
        UserDefaults.standard.set(queries, forKey: "hl.search.recent_queries")
        recentQueries = queries
    }

    private func clearRecentQueries() {
        UserDefaults.standard.removeObject(forKey: "hl.search.recent_queries")
        recentQueries = []
    }

    private func allEmpty(store: SearchStore) -> Bool {
        store.highlights.isEmpty
            && store.articles.isEmpty
            && store.communities.isEmpty
            && store.profiles.isEmpty
    }

    private func suggestedQueries() -> [String] {
        let evergreen = ["Dostoevsky", "Bitcoin", "Attention", "Borges", "Philosophy"]
        var queries: [String] = []
        var seen = Set<String>()
        for community in app.joinedCommunities.prefix(4) {
            let name = community.name.trimmingCharacters(in: .whitespaces)
            guard !name.isEmpty, seen.insert(name.lowercased()).inserted else { continue }
            queries.append(name)
        }
        for term in evergreen {
            guard queries.count < 8 else { break }
            let t = term.trimmingCharacters(in: .whitespaces)
            guard !t.isEmpty, seen.insert(t.lowercased()).inserted else { continue }
            queries.append(t)
        }
        return Array(queries.prefix(8))
    }

    private func displayRelay(_ url: String) -> String {
        url
            .replacingOccurrences(of: "wss://", with: "")
            .replacingOccurrences(of: "ws://", with: "")
    }

    private func handleDirectNavigation(_ navigation: SearchDirectNavigation?) {
        guard let navigation else { return }
        switch navigation {
        case .article(let target):
            directArticleTarget = target
            directArticleActive = true
        case .profile(let pubkey):
            directProfilePubkey = pubkey
            directProfileActive = true
        case .entity(let entity):
            directEntity = entity
            directEntityActive = true
        case .group(let groupId):
            directGroupId = groupId
            directGroupActive = true
        }
        store?.consumeDirectNavigation()
    }
}

// MARK: - Section header

private struct SearchSectionHeader: View {
    let title: String
    let count: Int
    let target: SearchSeeAllTarget?

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(title)
                .font(.system(.title3, design: .default).weight(.semibold))
                .foregroundStyle(Color.highlighterInkStrong)
            if count > 0 {
                Text("\(count)")
                    .font(.caption.weight(.semibold).monospacedDigit())
                    .foregroundStyle(Color.highlighterInkMuted)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background {
                        Capsule()
                            .fill(Color.highlighterRule.opacity(0.55))
                    }
            }
            Spacer()
            if let target {
                NavigationLink(value: target) {
                    HStack(spacing: 4) {
                        Text("See all")
                        Image(systemName: "chevron.right")
                            .font(.caption2.weight(.semibold))
                    }
                    .font(.footnote.weight(.semibold))
                    .foregroundStyle(Color.highlighterAccent)
                }
            }
        }
        .padding(.bottom, 4)
    }
}

// MARK: - Destination targets

enum SearchSeeAllTarget: Hashable {
    case highlights(query: String)
    case articles(query: String)
    case communities(query: String)
    case people(query: String)

    var title: String {
        switch self {
        case .highlights: "Highlights"
        case .articles: "Articles"
        case .communities: "Communities"
        case .people: "People"
        }
    }

    var query: String {
        switch self {
        case .highlights(let q), .articles(let q), .communities(let q), .people(let q): q
        }
    }
}

private struct NostrEntityReaderView: View {
    let entity: NostrEntityRef

    var body: some View {
        ScrollView {
            NostrEntityCard(entity: entity)
                .padding(.horizontal, 20)
                .padding(.top, 24)
                .padding(.bottom, 40)
        }
        .background(Color.highlighterPaper.ignoresSafeArea())
        .navigationTitle("Nostr event")
        .navigationBarTitleDisplayMode(.inline)
    }
}

// MARK: - Shared building blocks

private struct SectionKicker: View {
    let text: String

    var body: some View {
        HStack(spacing: 10) {
            Rectangle()
                .fill(Color.highlighterAccent)
                .frame(width: 14, height: 1.5)
                .clipShape(RoundedRectangle(cornerRadius: 0.5))
            Text(text.uppercased())
                .font(.caption2.weight(.semibold).monospaced())
                .tracking(1.2)
                .foregroundStyle(Color.highlighterInkMuted)
        }
    }
}

private struct RoomMiniTile: View {
    @Environment(HighlighterStore.self) private var app
    let room: CommunitySummary

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            RoomCoverArt(picture: room.picture, name: room.name, size: 86)
            Text(room.name)
                .font(.footnote.weight(.semibold))
                .foregroundStyle(Color.highlighterInkStrong)
                .lineLimit(2)
                .frame(width: 86, alignment: .leading)
        }
    }
}

private struct RoomCoverArt: View {
    let picture: String
    let name: String
    let size: CGFloat

    @Environment(HighlighterStore.self) private var app

    var body: some View {
        let avatar = avatarProjection
        ZStack {
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(
                    LinearGradient(
                        colors: [
                            Color.highlighterAccent.opacity(0.35),
                            Color.highlighterTintPale
                        ],
                        startPoint: .topLeading,
                        endPoint: .bottomTrailing
                    )
                )
            if let url = URL(string: avatar.pictureUrl), !avatar.pictureUrl.isEmpty {
                AsyncImage(url: url) { phase in
                    if case .success(let image) = phase {
                        image.resizable().aspectRatio(contentMode: .fill)
                    }
                }
                .frame(width: size, height: size)
                .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
            } else {
                Text(avatar.displayInitial)
                    .font(.system(size: size * 0.38, design: .default).weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong.opacity(0.75))
            }
        }
        .frame(width: size, height: size)
        .overlay {
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .stroke(Color.highlighterRule, lineWidth: 0.5)
        }
    }

    private var avatarProjection: RoomAvatarProjection {
        RoomAvatarProjection(
            pictureUrl: picture,
            displayInitial: name.first.map(String.init) ?? ""
        )
    }
}

// MARK: - Row views

private struct SearchHighlightRow: View {
    let highlight: HighlightRecord
    let query: String
    let pageImageUrl: String?

    var body: some View {
        HStack(alignment: .top, spacing: 14) {
            if let pageURL = pageImageURL {
                HighlightPageImage(url: pageURL, treatment: .row)
            } else {
                Rectangle()
                    .fill(Color.highlighterAccent)
                    .frame(width: 2.5)
                    .clipShape(RoundedRectangle(cornerRadius: 1.25))
            }
            VStack(alignment: .leading, spacing: 6) {
                HighlightMatchedText(
                    text: highlight.quote,
                    query: query,
                    font: .system(size: 18, design: .default).italic()
                )
                .foregroundStyle(Color.highlighterInkStrong)
                .lineSpacing(3)
                .lineLimit(4)

                if !highlight.note.isEmpty {
                    Text("— " + highlight.note)
                        .font(.footnote.italic())
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(2)
                }
            }
        }
        .padding(.vertical, 14)
        .contentShape(Rectangle())
    }

    private var pageImageURL: URL? {
        pageImageUrl.flatMap(URL.init(string:))
    }
}

private struct SearchCommunityRow: View {
    @Environment(HighlighterStore.self) private var app
    let community: CommunitySummary

    var body: some View {
        let projection = rowProjection
        HStack(alignment: .center, spacing: 14) {
            RoomCoverArt(picture: community.picture, name: community.name, size: 54)
            VStack(alignment: .leading, spacing: 4) {
                Text(projection.displayName)
                    .font(.callout.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(1)
                if let about = projection.about {
                    Text(about)
                        .font(.footnote)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(2)
                }
                HStack(spacing: 6) {
                    Text(projection.visibilityLabel)
                        .font(.caption2.weight(.semibold))
                        .tracking(0.6)
                    Text("·")
                    Text(projection.accessLabel)
                        .font(.caption2.weight(.semibold))
                        .tracking(0.6)
                    if let memberCountLabel = projection.memberCountLabel {
                        Text("·")
                        Text(memberCountLabel)
                            .font(.caption2)
                    }
                }
                .foregroundStyle(Color.highlighterInkMuted)
            }
            Spacer()
            Image(systemName: "chevron.right")
                .font(.caption.weight(.semibold))
                .foregroundStyle(Color.highlighterInkMuted.opacity(0.6))
        }
        .padding(.vertical, 10)
        .contentShape(Rectangle())
    }

    private var rowProjection: SearchCommunityRowProjection {
        SearchCommunityRowProjection(
            displayName: community.name,
            about: community.about.isEmpty ? nil : community.about,
            visibilityLabel: capitalizeFirst(community.visibility),
            accessLabel: capitalizeFirst(community.access),
            memberCountLabel: community.memberCount.map { "\($0) members" }
        )
    }

    private func capitalizeFirst(_ s: String) -> String {
        guard let first = s.first else { return s }
        return first.uppercased() + s.dropFirst()
    }
}

private struct SearchProfileRow: View {
    @Environment(HighlighterStore.self) private var app
    let profile: ProfileSearchRow

    private var displayName: String {
        if !profile.displayName.isEmpty { return profile.displayName }
        if !profile.name.isEmpty { return profile.name }
        return String(profile.pubkey.prefix(8))
    }
    private var displayInitial: String { String(displayName.prefix(1)) }

    var body: some View {
        HStack(spacing: 14) {
            AuthorAvatar(
                pubkey: profile.pubkey,
                pictureURL: profile.picture,
                displayInitial: displayInitial,
                size: 44
            )
            VStack(alignment: .leading, spacing: 2) {
                Text(displayName)
                    .font(.callout.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(1)
                if !profile.nip05.isEmpty {
                    Text(profile.nip05)
                        .font(.caption)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(1)
                } else if !profile.about.isEmpty {
                    Text(profile.about)
                        .font(.caption)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(1)
                }
            }
            Spacer()
            Image(systemName: "chevron.right")
                .font(.caption.weight(.semibold))
                .foregroundStyle(Color.highlighterInkMuted.opacity(0.6))
        }
        .padding(.vertical, 10)
        .contentShape(Rectangle())
    }
}

// MARK: - Matched-text rendering

/// Renders `text` with every case-insensitive occurrence of `query` wrapped in
/// a subtle highlighted span (terracotta ink, very faint background). Falls
/// back to plain text when the query is empty.
private struct HighlightMatchedText: View {
    let text: String
    let query: String
    let font: Font

    var body: some View {
        Text(attributed)
            .font(font)
    }

    private var attributed: AttributedString {
        var out = AttributedString(text)
        let trimmedQuery = query.trimmingCharacters(in: .whitespaces).lowercased()
        guard !trimmedQuery.isEmpty else { return out }
        let lowerText = text.lowercased()
        var searchFrom = lowerText.startIndex
        while searchFrom < lowerText.endIndex,
              let range = lowerText.range(of: trimmedQuery, range: searchFrom..<lowerText.endIndex) {
            let startOffset = lowerText.distance(from: lowerText.startIndex, to: range.lowerBound)
            let endOffset   = lowerText.distance(from: lowerText.startIndex, to: range.upperBound)
            if let s = out.index(out.startIndex, offsetByCharacters: startOffset),
               let e = out.index(out.startIndex, offsetByCharacters: endOffset),
               s < e {
                out[s..<e].foregroundColor = .highlighterAccent
                out[s..<e].backgroundColor = Color.laneArticleHighlightFill
            }
            searchFrom = range.upperBound
        }
        return out
    }
}

private extension AttributedString {
    /// Convenience — characters-based offset into the attributed string.
    func index(_ base: AttributedString.Index, offsetByCharacters n: Int) -> AttributedString.Index? {
        var idx = base
        var remaining = n
        while remaining > 0 {
            guard idx < endIndex else { return nil }
            idx = characters.index(after: idx)
            remaining -= 1
        }
        return idx
    }
}

// MARK: - Flow layout (chips)

/// Minimal flow layout that wraps child views left-to-right. Used for the
/// suggested-searches chip row so long terms wrap cleanly without the
/// default `HStack` cramming.
private struct FlowLayout: Layout {
    var spacing: CGFloat = 8
    var runSpacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let maxWidth = proposal.width ?? .infinity
        var x: CGFloat = 0
        var y: CGFloat = 0
        var runHeight: CGFloat = 0
        var totalHeight: CGFloat = 0

        for sub in subviews {
            let size = sub.sizeThatFits(.unspecified)
            if x + size.width > maxWidth, x > 0 {
                y += runHeight + runSpacing
                x = 0
                runHeight = 0
            }
            x += size.width + spacing
            runHeight = max(runHeight, size.height)
            totalHeight = y + runHeight
        }
        return CGSize(width: maxWidth.isFinite ? maxWidth : x, height: totalHeight)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let maxWidth = bounds.width
        var x: CGFloat = 0
        var y: CGFloat = 0
        var runHeight: CGFloat = 0
        for sub in subviews {
            let size = sub.sizeThatFits(.unspecified)
            if x + size.width > maxWidth, x > 0 {
                y += runHeight + runSpacing
                x = 0
                runHeight = 0
            }
            sub.place(at: CGPoint(x: bounds.minX + x, y: bounds.minY + y), proposal: .unspecified)
            x += size.width + spacing
            runHeight = max(runHeight, size.height)
        }
    }
}
