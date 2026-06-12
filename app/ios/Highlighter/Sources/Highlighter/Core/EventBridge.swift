import Foundation

/// Routes app-scope native capability notifications from Rust deltas.
///
/// Architecture: **nostrdb is source of truth.** The Rust core writes
/// every event to nostrdb, then emits `DataChangeType` deltas wrapped in
/// a `Delta` carrying the `subscription_id` that installed the pump.
/// `0` is reserved for app-scope deltas (signer state, joined-communities
/// summary). Any non-zero id routes to the view-scoped store that asked
/// for the subscription.
final class EventBridge: EventCallback, @unchecked Sendable {
    private weak var appStore: HighlighterStore?

    init(appStore: HighlighterStore) {
        self.appStore = appStore
    }

    // MARK: - EventCallback

    func onDataChanged(delta: Delta) {
        Task { @MainActor in
            let change = delta.change
            let id = delta.subscriptionId

            if id == 0 {
                self.dispatchAppScope(change)
            }
        }
    }

    @MainActor
    private func dispatchAppScope(_ change: DataChangeType) {
        switch change {
        case .signerConnected:
            if let appStore { Task { await appStore.completeLogin() } }
        case .relayStatusChanged:
            break
        case .bookmarksUpdated:
            break
        case .bunkerSignRequest:
            break
        default:
            break
        }
    }
}
