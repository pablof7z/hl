import Foundation
import Observation

enum SearchDirectNavigation: Equatable {
    case article(ArticleReaderTarget)
    case profile(String)
    case entity(NostrEntityRef)
    case group(String)
}

/// Drives `SearchView`. Owns UI query state, Rust-projected result
/// buckets (highlights / articles / communities / people), and the live
/// NIP-50 subscription whose deltas re-run the local article match so
/// relay-delivered events fade into the Articles section as they arrive.
///
/// Phase 7 cutover: the KERNEL owns the article / highlight / community buckets.
/// `query` changes dispatch `hl.search.run` (scope = articles + highlights, one
/// NIP-50 query) and run the kernel's local community scan; results stream back
/// via `kernel.searchSnapshot` → `applyKernelSnapshot()` (wired from the View's
/// `.onChange`). Swift buckets the mixed hits by kind. The PEOPLE bucket stays
/// on the live lane (kind:0 local scan — nmp #1697); it's read-only and coexists
/// (gotcha #5). `searchRelays` / recent queries stay on the bespoke chrome path.
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

    /// True while the live-lane people scan is running for the current query.
    private(set) var isLocalLoading: Bool = false
    /// True between dispatching the kernel search and the first snapshot. The
    /// kernel streams relay results into `kernel.searchSnapshot` as they arrive.
    private(set) var isRelayLoading: Bool = false

    /// The resolved set of relays the NIP-50 query hits, shown as a footnote.
    private(set) var searchRelays: [String] = []

    /// Set when the query is an exact Nostr entity or cached NIP-05 match.
    /// `SearchView` consumes and clears this so navigation remains
    /// programmatic UI state rather than another durable source of truth.
    private(set) var directNavigation: SearchDirectNavigation?
    private(set) var directOpenMessage: String?

    /// Set when the omnibox resolver (#1865) classified the current input as a
    /// secret key (`nsec` / `ncryptsec`). The view renders a safe inline notice
    /// and suppresses the normal result echo — the secret is NEVER displayed.
    private(set) var secretRejected: Bool = false

    // MARK: - Dependencies

    private let safeCore: SafeHighlighterCore
    private let eventBridge: EventBridge?
    /// The nmp-lane kernel that owns the omnibox resolver (#1865). The store
    /// dispatches `.runOmnibox` here and routes on the resolved `OmniboxOutcome`
    /// surfaced in the `ViewId.search` snapshot. `nil` in legacy/test contexts.
    private let kernel: HighlighterAppKernel?

    // MARK: - Internal state

    /// Monotonic token so a slow people-scan from a stale query can't overwrite
    /// the current results.
    private var searchToken: UInt64 = 0
    /// The query whose results currently populate the buckets.
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
    /// Gate for the omnibox outcome (#1865). Armed on each fresh `runOmnibox`
    /// dispatch (a genuine query change) and consumed once the resolved outcome
    /// is routed — so the kernel re-pushing the retained outcome (e.g. as relay
    /// hits stream in, or when the Search view re-opens) never re-navigates.
    private var omniboxArmed: Bool = false

    // MARK: - Init

    init(
        safeCore: SafeHighlighterCore,
        eventBridge: EventBridge?,
        kernel: HighlighterAppKernel? = nil
    ) {
        self.safeCore = safeCore
        self.eventBridge = eventBridge
        self.kernel = kernel
    }

    // MARK: - Lifecycle

    func start() async {
        kernel.openSearch()
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

    /// Disarm the omnibox gate so a retained outcome pushed on Search re-open
    /// (the kernel keeps the last classification in state) is not re-routed.
    /// The next genuine query change re-arms it. Call from `SearchView.task`.
    func disarmOmnibox() {
        omniboxArmed = false
    }

    private func scheduleSearch(for q: String) {
        searchTask?.cancel()
        directTask?.cancel()
        tearDownDirectEntity()
        directNavigation = nil
        directOpenMessage = nil
        secretRejected = false
        let projection = scheduleProjection(for: q)
        applyScheduleProjection(projection)
        guard projection.shouldRunSearch else { return }

        // Single brain (#1865): hand the raw input to NMP's input-intent
        // resolver. The kernel classifies (paste-nav / NIP-05 / group / relay /
        // secret / free text), performs the side effect (multi-kind relay
        // search for free text, NIP-05 reverse lookup), and surfaces the
        // resolved `OmniboxOutcome` in the `ViewId.search` snapshot, which the
        // view feeds back via `applyOmniboxOutcome`. Paste-navigation no longer
        // decodes eagerly here — it routes through the resolver outcome.
        omniboxArmed = true
        kernel?.app.dispatch(.runOmnibox(query: projection.searchQuery))

        let token = projection.searchToken
        searchTask = Task { [weak self] in
            guard let self else { return }
            await self.runSearch(for: projection.searchQuery, token: token)
        }
    }

    // MARK: - Omnibox routing (#1865)

    /// Route the resolver's classification of the current input. Called by
    /// `SearchView` whenever the `ViewId.search` snapshot's omnibox outcome
    /// changes. The resolver is the single classification brain; this method
    /// only maps each outcome onto hl's existing navigation / result surfaces.
    func applyOmniboxOutcome(_ outcome: OmniboxOutcome?) {
        guard let outcome, omniboxArmed else { return }
        // One-shot: consume this classification so the kernel re-pushing the
        // retained outcome (streaming hits, view re-open) doesn't re-route.
        omniboxArmed = false
        switch outcome {
        case .navigate(let uri):
            // Pasted NIP-19/21 reference → decode + route via hl's existing
            // entity navigation (profile / article / thread).
            secretRejected = false
            scheduleDirectNavigationIfPossible(query: uri, token: searchToken)

        case .resolveNip05(let identifier):
            // Async `.well-known` reverse lookup is in flight. Show a looking-up
            // state; the resolved profile lands reactively and the legacy
            // exact-NIP-05 match (`applyExactNip05NavigationIfNeeded`, run on the
            // parallel free-text scan) navigates to it.
            secretRejected = false
            if directNavigation == nil {
                directOpenMessage = "Looking up \(identifier)…"
            }

        case .openGroup(_, let localId):
            // NIP-29 group reference → hl's existing room/community route
            // (RoomHomeView is keyed by the local group id).
            secretRejected = false
            directOpenMessage = nil
            directNavigation = .group(localId)

        case .rejectSecret:
            // Secret key — never echoed. Clear buckets and show a safe notice.
            directNavigation = nil
            directOpenMessage = nil
            secretRejected = true
            highlights = []
            articles = []
            communities = []
            profiles = []

        case .freeText, .relayUrl, .noMatch:
            // Keep hl's existing bucketed results UI (the parallel free-text
            // scan populates highlights / articles / communities / people).
            secretRejected = false
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

        hasQuery = true
        isLocalLoading = true
        isRelayLoading = true

        // Kernel owns article/highlight/community buckets: dispatch the combined
        // NIP-50 search (results stream into kernel.searchSnapshot → onChange →
        // applyKernelSnapshot). Fire-and-forget (read path).
        kernel.app.dispatch(.runSearch(query: trimmed, scope: .articlesAndHighlights))

        // People bucket stays live (nmp #1697): local kind:0 scan via the bespoke
        // core. Only the profiles field of the snapshot is used; the article/
        // highlight/community fields are now kernel-owned and ignored here.
        searchTask = Task { [weak self] in
            guard let self else { return }
            let snapshot = await self.safeCore.getSearchResultsSnapshot(query: trimmed)
            guard !Task.isCancelled, token == self.searchToken else { return }
            self.appliedQuery = trimmed
            self.profiles = snapshot.profiles
            self.isLocalLoading = false
            // Reflect whatever kernel results have already arrived for this query.
            self.applyKernelSnapshot()
        }
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
        secretRejected = false
        tearDownRelaySearch()
        tearDownDirectEntity()
    }

    // MARK: - Kernel snapshot (article / highlight / community buckets)

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

    // MARK: - Kernel snapshot (article / highlight / community buckets)

    /// Apply the kernel search snapshot. Called from `SearchView.onChange(of:
    /// kernel.searchSnapshot)` and after the people scan resolves. Buckets the
    /// mixed `hits` by kind for Articles, reads the kernel-decoded `highlights`
    /// directly, and maps the local `communities` rows.
    func applyKernelSnapshot() {
        guard let snap = kernel?.searchSnapshot else { return }
        articles = snap.hits
            .filter { $0.kind == 30023 }
            .map(Self.articleRecord(from:))
        highlights = snap.highlights.map(HighlightRecord.init(kernelRow:))
        communities = snap.communities.map(Self.communitySummary(from:))
        // The first snapshot for a query ends the relay-loading flicker.
        isRelayLoading = false
    }

    // MARK: - Kernel row → bespoke record mapping (Phase 7)

    /// Build the bespoke `ArticleRecord` a search card renders from a raw
    /// kind:30023 hit. The article event is self-describing — title/summary/
    /// image/d/published_at are top-level tags (D1: Swift extracts; no kernel
    /// hydration needed for long-form).
    private static func articleRecord(from hit: KernelSearchHitRow) -> ArticleRecord {
        let identifier = tagValue(hit.tags, "d") ?? ""
        let publishedAt = tagValue(hit.tags, "published_at").flatMap(UInt64.init)
        return ArticleRecord(
            eventId: hit.id,
            address: "30023:\(hit.author):\(identifier)",
            pubkey: hit.author,
            identifier: identifier,
            title: tagValue(hit.tags, "title") ?? "",
            summary: tagValue(hit.tags, "summary") ?? "",
            image: tagValue(hit.tags, "image") ?? "",
            content: hit.content,
            hashtags: hit.tags.filter { $0.first == "t" && $0.count > 1 }.map { $0[1] },
            publishedAt: publishedAt,
            createdAt: hit.createdAt
        )
    }

    /// Map a kernel `CommunitySearchRow` (already filtered to public + open by the
    /// local scan) into the bespoke `CommunitySummary` the community card renders.
    private static func communitySummary(from row: CommunitySearchRow) -> CommunitySummary {
        CommunitySummary(
            id: row.groupId,
            name: row.name ?? "",
            about: row.about ?? "",
            picture: row.picture ?? "",
            access: "open",
            visibility: "public",
            adminPubkeys: [],
            memberCount: row.memberCount,
            relayUrl: row.hostRelayUrl,
            metadataEventId: "",
            createdAt: nil
        )
    }

    /// First value of the first tag whose name matches `name` (NIP-01 `[name, value, …]`).
    private static func tagValue(_ tags: [[String]], _ name: String) -> String? {
        tags.first { $0.first == name && $0.count > 1 }?[1]
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
