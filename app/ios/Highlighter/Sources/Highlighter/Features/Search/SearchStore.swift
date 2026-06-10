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

    private func scheduleSearch(for q: String) {
        searchTask?.cancel()
        let projection = scheduleProjection(for: q)
        applyScheduleProjection(projection)
        guard projection.shouldRunSearch else { return }
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
        tearDownRelaySearch()
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
        isLocalLoading = applyProjection.isLocalLoading

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
