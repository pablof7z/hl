import Foundation
import Observation

/// View-scoped reactive state for a room's Discussions tab. Phase 7: the kernel
/// owns the bounded kind:11+discussion rows (`ViewId.roomDiscussions`); this
/// store opens the kernel view and mirrors `kernel.roomDiscussions[groupId]`
/// into the `DiscussionRecord` rows the existing list renders (built Swift-side
/// from the raw kernel rows). Posting goes through `hl.discussion.post`.
@MainActor
@Observable
final class DiscussionStore {
    private(set) var discussions: [DiscussionRecord] = []
    private(set) var isLoading: Bool = true

    @ObservationIgnored private var groupId: String?
    @ObservationIgnored private weak var kernel: HighlighterAppKernel?

    func start(groupId: String, kernel: HighlighterAppKernel) async {
        if self.groupId != nil, self.groupId != groupId {
            stop()
        }
        self.groupId = groupId
        self.kernel = kernel
        isLoading = true
        kernel.openRoomDiscussions(groupId: groupId)
        applyKernelSnapshot()
        isLoading = false
    }

    func stop() {
        if let groupId {
            kernel?.closeRoomDiscussions(groupId: groupId)
        }
        groupId = nil
        kernel = nil
    }

    /// Re-apply the latest kernel snapshot. Called by the owning view's
    /// `onChange(of: kernel.roomDiscussions[groupId])` so a freshly-published
    /// kind:11 appears live.
    func reloadFromCache() async {
        applyKernelSnapshot()
    }

    /// Mirror `kernel.roomDiscussions[groupId]` into the rendered view model,
    /// mapping each raw kernel `DiscussionRow` into the bespoke `DiscussionRecord`
    /// shape the list renders (D1: kernel emits raw rows; Swift shapes the model).
    func applyKernelSnapshot() {
        guard let groupId, let snapshot = kernel?.roomDiscussions[groupId] else { return }
        discussions = snapshot.rows.map { row in
            // The kernel row carries the `r` URL attachment only. Reconstruct a
            // URL-kind DiscussionAttachment so the existing chip renders; richer
            // a/e/i artifact references are not in the kernel row (see note).
            let attachment: DiscussionAttachment? = row.attachmentUrl.map { url in
                DiscussionAttachment(
                    referenceTagName: "r",
                    referenceTagValue: url,
                    referenceKind: "",
                    url: url,
                    title: "",
                    author: "",
                    image: "",
                    summary: ""
                )
            }
            return DiscussionRecord(
                id: row.eventId,
                eventId: row.eventId,
                groupId: groupId,
                pubkey: row.authorPubkey,
                title: row.title,
                body: row.body,
                summary: "",
                createdAt: row.createdAt,
                attachment: attachment
            )
        }
    }
}
