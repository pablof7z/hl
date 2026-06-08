import Foundation
import Observation

/// View-scoped reactive state for the NIP-22 comment thread on a single
/// artifact. Holds the flat record list, the built tree, per-comment
/// reaction + bookmark state, and the in-flight composer drafts.
///
/// Pattern follows `RoomStore` — allocated by the consuming view in a
/// `.task { }`, deallocated on disappear; reads NostrDB via the Rust
/// core; never fabricates data the core doesn't have.
@MainActor
@Observable
final class CommentsStore {
    private(set) var records: [CommentRecord] = []
    private(set) var tree: [CommentNode] = []
    private(set) var isLoading: Bool = true
    private(set) var loadError: String?

    /// kind:7 like counts for any visible comment id. Updated optimistically
    /// on toggle and reconciled on refresh.
    private(set) var likeCounts: [String: Int] = [:]
    /// Reaction event id for the current user's like on a given comment id
    /// (allows quick "undo like" via NIP-09 deletion). Missing = not liked.
    private(set) var myLikeEventIds: [String: String] = [:]
    /// Bookmark membership for any visible comment id.
    private(set) var bookmarked: Set<String> = []

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
        let outcome = await core.getCommentsForScope(
            scope: scope,
            limit: 256
        )
        if outcome.error.isEmpty {
            records = outcome.values
            tree = CommentTreeBuilder.build(
                records: outcome.values,
                rootTagValue: scope.rootTagValue
            )
            await refreshReactionsAndBookmarks(for: outcome.values)
        } else {
            loadError = outcome.error
        }
        isLoading = false
    }

    /// Reaction counts + my-bookmark predicates for every visible comment.
    /// Runs in parallel; failures leave previous state in place.
    private func refreshReactionsAndBookmarks(for records: [CommentRecord]) async {
        guard let core else { return }
        let captured = core
        await withTaskGroup(of: (String, ReactionSummary?, Bool?).self) { group in
            for r in records {
                let id = r.eventId
                group.addTask {
                    let reactionOutcome = await captured.getLikeSummaryForEvent(targetEventId: id, limit: 128)
                    let summary = reactionOutcome.error.isEmpty ? reactionOutcome.value : nil
                    let bookmarkOutcome = await captured.isEventBookmarked(eventIdHex: id)
                    let bookmarked = bookmarkOutcome.error.isEmpty ? bookmarkOutcome.value : nil
                    return (id, summary, bookmarked)
                }
            }
            for await (id, summary, isBookmarked) in group {
                if let summary {
                    likeCounts[id] = Int(summary.likeCount)
                    if let myLikeEventId = summary.myLikeEventId {
                        myLikeEventIds[id] = myLikeEventId
                    } else {
                        myLikeEventIds.removeValue(forKey: id)
                    }
                }
                if let isBookmarked {
                    if isBookmarked { self.bookmarked.insert(id) }
                    else { self.bookmarked.remove(id) }
                }
            }
        }
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
    /// kind:1111 comment. Optimistically inserts the new record and
    /// rebuilds the tree.
    @discardableResult
    func publish(content: String, parentEventId: String?) async -> CommentOutcome {
        guard let core, let scope else {
            return CommentOutcome(value: nil, error: "store not started")
        }
        let trimmed = content.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return CommentOutcome(value: nil, error: "comment body must not be empty")
        }

        let outcome = await core.publishCommentForScope(
            scope: scope,
            parentEventId: parentEventId,
            content: trimmed
        )
        guard outcome.error.isEmpty, let record = outcome.value else { return outcome }

        // Optimistic insert
        if !records.contains(where: { $0.eventId == record.eventId }) {
            records.append(record)
            tree = CommentTreeBuilder.build(
                records: records,
                rootTagValue: scope.rootTagValue
            )
        }
        setDraft("", forParent: parentEventId)
        return outcome
    }

    // MARK: - Like (kind:7)

    func isLiked(_ commentId: String) -> Bool {
        myLikeEventIds[commentId] != nil
    }

    func likeCount(_ commentId: String) -> Int {
        likeCounts[commentId] ?? 0
    }

    /// Toggle a like on `comment`. If the user already liked, deletes the
    /// reaction via NIP-09. Optimistic count + state update.
    func toggleLike(_ comment: CommentRecord) async {
        guard let core else { return }
        let id = comment.eventId
        let alreadyLiked = isLiked(id)

        // Optimistic
        let prevCount = likeCount(id)
        if alreadyLiked {
            likeCounts[id] = max(0, prevCount - 1)
        } else {
            likeCounts[id] = prevCount + 1
            myLikeEventIds[id] = "pending"
        }

        if alreadyLiked, let myReactionId = myLikeEventIds[id], myReactionId != "pending" {
            let outcome = await core.unpublishReaction(reactionEventId: myReactionId)
            if outcome.error.isEmpty {
                myLikeEventIds.removeValue(forKey: id)
                return
            }
        } else {
            let outcome = await core.publishCommentLike(
                eventId: id,
                authorPubkeyHex: comment.pubkey
            )
            if outcome.error.isEmpty, let reaction = outcome.value {
                myLikeEventIds[id] = reaction.eventId
                return
            }
        }

        // Roll back on failure
        likeCounts[id] = prevCount
        if !alreadyLiked {
            myLikeEventIds.removeValue(forKey: id)
        }
    }

    // MARK: - Bookmark (kind:10003)

    func isBookmarked(_ commentId: String) -> Bool {
        bookmarked.contains(commentId)
    }

    func toggleBookmark(_ comment: CommentRecord) async {
        guard let core else { return }
        let id = comment.eventId
        let was = bookmarked.contains(id)
        if was { bookmarked.remove(id) } else { bookmarked.insert(id) }
        let outcome = await core.toggleEventBookmark(eventIdHex: id)
        if outcome.error.isEmpty {
            let now = outcome.value
            if now { bookmarked.insert(id) } else { bookmarked.remove(id) }
        } else {
            // Roll back
            if was { bookmarked.insert(id) } else { bookmarked.remove(id) }
        }
    }
}

// MARK: - Convenience accessors

extension CommentsStore {
    /// Total comment count across the whole tree (top-level + replies).
    var totalCount: Int {
        records.count
    }

    /// The N most-recent commenter pubkeys (for the toolbar avatar trio).
    func recentCommenterPubkeys(limit: Int = 3) -> [String] {
        let sorted = records.sorted { ($0.createdAt ?? 0) > ($1.createdAt ?? 0) }
        var seen = Set<String>()
        var out: [String] = []
        for r in sorted {
            if seen.insert(r.pubkey).inserted {
                out.append(r.pubkey)
                if out.count >= limit { break }
            }
        }
        return out
    }
}
