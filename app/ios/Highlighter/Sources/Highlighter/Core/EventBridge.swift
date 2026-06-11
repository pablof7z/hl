import Foundation
import os

/// Routes `Delta` notifications from Rust into the appropriate Swift
/// `@Observable` store.
///
/// Architecture: **nostrdb is source of truth.** The Rust core writes
/// every event to nostrdb, then emits `DataChangeType` deltas wrapped in
/// a `Delta` carrying the `subscription_id` that installed the pump.
/// `0` is reserved for app-scope deltas (signer state, joined-communities
/// summary). Any non-zero id routes to the view-scoped store that asked
/// for the subscription via `registerRoom` / `registerDiscussions`.
final class EventBridge: EventCallback, @unchecked Sendable {
    private weak var appStore: HighlighterStore?

    /// Weak registry of view-scoped stores keyed by subscription handle.
    /// Weak so a View deallocating automatically drops its store from the
    /// registry. Uses `OSAllocatedUnfairLock` (iOS 16+) so the lock is
    /// async-safe — `withLock { ... }` doesn't trip Swift 6's strict
    /// concurrency checks the way `NSLock` does.
    /// `@unchecked Sendable` is sound because every access goes through
    /// `OSAllocatedUnfairLock.withLock`, which serializes mutations. The
    /// `WeakBox` values hold weak references to `@MainActor`-isolated
    /// stores, so even if a reference survives into the wrong isolation
    /// it's nil or eventually nil'd by ARC.
    fileprivate struct Registry: @unchecked Sendable {
        var rooms: [UInt64: WeakBox<RoomStore>] = [:]
        var discussions: [UInt64: WeakBox<DiscussionStore>] = [:]
        var chats: [UInt64: WeakBox<ChatStore>] = [:]
        var chatPresence: [UInt64: WeakBox<ChatPresenceProbe>] = [:]
        var feedbackThreads: [UInt64: WeakBox<FeedbackStore>] = [:]
        var feedbackThreadDetails: [UInt64: WeakBox<FeedbackThreadStore>] = [:]
        /// App-scoped Network Settings store (subscription_id == 0). Weak
        /// so it goes away when the screen is dismissed.
        var networkStore: WeakBox<NetworkSettingsStore>? = nil
        mutating func prune() {
            rooms = rooms.filter { $0.value.value != nil }
            discussions = discussions.filter { $0.value.value != nil }
            chats = chats.filter { $0.value.value != nil }
            chatPresence = chatPresence.filter { $0.value.value != nil }
            feedbackThreads = feedbackThreads.filter { $0.value.value != nil }
            feedbackThreadDetails = feedbackThreadDetails.filter { $0.value.value != nil }
        }
    }
    private let registry = OSAllocatedUnfairLock(initialState: Registry())

    init(appStore: HighlighterStore) {
        self.appStore = appStore
    }

    // MARK: - Registration (called by view stores when they subscribe)

    func registerRoom(_ store: RoomStore, handle: UInt64) {
        registry.withLock { reg in
            reg.rooms[handle] = WeakBox(store)
            reg.prune()
        }
    }

    func registerDiscussions(_ store: DiscussionStore, handle: UInt64) {
        registry.withLock { reg in
            reg.discussions[handle] = WeakBox(store)
            reg.prune()
        }
    }

    func registerChat(_ store: ChatStore, handle: UInt64) {
        registry.withLock { reg in
            reg.chats[handle] = WeakBox(store)
            reg.prune()
        }
    }

    func registerChatPresence(_ probe: ChatPresenceProbe, handle: UInt64) {
        registry.withLock { reg in
            reg.chatPresence[handle] = WeakBox(probe)
            reg.prune()
        }
    }

    func registerFeedbackThreads(_ store: FeedbackStore, handle: UInt64) {
        registry.withLock { reg in
            reg.feedbackThreads[handle] = WeakBox(store)
            reg.prune()
        }
    }

    func registerFeedbackThread(_ store: FeedbackThreadStore, handle: UInt64) {
        registry.withLock { reg in
            reg.feedbackThreadDetails[handle] = WeakBox(store)
            reg.prune()
        }
    }

    func registerNetworkStore(_ store: NetworkSettingsStore) {
        registry.withLock { reg in
            reg.networkStore = WeakBox(store)
        }
    }

    func unregister(handle: UInt64) {
        registry.withLock { reg in
            _ = reg.rooms.removeValue(forKey: handle)
            _ = reg.discussions.removeValue(forKey: handle)
            _ = reg.chats.removeValue(forKey: handle)
            _ = reg.chatPresence.removeValue(forKey: handle)
            _ = reg.feedbackThreads.removeValue(forKey: handle)
            _ = reg.feedbackThreadDetails.removeValue(forKey: handle)
        }
    }

    // MARK: - EventCallback

    func onDataChanged(delta: Delta) {
        Task { @MainActor in
            let change = delta.change
            let id = delta.subscriptionId

            if id == 0 {
                self.dispatchAppScope(change)
                return
            }

            let routed = self.registry.withLock { reg -> RoutedStores in
                RoutedStores(
                    room: reg.rooms[id]?.value,
                    discussion: reg.discussions[id]?.value,
                    chat: reg.chats[id]?.value,
                    chatPresence: reg.chatPresence[id]?.value,
                    feedback: reg.feedbackThreads[id]?.value,
                    feedbackThread: reg.feedbackThreadDetails[id]?.value
                )
            }

            if let store = routed.room {
                self.dispatchRoom(change, store: store)
            } else if let store = routed.discussion {
                self.dispatchDiscussions(change, store: store)
            } else if let store = routed.chat {
                self.dispatchChat(change, store: store)
            } else if let probe = routed.chatPresence {
                self.dispatchChatPresence(change, probe: probe)
            } else if let store = routed.feedback {
                self.dispatchFeedbackThreads(change, store: store)
            } else if let store = routed.feedbackThread {
                self.dispatchFeedbackThread(change, store: store)
            }
        }
    }

    /// Snapshot of every view-scoped store that *might* own this delta's
    /// subscription handle. Routing is first-non-nil-wins; a handle is only
    /// ever registered to one store at a time.
    private struct RoutedStores {
        let room: RoomStore?
        let discussion: DiscussionStore?
        let chat: ChatStore?
        let chatPresence: ChatPresenceProbe?
        let feedback: FeedbackStore?
        let feedbackThread: FeedbackThreadStore?
    }

    @MainActor
    private func dispatchFeedbackThreads(_ change: DataChangeType, store: FeedbackStore) {
        if case .feedbackThreadsUpdated = change {
            Task { await store.refreshThreads() }
        }
    }

    @MainActor
    private func dispatchFeedbackThread(_ change: DataChangeType, store: FeedbackThreadStore) {
        if case .feedbackThreadEventUpserted(let event) = change {
            store.apply(event: event)
        }
    }

    @MainActor
    private func dispatchAppScope(_ change: DataChangeType) {
        switch change {
        case .signerConnected:
            if let appStore { Task { await appStore.completeLogin() } }
        case .relayStatusChanged(let url, let state):
            let store = registry.withLock { reg in reg.networkStore?.value }
            store?.applyStatus(url: url, state: state)
        case .bookmarksUpdated:
            break
        case .bunkerSignRequest:
            break
        default:
            break
        }
    }

    @MainActor
    private func dispatchRoom(_ change: DataChangeType, store: RoomStore) {
        switch change {
        case .artifactUpserted(_, let artifact):
            store.apply(artifact: artifact)
        case .highlightUpserted(_, let highlight):
            store.apply(highlight: highlight)
        case .highlightShared:
            // Kind:16 arrives as a hint that a new highlight belongs in the
            // room; the corresponding `highlightUpserted` (once the 9802 is
            // fetched) carries the body we display. No-op here.
            break
        default:
            break
        }
    }

    @MainActor
    private func dispatchDiscussions(_ change: DataChangeType, store: DiscussionStore) {
        switch change {
        case .discussionUpserted(_, let discussion):
            store.apply(discussion: discussion)
        default:
            break
        }
    }

    @MainActor
    private func dispatchChat(_ change: DataChangeType, store: ChatStore) {
        switch change {
        case .chatMessageUpserted(_, let message):
            store.apply(message: message)
        default:
            break
        }
    }

    @MainActor
    private func dispatchChatPresence(_ change: DataChangeType, probe: ChatPresenceProbe) {
        if case .chatMessageUpserted = change {
            probe.notifyActivity()
        }
    }

}

fileprivate final class WeakBox<T: AnyObject> {
    weak var value: T?
    init(_ value: T) { self.value = value }
}
