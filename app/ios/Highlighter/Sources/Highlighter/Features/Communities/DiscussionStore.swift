import Foundation
import Observation

/// View-scoped reactive state for a room's Discussions tab. Mirrors
/// `RoomStore.swift` — owns a per-view nostrdb read + subscription handle,
/// and applies `DiscussionUpserted` deltas routed by `EventBridge`.
@MainActor
@Observable
final class DiscussionStore {
    private(set) var discussions: [DiscussionRecord] = []
    private(set) var isLoading: Bool = true

    @ObservationIgnored private var groupId: String?
    @ObservationIgnored private var core: SafeHighlighterCore?
    @ObservationIgnored private weak var bridge: EventBridge?
    @ObservationIgnored private var subscriptionHandle: UInt64?

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
        let outcome = await core.subscribeRoomDiscussions(groupId: groupId)
        let projection = core.projectViewSubscriptionStart(
            input: ViewSubscriptionStartProjectionInput(start: outcome)
        )
        guard projection.shouldRegister else {
            // Subscription failure leaves cache-only rendering working.
            return
        }
        subscriptionHandle = projection.handle
        bridge?.registerDiscussions(self, handle: projection.handle)
    }

    func stop() {
        if let handle = subscriptionHandle, let core {
            Task { await core.unsubscribe(handle) }
            bridge?.unregister(handle: handle)
        }
        subscriptionHandle = nil
    }

    func reloadFromCache() async {
        await reloadSnapshot()
    }

    private func reloadSnapshot() async {
        guard let groupId, let core else { return }
        let snapshot = await core.getRoomDiscussionSnapshot(groupId: groupId)
        discussions = snapshot.discussions
    }
}
