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
        // Resolved thin previews for the a/e/i artifact references in these rows
        // (discussion-chip #1). Keyed by canonical coordinate.
        let previewsByCoord = Dictionary(
            snapshot.artifactPreviews.map { ($0.coordinate, $0) },
            uniquingKeysWith: { first, _ in first }
        )
        discussions = snapshot.rows.map { row in
            DiscussionRecord(
                id: row.eventId,
                eventId: row.eventId,
                groupId: groupId,
                pubkey: row.authorPubkey,
                title: row.title,
                body: row.body,
                summary: "",
                createdAt: row.createdAt,
                attachment: Self.attachment(for: row, previews: previewsByCoord)
            )
        }
    }

    /// Build the discussion's attachment chip. Prefers a RESOLVED rich artifact
    /// preview (title/image/author) keyed by the row's `a`/`e`/`i` coordinate;
    /// falls back to the bare `r` URL attachment when no coordinate resolves
    /// (preview pending or URL-only share). The chip projection
    /// (`attachment_projection`) labels with `title` when present, else `url`.
    private static func attachment(
        for row: DiscussionRow,
        previews: [String: ArtifactPreviewRow]
    ) -> DiscussionAttachment? {
        if let coordinate = row.artifactCoordinate,
           let preview = previews[coordinate],
           let title = preview.title, !title.isEmpty {
            let (tagName, tagValue) = splitCoordinate(coordinate)
            return DiscussionAttachment(
                referenceTagName: tagName,
                referenceTagValue: tagValue,
                referenceKind: "",
                url: preview.displayUrl ?? row.attachmentUrl ?? "",
                title: title,
                author: preview.authorPubkey ?? "",
                image: preview.imageUrl ?? "",
                summary: preview.summary ?? ""
            )
        }
        // Fall back to the bare URL attachment.
        return row.attachmentUrl.map { url in
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
    }

    /// Split a canonical coordinate (`"a:30023:pk:d"`, `"i:isbn:…"`, `"r:url"`)
    /// into its tag name and value on the first `:`.
    private static func splitCoordinate(_ coordinate: String) -> (String, String) {
        guard let idx = coordinate.firstIndex(of: ":") else { return ("", coordinate) }
        return (
            String(coordinate[..<idx]),
            String(coordinate[coordinate.index(after: idx)...])
        )
    }
}
