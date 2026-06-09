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
        self.coordinate = coordinate
        self.core = core
        self.bridge = bridge
        isLoading = true
        loadError = nil

        let snapshot = await core.getFeedbackThreadsSnapshot(coordinate: coordinate)
        if snapshot.error.isEmpty {
            apply(snapshot: snapshot)
        } else {
            loadError = snapshot.error
        }
        isLoading = false

        let outcome = await core.subscribeFeedbackThreads(coordinate: coordinate)
        guard outcome.error.isEmpty else {
            // Subscription failure leaves cache-only rendering working.
            return
        }
        subscriptionHandle = outcome.handle
        bridge?.registerFeedbackThreads(self, handle: outcome.handle)
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
        if snapshot.error.isEmpty {
            apply(snapshot: snapshot)
        }
    }

    func apply(snapshot: FeedbackThreadsSnapshot) {
        threads = snapshot.threads
        loadError = snapshot.error.isEmpty ? nil : snapshot.error
    }

}
