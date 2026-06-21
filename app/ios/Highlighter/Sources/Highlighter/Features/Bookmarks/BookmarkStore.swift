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

    private weak var bridge: EventBridge?
    private var core: SafeHighlighterCore?
    /// Phase 7 / #1653: kernel owns all bookmarks panes — articles, sets, and web.
    private var kernel: HighlighterAppKernel?

    func start(core: SafeHighlighterCore, bridge: EventBridge, kernel: HighlighterAppKernel) async {
        self.core = core
        self.bridge = bridge
        self.kernel = kernel

        kernel.openBookmarks()
        await reload()
    }

    func stop() {
        kernel?.closeBookmarks()
    }

    func reload() async {
        isLoading = true
        defer { isLoading = false }
        // All panes are now kernel-owned (#1653). Apply the latest snapshot.
        applyKernelSnapshot()
    }

    /// Apply the full kernel bookmarks snapshot — articles pane (Phase 7) and
    /// collections/web panes (#1653). Called from `reload()` and from
    /// `BookmarksView.onChange(of: kernel.bookmarks)` so panes that resolve
    /// after the initial load fade in automatically.
    func applyKernelSnapshot() {
        let snap = kernel?.bookmarks
        myArticles = (snap?.articlePreviews ?? []).map(ArticleRecord.init(bookmarkPreview:))
        myBookmarkSets = (snap?.myBookmarkSets ?? []).map(BookmarkSetRecord.init(row:))
        myCurationSets = (snap?.myCurationSets ?? []).map(BookmarkSetRecord.init(row:))
        followingCurationSets = (snap?.followingCurationSets ?? []).map(BookmarkSetRecord.init(row:))
        myWebBookmarks = (snap?.myWebBookmarks ?? []).map(WebBookmarkRecord.init(row:))
    }
}

// MARK: - Kernel row → bespoke record mappings (#1653)

extension BookmarkSetRecord {
    /// Build a `BookmarkSetRecord` (bespoke presentation type) from a kernel
    /// `BookmarkSetRow` (raw D1 snapshot field). Title/description/image use
    /// empty-string fallbacks to match the bespoke `parse_set_event` contract.
    init(row: BookmarkSetRow) {
        self.init(
            id: row.dTag,
            pubkey: row.pubkey,
            kind: row.kind,
            title: row.title ?? "",
            description: row.description ?? "",
            image: row.image ?? "",
            articleAddresses: row.articleAddresses,
            noteIds: row.noteIds,
            rRefs: row.rRefs,
            topics: row.topics,
            createdAt: row.createdAt
        )
    }
}

extension WebBookmarkRecord {
    /// Build a `WebBookmarkRecord` from a kernel `WebBookmarkRow`.
    init(row: WebBookmarkRow) {
        self.init(
            url: row.url,
            pubkey: row.pubkey,
            title: row.title ?? "",
            description: row.description ?? "",
            topics: row.topics,
            publishedAt: row.publishedAt,
            createdAt: row.createdAt
        )
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
