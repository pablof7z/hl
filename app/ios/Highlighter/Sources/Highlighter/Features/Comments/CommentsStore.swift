import Foundation
import Observation

/// View-scoped reactive state for the NIP-22 comment thread on a single
/// artifact. Phase 7: the kernel owns the flat record list + per-comment
/// interaction state (`ViewId.commentThread`); Swift builds the display tree
/// (`CommentTreeBuilder`) and keeps only transient composer drafts.
///
/// Allocated by the consuming view in a `.task { }`, torn down on disappear.
/// Kernel is the sole writer — posts/likes/bookmarks dispatch envelope actions;
/// the authoritative thread streams back via `kernel.commentThreads`.
@MainActor
@Observable
final class CommentsStore {
    private(set) var records: [CommentRecord] = []
    private(set) var tree: [CommentThreadNode] = []
    private(set) var isLoading: Bool = true
    private(set) var loadError: String?

    /// kind:7 like counts for any visible comment id (from the kernel rows).
    private(set) var likeCounts: [String: Int] = [:]
    /// Comment event ids the active viewer has liked (kernel `viewer_reacted`).
    private(set) var likedCommentIds: [String] = []
    /// Bookmark membership for any visible comment id (kernel `bookmarked`).
    private(set) var bookmarked: [String] = []

    /// Drafts keyed by `parentEventId ?? "root"`. In-memory only.
    private(set) var drafts: [String: String] = [:]

    @ObservationIgnored private var scope: CommentScope?
    @ObservationIgnored private weak var kernel: HighlighterAppKernel?

    // MARK: - Lifecycle

    func start(scope: CommentScope, kernel: HighlighterAppKernel) async {
        self.scope = scope
        self.kernel = kernel
        isLoading = true
        loadError = nil
        kernel.openCommentThread(rootTagValue: scope.rootTagValue)
        applyKernelSnapshot()
        isLoading = false
    }

    func stop() {
        if let scope {
            kernel?.closeCommentThread(rootTagValue: scope.rootTagValue)
        }
        scope = nil
        kernel = nil
    }

    /// Re-apply the latest kernel snapshot. Called by the owning view's
    /// `onChange(of: kernel.commentThreads[rootTagValue])` so live kind:1111 /
    /// kind:7 / kind:10003 deltas flow into the rendered tree.
    func applyKernelSnapshot() {
        guard let scope, let snapshot = kernel?.commentThreads[scope.rootTagValue] else { return }
        let rows = snapshot.records
        records = rows.map(CommentTreeBuilder.record(from:))
        tree = CommentTreeBuilder.build(from: rows)
        likeCounts = Dictionary(
            rows.map { ($0.eventId, Int($0.likeCount)) },
            uniquingKeysWith: { a, _ in a }
        )
        likedCommentIds = rows.filter { $0.viewerReacted }.map { $0.eventId }
        bookmarked = rows.filter { $0.bookmarked }.map { $0.eventId }
    }

    // MARK: - Drafts

    func draft(forParent parentId: String?) -> String {
        drafts[parentId ?? "root"] ?? ""
    }

    func setDraft(_ text: String, forParent parentId: String?) {
        let key = parentId ?? "root"
        if text.isEmpty {
            drafts.removeValue(forKey: key)
        } else {
            drafts[key] = text
        }
    }

    // MARK: - Publish (kind:1111)

    /// Publish a comment scoped to the artifact via the kernel (sole writer).
    /// `parentEventId == nil` posts a top-level comment; otherwise replies to
    /// that kind:1111 comment. Fire-and-forget — the rebuilt thread streams
    /// back through `kernel.commentThreads`. Returns `true` once dispatched.
    @discardableResult
    func publish(content: String, parentEventId: String?) async -> Bool {
        guard let scope, let kernel else { return false }
        let trimmed = content.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return false }

        // Resolve the parent comment's author (for the lowercase `p` tag) from
        // the visible records when replying.
        let parentAuthor = parentEventId.flatMap { id in
            records.first(where: { $0.eventId == id })?.pubkey
        }

        kernel.app.dispatch(.postComment(
            rootTagName: scope.rootTagName,
            rootTagValue: scope.rootTagValue,
            rootKind: UInt32(scope.rootKind),
            parentEventId: parentEventId,
            rootAuthorPubkey: nil,
            parentAuthorPubkey: parentAuthor,
            content: trimmed
        ))
        setDraft("", forParent: parentEventId)
        return true
    }

    // MARK: - Like (kind:7)

    func isLiked(_ commentId: String) -> Bool {
        likedCommentIds.contains(commentId)
    }

    func likeCount(_ commentId: String) -> Int {
        likeCounts[commentId] ?? 0
    }

    /// Toggle a like on `comment` via the kernel (kind:7 `+`). The kernel
    /// decides react-vs-unreact from its own viewer-reaction tracking (the
    /// reaction event id stays kernel-internal); the reaction projection updates
    /// `viewer_reacted` / `count` and the next snapshot push re-renders.
    func toggleLike(_ comment: CommentRecord) async {
        guard let kernel else { return }
        kernel.app.dispatch(.toggleReaction(
            targetEventId: comment.eventId,
            targetAuthorPubkey: comment.pubkey
        ))
    }

    // MARK: - Bookmark (kind:10003)

    func isBookmarked(_ commentId: String) -> Bool {
        bookmarked.contains(commentId)
    }

    /// Toggle a kind:10003 bookmark on `comment` via the kernel (sole writer).
    func toggleBookmark(_ comment: CommentRecord) async {
        guard let kernel else { return }
        let item = BookmarkRow.event(eventId: comment.eventId, relay: nil)
        if isBookmarked(comment.eventId) {
            kernel.app.dispatch(.removeBookmark(item: item))
        } else {
            kernel.app.dispatch(.addBookmark(item: item))
        }
    }
}
