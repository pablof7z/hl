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

    /// Read the Rust-owned shelf snapshot. Safe to call on every view appear —
    /// the snapshot reads cached ndb state and returns in milliseconds.
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

        let snapshot = await safeCore.getRoomExplorerSnapshot(joined: appStore.joinedCommunities)
        apply(snapshot)
        isFirstLoad = false
    }

    /// Publish a NIP-29 kind:9021 join-request for the given room. Rust owns
    /// request and membership-confirmation toast state.
    func requestJoin(room: CommunitySummary) async {
        guard let appStore else { return }
        let outcome = await appStore.safeCore.requestJoinRoom(groupId: room.id, roomName: room.name)
        let projection = appStore.safeCore.projectRoomExplorerJoinRequestResult(
            input: RoomExplorerJoinRequestResultInput(
                groupId: room.id,
                error: outcome.error
            )
        )
        if projection.shouldLog {
            print(projection.logMessage)
        }
    }

    /// Lightweight re-read of Rust's cached explorer snapshot — no subscription side-effects.
    /// Called by EventBridge whenever a CommunityUpserted delta arrives so
    /// newly-discovered rooms appear without a pull-to-refresh.
    func reloadFromCache() async {
        guard let appStore else { return }
        let safeCore = appStore.safeCore
        let snapshot = await safeCore.getRoomExplorerSnapshot(joined: appStore.joinedCommunities)
        apply(snapshot)
    }

    // MARK: - Private

    private func apply(_ snapshot: RoomExplorerSnapshot) {
        featured = snapshot.featured
        newNoteworthy = snapshot.newNoteworthy
        friendsShelf = snapshot.friendsShelf
        authorsShelf = snapshot.authorsShelf
    }

    private func ensureCurationSubscription(safeCore: SafeHighlighterCore) async {
        if !hasStartedCuration {
            let outcome = await safeCore.startRoomExplorerFeaturedRooms()
            let projection = safeCore.projectRoomExplorerFeaturedStartResult(
                input: RoomExplorerFeaturedStartResultInput(error: outcome.error)
            )
            if projection.shouldMarkStarted {
                hasStartedCuration = true
            }
            if projection.shouldLog {
                print(projection.logMessage)
            }
        }
    }
}
