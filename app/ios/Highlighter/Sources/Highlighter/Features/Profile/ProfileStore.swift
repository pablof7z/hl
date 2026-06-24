import Foundation
import Observation

/// View-scoped store for a single profile page.
///
/// Phase 3G cutover: state derives from the `HighlighterAppKernel`
/// typed `ProfileSnapshot`. The kernel view is opened via
/// `kernel.openProfile(pubkey:)` on `start()` and closed via
/// `kernel.closeProfile(pubkey:)` on `stop()`. Follow/Unfollow are dispatched
/// as `AppAction.follow`/`.unfollow` kernel actions.
///
/// Profile metadata stays live via the kernel observer
/// (`ProfileView.onChange(of: profileSnapshots)`). Articles/highlights remain
/// empty until the feed engine carries per-pubkey data.
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
        kernel: HighlighterAppKernel?
    ) {
        self.pubkey = pubkey
        self.viewerPubkey = viewerPubkey
        self.kernel = kernel
    }

    /// One-shot setup called from `ProfileView.task`. Opens the kernel profile
    /// view (which sends `ClaimProfile` to NMP) and applies any buffered
    /// profile snapshot.
    func start() async {
        // Phase 3G: open kernel view — sends Effect::ClaimProfile to NMP.
        kernel?.openProfile(pubkey: pubkey)

        // Apply any snapshot already buffered by the kernel (fast path if the
        // kernel received the profile card before the view appeared).
        applyKernelSnapshot()

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

    // MARK: - Deferred feed data

    func loadArticlesAndHighlights() async {
        // Wave 4 stub: the kernel ProfileSnapshot does not yet carry articles or
        // highlights. These tabs stay empty until the feed engine wires
        // per-pubkey article/highlight data.
        articles = []
        highlights = []
    }

    /// Kernel snapshot is the authoritative source; this remains as a local
    /// refresh hook for views that already own the store.
    func applyUpdate() async {
        // Kernel snapshot is updated automatically via the observer; apply it
        // here for callers that need an immediate local refresh.
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
        // NOTE: the optimistic isFollowing flip above is the only UI signal.
        // Writes must come from exactly one writer (the kernel) to avoid double
        // kind:3 publishes.
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
