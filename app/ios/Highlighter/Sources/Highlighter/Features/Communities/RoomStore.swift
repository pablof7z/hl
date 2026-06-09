import Foundation
import Observation

/// View-scoped reactive state for a single community's room home.
/// Lifetime is tied to the SwiftUI view that creates it — allocated on
/// `.task { }`, deallocated on view disappear. Owns its subscription
/// handle so granular Observation tracks only this room's data.
///
/// Data comes from nostrdb via the Rust core; this class never fabricates
/// or caches data that isn't also in nostrdb.
@MainActor
@Observable
final class RoomStore {
    private(set) var artifacts: [ArtifactRecord] = []
    private(set) var highlights: [HydratedHighlight] = []
    private(set) var highlightsByReference: [String: [HighlightRecord]] = [:]
    private(set) var commentsByReference: [String: [CommentRecord]] = [:]
    private(set) var lanes: [RoomLane] = []
    private(set) var isLoading: Bool = true

    @ObservationIgnored private var groupId: String?
    @ObservationIgnored private var core: SafeHighlighterCore?
    @ObservationIgnored private weak var bridge: EventBridge?
    @ObservationIgnored private var subscriptionHandle: UInt64?

    /// Called from the View's `.task { }`. Reads nostrdb immediately for
    /// instant offline rendering, then installs a live subscription so
    /// incoming events flow in as deltas routed by `EventBridge`.
    func start(groupId: String, core: SafeHighlighterCore, bridge: EventBridge?) async {
        if self.groupId != nil, self.groupId != groupId {
            stop()
        }
        self.groupId = groupId
        self.core = core
        self.bridge = bridge
        isLoading = true
        await reloadSnapshot()
        isLoading = false

        guard subscriptionHandle == nil else { return }
        let outcome = await core.subscribeRoom(groupId: groupId)
        guard outcome.error.isEmpty else {
            // Subscription failure leaves cache-only rendering working.
            return
        }
        subscriptionHandle = outcome.handle
        bridge?.registerRoom(self, handle: outcome.handle)
    }

    func stop() {
        if let handle = subscriptionHandle, let core {
            Task { await core.unsubscribe(handle) }
            bridge?.unregister(handle: handle)
        }
        subscriptionHandle = nil
    }

    // MARK: - Delta application (called by EventBridge)

    func reloadFromCache() async {
        await reloadSnapshot()
    }

    func commentCount(for artifact: ArtifactRecord) -> Int {
        guard let core else {
            return 0
        }
        let buckets = commentsByReference.map { key, values in
            CommentReferenceBucket(commentKey: key, comments: values)
        }
        return Int(core.countArtifactComments(
            artifact: artifact,
            commentsByReference: buckets
        ))
    }

    private func reloadSnapshot() async {
        guard let groupId, let core else { return }
        let snapshot = await core.getRoomHomeSnapshot(groupId: groupId)
        artifacts = snapshot.artifacts
        highlights = snapshot.highlights
        highlightsByReference = snapshot.highlightsByReference.reduce(into: [:]) { buckets, bucket in
            buckets[bucket.lookupKey] = bucket.highlights
        }
        commentsByReference = snapshot.commentsByReference.reduce(into: [:]) { buckets, bucket in
            buckets[bucket.commentKey] = bucket.comments
        }
        lanes = snapshot.lanes
    }
}
