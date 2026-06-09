import Observation

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

    var scope: BookmarkLibraryScope = .mine
    var isLoading = false

    private var setsHandle: UInt64?
    private var followingHandle: UInt64?
    private var webHandle: UInt64?

    private weak var bridge: EventBridge?
    private var core: SafeHighlighterCore?

    func start(core: SafeHighlighterCore, bridge: EventBridge) async {
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

        await reload()
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

        let snapshot = await core.getBookmarkLibrarySnapshot()
        myArticles = snapshot.myArticles
        myBookmarkSets = snapshot.myBookmarkSets
        myCurationSets = snapshot.myCurationSets
        myWebBookmarks = snapshot.myWebBookmarks
        followingCurationSets = snapshot.followingCurationSets
    }
}
