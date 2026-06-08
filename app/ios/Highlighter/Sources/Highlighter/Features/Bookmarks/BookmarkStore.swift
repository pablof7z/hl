import Foundation
import Observation

enum BookmarkScope {
    case mine, explore
}

@MainActor
@Observable
final class BookmarkStore {
    // Mine-mode data
    var myArticles: [ArticleRecord] = []
    var myBookmarkSets: [BookmarkSetRecord] = []
    var myCurationSets: [BookmarkSetRecord] = []
    var myWebBookmarks: [WebBookmarkRecord] = []

    // Explore-mode data
    var followingCurationSets: [BookmarkSetRecord] = []

    var scope: BookmarkScope = .mine
    var isLoading = false

    private var setsHandle: UInt64?
    private var followingHandle: UInt64?
    private var webHandle: UInt64?

    private weak var bridge: EventBridge?
    private var core: SafeHighlighterCore?

    func start(addresses: Set<String>, core: SafeHighlighterCore, bridge: EventBridge) async {
        self.core = core
        self.bridge = bridge

        let setsOutcome = await core.subscribeBookmarkSets()
        if setsOutcome.error.isEmpty {
            setsHandle = setsOutcome.handle
            bridge.registerBookmarkStore(self, handle: setsOutcome.handle)
        }
        let followingOutcome = await core.subscribeFollowingCurationSets()
        if followingOutcome.error.isEmpty {
            followingHandle = followingOutcome.handle
            bridge.registerBookmarkStore(self, handle: followingOutcome.handle)
        }
        let webOutcome = await core.subscribeWebBookmarks()
        if webOutcome.error.isEmpty {
            webHandle = webOutcome.handle
            bridge.registerBookmarkStore(self, handle: webOutcome.handle)
        }

        await withTaskGroup(of: Void.self) { group in
            group.addTask { await self.reload() }
            group.addTask { await self.loadArticles(addresses: addresses) }
        }
    }

    func stop() {
        if let h = setsHandle { bridge?.unregister(handle: h); setsHandle = nil }
        if let h = followingHandle { bridge?.unregister(handle: h); followingHandle = nil }
        if let h = webHandle { bridge?.unregister(handle: h); webHandle = nil }
    }

    func reload() async {
        guard let core else { return }
        isLoading = true
        defer { isLoading = false }

        async let setsOutcome = core.getMyBookmarkSets()
        async let curationsOutcome = core.getMyCurationSets()
        async let websOutcome = core.getMyWebBookmarks()
        async let followingOutcome = core.getFollowingCurationSets()

        let sets = await setsOutcome
        let curations = await curationsOutcome
        let webs = await websOutcome
        myBookmarkSets = sets.error.isEmpty ? sets.values : []
        myCurationSets = curations.error.isEmpty ? curations.values : []
        myWebBookmarks = webs.error.isEmpty ? webs.values : []

        // Drop curations from Explore that would render as "Empty Collection"
        // — either zero items at all, or every articleAddress fails to resolve
        // against the local NostrDB cache and there are no note refs to
        // fall back on. Mine keeps empty sets so authors can edit drafts.
        let following = await followingOutcome
        let raw = following.error.isEmpty ? following.values : []
        followingCurationSets = await Self.dropEmpty(raw, core: core)
    }

    /// Returns the subset of `sets` whose detail view would actually render
    /// at least one item. Article address parsing and local cache resolution
    /// stay in Rust; Swift only keeps or drops the projected set rows.
    private static func dropEmpty(
        _ sets: [BookmarkSetRecord],
        core: SafeHighlighterCore
    ) async -> [BookmarkSetRecord] {
        await withTaskGroup(of: (Int, Bool).self) { group in
            for (idx, set) in sets.enumerated() {
                group.addTask {
                    (idx, await hasResolvableItem(set, core: core))
                }
            }
            var keep = Set<Int>()
            for await (idx, ok) in group where ok {
                keep.insert(idx)
            }
            return sets.enumerated()
                .compactMap { keep.contains($0.offset) ? $0.element : nil }
        }
    }

    private static func hasResolvableItem(
        _ set: BookmarkSetRecord,
        core: SafeHighlighterCore
    ) async -> Bool {
        if !set.noteIds.isEmpty { return true }
        if set.articleAddresses.isEmpty { return false }
        let outcome = await core.getBookmarkSetArticles(record: set)
        return outcome.error.isEmpty && !outcome.values.isEmpty
    }

    func loadArticles(addresses: Set<String>) async {
        guard let core, !addresses.isEmpty else {
            myArticles = []
            return
        }
        let outcome = await core.getBookmarkedArticles(addresses: Array(addresses))
        myArticles = outcome.error.isEmpty ? outcome.values : []
    }
}
