import Foundation
import Observation

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
        didSet { scheduleSearch(for: query) }
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

    // MARK: - Dependencies

    private let safeCore: SafeHighlighterCore
    private let eventBridge: EventBridge?

    // MARK: - Internal state

    /// Monotonically increasing token — every scheduled query bumps it so
    /// in-flight callbacks for a stale query can no-op.
    private var searchToken: UInt64 = 0
    /// Most-recent applied query (the one whose results populate the buckets).
    private var appliedQuery: String = ""
    private var searchTask: Task<Void, Never>?
    private var activeSearchHandle: UInt64?
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
        let outcome = await safeCore.getSearchRelays()
        if outcome.error.isEmpty {
            searchRelays = outcome.values
        }
    }

    func stop() {
        searchTask?.cancel()
        searchTask = nil
        if let handle = activeSearchHandle {
            Task { [safeCore, eventBridge] in
                await safeCore.unsubscribe(handle)
                eventBridge?.unregister(handle: handle)
            }
            activeSearchHandle = nil
        }
    }

    // MARK: - Query orchestration

    /// Re-applies a search explicitly (e.g. tapping a recent search chip).
    func submit(_ query: String) {
        self.query = query
        // Fire immediately, replacing any in-flight query.
        searchTask?.cancel()
        let projection = queryProjection(for: query)
        guard projection.hasQuery else {
            clearResultsForEmptyQuery()
            return
        }
        hasQuery = true
        isLocalLoading = true
        let token = bumpToken()
        searchTask = Task { [weak self] in
            guard let self else { return }
            await self.runSearch(for: projection.searchQuery, token: token)
        }
    }

    func clear() {
        searchTask?.cancel()
        searchTask = nil
        query = ""
        clearResultsForEmptyQuery()
    }

    private func scheduleSearch(for q: String) {
        searchTask?.cancel()
        let projection = queryProjection(for: q)
        if !projection.hasQuery {
            clearResultsForEmptyQuery()
            return
        }
        hasQuery = true
        isLocalLoading = true
        let token = bumpToken()
        searchTask = Task { [weak self] in
            guard let self else { return }
            await self.runSearch(for: projection.searchQuery, token: token)
        }
    }

    private func queryProjection(for query: String) -> SearchQueryProjection {
        safeCore.projectSearchQuery(input: SearchQueryProjectionInput(query: query))
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
        tearDownRelaySearch()
    }

    private func bumpToken() -> UInt64 {
        searchToken &+= 1
        return searchToken
    }

    private func runSearch(for q: String, token: UInt64) async {
        let snapshot = await safeCore.getSearchResultsSnapshot(query: q)

        guard token == searchToken else { return }

        appliedQuery = q
        apply(snapshot)
        isLocalLoading = false

        if activeRelayQuery != q {
            await refreshRelaySubscription(for: q)
        }
    }

    // MARK: - NIP-50 relay subscription

    private func refreshRelaySubscription(for q: String) async {
        tearDownRelaySearch()
        activeRelayQuery = q
        isRelayLoading = true
        let outcome = await safeCore.subscribeArticleSearch(query: q)
        guard outcome.error.isEmpty else {
            isRelayLoading = false
            return
        }
        if appliedQuery != q {
            // Query moved on while we were opening — tear down immediately.
            await safeCore.unsubscribe(outcome.handle)
            return
        }
        activeSearchHandle = outcome.handle
        eventBridge?.registerSearch(self, handle: outcome.handle)
        isRelayLoading = false
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

    /// EventBridge callback: the relay search delivered new matching events
    /// into ndb. Re-run the local article scan to pick them up. Guarded by
    /// query string so a late delta for a stale query doesn't clobber fresh
    /// results.
    func applyRelaySearchUpdate(query incomingQuery: String) {
        guard incomingQuery == appliedQuery, !appliedQuery.isEmpty else { return }
        isRelayLoading = false
        let q = appliedQuery
        let token = searchToken
        Task { [weak self] in
            guard let self else { return }
            let snapshot = await self.safeCore.getSearchArticleResultsSnapshot(query: q)
            guard token == self.searchToken, q == self.appliedQuery else { return }
            self.articles = snapshot.articles
        }
    }

    private func apply(_ snapshot: SearchResultsSnapshot) {
        highlights = snapshot.highlights
        articles = snapshot.articles
        communities = snapshot.communities
        profiles = snapshot.profiles
    }
}
