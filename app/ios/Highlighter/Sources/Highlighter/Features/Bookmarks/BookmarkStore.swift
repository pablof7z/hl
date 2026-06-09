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

        await installSubscriptions(core: core, bridge: bridge)

        await reload()
    }

    func stop() {
        let handles = [setsHandle, followingHandle, webHandle].compactMap { $0 }
        setsHandle = nil
        followingHandle = nil
        webHandle = nil
        for handle in handles {
            bridge?.unregister(handle: handle)
        }
        if let core, !handles.isEmpty {
            Task { [core, handles] in
                for handle in handles {
                    await core.unsubscribe(handle)
                }
            }
        }
    }

    private func installSubscriptions(core: SafeHighlighterCore, bridge: EventBridge) async {
        if setsHandle == nil {
            let setsStart = await core.subscribeBookmarkSets()
            if setsStart.error.isEmpty {
                setsHandle = setsStart.handle
                bridge.registerBookmarkStore(self, handle: setsStart.handle)
            }
        }
        if followingHandle == nil {
            let followingStart = await core.subscribeFollowingCurationSets()
            if followingStart.error.isEmpty {
                followingHandle = followingStart.handle
                bridge.registerBookmarkStore(self, handle: followingStart.handle)
            }
        }
        if webHandle == nil {
            let webStart = await core.subscribeWebBookmarks()
            if webStart.error.isEmpty {
                webHandle = webStart.handle
                bridge.registerBookmarkStore(self, handle: webStart.handle)
            }
        }
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
