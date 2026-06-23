import Foundation
import Observation

/// View-scoped store for a single profile page.
///
/// Phase 7 cutover: bespoke `subscribeUserProfile` subscription removed;
/// `safeCore.projectProfileRelationship` and `safeCore.projectProfileFollowAction`
/// replaced with inline Swift. Kernel owns profile subscription via
/// `kernel.openProfile(pubkey:)` / `kernel.closeProfile(pubkey:)`.
/// Follow/Unfollow are dispatched as `AppAction.follow`/`.unfollow` kernel actions.
///
/// Articles and highlights (Phase 4 deferred) still load from the live lane via
/// `safeCore.getProfilePageSnapshot`; that call is intentionally kept.
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

    var isOwnProfile: Bool {
        pubkey == viewerPubkey
    }

    /// Computed inline in Phase 7 (safeCore.projectProfileRelationship removed).
    /// ProfileView.ActionRow still reads `canShowFollowAction` from this property.
    var relationshipProjection: ProfileRelationshipProjection {
        let isOwn = pubkey == viewerPubkey
        return ProfileRelationshipProjection(
            targetPubkey: pubkey,
            isOwnProfile: isOwn,
            canShowFollowAction: !isOwn && viewerPubkey != nil,
            shouldRefreshFollowState: false
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
        // Phase 7: bespoke subscription removed; kernel.openProfile owns the subscription.
    }

    /// Called from `ProfileView.onDisappear`. Sends `ReleaseProfile` to NMP.
    func stop() {
        // Phase 3G: close kernel view — sends Effect::ReleaseProfile to NMP.
        kernel?.closeProfile(pubkey: pubkey)
        // Phase 7: subscriptionHandle removed; bespoke subscription cleanup no longer needed.
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
        // Articles and highlights still come from the live lane (those profile
        // tabs stay live per the Phase 7 cutover plan). Profile metadata,
        // isFollowing, and communities are kernel-owned: the live-lane FALLBACK
        // was removed in Phase 7 so the kernel ProfileSnapshot is the SOLE
        // metadata source. The view renders metadata from applyKernelSnapshot();
        // first paint waits for the kernel card rather than the live snapshot.
        let snapshot = await safeCore.getProfilePageSnapshot(pubkeyHex: pubkey)
        articles = snapshot.articles
        highlights = snapshot.highlights
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
        // Phase 7: guard inlined from safeCore.projectProfileFollowAction.
        // canStart = !isMutating && canShowFollowAction = !isMutating && !isOwn && viewer != nil.
        guard !isMutatingFollow,
              viewerPubkey != nil,
              pubkey != viewerPubkey else { return }

        let wantFollow = !isFollowing
        isMutatingFollow = true
        followError = nil
        isFollowing = wantFollow  // optimistic flip

        // Phase 3G: dispatch Follow/Unfollow as kernel AppActions
        // (fire-and-forget, D6). The `FollowListUpdated` projection event
        // flows back via the NMP update callback → kernel observer →
        // `ProfileSnapshot.isFollowing` update → next snapshot push.
        //
        // NOTE: the optimistic isFollowing flip above is the only UI signal —
        // do NOT call the live lane's applyProfileFollowMutation here.
        // Coexistence keeps the live lane for READS only; writes must come
        // from exactly one writer (the kernel) to avoid double kind:3 publishes.
        if wantFollow {
            kernel?.app.dispatch(.follow(pubkey: pubkey))
        } else {
            kernel?.app.dispatch(.unfollow(pubkey: pubkey))
        }

        // isMutatingFollow is cleared when the kernel's FollowListUpdated event
        // arrives and applyKernelSnapshot() flips isFollowing.  As a safety net,
        // also clear it here so the button never gets permanently stuck if the
        // NMP round-trip is delayed.
        isMutatingFollow = false
    }

}
