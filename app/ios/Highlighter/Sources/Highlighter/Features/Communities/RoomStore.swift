import Foundation
import Observation

/// View-scoped reactive state for a single community's room home.
///
/// Phase 7: kernel-backed. The kernel owns the room-home aggregation
/// (`ViewId.roomHome`): artifact library, hydrated highlights, per-artifact
/// comment buckets, and assembled lanes — all already computed in
/// `project_room_home_snapshot`. This store mirrors
/// `kernel.roomHomeSnapshots[groupId]` into the bespoke view-model types the
/// existing views render (D1: kernel emits raw/assembled rows; Swift shapes the
/// model). The owning `RoomHomeView` owns the kernel view lifecycle
/// (`openRoomHome`/`closeRoomHome`); this store only reads the snapshot.
///
/// The rich `artifact_record` on each lane / library row (enriched in the
/// kernel so podcast `audio_url`/GUIDs, book `catalog_id`, chapters, and
/// reference tags survive) is the bespoke `ArtifactRecord` verbatim — so the
/// presentation projections (`projectRoomLibraryCardKind`,
/// `getArtifactDetailProjection`, …) consume it unchanged.
@MainActor
@Observable
final class RoomStore {
    private(set) var artifacts: [ArtifactRecord] = []
    private(set) var highlights: [HydratedHighlight] = []
    private(set) var lanes: [RoomLane] = []
    private(set) var isLoading: Bool = true

    @ObservationIgnored private var groupId: String?
    @ObservationIgnored private weak var kernel: HighlighterAppKernel?
    /// share_event_id → comment count, derived from the kernel assembled lanes.
    @ObservationIgnored private var commentCountByShareId: [String: Int] = [:]

    /// Called from the View's `.task { }` AFTER the view has opened the kernel
    /// room-home view. Mirrors whatever snapshot is already cached; live updates
    /// flow via the view's `onChange(of: kernel.roomHomeSnapshots[groupId])`.
    func start(groupId: String, kernel: HighlighterAppKernel) {
        self.groupId = groupId
        self.kernel = kernel
        isLoading = true
        applyKernelSnapshot()
    }

    func stop() {
        groupId = nil
        kernel = nil
    }

    /// Mirror `kernel.roomHomeSnapshots[groupId]` into the rendered view models.
    func applyKernelSnapshot() {
        guard let groupId, let snapshot = kernel?.roomHomeSnapshots[groupId] else { return }

        artifacts = snapshot.artifactLibrary.map(\.artifactRecord)

        highlights = snapshot.highlights.map { row in
            HydratedHighlight(
                highlight: HighlightRecord(kernelRow: row),
                artifact: nil,
                sharedByEventId: nil,
                sharedByPubkey: nil
            )
        }

        lanes = snapshot.assembledLanes.map { lane in
            RoomLane(
                id: lane.shareEventId,
                artifact: lane.artifactRecord,
                highlights: lane.highlights.map { row in
                    HydratedHighlight(
                        highlight: HighlightRecord(kernelRow: row),
                        artifact: lane.artifactRecord,
                        sharedByEventId: lane.shareEventId,
                        sharedByPubkey: nil
                    )
                },
                comments: lane.comments.map(CommentTreeBuilder.record(from:))
            )
        }

        commentCountByShareId = Dictionary(
            snapshot.assembledLanes.map { ($0.shareEventId, Int($0.comments.count)) },
            uniquingKeysWith: { first, _ in first }
        )

        isLoading = false
    }

    /// Resolve the count of NIP-22 comments anchored to an artifact. The kernel
    /// already grouped comments per artifact into the assembled lane, so this is
    /// a direct lookup by the share event id (dormant artifacts with no
    /// highlights AND no comments are absent → count 0).
    func commentCount(for artifact: ArtifactRecord) -> Int {
        commentCountByShareId[artifact.shareEventId] ?? 0
    }
}
