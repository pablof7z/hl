import Foundation
import Observation

/// View-scoped store backing the open feedback-thread chat view. Phase 7: the
/// kernel owns the bounded row snapshot + message grouping
/// (`ViewId.feedbackThread`); this store opens the kernel view, mirrors
/// `kernel.feedbackThread[rootEventId]` into the existing
/// `FeedbackMessageRowProjection` rows (built Swift-side), and posts replies via
/// `hl.feedback.post_reply`.
@MainActor
@Observable
final class FeedbackThreadStore {
    private(set) var rows: [FeedbackMessageRowProjection] = []
    private(set) var isLoading: Bool = true
    private(set) var loadError: String?
    private(set) var isPublishing: Bool = false

    @ObservationIgnored private var rootEventId: String?
    @ObservationIgnored private weak var kernel: HighlighterAppKernel?

    func start(rootEventId: String, kernel: HighlighterAppKernel) async {
        if self.rootEventId != nil, self.rootEventId != rootEventId {
            stop()
        }
        self.rootEventId = rootEventId
        self.kernel = kernel
        isLoading = true
        loadError = nil
        kernel.openFeedbackThread(rootEventId: rootEventId)
        applyKernelSnapshot()
        isLoading = false
    }

    func stop() {
        if let rootEventId {
            kernel?.closeFeedbackThread(rootEventId: rootEventId)
        }
        rootEventId = nil
        kernel = nil
    }

    /// Re-apply the latest kernel snapshot. Called by the owning view's
    /// `onChange(of: kernel.feedbackThread[rootEventId])`.
    func refreshThread() async {
        applyKernelSnapshot()
    }

    /// Mirror `kernel.feedbackThread[rootEventId]` into the rendered rows,
    /// mapping each raw kernel `FeedbackMessageRow` into the bespoke
    /// `FeedbackMessageRowProjection` the view renders.
    func applyKernelSnapshot() {
        guard let rootEventId, let snapshot = kernel?.feedbackThread[rootEventId] else { return }
        rows = snapshot.rows.map { row in
            FeedbackMessageRowProjection(
                event: FeedbackEventRecord(
                    eventId: row.eventId,
                    rootEventId: row.rootEventId,
                    authorPubkey: row.authorPubkey,
                    createdAt: row.createdAt,
                    content: row.content
                ),
                showHeader: row.showHeader
            )
        }
        loadError = snapshot.error
        isPublishing = snapshot.isPublishing
    }

    /// Send a reply into the open thread via the kernel (sole writer). The
    /// kernel resolves NIP-22 root tagging; the rebuilt thread streams back.
    func sendReply(body: String) async {
        guard let rootEventId, let kernel else { return }
        let trimmed = body.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        kernel.app.dispatch(.feedbackPostReply(
            rootEventId: rootEventId,
            content: trimmed,
            parentAuthorPubkey: nil
        ))
    }
}
