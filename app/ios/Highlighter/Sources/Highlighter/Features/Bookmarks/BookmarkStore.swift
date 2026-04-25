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

        if let h = try? await core.subscribeBookmarkSets() {
            setsHandle = h
            bridge.registerBookmarkStore(self, handle: h)
        }
        if let h = try? await core.subscribeFollowingCurationSets() {
            followingHandle = h
            bridge.registerBookmarkStore(self, handle: h)
        }
        if let h = try? await core.subscribeWebBookmarks() {
            webHandle = h
            bridge.registerBookmarkStore(self, handle: h)
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

        async let sets = (try? await core.getMyBookmarkSets()) ?? []
        async let curations = (try? await core.getMyCurationSets()) ?? []
        async let webs = (try? await core.getMyWebBookmarks()) ?? []
        async let following = (try? await core.getFollowingCurationSets()) ?? []

        myBookmarkSets = await sets
        myCurationSets = await curations
        myWebBookmarks = await webs
        followingCurationSets = await following
    }

    func loadArticles(addresses: Set<String>) async {
        guard let core, !addresses.isEmpty else {
            myArticles = []
            return
        }
        var loaded: [ArticleRecord] = []
        for address in addresses {
            let parts = address.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false)
            guard parts.count == 3, parts[0] == "30023" else { continue }
            let pubkey = String(parts[1])
            let dTag = String(parts[2])
            guard !pubkey.isEmpty, !dTag.isEmpty else { continue }
            if let article = try? await core.getArticle(pubkeyHex: pubkey, dTag: dTag) {
                loaded.append(article)
            }
        }
        myArticles = loaded.sorted {
            ($0.publishedAt ?? $0.createdAt ?? 0) > ($1.publishedAt ?? $1.createdAt ?? 0)
        }
    }
}
