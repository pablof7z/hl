import Foundation
import Observation

/// Home feed — composes friend highlights and friend-surfaced reads into
/// Rust-owned screen rows. Rust owns source queries, grouping, dedupe, stable
/// ids, and ordering; Swift owns subscription handles and rendering state.
@MainActor
@Observable
final class HomeFeedStore {
    typealias Item = HomeFeedItem

    var items: [Item] = []
    var isLoadingInitial: Bool = true
    var loadError: String?

    @ObservationIgnored private let core: SafeHighlighterCore
    @ObservationIgnored weak var eventBridge: EventBridge?
    @ObservationIgnored private var subscriptionHandles: [UInt64] = []

    init(safeCore: SafeHighlighterCore, eventBridge: EventBridge?) {
        self.core = safeCore
        self.eventBridge = eventBridge
    }

    func start() async {
        await refresh()
        isLoadingInitial = false
        await installSubscriptions()
    }

    func stop() {
        let handles = subscriptionHandles
        subscriptionHandles = []
        for handle in handles {
            Task { [core] in await core.unsubscribe(handle) }
            eventBridge?.unregister(handle: handle)
        }
    }

    func refresh() async {
        let snapshot = await core.getHomeFeedSnapshot()
        if snapshot.error.isEmpty {
            items = snapshot.items
            loadError = nil
        } else {
            items = snapshot.items
            loadError = snapshot.error
        }
    }

    private func installSubscriptions() async {
        guard subscriptionHandles.isEmpty, let bridge = eventBridge else { return }

        let reads = await core.subscribeFollowingReads()
        let readsProjection = core.projectViewSubscriptionStart(
            input: ViewSubscriptionStartProjectionInput(start: reads)
        )
        if readsProjection.shouldRegister {
            subscriptionHandles.append(readsProjection.handle)
            bridge.registerHomeFeed(self, handle: readsProjection.handle)
        }

        let highlights = await core.subscribeFollowingHighlights()
        let highlightsProjection = core.projectViewSubscriptionStart(
            input: ViewSubscriptionStartProjectionInput(start: highlights)
        )
        if highlightsProjection.shouldRegister {
            subscriptionHandles.append(highlightsProjection.handle)
            bridge.registerHomeFeed(self, handle: highlightsProjection.handle)
        }
    }
}
