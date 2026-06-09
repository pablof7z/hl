import Foundation
import Observation

/// View-scoped store backing the open-thread chat view. Rust owns the bounded
/// row snapshot and message grouping; Swift keeps composer/subscription flags.
@MainActor
@Observable
final class FeedbackThreadStore {
    private(set) var rows: [FeedbackMessageRowProjection] = []
    private(set) var isLoading: Bool = true
    private(set) var loadError: String?
    private(set) var isPublishing: Bool = false

    @ObservationIgnored private var rootEventId: String?
    @ObservationIgnored private var coordinate: String?
    @ObservationIgnored private var core: SafeHighlighterCore?
    @ObservationIgnored private weak var bridge: EventBridge?
    @ObservationIgnored private var subscriptionHandle: UInt64?

    func start(
        rootEventId: String,
        coordinate: String,
        core: SafeHighlighterCore,
        bridge: EventBridge?
    ) async {
        self.rootEventId = rootEventId
        self.coordinate = coordinate
        self.core = core
        self.bridge = bridge
        isLoading = true
        loadError = nil

        let snapshot = await core.getFeedbackThreadSnapshot(rootEventId: rootEventId)
        if snapshot.error.isEmpty {
            apply(snapshot: snapshot)
        } else {
            loadError = snapshot.error
        }
        isLoading = false

        let outcome = await core.subscribeFeedbackThread(rootEventId: rootEventId)
        guard outcome.error.isEmpty else {
            // Cache-only rendering still works.
            return
        }
        subscriptionHandle = outcome.handle
        bridge?.registerFeedbackThread(self, handle: outcome.handle)
    }

    func stop() {
        if let handle = subscriptionHandle, let core {
            Task { await core.unsubscribe(handle) }
            bridge?.unregister(handle: handle)
        }
        subscriptionHandle = nil
    }

    func refreshThread() async {
        guard let core, let rootEventId else { return }
        let snapshot = await core.getFeedbackThreadSnapshot(rootEventId: rootEventId)
        if snapshot.error.isEmpty {
            apply(snapshot: snapshot)
        }
    }

    func apply(snapshot: FeedbackThreadSnapshot) {
        rows = snapshot.rows
        loadError = snapshot.error.isEmpty ? nil : snapshot.error
    }

    /// Send a reply into the open thread. Rust resolves feedback agent
    /// routing and NIP-10 root tagging.
    @discardableResult
    func sendReply(body: String) async -> FeedbackReplyPublishSnapshot? {
        guard let core, let coordinate, let rootEventId else {
            return nil
        }

        isPublishing = true
        defer { isPublishing = false }

        let outcome = await core.publishFeedbackThreadReplySnapshot(
            coordinate: coordinate,
            parentEventId: rootEventId,
            body: body
        )
        if outcome.error.isEmpty {
            apply(snapshot: outcome.snapshot)
        }
        return outcome
    }
}
