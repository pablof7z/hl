import Foundation
import Observation

/// View-scoped store for a single profile page. Lifetime matches the
/// `ProfileView` that owns it — created in `onAppear`, torn down in
/// `onDisappear`. Subscribes via `subscribe_user_profile` so live profile
/// deltas trigger Rust-classified re-queries.
@MainActor
@Observable
final class ProfileStore {
    enum Tab: Hashable {
        case articles, highlights, communities
    }

    // Reactive state
    var profile: ProfileMetadata?
    var articles: [ArticleRecord] = []
    var highlights: [HighlightRecord] = []
    var communities: [CommunitySummary] = []
    var isFollowing: Bool = false
    var isMutatingFollow: Bool = false
    var followError: String?
    var isLoadingInitial: Bool = true
    var activeTab: Tab = .articles

    // Plumbing
    @ObservationIgnored let pubkey: String
    @ObservationIgnored let viewerPubkey: String?
    @ObservationIgnored let safeCore: SafeHighlighterCore
    @ObservationIgnored weak var eventBridge: EventBridge?
    @ObservationIgnored private var subscriptionHandle: UInt64?

    var isOwnProfile: Bool {
        relationshipProjection.isOwnProfile
    }

    var relationshipProjection: ProfileRelationshipProjection {
        safeCore.projectProfileRelationship(
            input: ProfileRelationshipProjectionInput(
                profilePubkey: pubkey,
                viewerPubkey: viewerPubkey
            )
        )
    }

    init(
        pubkey: String,
        viewerPubkey: String?,
        safeCore: SafeHighlighterCore,
        eventBridge: EventBridge?
    ) {
        self.pubkey = pubkey
        self.viewerPubkey = viewerPubkey
        self.safeCore = safeCore
        self.eventBridge = eventBridge
    }

    /// One-shot setup called from `ProfileView.task`. Kicks off the initial
    /// parallel loads, installs the subscription, and routes live deltas.
    func start() async {
        await loadAll()
        isLoadingInitial = false
        await installSubscription()
    }

    /// Called from `ProfileView.onDisappear`.
    func stop() {
        if let handle = subscriptionHandle {
            Task { [safeCore] in await safeCore.unsubscribe(handle) }
            eventBridge?.unregister(handle: handle)
            subscriptionHandle = nil
        }
    }

    // MARK: - Loads

    func loadAll() async {
        let snapshot = await safeCore.getProfilePageSnapshot(pubkeyHex: pubkey)
        profile = snapshot.profile ?? profile
        articles = snapshot.articles
        highlights = snapshot.highlights
        communities = snapshot.communities
        isFollowing = snapshot.isFollowing
    }

    /// Called by `EventBridge` when a `UserProfileUpdated` delta arrives.
    /// Re-queries Rust's full profile page snapshot so native code does not
    /// branch on protocol event kinds for this page.
    func applyUpdate() async {
        await loadAll()
    }

    // MARK: - Follow

    func toggleFollow() async {
        let relationship = relationshipProjection
        let action = safeCore.projectProfileFollowAction(
            relationship: relationship,
            input: ProfileFollowActionInput(
                isFollowing: isFollowing,
                isMutating: isMutatingFollow
            )
        )
        guard action.canStart, let mutation = action.mutation else {
            if !action.errorMessage.isEmpty {
                followError = action.errorMessage
            }
            return
        }
        isMutatingFollow = true
        followError = nil
        isFollowing = action.optimisticIsFollowing
        let snapshot = await safeCore.applyProfileFollowMutation(input: mutation)
        isFollowing = snapshot.isFollowing
        if !snapshot.error.isEmpty {
            followError = snapshot.error
        }
        isMutatingFollow = false
    }

    // MARK: - Private

    private func installSubscription() async {
        guard subscriptionHandle == nil, let bridge = eventBridge else { return }
        let outcome = await safeCore.subscribeUserProfile(pubkeyHex: pubkey)
        guard outcome.error.isEmpty else {
            // Non-fatal: the profile view still has its initial load. Live
            // updates will simply not stream in until the next visit.
            return
        }
        subscriptionHandle = outcome.handle
        bridge.registerProfile(self, handle: outcome.handle)
    }
}
