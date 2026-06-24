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
    /// Kernel handle. Communities, bookmarks, and profiles are owned by the
    /// kernel's typed snapshots; App.swift pushes them into this store via
    /// `onChange` bridges, and writes (bookmark toggles) dispatch through here.
    @ObservationIgnored weak var kernel: HighlighterAppKernel?
    @ObservationIgnored private var networkPathMonitor: NWPathMonitor?
    /// In-flight `requestWebMetadata` calls coalesce here so multiple rows
    /// referencing the same URL share a single Task. Cleared once the
    /// fetch completes (success or failure).
    @ObservationIgnored private var webMetadataInflight: [String: Task<Void, Never>] = [:]

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
        kernel?.app.dispatch(.completeOnboarding)
        isOnboardingComplete = true
        return MutationSnapshot(applied: true, error: "")
    }

    // MARK: - Bookmarks

    /// Optimistic toggle: flip local state immediately for snappy UI, then
    /// dispatch the write to the kernel (sole writer of kind:10003). The
    /// authoritative `BookmarksSnapshot` pushed back via App.swift's `onChange`
    /// reconciles `bookmarkedArticleAddresses`.
    func toggleBookmark(articleAddress: String) async {
        guard currentUser != nil, let kernel else { return }
        if bookmarkedArticleAddresses.contains(articleAddress) {
            bookmarkedArticleAddresses.removeAll { $0 == articleAddress }
            kernel.app.dispatch(.removeBookmark(item: .address(coordinate: articleAddress, relay: nil)))
        } else {
            bookmarkedArticleAddresses.append(articleAddress)
            kernel.app.dispatch(.addBookmark(item: .address(coordinate: articleAddress, relay: nil)))
        }
    }

    func refreshBookmarks() async {
        guard let bookmarks = kernel?.bookmarks else { return }
        applyBookmarksSnapshot(bookmarks)
    }

    func isBookmarked(articleAddress: String) -> Bool {
        bookmarkedArticleAddresses.contains(articleAddress)
    }

    /// Triggers the kernel's profile subscription for `pubkeyHex`. Profile data
    /// arrives asynchronously via App.swift's `onChange(of: kernel.profileSnapshots)`
    /// bridge, which calls `applyKernelProfiles`. Safe to call from multiple
    /// views for the same pubkey (the kernel's claim is idempotent).
    func requestProfile(pubkeyHex: String) async {
        kernel?.openProfile(pubkey: pubkeyHex)
    }

    /// Called by `EventBridge` when a subscribed profile's kind:0 arrives.
    /// The kernel now owns profile projection; re-claim so it re-projects.
    func applyProfileSnapshotUpdate(pubkeyHex: String) async {
        kernel?.openProfile(pubkey: pubkeyHex)
    }

    func applyProfileSnapshot(_ profile: ProfileMetadata) {
        profileSnapshots[profile.pubkey] = profile
    }

    /// Maps the kernel's always-open `CommunitiesSnapshot` into the bespoke
    /// `joinedCommunities` list via the `CommunityRow` bridge.
    func applyCommunitiesSnapshot(_ snap: CommunitiesSnapshot) {
        joinedCommunities = snap.groups.map { $0.asCommunitySummary() }
    }

    /// Derives the NIP-51 kind:10003 article-bookmark addresses from the
    /// kernel's `BookmarksSnapshot` (kind:30023 address rows only).
    func applyBookmarksSnapshot(_ snap: BookmarksSnapshot) {
        bookmarkedArticleAddresses = snap.rows.compactMap { row -> String? in
            if case .address(let coordinate, _) = row,
               coordinate.hasPrefix("30023:") { return coordinate }
            return nil
        }
    }

    /// Maps the kernel's profile snapshot dict into the bespoke
    /// `profileSnapshots` projection, refreshing `currentUserProfile` when the
    /// active user's profile is present.
    func applyKernelProfiles(_ profiles: [String: ProfileSnapshot]) {
        for (pubkey, snap) in profiles {
            profileSnapshots[pubkey] = snap.asProfileMetadata()
        }
        if let user = currentUser, let snap = profiles[user.pubkey] {
            currentUserProfile = snap.asProfileMetadata()
        }
    }

    /// Fetch OpenGraph + favicon metadata for a web URL via the Rust core
    /// (which owns the disk cache + in-flight coalescing). Safe to call from
    /// multiple views for the same URL — the in-memory `webMetadataInflight`
    /// map deduplicates Swift-side, the Rust store deduplicates HTTP-side.
    /// No-op when the URL is already cached in `webMetadataCache`.
    func requestWebMetadata(url: String) async {
        // D1: the request gate is a pure projection. The Rust core re-canonicalizes
        // the URL inside `getWebMetadata` (and keys its on-disk cache by the
        // canonical form), so Swift only needs a lightweight validity gate here.
        let trimmed = url.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty,
              let parsed = URL(string: trimmed),
              let scheme = parsed.scheme?.lowercased(),
              scheme == "http" || scheme == "https" else { return }
        let canonicalUrl = trimmed
        let cacheKeys = [trimmed, url].filter { !$0.isEmpty }
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

    /// Fetch + cache an ISBN preview. Dispatches to the kernel; the result
    /// arrives via the kernel's artifact-preview snapshot in a later wave.
    func requestIsbnPreview(isbn: String) async {
        guard let key = normalizeIsbn(isbn) else { return }
        if isbnPreviewCache[key] != nil { return }
        kernel?.app.dispatch(.lookupIsbn(isbn: key))
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
        let wifiOnly = UserDefaults.standard.bool(forKey: "hl.network.wifi_only")
        applyNetworkPathMonitorEnabled(wifiOnly)
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

    /// Public so `EventBridge` can re-sync on a `MembershipChanged` delta. The
    /// kernel's always-open `CommunitiesSnapshot` is authoritative; this just
    /// re-applies the latest snapshot (idempotent).
    func refreshJoinedCommunities() async {
        guard let communities = kernel?.communities else { return }
        applyCommunitiesSnapshot(communities)
    }

    private func loadAppScopeData() async {
        refreshNetworkPathCapabilityPreference()

        // Communities, bookmarks, and the current user's profile are owned by
        // the kernel's typed snapshots and pushed in via App.swift's `onChange`
        // bridges. Apply whatever the kernel already has for an immediate render.
        if let communities = kernel?.communities {
            applyCommunitiesSnapshot(communities)
        }
        if let bookmarks = kernel?.bookmarks {
            applyBookmarksSnapshot(bookmarks)
        }
        if let user = currentUser,
           let snap = kernel?.profileSnapshots[user.pubkey] {
            currentUserProfile = snap.asProfileMetadata()
        }

        // Default Blossom server setup is now handled by the kernel's
        // DEFAULT_BLOSSOM_SERVER constant; no bespoke init needed.
    }

    private func startNetworkPathMonitor() {
        guard networkPathMonitor == nil else { return }
        let monitor = NWPathMonitor()
        monitor.pathUpdateHandler = { [weak self] path in
            let isWifi = path.status == .satisfied && path.usesInterfaceType(.wifi)
            Task { @MainActor [weak self] in
                self?.applyNetworkPathStatus(isWifi: isWifi)
            }
        }
        monitor.start(queue: .global(qos: .utility))
        networkPathMonitor = monitor
    }

    private func applyNetworkPathStatus(isWifi: Bool) {
        let wifiOnly = UserDefaults.standard.bool(forKey: "hl.network.wifi_only")
        kernel?.app.dispatch(.applyNetworkPath(isWifi: isWifi, wifiOnly: wifiOnly))
    }
}
