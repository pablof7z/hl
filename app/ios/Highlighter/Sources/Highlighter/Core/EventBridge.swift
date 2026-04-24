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
        var profiles: [UInt64: WeakBox<ProfileStore>] = [:]
        var articles: [UInt64: WeakBox<ArticleReaderStore>] = [:]
        var reads: [UInt64: WeakBox<ReadsStore>] = [:]
        var highlights: [UInt64: WeakBox<HighlightsStore>] = [:]

        mutating func prune() {
            rooms = rooms.filter { $0.value.value != nil }
            discussions = discussions.filter { $0.value.value != nil }
            profiles = profiles.filter { $0.value.value != nil }
            articles = articles.filter { $0.value.value != nil }
            reads = reads.filter { $0.value.value != nil }
            highlights = highlights.filter { $0.value.value != nil }
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

    func registerProfile(_ store: ProfileStore, handle: UInt64) {
        registry.withLock { reg in
            reg.profiles[handle] = WeakBox(store)
            reg.prune()
        }
    }

    func registerArticle(_ store: ArticleReaderStore, handle: UInt64) {
        registry.withLock { reg in
            reg.articles[handle] = WeakBox(store)
            reg.prune()
        }
    }

    func registerReads(_ store: ReadsStore, handle: UInt64) {
        registry.withLock { reg in
            reg.reads[handle] = WeakBox(store)
            reg.prune()
        }
    }

    func registerHighlights(_ store: HighlightsStore, handle: UInt64) {
        registry.withLock { reg in
            reg.highlights[handle] = WeakBox(store)
            reg.prune()
        }
    }

    func unregister(handle: UInt64) {
        registry.withLock { reg in
            _ = reg.rooms.removeValue(forKey: handle)
            _ = reg.discussions.removeValue(forKey: handle)
            _ = reg.profiles.removeValue(forKey: handle)
            _ = reg.articles.removeValue(forKey: handle)
            _ = reg.reads.removeValue(forKey: handle)
            _ = reg.highlights.removeValue(forKey: handle)
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

            let (roomStore, discussionStore, profileStore, articleStore, readsStore, highlightsStore) = self.registry.withLock { reg in
                (
                    reg.rooms[id]?.value,
                    reg.discussions[id]?.value,
                    reg.profiles[id]?.value,
                    reg.articles[id]?.value,
                    reg.reads[id]?.value,
                    reg.highlights[id]?.value
                )
            }

            if let roomStore {
                self.dispatchRoom(change, store: roomStore)
            } else if let discussionStore {
                self.dispatchDiscussions(change, store: discussionStore)
            } else if let profileStore {
                self.dispatchProfile(change, store: profileStore)
            } else if let articleStore {
                self.dispatchArticle(change, store: articleStore)
            } else if let readsStore {
                self.dispatchReads(change, store: readsStore)
            } else if let highlightsStore {
                self.dispatchHighlights(change, store: highlightsStore)
            }
        }
    }

    @MainActor
    private func dispatchArticle(_ change: DataChangeType, store: ArticleReaderStore) {
        if case .articleUpdated(_, let kind) = change {
            Task { await store.applyUpdate(kind: kind) }
        }
    }

    @MainActor
    private func dispatchProfile(_ change: DataChangeType, store: ProfileStore) {
        if case .userProfileUpdated(_, let kind) = change {
            Task { await store.applyUpdate(kind: kind) }
        }
    }

    @MainActor
    private func dispatchAppScope(_ change: DataChangeType) {
        switch change {
        case .signerConnected(let user):
            appStore?.currentUser = user
        case .communityUpserted, .membershipChanged:
            // Any group-related event arrived — re-query nostrdb for the
            // authoritative joined set. A single refresh path eliminates the
            // race where incremental upserts (CommunityUpserted) and
            // full-replace refreshes (MembershipChanged) contradicted each
            // other. The query is now membership-driven so missing metadata
            // never wipes the list.
            if let appStore { Task { await appStore.refreshJoinedCommunities() } }
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
    private func dispatchReads(_ change: DataChangeType, store: ReadsStore) {
        if case .followingReadsUpdated = change {
            Task { await store.refresh() }
        }
    }

    @MainActor
    private func dispatchHighlights(_ change: DataChangeType, store: HighlightsStore) {
        if case .followingHighlightsUpdated = change {
            Task { await store.refresh() }
        }
    }

}

fileprivate final class WeakBox<T: AnyObject> {
    weak var value: T?
    init(_ value: T) { self.value = value }
}
