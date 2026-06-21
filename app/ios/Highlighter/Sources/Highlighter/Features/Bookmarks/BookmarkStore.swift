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
    /// Phase 7: the kernel owns the Articles pane (bookmarked kind:30023 →
    /// artifact-preview keystone). Collections/web stay on the live lane (nmp #1653).
    private var kernel: HighlighterAppKernel?

    func start(core: SafeHighlighterCore, bridge: EventBridge, kernel: HighlighterAppKernel) async {
        self.core = core
        self.bridge = bridge
        self.kernel = kernel

        kernel.openBookmarks()
        await installSubscriptions(core: core, bridge: bridge)

        await reload()
    }

    func stop() {
        kernel?.closeBookmarks()
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
            let projection = core.projectViewSubscriptionStart(
                input: ViewSubscriptionStartProjectionInput(start: setsStart)
            )
            if projection.shouldRegister {
                setsHandle = projection.handle
                bridge.registerBookmarkStore(self, handle: projection.handle)
            }
        }
        if followingHandle == nil {
            let followingStart = await core.subscribeFollowingCurationSets()
            let projection = core.projectViewSubscriptionStart(
                input: ViewSubscriptionStartProjectionInput(start: followingStart)
            )
            if projection.shouldRegister {
                followingHandle = projection.handle
                bridge.registerBookmarkStore(self, handle: projection.handle)
            }
        }
        if webHandle == nil {
            let webStart = await core.subscribeWebBookmarks()
            let projection = core.projectViewSubscriptionStart(
                input: ViewSubscriptionStartProjectionInput(start: webStart)
            )
            if projection.shouldRegister {
                webHandle = projection.handle
                bridge.registerBookmarkStore(self, handle: projection.handle)
            }
        }
    }

    func reload() async {
        guard let core else { return }
        isLoading = true
        defer { isLoading = false }

        // Articles pane is kernel-owned (Phase 7); the collections/web panes stay
        // on the live lane (nmp #1653).
        let snapshot = await core.getBookmarkLibrarySnapshot()
        myBookmarkSets = snapshot.myBookmarkSets
        myCurationSets = snapshot.myCurationSets
        myWebBookmarks = snapshot.myWebBookmarks
        followingCurationSets = snapshot.followingCurationSets
        applyKernelSnapshot()
    }

    /// Apply the kernel bookmarks snapshot's Articles pane. Called from `reload()`
    /// and from `BookmarksView.onChange(of: kernel.bookmarks)` so previews that
    /// resolve after the initial load (the keystone fetches missing coords) fade in.
    func applyKernelSnapshot() {
        myArticles = (kernel?.bookmarks?.articlePreviews ?? []).map(ArticleRecord.init(bookmarkPreview:))
    }
}

// MARK: - Kernel preview → bespoke record mapping (Phase 7)

extension ArticleRecord {
    /// Build the `ArticleRecord` a bookmark Articles-pane card renders from a
    /// keystone `ArtifactPreviewRow` (title/summary/image/author). The card shows
    /// metadata only — no body — so `content` is empty and `eventId` is unset;
    /// the pane keys its `ForEach` on `address` (the stable bookmark coordinate).
    init(bookmarkPreview preview: ArtifactPreviewRow) {
        // coordinate is `30023:<pubkey>:<d>` — the `d` is everything after the 2nd colon.
        let parts = preview.coordinate.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false)
        let identifier = parts.count == 3 ? String(parts[2]) : ""
        self.init(
            eventId: "",
            address: preview.coordinate,
            pubkey: preview.authorPubkey ?? "",
            identifier: identifier,
            title: preview.title ?? "",
            summary: preview.summary ?? "",
            image: preview.imageUrl ?? "",
            content: "",
            hashtags: [],
            publishedAt: nil,
            createdAt: nil
        )
    }
}
