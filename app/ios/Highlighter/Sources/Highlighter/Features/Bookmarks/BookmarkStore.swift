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

        let setsStart = await core.subscribeBookmarkSets()
        if setsStart.error.isEmpty {
            setsHandle = setsStart.handle
            bridge.registerBookmarkStore(self, handle: setsStart.handle)
        }
        let followingStart = await core.subscribeFollowingCurationSets()
        if followingStart.error.isEmpty {
            followingHandle = followingStart.handle
            bridge.registerBookmarkStore(self, handle: followingStart.handle)
        }
        let webStart = await core.subscribeWebBookmarks()
        if webStart.error.isEmpty {
            webHandle = webStart.handle
            bridge.registerBookmarkStore(self, handle: webStart.handle)
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
