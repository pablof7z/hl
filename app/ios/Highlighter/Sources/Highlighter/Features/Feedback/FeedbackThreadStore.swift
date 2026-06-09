import Foundation
import Observation

/// View-scoped store backing the open-thread chat view. Loads every kind:1
/// `e`-tagged to the root (regardless of author so agent replies appear),
/// then receives per-event upserts from the bridge.
@MainActor
@Observable
final class FeedbackThreadStore {
    private(set) var events: [FeedbackEventRecord] = []
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

        let eventsOutcome = await core.getFeedbackThreadEvents(rootEventId: rootEventId)
        if eventsOutcome.error.isEmpty {
            events = eventsOutcome.values
        } else {
            loadError = eventsOutcome.error
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

    func apply(event: FeedbackEventRecord) {
        guard let core else {
            preconditionFailure("FeedbackThreadStore.apply called before start")
        }
        events = core.upsertFeedbackThreadEvent(
            events: events,
            event: event
        )
    }

    /// Send a reply into the open thread. Rust/nostrdb resolves the current
    /// agent pubkey at send time; replies publish without a `p` tag when the
    /// project event isn't available.
    @discardableResult
    func sendReply(body: String) async -> FeedbackEventOutcome? {
        guard let core, let coordinate, let rootEventId else {
            return nil
        }
        let agentOutcome = await core.getProjectFirstAgentPubkey(coordinate: coordinate)
        let agent = agentOutcome.error.isEmpty ? agentOutcome.value : nil

        isPublishing = true
        defer { isPublishing = false }

        let outcome = await core.publishFeedbackNote(
            coordinate: coordinate,
            agentPubkey: agent,
            parentEventId: rootEventId,
            body: body
        )
        guard outcome.error.isEmpty, let record = outcome.value else { return outcome }
        apply(event: record)
        return outcome
    }
}
