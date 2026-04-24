import Foundation
import Observation

/// View-scoped store for the rooms explorer. Owns the shelves that appear on
/// the explorer home — featured, friends, authors, new — and orchestrates
/// the join-request flow by delegating toast state to the app-scope store.
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
    func refresh() async {
        guard let appStore else { return }
        let safeCore = appStore.safeCore

        // Install backfill subscriptions on first appear so the relay sends
        // fresh catalogue entries while the user browses. Idempotent in
        // Rust; cheap to call repeatedly but we gate here anyway.
        if !hasStartedDiscovery {
            await safeCore.startRoomDiscovery()
            hasStartedDiscovery = true
        }
        // Pull kind:10009 from each follow + kind:39001/39002 where any follow
        // is #p-tagged. Powers the "Friends are here" shelf.
        try? await safeCore.startFriendsRoomsDiscovery()
        await ensureCurationSubscription(safeCore: safeCore)

        let curatorPubkey = RoomExplorerConfig.cachedCuratorPubkeyHex

        async let featuredTask: [CommunitySummary] = {
            guard !curatorPubkey.isEmpty else { return [] }
            return (try? await safeCore.getFeaturedRooms(curatorPubkeyHex: curatorPubkey)) ?? []
        }()
        async let newTask: [CommunitySummary] = (try? await safeCore.getNewRooms(limit: 24)) ?? []
        async let friendsTask: [RoomRecommendation] =
            (try? await safeCore.getRoomsWithFriends(limit: 16)) ?? []
        async let authorsTask: [RoomRecommendation] =
            (try? await safeCore.getRoomsFromReadAuthors(limit: 16)) ?? []

        let (fetchedFeatured, fetchedNew, fetchedFriends, fetchedAuthors) =
            await (featuredTask, newTask, friendsTask, authorsTask)

        featured = fetchedFeatured
        newNoteworthy = filter(fetchedNew, excludingJoined: appStore.joinedCommunities)
        friendsShelf = fetchedFriends
        authorsShelf = fetchedAuthors
        isFirstLoad = false
    }

    /// Publish a NIP-29 kind:9021 join-request for the given room and set
    /// the "Join requested" toast. The follow-up "You're in ✓" toast fires
    /// automatically from `HighlighterStore.refreshJoinedCommunities` once
    /// the relay admits the request and the `MembershipChanged` delta
    /// lands. Fire-and-forget — errors are logged, not surfaced.
    func requestJoin(room: CommunitySummary) async {
        guard let appStore else { return }
        appStore.noteJoinRequested(groupId: room.id, roomName: room.name)
        do {
            _ = try await appStore.safeCore.requestJoinRoom(groupId: room.id)
        } catch {
            // The toast already said "Join requested". Rather than contradict
            // it, let the user see nothing change — logging is sufficient for
            // debugging, and the relay-error path is rare.
            print("requestJoinRoom failed for \(room.id): \(error)")
        }
    }

    // MARK: - Private

    private func ensureCurationSubscription(safeCore: SafeHighlighterCore) async {
        if !hasStartedCuration {
            let cached = RoomExplorerConfig.cachedCuratorPubkeyHex
            if cached.isEmpty {
                // No cached pubkey yet — fetch NIP-11 once, cache it, then
                // install the sub. On first app install this means the
                // featured shelf may be empty for the first session; the
                // very next appear picks up the curator and populates it.
                if let fresh = await RoomExplorerConfig.fetchCuratorPubkey(), !fresh.isEmpty {
                    try? await safeCore.startFeaturedRooms(curatorPubkeyHex: fresh)
                    hasStartedCuration = true
                }
            } else {
                try? await safeCore.startFeaturedRooms(curatorPubkeyHex: cached)
                hasStartedCuration = true
            }
        }
    }

    private func filter(
        _ rooms: [CommunitySummary],
        excludingJoined joined: [CommunitySummary]
    ) -> [CommunitySummary] {
        let joinedIds = Set(joined.map(\.id))
        return rooms.filter { !joinedIds.contains($0.id) }
    }
}
