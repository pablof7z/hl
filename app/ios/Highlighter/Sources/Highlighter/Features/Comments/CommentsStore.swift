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
    func publish(content: String, parentEventId: String?) async -> CommentPublishSnapshotOutcome? {
        guard let core, let scope else {
            return nil
        }

        let outcome = await core.publishCommentForScopeSnapshot(
            scope: scope,
            parentEventId: parentEventId,
            content: content
        )
        guard outcome.error.isEmpty else { return outcome }
        apply(snapshot: outcome.snapshot)
        setDraft("", forParent: parentEventId)
        return outcome
    }

    // MARK: - Like (kind:7)

    func isLiked(_ commentId: String) -> Bool {
        commentLikeStateProjection(eventIdHex: commentId)?.isLiked ?? false
    }

    func likeCount(_ commentId: String) -> Int {
        likeCounts[commentId] ?? 0
    }

    /// Toggle a like on `comment`. If the user already liked, deletes the
    /// reaction via NIP-09. Optimistic count + state update.
    func toggleLike(_ comment: CommentRecord) async {
        guard let core else { return }
        let id = comment.eventId
        guard let projection = commentLikeStateProjection(eventIdHex: id),
              projection.canApply else { return }
        let wasLiked = projection.isLiked
        likedCommentIds = projection.optimisticLikedEventIds
        likeCounts[id] = Int(projection.likeCount)

        let outcome = await core.toggleCommentLike(
            eventId: id,
            authorPubkeyHex: comment.pubkey
        )
        if outcome.error.isEmpty {
            if let confirmed = commentLikeStateProjection(
                eventIdHex: projection.canonicalEventIdHex,
                likeCount: UInt32(likeCount(id)),
                desiredLiked: outcome.value
            ) {
                likedCommentIds = confirmed.optimisticLikedEventIds
                likeCounts[id] = Int(confirmed.likeCount)
            }
            return
        }

        if let rollback = commentLikeStateProjection(
            eventIdHex: projection.canonicalEventIdHex,
            likeCount: UInt32(likeCount(id)),
            desiredLiked: wasLiked,
            adjustCount: true
        ) {
            likedCommentIds = rollback.optimisticLikedEventIds
            likeCounts[id] = Int(rollback.likeCount)
        }
    }

    // MARK: - Bookmark (kind:10003)

    func isBookmarked(_ commentId: String) -> Bool {
        eventBookmarkStateProjection(eventIdHex: commentId)?.isBookmarked ?? false
    }

    func toggleBookmark(_ comment: CommentRecord) async {
        guard let core else { return }
        guard let projection = eventBookmarkStateProjection(eventIdHex: comment.eventId),
              projection.canApply else { return }
        bookmarked = projection.optimisticEventIds
        let outcome = await core.toggleEventBookmark(eventIdHex: projection.canonicalEventIdHex)
        if outcome.error.isEmpty {
            if let confirmed = eventBookmarkStateProjection(
                eventIdHex: projection.canonicalEventIdHex,
                desiredMember: outcome.value
            ) {
                bookmarked = confirmed.optimisticEventIds
            }
        } else {
            if let rollback = eventBookmarkStateProjection(
                eventIdHex: projection.canonicalEventIdHex,
                desiredMember: projection.isBookmarked
            ) {
                bookmarked = rollback.optimisticEventIds
            }
        }
    }

    private func eventBookmarkStateProjection(
        eventIdHex: String,
        desiredMember: Bool? = nil
    ) -> EventBookmarkStateProjection? {
        guard let core else { return nil }
        return core.projectEventBookmarkState(input: EventBookmarkStateProjectionInput(
            eventIds: bookmarked,
            eventIdHex: eventIdHex,
            desiredMember: desiredMember
        ))
    }

    private func commentLikeStateProjection(
        eventIdHex: String,
        likeCount: UInt32? = nil,
        desiredLiked: Bool? = nil,
        adjustCount: Bool = false
    ) -> CommentLikeStateProjection? {
        guard let core else { return nil }
        return core.projectCommentLikeState(input: CommentLikeStateProjectionInput(
            likedEventIds: likedCommentIds,
            eventIdHex: eventIdHex,
            likeCount: likeCount ?? UInt32(likeCounts[eventIdHex] ?? 0),
            desiredLiked: desiredLiked,
            adjustCount: adjustCount
        ))
    }
}
