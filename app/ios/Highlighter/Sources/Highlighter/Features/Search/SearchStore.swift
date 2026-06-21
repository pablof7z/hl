import Foundation
import Observation

enum SearchDirectNavigation: Equatable {
    case article(ArticleReaderTarget)
    case profile(String)
    case entity(NostrEntityRef)
}

/// Drives `SearchView`. Owns UI query state, Rust-projected result
/// buckets (highlights / articles / communities / people), and the live
/// NIP-50 subscription whose deltas re-run the local article match so
/// relay-delivered events fade into the Articles section as they arrive.
///
/// Architecture note: every bucket is read from nostrdb via the Rust core —
/// the NIP-50 relay sub just ingests into ndb, which in turn triggers a
/// `SearchArticlesUpdated` delta that the store reacts to by re-running
/// Rust's article snapshot locally. NostrDB stays the only source of truth.
@MainActor
@Observable
final class SearchStore {
    // MARK: - Inputs

    /// Raw text from the search field. Writes schedule a query.
    var query: String = "" {
        didSet {
            guard query != oldValue else { return }
            scheduleSearch(for: query)
        }
    }

    // MARK: - Outputs (reactive)

    private(set) var highlights: [HighlightRecord] = []
    private(set) var articles: [ArticleRecord] = []
    private(set) var communities: [CommunitySummary] = []
    private(set) var profiles: [ProfileMetadata] = []
    private(set) var hasQuery: Bool = false

    /// True while a local scan is running for the current query — flickers to
    /// avoid a blank frame on a fresh query.
    private(set) var isLocalLoading: Bool = false
    /// True while the current NIP-50 relay subscription is being opened.
    /// Relay-delivered events continue to fade into Articles after this flips
    /// off via `SearchArticlesUpdated` deltas.
    private(set) var isRelayLoading: Bool = false

    /// The resolved set of relays the NIP-50 query is hitting. Rendered as a
    /// subtle footnote under the Articles section so the user can see their
    /// configured NIP-51 search relays are actually in use.
    private(set) var searchRelays: [String] = []

    /// Set when the query is an exact Nostr entity or cached NIP-05 match.
    /// `SearchView` consumes and clears this so navigation remains
    /// programmatic UI state rather than another durable source of truth.
    private(set) var directNavigation: SearchDirectNavigation?
    private(set) var directOpenMessage: String?

    // MARK: - Dependencies

    private let safeCore: SafeHighlighterCore
    private let eventBridge: EventBridge?

    // MARK: - Internal state

    /// Rust-projected token for the currently scheduled query. In-flight
    /// callbacks pass it back through Rust before applying results.
    private var searchToken: UInt64 = 0
    /// Most-recent applied query (the one whose results populate the buckets).
    private var appliedQuery: String = ""
    private var searchTask: Task<Void, Never>?
    private var activeSearchHandle: UInt64?
    private var directEntityHandle: UInt64?
    private var directEntityKey: String?
    private var directEntity: NostrEntityRef?
    private var directEntityToken: UInt64 = 0
    private var directTask: Task<Void, Never>?
    /// Query the current NIP-50 subscription was opened with. If the user
    /// edits the query, we tear down + re-open.
    private var activeRelayQuery: String = ""

    // MARK: - Init

    init(safeCore: SafeHighlighterCore, eventBridge: EventBridge?) {
        self.safeCore = safeCore
        self.eventBridge = eventBridge
    }

    // MARK: - Lifecycle

    func start() async {
        let snapshot = await safeCore.getSearchChromeSnapshot()
        searchRelays = snapshot.searchRelays
    }

    func stop() {
        searchTask?.cancel()
        searchTask = nil
        directTask?.cancel()
        directTask = nil
        if let handle = activeSearchHandle {
            Task { [safeCore, eventBridge] in
                await safeCore.unsubscribe(handle)
                eventBridge?.unregister(handle: handle)
            }
            activeSearchHandle = nil
        }
        tearDownDirectEntity()
    }

    // MARK: - Query orchestration

    /// Re-applies a search explicitly (e.g. tapping a recent search chip).
    func submit(_ query: String) {
        if self.query != query {
            self.query = query
        } else {
            scheduleSearch(for: query)
        }
    }

    func clear() {
        if query.isEmpty {
            scheduleSearch(for: "")
        } else {
            query = ""
        }
    }

    func consumeDirectNavigation() {
        directNavigation = nil
    }

    private func scheduleSearch(for q: String) {
        searchTask?.cancel()
        directTask?.cancel()
        tearDownDirectEntity()
        directNavigation = nil
        directOpenMessage = nil
        let projection = scheduleProjection(for: q)
        applyScheduleProjection(projection)
        guard projection.shouldRunSearch else { return }
        scheduleDirectNavigationIfPossible(
            query: projection.searchQuery,
            token: projection.searchToken
        )
        let token = projection.searchToken
        searchTask = Task { [weak self] in
            guard let self else { return }
            await self.runSearch(for: projection.searchQuery, token: token)
        }
    }

    private func scheduleProjection(for query: String) -> SearchScheduleProjection {
        safeCore.projectSearchSchedule(
            input: SearchScheduleInput(
                query: query,
                currentToken: searchToken
            )
        )
    }

    private func applyScheduleProjection(_ projection: SearchScheduleProjection) {
        searchToken = projection.searchToken
        if projection.shouldClearResults {
            clearResultsForEmptyQuery()
            return
        }
        hasQuery = projection.hasQuery
        isLocalLoading = projection.isLocalLoading
    }

    private func clearResultsForEmptyQuery() {
        hasQuery = false
        appliedQuery = ""
        highlights = []
        articles = []
        communities = []
        profiles = []
        isLocalLoading = false
        isRelayLoading = false
        directOpenMessage = nil
        tearDownRelaySearch()
        tearDownDirectEntity()
    }

    private func runSearch(for q: String, token: UInt64) async {
        let snapshot = await safeCore.getSearchResultsSnapshot(query: q)

        let applyProjection = safeCore.projectSearchResultsApply(
            input: SearchResultsApplyInput(
                requestToken: token,
                currentToken: searchToken
            )
        )
        guard applyProjection.shouldApply else { return }

        appliedQuery = q
        apply(snapshot)
        applyExactNip05NavigationIfNeeded(query: q, token: token, profiles: snapshot.profiles)
        isLocalLoading = applyProjection.isLocalLoading

        let relayProjection = safeCore.projectSearchRelayRefresh(
            input: SearchRelayRefreshInput(
                requestedQuery: q,
                activeRelayQuery: activeRelayQuery
            )
        )
        guard relayProjection.shouldRefresh else { return }
        await refreshRelaySubscription(using: relayProjection)
    }

    // MARK: - NIP-50 relay subscription

    private func refreshRelaySubscription(using projection: SearchRelayRefreshProjection) async {
        tearDownRelaySearch()
        activeRelayQuery = projection.activeRelayQuery
        isRelayLoading = projection.isRelayLoading
        let outcome = await safeCore.subscribeArticleSearch(query: projection.subscribeQuery)
        let startProjection = safeCore.projectSearchRelayStartResult(
            input: SearchRelayStartResultInput(
                requestedQuery: projection.subscribeQuery,
                appliedQuery: appliedQuery,
                error: outcome.error
            )
        )
        activeRelayQuery = startProjection.activeRelayQuery
        isRelayLoading = startProjection.isRelayLoading
        if startProjection.shouldUnsubscribeHandle {
            await safeCore.unsubscribe(outcome.handle)
            return
        }
        guard startProjection.shouldRegisterHandle else { return }
        activeSearchHandle = outcome.handle
        eventBridge?.registerSearch(self, handle: outcome.handle)
    }

    private func tearDownRelaySearch() {
        if let handle = activeSearchHandle {
            let bridge = eventBridge
            let core = safeCore
            Task {
                await core.unsubscribe(handle)
                bridge?.unregister(handle: handle)
            }
            activeSearchHandle = nil
        }
        activeRelayQuery = ""
        isRelayLoading = false
    }

    private func tearDownDirectEntity() {
        directTask?.cancel()
        directTask = nil
        if let handle = directEntityHandle {
            let bridge = eventBridge
            let core = safeCore
            Task {
                await core.unsubscribe(handle)
                bridge?.unregister(handle: handle)
            }
            directEntityHandle = nil
        }
        directEntityKey = nil
        directEntity = nil
        directEntityToken = 0
    }

    /// EventBridge callback: the relay search delivered new matching events
    /// into ndb. Re-run the local article scan only when Rust projects the
    /// delta as current for the open query.
    func applyRelaySearchUpdate(query incomingQuery: String) {
        let updateProjection = safeCore.projectSearchRelayUpdate(
            input: SearchRelayUpdateInput(
                incomingQuery: incomingQuery,
                appliedQuery: appliedQuery,
                currentToken: searchToken
            )
        )
        guard updateProjection.shouldRefreshArticles else { return }
        isRelayLoading = updateProjection.isRelayLoading
        let q = updateProjection.articleQuery
        let token = updateProjection.requestToken
        Task { [weak self] in
            guard let self else { return }
            let snapshot = await self.safeCore.getSearchArticleResultsSnapshot(query: q)
            let applyProjection = self.safeCore.projectSearchRelayArticlesApply(
                input: SearchRelayArticlesApplyInput(
                    requestToken: token,
                    currentToken: self.searchToken,
                    requestQuery: q,
                    appliedQuery: self.appliedQuery
                )
            )
            guard applyProjection.shouldApply else { return }
            self.articles = snapshot.articles
        }
    }

    func applyDirectNostrEntity(event: NostrEntityEvent) {
        guard directEntity != nil else { return }
        routeResolvedDirectEntity(event)
        tearDownDirectEntity()
    }

    private func apply(_ snapshot: SearchResultsSnapshot) {
        highlights = snapshot.highlights
        articles = snapshot.articles
        communities = snapshot.communities
        profiles = snapshot.profiles
    }

    private func scheduleDirectNavigationIfPossible(query: String, token: UInt64) {
        let decoded = safeCore.decodeNostrEntity(query)
        guard decoded.decoded, let entity = decoded.entity else { return }

        switch entity {
        case .profile(let pubkeyHex, _):
            directNavigation = .profile(pubkeyHex)
        case .address(let kind, let pubkeyHex, let dTag, _):
            if kind == 30023, !dTag.isEmpty {
                let route = ArticleReaderRoute(
                    address: "\(kind):\(pubkeyHex):\(dTag)",
                    pubkey: pubkeyHex,
                    dTag: dTag
                )
                directNavigation = .article(ArticleReaderTarget(route: route))
            } else {
                directNavigation = .entity(entity)
            }
        case .event:
            directOpenMessage = "Fetching Nostr event…"
            startDirectEntityResolution(entity: entity, token: token)
        }
    }

    private func startDirectEntityResolution(entity: NostrEntityRef, token: UInt64) {
        let key = safeCore.nostrEntityIdentityKey(entity: entity)
        directEntityKey = key
        directEntity = entity
        directEntityToken = token
        directTask = Task { [weak self] in
            guard let self else { return }
            await self.resolveOrSubscribeDirectEntity(entity: entity, key: key, token: token)
        }
    }

    private func resolveOrSubscribeDirectEntity(
        entity: NostrEntityRef,
        key: String,
        token: UInt64
    ) async {
        let snapshot = await safeCore.resolveNostrEntity(entity)
        guard isCurrentDirectEntity(key: key, token: token) else { return }
        if let event = snapshot.event {
            routeResolvedDirectEntity(event)
            tearDownDirectEntity()
            return
        }

        let outcome = await safeCore.subscribeNostrEntity(entity)
        let projection = safeCore.projectViewSubscriptionStart(
            input: ViewSubscriptionStartProjectionInput(start: outcome)
        )
        guard isCurrentDirectEntity(key: key, token: token) else {
            if outcome.handle != 0 { await safeCore.unsubscribe(outcome.handle) }
            return
        }
        guard projection.shouldRegister else {
            directOpenMessage = outcome.error.isEmpty
                ? "Couldn't open that Nostr event."
                : outcome.error
            directNavigation = .entity(entity)
            return
        }
        directEntityHandle = projection.handle
        eventBridge?.registerSearch(self, handle: projection.handle)
    }

    private func isCurrentDirectEntity(key: String, token: UInt64) -> Bool {
        directEntityKey == key && directEntityToken == token && searchToken == token
    }

    private func routeResolvedDirectEntity(_ event: NostrEntityEvent) {
        directOpenMessage = nil
        switch event.renderKind {
        case .article:
            let projection = safeCore.projectNostrEntityArticleCard(
                input: NostrEntityArticleCardProjectionInput(event: event)
            )
            if let route = projection.readerRoute {
                directNavigation = .article(ArticleReaderTarget(route: route))
            } else if let entity = directEntity {
                directNavigation = .entity(entity)
            }
        case .profile:
            directNavigation = .profile(event.pubkeyHex)
        case .note, .highlight, .generic:
            if let entity = directEntity {
                directNavigation = .entity(entity)
            }
        }
    }

    private func applyExactNip05NavigationIfNeeded(
        query: String,
        token: UInt64,
        profiles: [ProfileMetadata]
    ) {
        guard searchToken == token, directNavigation == nil else { return }
        let normalized = normalizedNip05(query)
        guard isLikelyNip05(normalized) else { return }
        if let profile = profiles.first(where: { normalizedNip05($0.nip05) == normalized }) {
            directNavigation = .profile(profile.pubkey)
        }
    }

    private func isLikelyNip05(_ value: String) -> Bool {
        value.contains(".")
            && !value.contains(" ")
            && !value.hasPrefix("http://")
            && !value.hasPrefix("https://")
    }

    private func normalizedNip05(_ value: String) -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if trimmed.hasPrefix("_@") {
            return String(trimmed.dropFirst(2))
        }
        return trimmed
    }
}
