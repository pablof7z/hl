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

    func start(addresses: [String], core: SafeHighlighterCore, bridge: EventBridge) async {
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

        let following = await followingOutcome
        followingCurationSets = following.error.isEmpty ? following.values : []
    }

    func loadArticles(addresses: [String]) async {
        guard let core, !addresses.isEmpty else {
            myArticles = []
            return
        }
        let outcome = await core.getBookmarkedArticles(addresses: addresses)
        myArticles = outcome.error.isEmpty ? outcome.values : []
    }
}
