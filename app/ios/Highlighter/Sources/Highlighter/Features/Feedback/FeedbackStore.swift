import Foundation
import Observation

/// View-scoped reactive state for the shake-to-share feedback list. Phase 7:
/// the kernel owns the bounded thread snapshot (`ViewId.feedbackThreads`); this
/// store opens the kernel view and mirrors `kernel.feedbackThreads` into the
/// `FeedbackThreadRecord` rows the list renders (built Swift-side from the raw
/// kernel rows). Posting goes through `hl.feedback.post_root`.
@MainActor
@Observable
final class FeedbackStore {
    private(set) var threads: [FeedbackThreadRecord] = []
    private(set) var isLoading: Bool = true
    private(set) var loadError: String?

    @ObservationIgnored private weak var kernel: HighlighterAppKernel?

    func start(kernel: HighlighterAppKernel) async {
        self.kernel = kernel
        isLoading = true
        loadError = nil
        kernel.openFeedbackThreads()
        applyKernelSnapshot()
        isLoading = false
    }

    func stop() {
        kernel?.closeFeedbackThreads()
        kernel = nil
    }

    /// Re-apply the latest kernel snapshot. Called by the owning view's
    /// `onChange(of: kernel.feedbackThreads)`.
    func refreshThreads() async {
        applyKernelSnapshot()
    }

    /// Publish a new feedback root note via the kernel (sole writer). The new
    /// thread streams back into `kernel.feedbackThreads`.
    func postRoot(content: String) {
        let trimmed = content.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        kernel?.app.dispatch(.feedbackPostRoot(content: trimmed))
    }

    /// Mirror `kernel.feedbackThreads` into the rendered view model, mapping each
    /// raw kernel `FeedbackThreadRow` into the bespoke `FeedbackThreadRecord`.
    func applyKernelSnapshot() {
        guard let snapshot = kernel?.feedbackThreads else { return }
        threads = snapshot.threads.map { row in
            FeedbackThreadRecord(
                rootEventId: row.rootEventId,
                authorPubkey: row.authorPubkey,
                createdAt: row.createdAt,
                lastActivityAt: row.lastActivityAt,
                title: row.title,
                summary: row.summary,
                statusLabel: row.statusLabel,
                preview: row.preview
            )
        }
        loadError = snapshot.error
    }
}
