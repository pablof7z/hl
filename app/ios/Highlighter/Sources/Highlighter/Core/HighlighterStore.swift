import Foundation
import Observation

/// App-scoped reactive state. Only holds data that's genuinely global:
/// the current user, the set of joined communities (used by the tab root
/// and by the Capture flow's community picker), and connection health.
///
/// **Per-view data — a room's feed — does NOT live here.** Each view owns
/// a dedicated `@Observable` store (e.g. `RoomStore`) whose lifetime
/// matches the view. That keeps
/// Swift Observation granular and keeps the architectural contract that
/// nostrdb is the only source of truth: any data Swift shows must have
/// been read from (or written to) nostrdb first.
@MainActor
@Observable
final class HighlighterStore {
    // Reactive — drives UI
    var currentUser: CurrentUser?
    var currentUserProfile: ProfileMetadata?
    var joinedCommunities: [CommunitySummary] = [] {
        didSet { mirrorCommunitiesToAppGroup() }
    }
    var connectionState: ConnectionState = .unknown
    var isBootstrapping: Bool = false
    /// Transient toast shown when the Share Extension handoff publishes.
    /// Cleared by the banner after a few seconds.
    var shareToast: String?

    // Internal plumbing
    @ObservationIgnored let core: HighlighterCore
    @ObservationIgnored let safeCore: SafeHighlighterCore
    @ObservationIgnored private(set) var eventBridge: EventBridge?
    @ObservationIgnored private var joinedCommunitiesHandle: UInt64?

    var isLoggedIn: Bool { currentUser != nil }

    enum ConnectionState {
        case unknown, connecting, online, offline
    }

    init() {
        let core = HighlighterCore()
        self.core = core
        self.safeCore = SafeHighlighterCore(core: core)
    }

    func bootstrap() async {
        guard !isBootstrapping else { return }
        isBootstrapping = true
        defer { isBootstrapping = false }

        if let user = await AppSessionStore.shared.restoreSession(into: safeCore) {
            currentUser = user
            registerEventBridge()
            await loadAppScopeData()
        }
    }

    func completeLogin(user: CurrentUser) async {
        currentUser = user
        registerEventBridge()
        await loadAppScopeData()
    }

    func logout() {
        if let handle = joinedCommunitiesHandle {
            core.unsubscribe(handle: handle)
            eventBridge?.unregister(handle: handle)
            joinedCommunitiesHandle = nil
        }
        core.logout()
        AppSessionStore.shared.clear()
        currentUser = nil
        currentUserProfile = nil
        joinedCommunities.removeAll()
        connectionState = .unknown
        SharedCommunitiesCache.clear()
    }

    /// Snapshot `joinedCommunities` into the App Group cache so the Share
    /// Extension can render its community picker without loading the Rust
    /// core. Cheap — a JSON encode + UserDefaults set.
    private func mirrorCommunitiesToAppGroup() {
        let snapshot = joinedCommunities.map {
            SharedCommunitySummary(id: $0.id, name: $0.name, picture: $0.picture)
        }
        SharedCommunitiesCache.save(snapshot)
    }

    // MARK: - Private

    private func registerEventBridge() {
        let bridge = EventBridge(appStore: self)
        core.setEventCallback(callback: bridge)
        eventBridge = bridge
    }

    /// Public so `EventBridge` can re-query on a `MembershipChanged` delta.
    func refreshJoinedCommunities() async {
        if let updated = try? await safeCore.getJoinedCommunities() {
            joinedCommunities = updated
        }
    }

    private func loadAppScopeData() async {
        // Immediate read from nostrdb via the Rust core. Non-blocking on
        // relays — the cache answers first, subscriptions catch up later.
        if let cached = try? await safeCore.getJoinedCommunities() {
            joinedCommunities = cached
        }

        // Fetch the user's own kind:0 so the top-bar avatar shows their real
        // picture. Cheap — single nostrdb read. Lives on the app-scope store
        // because multiple surfaces (toolbar + future editors) need it.
        if let user = currentUser,
           let profile = try? await safeCore.getUserProfile(pubkeyHex: user.pubkey) {
            currentUserProfile = profile
        }

        // Install the joined-communities pump so future 39000/39001/39002
        // events apply to the app-scope store as CommunityUpserted /
        // MembershipChanged deltas (subscription_id == new handle, routed
        // by EventBridge).
        if joinedCommunitiesHandle == nil {
            if let handle = try? await safeCore.subscribeJoinedCommunities() {
                joinedCommunitiesHandle = handle
                // Joined-communities deltas are dispatched via the appStore
                // path in EventBridge (not per-view). No store registration
                // needed; we only hold the handle so logout can unsubscribe.
            }
        }
    }
}
