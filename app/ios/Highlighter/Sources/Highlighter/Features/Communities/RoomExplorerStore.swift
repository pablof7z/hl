import Foundation
import Observation

/// View-scoped store for the rooms explorer. Owns the shelves that appear on
/// the explorer home — featured, friends, authors, new — and dispatches
/// join-request actions to the Rust core.
///
/// Source of truth: nostrdb via `SafeHighlighterCore`. This store never
/// caches raw events; it only holds `CommunitySummary` / `RoomRecommendation`
/// snapshots produced by Rust queries.
@MainActor
@Observable
final class RoomExplorerStore {
    var featured: [CommunitySummary] = []
    var newNoteworthy: [CommunitySummary] = []
    var friendsShelf: [RoomRecommendation] = []
    var authorsShelf: [RoomRecommendation] = []
    /// True while the very first `refresh` is in flight so the UI can show
    /// shimmer placeholders instead of an empty shell.
    var isFirstLoad: Bool = true

    @ObservationIgnored private weak var appStore: HighlighterStore?
    @ObservationIgnored private var hasStartedDiscovery = false
    @ObservationIgnored private var hasStartedCuration = false

    init(appStore: HighlighterStore) {
        self.appStore = appStore
    }

    /// Run all shelf queries in parallel. Safe to call on every view appear —
    /// each query reads cached ndb state and returns in milliseconds.
    /// Relay subscriptions are fired-and-forgotten so they never block the
    /// nostrdb reads or delay `isFirstLoad → false`.
    func refresh() async {
        guard let appStore else { return }
        let safeCore = appStore.safeCore

        // Fire relay subscriptions in background — don't await them.
        // They'll push events into nostrdb; the user can pull-to-refresh
        // or the next appear will pick up the new data.
        if !hasStartedDiscovery {
            hasStartedDiscovery = true
            Task { await safeCore.startRoomDiscovery() }
        }
        Task { _ = await safeCore.startFriendsRoomsDiscovery() }
        Task { await ensureCurationSubscription(safeCore: safeCore) }

        let curatorOutcome = await safeCore.getRoomExplorerCuratorPubkey()
        let curatorPubkey = curatorOutcome.error.isEmpty ? curatorOutcome.value : ""

        async let featuredTask: [CommunitySummary] = {
            let outcome = await safeCore.getFeaturedRooms(curatorPubkeyHex: curatorPubkey)
            return outcome.error.isEmpty ? outcome.values : []
        }()
        async let newTask: [CommunitySummary] = {
            let outcome = await safeCore.getNewRooms(limit: 24)
            return outcome.error.isEmpty ? outcome.values : []
        }()
        async let friendsTask: [RoomRecommendation] = {
            let outcome = await safeCore.getRoomsWithFriends(limit: 16)
            return outcome.error.isEmpty ? outcome.values : []
        }()
        async let authorsTask: [RoomRecommendation] = {
            let outcome = await safeCore.getRoomsFromReadAuthors(limit: 16)
            return outcome.error.isEmpty ? outcome.values : []
        }()

        let (fetchedFeatured, fetchedNew, fetchedFriends, fetchedAuthors) =
            await (featuredTask, newTask, friendsTask, authorsTask)

        featured = fetchedFeatured
        newNoteworthy = safeCore.excludeJoinedRooms(
            rooms: fetchedNew,
            joined: appStore.joinedCommunities
        )
        friendsShelf = fetchedFriends
        authorsShelf = fetchedAuthors
        isFirstLoad = false
    }

    /// Publish a NIP-29 kind:9021 join-request for the given room. Rust owns
    /// request and membership-confirmation toast state.
    func requestJoin(room: CommunitySummary) async {
        guard let appStore else { return }
        let outcome = await appStore.safeCore.requestJoinRoom(groupId: room.id, roomName: room.name)
        if !outcome.error.isEmpty {
            print("requestJoinRoom failed for \(room.id): \(outcome.error)")
        }
    }

    /// Lightweight re-read of nostrdb — no subscription side-effects.
    /// Called by EventBridge whenever a CommunityUpserted delta arrives so
    /// newly-discovered rooms appear without a pull-to-refresh.
    func reloadFromCache() async {
        guard let appStore else { return }
        let safeCore = appStore.safeCore
        let curatorOutcome = await safeCore.getRoomExplorerCuratorPubkey()
        let curatorPubkey = curatorOutcome.error.isEmpty ? curatorOutcome.value : ""

        async let featuredTask: [CommunitySummary] = {
            let outcome = await safeCore.getFeaturedRooms(curatorPubkeyHex: curatorPubkey)
            return outcome.error.isEmpty ? outcome.values : []
        }()
        async let newTask: [CommunitySummary] = {
            let outcome = await safeCore.getNewRooms(limit: 24)
            return outcome.error.isEmpty ? outcome.values : []
        }()
        async let friendsTask: [RoomRecommendation] = {
            let outcome = await safeCore.getRoomsWithFriends(limit: 16)
            return outcome.error.isEmpty ? outcome.values : []
        }()
        async let authorsTask: [RoomRecommendation] = {
            let outcome = await safeCore.getRoomsFromReadAuthors(limit: 16)
            return outcome.error.isEmpty ? outcome.values : []
        }()

        let (f, n, fr, a) = await (featuredTask, newTask, friendsTask, authorsTask)
        featured = f
        newNoteworthy = safeCore.excludeJoinedRooms(
            rooms: n,
            joined: appStore.joinedCommunities
        )
        friendsShelf = fr
        authorsShelf = a
    }

    // MARK: - Private

    private func ensureCurationSubscription(safeCore: SafeHighlighterCore) async {
        if !hasStartedCuration {
            let outcome = await safeCore.startRoomExplorerFeaturedRooms()
            if outcome.error.isEmpty {
                hasStartedCuration = true
            } else {
                print("startRoomExplorerFeaturedRooms failed: \(outcome.error)")
            }
        }
    }
}
