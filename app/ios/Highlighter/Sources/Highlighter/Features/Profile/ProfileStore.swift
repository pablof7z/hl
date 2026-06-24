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
/// Phase 7 cutover: the bespoke `subscribeUserProfile` live-lane subscription
/// (and its `EventBridge` registration) is removed. Profile metadata stays
/// live via the kernel observer (`ProfileView.onChange(of: profileSnapshots)`);
/// articles/highlights now load once on `start()` via the still-live-lane
/// `getProfilePageSnapshot` (kernel `ProfileSnapshot` carries no
/// articles/highlights yet — Wave 4 wires those into the feed engine).
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
    @ObservationIgnored weak var eventBridge: EventBridge?
    @ObservationIgnored private weak var kernel: HighlighterAppKernel?

    var isOwnProfile: Bool {
        relationshipProjection.isOwnProfile
    }

    var relationshipProjection: ProfileRelationshipProjection {
        let targetPubkey = pubkey.trimmingCharacters(in: .whitespaces)
        let viewer: String? = viewerPubkey.flatMap { v in
            let t = v.trimmingCharacters(in: .whitespaces)
            return t.isEmpty ? nil : t
        }
        let hasTarget = !targetPubkey.isEmpty
        let isOwn = viewer.map { hasTarget && $0.caseInsensitiveCompare(targetPubkey) == .orderedSame } ?? false
        let canShowFollow = viewer != nil && hasTarget && !isOwn
        return ProfileRelationshipProjection(
            targetPubkey: targetPubkey,
            isOwnProfile: isOwn,
            canShowFollowAction: canShowFollow,
            shouldRefreshFollowState: canShowFollow
        )
    }

    init(
        pubkey: String,
        viewerPubkey: String?,
        eventBridge: EventBridge?,
        kernel: HighlighterAppKernel?
    ) {
        self.pubkey = pubkey
        self.viewerPubkey = viewerPubkey
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
    }

    /// Called from `ProfileView.onDisappear`. Sends `ReleaseProfile` to NMP.
    func stop() {
        // Phase 3G: close kernel view — sends Effect::ReleaseProfile to NMP.
        kernel?.closeProfile(pubkey: pubkey)
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
        // Wave 4 stub: the kernel ProfileSnapshot does not yet carry articles or
        // highlights, and the bespoke live-lane getProfilePageSnapshot read path
        // is removed in Phase 7. These tabs stay empty until the feed engine
        // wires per-pubkey article/highlight data (Wave 4).
        articles = []
        highlights = []
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
        // Phase 7 D1 inline of `profile_follow_action_projection`: a follow
        // toggle can only start when not already mutating and the relationship
        // permits a follow action; the requested state is the inverse of the
        // current one (optimistically applied below).
        let relationship = relationshipProjection
        guard !isMutatingFollow, relationship.canShowFollowAction else { return }
        let requestedFollowState = !isFollowing
        isMutatingFollow = true
        followError = nil
        isFollowing = requestedFollowState

        // Phase 3G: dispatch Follow/Unfollow as kernel AppActions
        // (fire-and-forget, D6). The `FollowListUpdated` projection event
        // flows back via the NMP update callback → kernel observer →
        // `ProfileSnapshot.isFollowing` update → next snapshot push.
        //
        // NOTE: the optimistic isFollowing flip above is the only UI signal —
        // do NOT call the live lane's applyProfileFollowMutation here.
        // Coexistence keeps the live lane for READS only; writes must come
        // from exactly one writer (the kernel) to avoid double kind:3 publishes.
        if requestedFollowState {
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
