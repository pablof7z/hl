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
        let relationship = relationshipProjection
        async let profileTask: ProfileMetadata? = {
            let outcome = await safeCore.getUserProfile(pubkeyHex: pubkey)
            return outcome.error.isEmpty ? outcome.value : nil
        }()
        async let articlesTask: [ArticleRecord] = {
            let outcome = await safeCore.getUserArticles(pubkeyHex: pubkey)
            return outcome.error.isEmpty ? outcome.values : []
        }()
        async let highlightsTask: [HighlightRecord] = {
            let outcome = await safeCore.getUserHighlights(pubkeyHex: pubkey)
            return outcome.error.isEmpty ? outcome.values : []
        }()
        async let communitiesTask: [CommunitySummary] = {
            let outcome = await safeCore.getUserCommunities(pubkeyHex: pubkey)
            return outcome.error.isEmpty ? outcome.values : []
        }()
        async let followTask: Bool = {
            guard relationship.shouldRefreshFollowState else {
                return false
            }
            let outcome = await safeCore.isFollowing(targetPubkeyHex: relationship.targetPubkey)
            return outcome.error.isEmpty ? outcome.value : false
        }()

        let (profile, articles, highlights, communities, following) = await (
            profileTask, articlesTask, highlightsTask, communitiesTask, followTask
        )
        self.profile = profile ?? self.profile
        self.articles = articles
        self.highlights = highlights
        self.communities = communities
        self.isFollowing = following
    }

    /// Called by `EventBridge` when a `UserProfileUpdated` delta arrives.
    /// Re-queries only the slice Rust says is affected by the event kind.
    func applyUpdate(kind: UInt32) async {
        switch safeCore.getProfileUpdateAction(kind: kind) {
        case .refreshProfile:
            let outcome = await safeCore.getUserProfile(pubkeyHex: pubkey)
            if outcome.error.isEmpty, let p = outcome.value {
                self.profile = p
            }
        case .refreshFollowState:
            let relationship = relationshipProjection
            if relationship.shouldRefreshFollowState {
                let outcome = await safeCore.isFollowing(targetPubkeyHex: relationship.targetPubkey)
                if outcome.error.isEmpty {
                    self.isFollowing = outcome.value
                }
            }
        case .refreshArticles:
            let outcome = await safeCore.getUserArticles(pubkeyHex: pubkey)
            if outcome.error.isEmpty {
                self.articles = outcome.values
            }
        case .refreshHighlights:
            let outcome = await safeCore.getUserHighlights(pubkeyHex: pubkey)
            if outcome.error.isEmpty {
                self.highlights = outcome.values
            }
        case .refreshCommunities:
            let outcome = await safeCore.getUserCommunities(pubkeyHex: pubkey)
            if outcome.error.isEmpty {
                self.communities = outcome.values
            }
        case .ignore:
            break
        }
    }

    // MARK: - Follow

    func toggleFollow() async {
        let relationship = relationshipProjection
        guard relationship.canShowFollowAction else { return }
        guard !isMutatingFollow else { return }
        isMutatingFollow = true
        followError = nil
        let wasFollowing = isFollowing
        isFollowing = !wasFollowing
        let outcome = await safeCore.setFollow(
            targetPubkeyHex: relationship.targetPubkey,
            follow: !wasFollowing
        )
        if !outcome.error.isEmpty {
            isFollowing = wasFollowing
            followError = outcome.error
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
