import Foundation
import Observation

/// Drives `SearchView`. Owns the query state + the result buckets.
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

    // MARK: - Dependencies

    private let safeCore: SafeHighlighterCore
    /// Phase 7: the kernel owns article/highlight/community search.
    @ObservationIgnored private let kernel: HighlighterAppKernel

    // MARK: - Internal state

    /// Monotonic token so a slow people-scan from a stale query can't overwrite
    /// the current results.
    private var searchToken: UInt64 = 0
    /// The query whose results currently populate the buckets.
    private var appliedQuery: String = ""
    private var searchTask: Task<Void, Never>?

    // MARK: - Init

    init(safeCore: SafeHighlighterCore, kernel: HighlighterAppKernel) {
        self.safeCore = safeCore
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
        kernel.closeSearch()
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

    private func scheduleSearch(for raw: String) {
        searchTask?.cancel()
        searchToken &+= 1
        let token = searchToken
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)

        guard !trimmed.isEmpty else {
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
    }

    // MARK: - Kernel snapshot (article / highlight / community buckets)

    /// Apply the kernel search snapshot. Called from `SearchView.onChange(of:
    /// kernel.searchSnapshot)` and after the people scan resolves. Buckets the
    /// mixed `hits` by kind for Articles, reads the kernel-decoded `highlights`
    /// directly, and maps the local `communities` rows.
    func applyKernelSnapshot() {
        guard let snap = kernel.searchSnapshot else { return }
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
}
