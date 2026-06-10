import Foundation
import Observation

/// View-scoped reactive state for the NIP-22 comment thread on a single
/// artifact. Rust owns the flat record list, built tree, and per-comment
/// reaction + bookmark snapshot; Swift keeps only transient composer drafts.
///
/// Pattern follows `RoomStore` — allocated by the consuming view in a
/// `.task { }`, deallocated on disappear; reads NostrDB via the Rust
/// core; never fabricates data the core doesn't have.
@MainActor
@Observable
final class CommentsStore {
    private(set) var records: [CommentRecord] = []
    private(set) var tree: [CommentThreadNode] = []
    private(set) var isLoading: Bool = true
    private(set) var loadError: String?

    /// kind:7 like counts for any visible comment id. Updated optimistically
    /// on toggle and reconciled on refresh.
    private(set) var likeCounts: [String: Int] = [:]
    /// Comment event ids the current user has liked. Rust owns canonicalization,
    /// dedupe, optimistic membership, and publish-vs-delete decisions.
    private(set) var likedCommentIds: [String] = []
    /// Bookmark membership for any visible comment id. Rust owns event-id
    /// canonicalization, dedupe, and optimistic membership projection.
    private(set) var bookmarked: [String] = []

    /// Drafts keyed by `parentEventId ?? "root"`. In-memory only — survives
    /// detent transitions but not view recreation. (Persistent drafts are
    /// deferred per design doc.)
    private(set) var drafts: [String: String] = [:]

    @ObservationIgnored private var scope: CommentScope?
    @ObservationIgnored private var core: SafeHighlighterCore?

    // MARK: - Lifecycle

    func start(
        scope: CommentScope,
        core: SafeHighlighterCore
    ) async {
        self.scope = scope
        self.core = core
        await refresh()
    }

    func refresh() async {
        guard let core, let scope else { return }
        isLoading = true
        loadError = nil
        let snapshot = await core.getCommentThreadSnapshot(scope: scope)
        if snapshot.error.isEmpty {
            apply(snapshot: snapshot)
        } else {
            loadError = snapshot.error
        }
        isLoading = false
    }

    private func apply(snapshot: CommentThreadSnapshot) {
        records = snapshot.records
        tree = snapshot.tree
        apply(interactions: snapshot.interactions)
        loadError = snapshot.error.isEmpty ? nil : snapshot.error
    }

    private func apply(interactions snapshot: CommentInteractionSnapshot) {
        likeCounts = Dictionary(
            uniqueKeysWithValues: snapshot.rows.map { ($0.eventId, Int($0.likeCount)) }
        )
        likedCommentIds = snapshot.likedEventIds
        bookmarked = snapshot.bookmarkedEventIds
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

    // MARK: - Publish

    /// Publish a comment scoped to the artifact. `parentEventId == nil`
    /// posts a top-level thread; otherwise posts as a reply to that
    /// kind:1111 comment. Rust returns the rebuilt snapshot.
    @discardableResult
    func publish(content: String, parentEventId: String?) async -> CommentPublishSnapshot? {
        guard let core, let scope else {
            return nil
        }

        let outcome = await core.publishCommentForScopeSnapshot(
            scope: scope,
            parentEventId: parentEventId,
            content: content
        )
        let result = core.projectCommentPublishResult(
            input: CommentPublishResultInput(error: outcome.error)
        )
        guard result.didPublish else { return outcome }
        apply(snapshot: outcome.snapshot)
        setDraft("", forParent: parentEventId)
        return outcome
    }

    // MARK: - Like (kind:7)

    func isLiked(_ commentId: String) -> Bool {
        likedCommentIds.contains(commentId)
    }

    func likeCount(_ commentId: String) -> Int {
        likeCounts[commentId] ?? 0
    }

    /// Toggle a like on `comment`. Rust publishes/deletes and returns the
    /// interaction snapshot to render.
    func toggleLike(_ comment: CommentRecord) async {
        guard let core else { return }
        let outcome = await core.toggleCommentLikeSnapshot(
            records: records,
            eventId: comment.eventId,
            authorPubkeyHex: comment.pubkey
        )
        apply(interactions: outcome.interactions)
    }

    // MARK: - Bookmark (kind:10003)

    func isBookmarked(_ commentId: String) -> Bool {
        bookmarked.contains(commentId)
    }

    func toggleBookmark(_ comment: CommentRecord) async {
        guard let core else { return }
        let outcome = await core.toggleCommentBookmarkSnapshot(
            records: records,
            eventIdHex: comment.eventId
        )
        apply(interactions: outcome.interactions)
    }
}
