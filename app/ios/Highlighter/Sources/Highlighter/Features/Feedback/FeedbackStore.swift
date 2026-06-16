import Foundation
import Observation

/// View-scoped reactive state for the shake-to-share feedback list. Rust owns
/// the bounded thread snapshot; Swift keeps only loading/subscription flags.
@MainActor
@Observable
final class FeedbackStore {
    private(set) var threads: [FeedbackThreadRecord] = []
    private(set) var isLoading: Bool = true
    private(set) var loadError: String?

    @ObservationIgnored private var coordinate: String?
    @ObservationIgnored private var core: SafeHighlighterCore?
    @ObservationIgnored private weak var bridge: EventBridge?
    @ObservationIgnored private var subscriptionHandle: UInt64?

    func start(coordinate: String, core: SafeHighlighterCore, bridge: EventBridge?) async {
        if self.coordinate != nil, self.coordinate != coordinate {
            stop()
        }
        self.coordinate = coordinate
        self.core = core
        self.bridge = bridge
        isLoading = true
        loadError = nil

        let snapshot = await core.getFeedbackThreadsSnapshot(coordinate: coordinate)
        let applyProjection = core.projectFeedbackSnapshotApply(
            input: FeedbackSnapshotApplyInput(error: snapshot.error)
        )
        if applyProjection.shouldApplySnapshot {
            apply(snapshot: snapshot)
        } else {
            loadError = applyProjection.loadError
        }
        isLoading = false

        guard subscriptionHandle == nil else { return }
        let outcome = await core.subscribeFeedbackThreads(coordinate: coordinate)
        let projection = core.projectViewSubscriptionStart(
            input: ViewSubscriptionStartProjectionInput(start: outcome)
        )
        guard projection.shouldRegister else {
            // Subscription failure leaves cache-only rendering working.
            return
        }
        subscriptionHandle = projection.handle
        bridge?.registerFeedbackThreads(self, handle: projection.handle)
    }

    func stop() {
        if let handle = subscriptionHandle, let core {
            Task { await core.unsubscribe(handle) }
            bridge?.unregister(handle: handle)
        }
        subscriptionHandle = nil
    }

    /// Re-query the Rust snapshot from nostrdb. Called by the bridge when a
    /// new kind:1 root or kind:513 metadata event lands.
    func refreshThreads() async {
        guard let core, let coordinate else { return }
        let snapshot = await core.getFeedbackThreadsSnapshot(coordinate: coordinate)
        let applyProjection = core.projectFeedbackSnapshotApply(
            input: FeedbackSnapshotApplyInput(error: snapshot.error)
        )
        if applyProjection.shouldApplySnapshot {
            apply(snapshot: snapshot)
        }
    }

    func apply(snapshot: FeedbackThreadsSnapshot) {
        let applyProjection = core?.projectFeedbackSnapshotApply(
            input: FeedbackSnapshotApplyInput(error: snapshot.error)
        )
        threads = snapshot.threads
        loadError = applyProjection?.loadError
    }

}
