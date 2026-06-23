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
/// for the subscription via `registerProfile` / `registerArticle` / etc.
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
        var profiles: [UInt64: WeakBox<ProfileStore>] = [:]
        var articles: [UInt64: WeakBox<ArticleReaderStore>] = [:]
        var bookmarks: [UInt64: WeakBox<BookmarkStore>] = [:]
        var nostrEntities: [UInt64: WeakBox<NostrEntityCardStore>] = [:]
        /// App-scoped Network Settings store (subscription_id == 0). Weak
        /// so it goes away when the screen is dismissed.
        var networkStore: WeakBox<NetworkSettingsStore>? = nil
        /// Maps subscription handles to app-scoped profile projection pubkeys.
        var profileSnapshotHandles: [UInt64: String] = [:]

        mutating func prune() {
            profiles = profiles.filter { $0.value.value != nil }
            articles = articles.filter { $0.value.value != nil }
            bookmarks = bookmarks.filter { $0.value.value != nil }
            nostrEntities = nostrEntities.filter { $0.value.value != nil }
        }
    }
    private let registry = OSAllocatedUnfairLock(initialState: Registry())

    init(appStore: HighlighterStore) {
        self.appStore = appStore
    }

    // MARK: - Registration (called by view stores when they subscribe)

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

    func registerBookmarkStore(_ store: BookmarkStore, handle: UInt64) {
        registry.withLock { reg in
            reg.bookmarks[handle] = WeakBox(store)
            reg.prune()
        }
    }

    func registerNostrEntity(_ store: NostrEntityCardStore, handle: UInt64) {
        registry.withLock { reg in
            reg.nostrEntities[handle] = WeakBox(store)
            reg.prune()
        }
    }

    func registerProfileSnapshot(pubkeyHex: String, handle: UInt64) {
        registry.withLock { reg in
            reg.profileSnapshotHandles[handle] = pubkeyHex
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
            _ = reg.profiles.removeValue(forKey: handle)
            _ = reg.articles.removeValue(forKey: handle)
            _ = reg.bookmarks.removeValue(forKey: handle)
            _ = reg.nostrEntities.removeValue(forKey: handle)
            _ = reg.profileSnapshotHandles.removeValue(forKey: handle)
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
                    profile: reg.profiles[id]?.value,
                    article: reg.articles[id]?.value,
                    bookmark: reg.bookmarks[id]?.value,
                    nostrEntity: reg.nostrEntities[id]?.value,
                    profileSnapshotPubkey: reg.profileSnapshotHandles[id]
                )
            }

            if let store = routed.profile {
                self.dispatchProfile(change, store: store)
            } else if let store = routed.article {
                self.dispatchArticle(change, store: store)
            } else if let store = routed.bookmark {
                self.dispatchBookmarkStore(change, store: store)
            } else if let store = routed.nostrEntity {
                self.dispatchNostrEntity(change, store: store)
            } else if let pubkey = routed.profileSnapshotPubkey {
                self.dispatchProfileSnapshot(change, pubkey: pubkey)
            }
        }
    }

    /// Snapshot of every view-scoped store that *might* own this delta's
    /// subscription handle. Routing is first-non-nil-wins; a handle is only
    /// ever registered to one store at a time.
    private struct RoutedStores {
        let profile: ProfileStore?
        let article: ArticleReaderStore?
        let bookmark: BookmarkStore?
        let nostrEntity: NostrEntityCardStore?
        let profileSnapshotPubkey: String?
    }

    @MainActor
    private func dispatchArticle(_ change: DataChangeType, store: ArticleReaderStore) {
        if case .articleUpdated = change {
            Task { await store.applyUpdate() }
        }
    }

    @MainActor
    private func dispatchProfile(_ change: DataChangeType, store: ProfileStore) {
        if case .userProfileUpdated = change {
            Task { await store.applyUpdate() }
        }
    }

    @MainActor
    private func dispatchProfileSnapshot(_ change: DataChangeType, pubkey: String) {
        guard case .userProfileUpdated(_, let kind) = change,
              let appStore,
              kind == 0 else { // kind:0 = NIP-01 metadata → profile refresh
            return
        }
        Task { await appStore.applyProfileSnapshotUpdate(pubkeyHex: pubkey) }
    }

    @MainActor
    private func dispatchAppScope(_ change: DataChangeType) {
        switch change {
        case .signerConnected(let user):
            if let appStore { Task { await appStore.completeLogin(user: user) } }
        case .relayDiagnosticsUpdated(let diagnostics):
            let store = registry.withLock { reg in reg.networkStore?.value }
            if let store { Task { await store.applyDiagnostics(diagnostics) } }
        case .relayStatusChanged(let url, let state):
            let store = registry.withLock { reg in reg.networkStore?.value }
            store?.applyStatus(url: url, state: state)
        case .appToastRequested(let message):
            appStore?.shareToast = message
        case .membershipChanged(let groupId):
            if let appStore {
                Task {
                    await appStore.safeCore.confirmPendingJoin(groupId: groupId)
                    await appStore.refreshJoinedCommunities()
                }
            }
        case .communityUpserted:
            // Any group-related event arrived — re-query nostrdb for the
            // authoritative joined set. A single refresh path eliminates the
            // race where incremental upserts (CommunityUpserted) and
            // full-replace refreshes (MembershipChanged) contradicted each
            // other. The query is now membership-driven so missing metadata
            // never wipes the list.
            if let appStore { Task { await appStore.refreshJoinedCommunities() } }
        case .bookmarksUpdated:
            if let appStore { Task { await appStore.refreshBookmarks() } }
        case .bunkerSignRequest:
            break
        default:
            break
        }
    }

    @MainActor
    private func dispatchBookmarkStore(_ change: DataChangeType, store: BookmarkStore) {
        switch change {
        case .bookmarkSetsUpdated, .followingCurationSetsUpdated, .webBookmarksUpdated:
            Task { await store.reload() }
        default:
            break
        }
    }

    @MainActor
    private func dispatchNostrEntity(_ change: DataChangeType, store: NostrEntityCardStore) {
        if case .nostrEntityResolved(let event) = change {
            store.apply(event: event)
        }
    }

}

fileprivate final class WeakBox<T: AnyObject> {
    weak var value: T?
    init(_ value: T) { self.value = value }
}
