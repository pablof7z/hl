import Foundation
import Observation

/// View-scoped store for a single profile page.
///
/// Phase 3G cutover: state now derives from the `HighlighterAppKernel`
/// typed `ProfileSnapshot` rather than the live lane's
/// `SafeHighlighterCore.getProfilePageSnapshot`. The kernel view is opened
/// via `kernel.openProfile(pubkey:)` on `start()` and closed via
/// `kernel.closeProfile(pubkey:)` on `stop()`. Follow/Unfollow are
/// dispatched as `AppAction.follow`/`.unfollow` kernel actions.
///
/// Articles and highlights are deferred to Phase 4 (spec §2.2 / Phase 3D
/// scope); the tabbed profile page still shows their tabs but they arrive
/// from the live lane unchanged until Phase 4 wires feed data.
///
/// The live lane subscription (`subscribeUserProfile`) is intentionally
/// kept for the articles/highlights tab content — it is removed in Phase 7.
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
    @ObservationIgnored private weak var kernel: HighlighterAppKernel?
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
        eventBridge: EventBridge?,
        kernel: HighlighterAppKernel?
    ) {
        self.pubkey = pubkey
        self.viewerPubkey = viewerPubkey
        self.safeCore = safeCore
        self.eventBridge = eventBridge
        self.kernel = kernel
    }

    /// One-shot setup called from `ProfileView.task`. Opens the kernel profile
    /// view (which sends `ClaimProfile` to NMP), kicks off the initial live-lane
    /// article/highlight loads, and installs the subscription for live deltas.
    func start() async {
        // Phase 3G: open kernel view — sends Effect::ClaimProfile to NMP.
        kernel?.openProfile(pubkey: pubkey)

        // Apply any snapshot already buffered by the kernel (fast path if the
        // kernel received the profile card before the view appeared).
        applyKernelSnapshot()

        // Phase 3D deferred: articles and highlights still come from the live lane.
        await loadArticlesAndHighlights()
        isLoadingInitial = false
        await installSubscription()
    }

    /// Called from `ProfileView.onDisappear`. Sends `ReleaseProfile` to NMP.
    func stop() {
        // Phase 3G: close kernel view — sends Effect::ReleaseProfile to NMP.
        kernel?.closeProfile(pubkey: pubkey)
        if let handle = subscriptionHandle {
            Task { [safeCore] in await safeCore.unsubscribe(handle) }
            eventBridge?.unregister(handle: handle)
            subscriptionHandle = nil
        }
    }

    // MARK: - Kernel snapshot ingestion

    /// Called whenever the kernel's `profileSnapshots[pubkey]` changes.
    /// Converts the raw `ProfileSnapshot` to the presentation types the
    /// view expects (D3 — Swift owns all formatting).
    func applyKernelSnapshot() {
        guard let snap = kernel?.profileSnapshots[pubkey] else { return }
        profile = snap.asProfileMetadata()
        isFollowing = snap.isFollowing
        // Communities: kernel gives the active account's joined-groups list as
        // context (Phase 3D scope). Phase 4 adds per-pubkey group membership.
        communities = snap.communities.map { $0.asCommunitySummary() }
    }

    // MARK: - Live-lane loads (articles + highlights, Phase 4 deferred)

    func loadArticlesAndHighlights() async {
        // Articles and highlights still come from the live lane until
        // Phase 4 wires feed interests. Profile metadata is now kernel-owned.
        let snapshot = await safeCore.getProfilePageSnapshot(pubkeyHex: pubkey)
        articles = snapshot.articles
        highlights = snapshot.highlights
        // Fallback: if kernel hasn't delivered a profile card yet, use the
        // live lane's profile metadata so the view renders immediately.
        if profile == nil {
            profile = snapshot.profile ?? profile
            isFollowing = snapshot.isFollowing
            communities = snapshot.communities
        }
    }

    /// Called by `EventBridge` when a `UserProfileUpdated` delta arrives.
    /// Phase 3G: kernel snapshot is the authoritative source; this path is
    /// kept for article/highlight deltas that still flow through the live lane.
    func applyUpdate() async {
        // Kernel snapshot is updated automatically via the observer; apply it
        // here in case the EventBridge fires before the Task hop completes.
        applyKernelSnapshot()
        await loadArticlesAndHighlights()
    }

    // MARK: - Follow / Unfollow

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
            return
        }
        isMutatingFollow = true
        followError = nil
        isFollowing = action.optimisticIsFollowing

        // Phase 3G: dispatch Follow/Unfollow as kernel AppActions
        // (fire-and-forget, D6). The `FollowListUpdated` projection event
        // flows back via the NMP update callback → kernel observer →
        // `ProfileSnapshot.isFollowing` update → next snapshot push.
        //
        // NOTE: the optimistic isFollowing flip above is the only UI signal —
        // do NOT call the live lane's applyProfileFollowMutation here.
        // Coexistence keeps the live lane for READS only; writes must come
        // from exactly one writer (the kernel) to avoid double kind:3 publishes.
        if mutation.requestedFollowState {
            kernel?.app.dispatch(action: .follow(pubkey: pubkey))
        } else {
            kernel?.app.dispatch(action: .unfollow(pubkey: pubkey))
        }

        // isMutatingFollow is cleared when the kernel's FollowListUpdated event
        // arrives and applyKernelSnapshot() flips isFollowing.  As a safety net,
        // also clear it here so the button never gets permanently stuck if the
        // NMP round-trip is delayed.
        isMutatingFollow = false
    }

    // MARK: - Private

    private func installSubscription() async {
        guard subscriptionHandle == nil, let bridge = eventBridge else { return }
        let outcome = await safeCore.subscribeUserProfile(pubkeyHex: pubkey)
        let projection = safeCore.projectViewSubscriptionStart(
            input: ViewSubscriptionStartProjectionInput(start: outcome)
        )
        guard projection.shouldRegister else {
            // Non-fatal: the profile view still has its initial load. Live
            // updates will simply not stream in until the next visit.
            return
        }
        subscriptionHandle = projection.handle
        bridge.registerProfile(self, handle: projection.handle)
    }
}
