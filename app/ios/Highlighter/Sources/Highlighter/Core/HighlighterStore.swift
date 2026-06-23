import Foundation
import Network
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
    var podcastPlayer: PodcastPlayerStore
    var currentUser: CurrentUser?
    var currentUserProfile: ProfileMetadata?
    var joinedCommunities: [CommunitySummary] = [] {
        didSet { mirrorCommunitiesToAppGroup() }
    }
    var connectionState: ConnectionState = .unknown
    var isBootstrapping: Bool = false
    var isOnboardingComplete: Bool = false
    /// Transient toast shown when Rust or a platform handoff requests an
    /// app-scope banner. Cleared by the banner after a few seconds.
    var shareToast: String?
    /// Render projection for requested profile pubkeys. Rust/nostrdb remain
    /// the source of truth; this app-scope snapshot only lets existing
    /// SwiftUI surfaces render names and avatars synchronously, then refresh
    /// when a fresh kind:0 arrives from a relay.
    private(set) var profileSnapshots: [String: ProfileMetadata] = [:]
    /// OpenGraph + favicon cache for web URL highlights, keyed by the
    /// canonical URL the metadata was fetched for. Mirrors `profileSnapshots`'s
    /// shape so card views can look up enrichment synchronously and
    /// re-render when a fetch lands. The Rust core owns the on-disk cache;
    /// this dictionary is the in-memory mirror SwiftUI observes.
    var webMetadataCache: [String: WebMetadata] = [:]
    /// ArtifactPreview cache for ISBN lookups, keyed by bare ISBN-13 (e.g. "9780593716717").
    var isbnPreviewCache: [String: ArtifactPreview] = [:]
    /// NIP-51 kind:10003 article bookmark addresses. Reactive so every row
    /// showing a bookmark affordance updates when the user toggles one
    /// anywhere. Rust owns canonicalization, dedupe, and optimistic membership
    /// projection; Swift keeps the ordered list it receives from the core.
    var bookmarkedArticleAddresses: [String] = []

    // Internal plumbing
    @ObservationIgnored let core: HighlighterCore
    @ObservationIgnored let safeCore: SafeHighlighterCore
    @ObservationIgnored private(set) var eventBridge: EventBridge?
    @ObservationIgnored private var profileSnapshotHandles: [String: UInt64] = [:]
    @ObservationIgnored private var networkPathMonitor: NWPathMonitor?
    /// Weak reference to the kernel, set by `AppEntry` after both objects are
    /// initialised. Used to route `requestProfile` through the kernel lane
    /// (Phase 7) instead of the legacy `safeCore` subscription path.
    /// Weak to avoid a retain cycle: `AppEntry` (@State) is the sole strong owner.
    @ObservationIgnored weak var kernel: HighlighterAppKernel?
    /// Pubkeys whose profile views were opened via the kernel so `logout()`
    /// can close them and release NMP resources.
    @ObservationIgnored private var kernelManagedPubkeys: Set<String> = []
    /// In-flight `requestWebMetadata` calls coalesce here so multiple rows
    /// referencing the same URL share a single Task. Cleared once the
    /// fetch completes (success or failure).
    @ObservationIgnored private var webMetadataInflight: [String: Task<Void, Never>] = [:]
    @ObservationIgnored private var isbnInflight: [String: Task<Void, Never>] = [:]

    var isLoggedIn: Bool { currentUser != nil }

    enum ConnectionState {
        case unknown, connecting, online, offline
    }

    init() {
        let core = HighlighterCore()
        let safeCore = SafeHighlighterCore(core: core)
        self.core = core
        self.safeCore = safeCore
        let podcastPlayer = PodcastPlayerStore(core: safeCore)
        self.podcastPlayer = podcastPlayer
        self.isOnboardingComplete = core.isOnboardingComplete()
        // Surface the MiniPlayer (paused) with whatever episode the user was
        // last listening to, if any. Tapping play wires AVPlayer through the
        // normal `load(artifact:)` path which seeks to the saved position.
        Task { @MainActor in
            await podcastPlayer.rehydrateFromSavedRecord()
        }
    }

    func bootstrap() async {
        guard !isBootstrapping else { return }
        isBootstrapping = true
        defer { isBootstrapping = false }

        // Register the EventBridge unconditionally, before any login attempt.
        // The NIP-46 nostrconnect:// flow fires `SignerConnected` from a
        // background tokio task; if no callback is wired by then, the delta
        // is dropped silently and the UI never transitions to logged-in.
        registerEventBridge()

        if let user = await AppSessionStore.shared.restoreSession(into: safeCore) {
            currentUser = user
            await loadAppScopeData()
        }
    }

    func completeLogin(user: CurrentUser) async {
        currentUser = user
        if eventBridge == nil {
            registerEventBridge()
        }
        await loadAppScopeData()
    }

    func logout() {
        kernel?.closeCommunities()
        kernel?.closeBookmarks()
        for (_, handle) in profileSnapshotHandles {
            core.unsubscribe(handle: handle)
            eventBridge?.unregister(handle: handle)
        }
        profileSnapshotHandles.removeAll()
        // Close any profile views that were opened via the kernel lane so
        // NMP resources (relay subscription slots, in-memory projections)
        // are released on sign-out.
        for pubkey in kernelManagedPubkeys {
            kernel?.closeProfile(pubkey: pubkey)
        }
        kernelManagedPubkeys.removeAll()
        profileSnapshots.removeAll()
        for (_, task) in webMetadataInflight { task.cancel() }
        webMetadataInflight.removeAll()
        webMetadataCache.removeAll()
        bookmarkedArticleAddresses.removeAll()
        applyNetworkPathMonitorEnabled(false)
        core.logout()
        eventBridge = nil
        AppSessionStore.shared.clear()
        _ = core.setOnboardingComplete(complete: false)
        isOnboardingComplete = false
        currentUser = nil
        currentUserProfile = nil
        joinedCommunities.removeAll()
        connectionState = .unknown
        SharedCommunitiesSnapshot.clear()
    }

    func markOnboardingComplete() -> MutationSnapshot {
        let outcome = core.setOnboardingComplete(complete: true)
        if outcome.applied {
            isOnboardingComplete = true
        }
        return outcome
    }

    func completeOnboardingInterests(selectedIds: [String]) async -> MutationSnapshot {
        let outcome = await safeCore.completeOnboardingInterests(selectedIds: selectedIds)
        if outcome.applied {
            isOnboardingComplete = true
        }
        return outcome
    }

    // MARK: - Bookmarks

    /// Optimistic toggle: flip local state immediately for snappy UI, then
    /// dispatch to the kernel. The kernel will push a new BookmarksSnapshot
    /// when the event publishes, which calls back via applyKernelBookmarks.
    func toggleBookmark(articleAddress: String) async {
        guard let kernel else { return }
        let isCurrentlyBookmarked = bookmarkedArticleAddresses.contains(articleAddress)
        // Optimistic update
        if isCurrentlyBookmarked {
            bookmarkedArticleAddresses.removeAll { $0 == articleAddress }
            kernel.app.dispatch(.removeBookmark(item: .address(coordinate: articleAddress, relay: nil)))
        } else {
            bookmarkedArticleAddresses.append(articleAddress)
            kernel.app.dispatch(.addBookmark(item: .address(coordinate: articleAddress, relay: nil)))
        }
        // No refresh needed — kernel will push a new BookmarksSnapshot when the event publishes
    }

    func refreshBookmarks() async {
        guard let kernel else { return }
        // kernel.bookmarks is populated when openBookmarks() is called (in loadAppScopeData)
        applyBookmarkRows(kernel.bookmarks?.rows ?? [])
    }

    func isBookmarked(articleAddress: String) -> Bool {
        bookmarkedArticleAddresses.contains(articleAddress)
    }

    /// Called by HighlighterAppKernel when a new BookmarksSnapshot arrives
    /// from the kernel observer. Updates bookmarkedArticleAddresses reactively.
    func applyKernelBookmarks(_ rows: [BookmarkRow]) {
        applyBookmarkRows(rows)
    }

    private func applyBookmarkRows(_ rows: [BookmarkRow]) {
        bookmarkedArticleAddresses = rows.compactMap {
            guard case .address(let coord, _) = $0, coord.hasPrefix("30023:") else { return nil }
            return coord
        }
    }

    /// Opens a profile subscription and seeds the local snapshot via the kernel.
    ///
    /// Calls `kernel.openProfile(pubkey:)` which dispatches `ClaimProfile` to NMP
    /// and opens a `ViewId.profile` projection. The `KernelObserver` pushes fresh
    /// `ProfileSnapshot` deltas back into `kernel.profileSnapshots`; the bridge in
    /// `HighlighterAppKernel.receive` then mirrors them here via `applyProfileSnapshot(_:)`.
    ///
    /// Safe to call from multiple views for the same pubkey — idempotent.
    func requestProfile(pubkeyHex: String) async {
        // Kernel lane: open the profile view (idempotent if already open)
        // and seed the local snapshot from whatever the kernel already has.
        kernel?.openProfile(pubkey: pubkeyHex)
        kernelManagedPubkeys.insert(pubkeyHex)
        if let snapshot = kernel?.profileSnapshots[pubkeyHex] {
            applyProfileSnapshot(snapshot.asProfileMetadata())
        }
    }

    /// Called by the kernel bridge when `KernelObserver` pushes a `ProfileSnapshot`
    /// update (via `HighlighterAppKernel.receive`). Reads the latest snapshot the
    /// kernel already holds and converts it to `ProfileMetadata` for the UI.
    func applyProfileSnapshotUpdate(pubkeyHex: String) async {
        if let snapshot = kernel?.profileSnapshots[pubkeyHex] {
            applyProfileSnapshot(snapshot.asProfileMetadata())
        }
    }

    func applyProfileSnapshot(_ profile: ProfileMetadata) {
        profileSnapshots[profile.pubkey] = profile
    }

    /// Fetch OpenGraph + favicon metadata for a web URL via the Rust core
    /// (which owns the disk cache + in-flight coalescing). Safe to call from
    /// multiple views for the same URL — the in-memory `webMetadataInflight`
    /// map deduplicates Swift-side, the Rust store deduplicates HTTP-side.
    /// No-op when the URL is already cached in `webMetadataCache`.
    func requestWebMetadata(url: String) async {
        let projection = safeCore.projectWebMetadataRequest(input: WebMetadataRequestProjectionInput(url: url))
        guard projection.canRequest else { return }
        let canonicalUrl = projection.canonicalUrl
        let cacheKeys = projection.cacheKeys
        if let metadata = cachedWebMetadata(for: cacheKeys) {
            applyWebMetadata(metadata, cacheKeys: cacheKeys)
            return
        }
        if let existing = webMetadataInflight[canonicalUrl] {
            await existing.value
            if let metadata = cachedWebMetadata(for: cacheKeys) {
                applyWebMetadata(metadata, cacheKeys: cacheKeys)
            }
            return
        }
        let task = Task { [weak self, canonicalUrl, cacheKeys] in
            guard let self else { return }
            let metadata = await self.safeCore.getWebMetadata(url: canonicalUrl)
            await MainActor.run {
                if let metadata {
                    self.applyWebMetadata(metadata, cacheKeys: cacheKeys)
                }
                self.webMetadataInflight.removeValue(forKey: canonicalUrl)
            }
        }
        webMetadataInflight[canonicalUrl] = task
        await task.value
    }

    private func cachedWebMetadata(for cacheKeys: [String]) -> WebMetadata? {
        for key in cacheKeys {
            if let metadata = webMetadataCache[key] {
                return metadata
            }
        }
        return nil
    }

    private func applyWebMetadata(_ metadata: WebMetadata, cacheKeys: [String]) {
        for key in cacheKeys {
            webMetadataCache[key] = metadata
        }
        if !metadata.url.isEmpty {
            webMetadataCache[metadata.url] = metadata
        }
    }

    /// Fetch + cache an ISBN preview. Concurrent callers for the same ISBN
    /// coalesce onto one in-flight Task. No-op when already cached.
    /// Rust canonicalizes the input to ISBN-13 before lookup.
    func requestIsbnPreview(isbn: String) async {
        let projection = safeCore.projectIsbnPreviewRequest(input: IsbnPreviewRequestProjectionInput(isbn: isbn))
        guard projection.canRequest else { return }
        let key = projection.normalizedIsbn
        if isbnPreviewCache[key] != nil { return }
        if let existing = isbnInflight[key] {
            await existing.value
            return
        }
        let task = Task { [weak self] in
            guard let self else { return }
            let outcome = await self.safeCore.lookupIsbn(key)
            await MainActor.run {
                let projection = self.safeCore.projectIsbnPreviewLookupApply(
                    input: IsbnPreviewLookupApplyInput(
                        preview: outcome.preview,
                        error: outcome.error
                    )
                )
                if let preview = projection.preview {
                    self.isbnPreviewCache[key] = preview
                }
                self.isbnInflight.removeValue(forKey: key)
            }
        }
        isbnInflight[key] = task
        await task.value
    }

    /// Snapshot `joinedCommunities` into the App Group handoff store so the
    /// Share Extension can render its community picker without loading the
    /// Rust core. Rust owns the projection bytes; Swift only writes them to
    /// the platform container.
    private func mirrorCommunitiesToAppGroup() {
        let snapshot = core.shareExtensionCommunitiesSnapshot(communities: joinedCommunities)
        SharedCommunitiesSnapshot.save(snapshot)
    }

    func refreshNetworkPathCapabilityPreference() {
        let snapshot = safeCore.getNetworkWifiOnlyPreferenceSnapshot()
        applyNetworkPathMonitorEnabled(snapshot.pathMonitorEnabled)
    }

    func applyNetworkPathMonitorEnabled(_ enabled: Bool) {
        if enabled {
            startNetworkPathMonitor()
        } else {
            networkPathMonitor?.cancel()
            networkPathMonitor = nil
        }
    }

    // MARK: - Private

    private func registerEventBridge() {
        let bridge = EventBridge(appStore: self)
        core.setEventCallback(callback: bridge)
        eventBridge = bridge
    }

    /// Public so `EventBridge` can re-query on a `MembershipChanged` delta.
    /// Reads the latest snapshot from the kernel (always resident since kernel
    /// opens the communities view on login).
    func refreshJoinedCommunities() async {
        joinedCommunities = kernel?.communities?.groups.map { $0.asCommunitySummary() } ?? []
    }

    private func loadAppScopeData() async {
        refreshNetworkPathCapabilityPreference()

        // Open the communities view via the kernel (idempotent) and seed the
        // joined list from whatever the kernel already holds. applyKernelCommunities
        // refreshes joinedCommunities reactively as CommunitiesSnapshot deltas arrive.
        kernel?.openCommunities()
        joinedCommunities = kernel?.communities?.groups.map { $0.asCommunitySummary() } ?? []

        // Seed the user's own profile from the kernel's projection if available.
        // The kernel delivers a fresh kind:0 asynchronously; the observer updates
        // currentUserProfile again via the profile snapshot bridge.
        if let user = currentUser {
            if let profile = kernel?.profileSnapshots[user.pubkey]?.asProfileMetadata() {
                currentUserProfile = profile
            }
        }

        // Publish the default Blossom server list if the user has never set one.
        // No-op when a kind:10063 is already cached. Fire-and-forget.
        _ = await safeCore.initDefaultBlossomServers()

        // Open the bookmarks view via the kernel so kind:10003 snapshots
        // stream into kernel.bookmarks and back into bookmarkedArticleAddresses
        // via applyKernelBookmarks. Initial hydration fires immediately as the
        // kernel delivers its first BookmarksSnapshot.
        kernel?.openBookmarks()
        await refreshBookmarks()
    }

    private func startNetworkPathMonitor() {
        guard networkPathMonitor == nil else { return }
        let monitor = NWPathMonitor()
        monitor.pathUpdateHandler = { [weak self] path in
            let isWifi = path.status == .satisfied && path.usesInterfaceType(.wifi)
            Task { @MainActor [weak self] in
                await self?.applyNetworkPathStatus(isWifi: isWifi)
            }
        }
        monitor.start(queue: .global(qos: .utility))
        networkPathMonitor = monitor
    }

    private func applyNetworkPathStatus(isWifi: Bool) async {
        let snapshot = await safeCore.applyNetworkPathStatus(isWifi: isWifi)
        applyNetworkPathMonitorEnabled(snapshot.pathMonitorEnabled)
    }

    /// Called by HighlighterAppKernel when a new CommunitiesSnapshot arrives
    /// from the kernel observer. Updates joinedCommunities reactively.
    func applyKernelCommunities(_ communities: [CommunitySummary]) {
        joinedCommunities = communities
    }
}
