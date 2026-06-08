import Foundation
import Observation

/// View-scoped reactive state for the shake-to-share feedback list. Mirrors
/// `RoomStore` / `DiscussionStore`: owns a per-view nostrdb read plus a
/// subscription handle, and refreshes the thread list whenever the bridge
/// routes a `feedbackThreadsUpdated` delta.
@MainActor
@Observable
final class FeedbackStore {
    private(set) var threads: [FeedbackThreadRecord] = []
    private(set) var isLoading: Bool = true
    private(set) var loadError: String?

    /// Cached so the composer doesn't refetch on every send. Resolved lazily
    /// the first time the user posts a new thread; stays valid for the
    /// lifetime of the store.
    @ObservationIgnored private(set) var cachedAgentPubkey: String?

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

        let threadsOutcome = await core.getFeedbackThreads(coordinate: coordinate)
        if threadsOutcome.error.isEmpty {
            threads = threadsOutcome.values
        } else {
            loadError = threadsOutcome.error
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

    /// Re-query the thread list from nostrdb. Called by the bridge when a new
    /// kind:1 root or kind:513 metadata event lands.
    func refreshThreads() async {
        guard let core, let coordinate else { return }
        let outcome = await core.getFeedbackThreads(coordinate: coordinate)
        if outcome.error.isEmpty {
            threads = outcome.values
        }
    }

    /// Optimistically insert a freshly-published root note so the UI updates
    /// immediately, then let the subscription's eventual delta reconcile.
    func optimisticallyInsert(rootEvent: FeedbackEventRecord) {
        if threads.contains(where: { $0.rootEventId == rootEvent.eventId }) {
            return
        }
        let record = FeedbackThreadRecord(
            rootEventId: rootEvent.eventId,
            authorPubkey: rootEvent.authorPubkey,
            createdAt: rootEvent.createdAt,
            lastActivityAt: rootEvent.createdAt,
            title: nil,
            summary: nil,
            statusLabel: nil,
            preview: previewFromBody(rootEvent.content)
        )
        threads = ([record] + threads).sorted { $0.lastActivityAt > $1.lastActivityAt }
    }

    /// Resolve and cache the project's first agent pubkey. Returns `nil` if
    /// the project event isn't in nostrdb yet — callers should still publish,
    /// just without a `p` tag, and let the agent discover the note via its
    /// own `a`-tag subscription.
    func resolveAgentPubkey() async -> String? {
        if let cachedAgentPubkey { return cachedAgentPubkey }
        guard let core, let coordinate else { return nil }
        let outcome = await core.getProjectFirstAgentPubkey(coordinate: coordinate)
        if outcome.error.isEmpty, let agent = outcome.value {
            cachedAgentPubkey = agent
            return agent
        }
        return nil
    }

    private func previewFromBody(_ body: String) -> String {
        let collapsed = body.split(whereSeparator: \.isWhitespace).joined(separator: " ")
        if collapsed.count <= 140 { return collapsed }
        return String(collapsed.prefix(139)) + "…"
    }
}

enum FeedbackError: LocalizedError {
    case notReady

    var errorDescription: String? {
        switch self {
        case .notReady: return "Feedback isn't ready yet."
        }
    }
}
