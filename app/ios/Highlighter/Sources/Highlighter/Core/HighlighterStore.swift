import Foundation
import Network
import Observation

/// App-scoped facade over Rust-owned TEA state. Swift renders snapshots and
/// dispatches typed actions; remaining legacy view stores are narrow routing
/// adapters while their slices move into `HighlighterNmpApp`.
@MainActor
@Observable
final class HighlighterStore {
    // Reactive — drives UI
    var podcastPlayer = PodcastPlayerStore()
    var nmpState: HighlighterAppState
    /// Transient toast shown when the Share Extension handoff publishes, a
    /// join request is sent, or a membership is confirmed. Cleared by the
    /// banner after a few seconds.
    var shareToast: String?

    // Internal plumbing
    @ObservationIgnored let nmpApp: HighlighterNmpApp
    @ObservationIgnored let core: HighlighterCore
    @ObservationIgnored let safeCore: SafeHighlighterCore
    @ObservationIgnored private(set) var eventBridge: EventBridge?
    @ObservationIgnored private var nmpReconciler: HighlighterAppStateReconciler?
    @ObservationIgnored private var lastNmpToastMessage: String?
    @ObservationIgnored private var mirroredCommunityIds: [String] = []
    @ObservationIgnored private var externalUrlHandler: ((String, @escaping (Bool) -> Void) -> Void)?
    @ObservationIgnored private var networkPathMonitor: NWPathMonitor?

    var currentUser: CurrentUser? { nmpState.chrome.currentUser }
    var currentUserProfile: ProfileMetadata? { nmpState.chrome.currentUserProfile }
    var joinedCommunities: [CommunitySummary] { nmpState.chrome.joinedCommunities }
    var bookmarkedArticleAddresses: Set<String> {
        Set(nmpState.chrome.bookmarkedArticleAddresses)
    }
    var isBootstrapping: Bool { nmpState.isBootstrapping }
    var isAuthenticating: Bool { nmpState.auth.isSigningIn }
    var isLoggedIn: Bool { currentUser != nil }

    init() {
        let nmpApp = HighlighterNmpApp(
            config: HighlighterAppConfig(dataDir: nil, visibleLimit: 250, emitHz: 30)
        )
        let core = nmpApp.legacyCore()
        self.nmpApp = nmpApp
        self.nmpState = nmpApp.state()
        self.core = core
        self.safeCore = SafeHighlighterCore(core: core)
        // Surface the MiniPlayer (paused) with whatever episode the user was
        // last listening to, if any. Tapping play wires AVPlayer through the
        // normal `load(artifact:)` path which seeks to the saved position.
        podcastPlayer.rehydrateFromSavedRecord()
        let reconciler = HighlighterAppStateReconciler(appStore: self)
        self.nmpReconciler = reconciler
        nmpApp.listenForUpdates(reconciler: reconciler)
        syncNetworkPathMonitor(wifiOnlyEnabled: nmpState.network.wifiOnlyEnabled)
    }

    func bootstrap() async {
        guard !isBootstrapping else { return }

        // Register the EventBridge unconditionally, before any login attempt.
        // The NIP-46 nostrconnect:// flow fires `SignerConnected` from a
        // background tokio task; if no callback is wired by then, the delta
        // is dropped silently and the UI never transitions to logged-in.
        registerEventBridge()
        nmpApp.dispatch(action: .bootstrap)

        if let credential = AppSessionStore.shared.storedCredential() {
            dispatchStoredCredential(credential)
        }
    }

    func appForegrounded() {
        nmpApp.dispatch(action: .appForegrounded)
    }

    func completeLogin() async {
        if eventBridge == nil {
            registerEventBridge()
        }
        nmpApp.dispatch(action: .refreshAppChrome)
    }

    func signInNsec(_ nsec: String, remember: Bool = true) {
        let trimmed = nsec.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nmpApp.dispatch(
            action: .signInNsec(
                nsec: trimmed,
                persist: remember,
                clearStoredOnFailure: false
            )
        )
    }

    func pairBunker(_ uri: String, remember: Bool = true) {
        let trimmed = uri.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nmpApp.dispatch(
            action: .pairBunker(
                uri: trimmed,
                persist: remember,
                clearStoredOnFailure: false
            )
        )
    }

    func startNostrConnect(callbackUrl: String) {
        nmpApp.dispatch(action: .startNostrConnect(callbackUrl: callbackUrl))
    }

    func setExternalUrlHandler(_ handler: @escaping (String, @escaping (Bool) -> Void) -> Void) {
        externalUrlHandler = handler
    }

    func clearExternalUrlHandler() {
        externalUrlHandler = nil
    }

    func logout() {
        nmpApp.clearLegacyEventCallback()
        nmpApp.dispatch(action: .logout)
        eventBridge = nil
        AppSessionStore.shared.clear()
        mirroredCommunityIds.removeAll()
        SharedCommunitiesCache.clear()
    }

    func clearToast() {
        shareToast = nil
        lastNmpToastMessage = nil
        nmpApp.dispatch(action: .clearToast)
    }

    var search: HighlighterSearchSnapshot {
        nmpState.search
    }

    var roomExplorer: HighlighterRoomExplorerSnapshot {
        nmpState.roomExplorer
    }

    var homeFeed: HighlighterHomeFeedSnapshot {
        nmpState.homeFeed
    }

    var bookmarks: HighlighterBookmarksSnapshot {
        nmpState.bookmarks
    }

    var curationMenu: HighlighterCurationMenuSnapshot {
        nmpState.curationMenu
    }

    var profileView: HighlighterProfileViewSnapshot {
        nmpState.profileView
    }

    var articleReader: HighlighterArticleReaderSnapshot {
        nmpState.articleReader
    }

    var network: HighlighterNetworkSnapshot {
        nmpState.network
    }

    func openSearch() {
        nmpApp.dispatch(action: .searchOpened)
    }

    func closeSearch() {
        nmpApp.dispatch(action: .searchClosed)
    }

    func setSearchQuery(_ query: String) {
        nmpApp.dispatch(action: .setSearchQuery(query: query))
    }

    func submitSearch(_ query: String) {
        nmpApp.dispatch(action: .submitSearch(query: query))
    }

    func clearSearch() {
        nmpApp.dispatch(action: .clearSearch)
    }

    func clearRecentSearches() {
        nmpApp.dispatch(action: .clearRecentSearches)
    }

    func openRoomExplorer() {
        nmpApp.dispatch(action: .openRoomExplorer)
    }

    func refreshRoomExplorer() {
        nmpApp.dispatch(action: .refreshRoomExplorer)
    }

    func refreshRoomBrowseAll() {
        nmpApp.dispatch(action: .refreshRoomBrowseAll)
    }

    func requestJoinRoom(groupId: String, roomName: String) {
        let trimmedId = groupId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedId.isEmpty else { return }
        let cleanName = roomName.trimmingCharacters(in: .whitespacesAndNewlines)
        nmpApp.dispatch(
            action: .requestJoinRoom(
                groupId: trimmedId,
                roomName: cleanName.isEmpty ? "this room" : cleanName
            )
        )
    }

    func openHomeFeed() {
        nmpApp.dispatch(action: .openHomeFeed)
    }

    func refreshHomeFeed() {
        nmpApp.dispatch(action: .refreshHomeFeed)
    }

    func closeHomeFeed() {
        nmpApp.dispatch(action: .closeHomeFeed)
    }

    func setCreateAccountDisplayName(_ displayName: String) {
        nmpApp.dispatch(action: .setCreateAccountDisplayName(displayName: displayName))
    }

    func setCreateAccountUsername(_ username: String) {
        nmpApp.dispatch(action: .setCreateAccountUsername(username: username))
    }

    func submitCreateAccount() {
        nmpApp.dispatch(action: .submitCreateAccount)
    }

    func toggleOnboardingInterest(id: String) {
        nmpApp.dispatch(action: .toggleOnboardingInterest(interestId: id))
    }

    func completeOnboarding() {
        nmpApp.dispatch(action: .completeOnboarding)
    }

    // MARK: - Bookmarks

    /// Rust-owned toggle: the actor publishes and emits the next bookmark
    /// snapshot; Swift does not run optimistic bookmark policy.
    func toggleBookmark(articleAddress: String) async {
        let trimmed = articleAddress.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nmpApp.dispatch(action: .toggleArticleBookmark(address: trimmed))
    }

    func isBookmarked(articleAddress: String) -> Bool {
        bookmarkedArticleAddresses.contains(articleAddress)
    }

    func openBookmarks() {
        nmpApp.dispatch(action: .openBookmarks)
    }

    func refreshBookmarks() {
        nmpApp.dispatch(action: .refreshBookmarks)
    }

    func closeBookmarks() {
        nmpApp.dispatch(action: .closeBookmarks)
    }

    func openBookmarkCollection(_ record: BookmarkSetRecord) {
        nmpApp.dispatch(
            action: .openBookmarkCollection(
                pubkeyHex: record.pubkey,
                dTag: record.id,
                kind: record.kind
            )
        )
    }

    func refreshBookmarkCollection() {
        nmpApp.dispatch(action: .refreshBookmarkCollection)
    }

    func openCurationMenu(articleAddress: String) {
        nmpApp.dispatch(action: .openCurationMenu(articleAddress: articleAddress))
    }

    func closeCurationMenu() {
        nmpApp.dispatch(action: .closeCurationMenu)
    }

    func setAddressInCurationSet(dTag: String, address: String, member: Bool) {
        nmpApp.dispatch(
            action: .setAddressInCurationSet(
                dTag: dTag,
                address: address,
                member: member
            )
        )
    }

    func createCurationSetAndAdd(title: String, address: String) {
        nmpApp.dispatch(action: .createCurationSetAndAdd(title: title, address: address))
    }

    /// Ask Rust-owned app state to resolve and subscribe to a profile.
    func requestProfile(pubkeyHex: String) {
        let trimmed = pubkeyHex.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nmpApp.dispatch(action: .requestProfile(pubkeyHex: trimmed))
    }

    func openProfile(pubkeyHex: String) {
        let trimmed = pubkeyHex.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nmpApp.dispatch(action: .openProfile(pubkeyHex: trimmed))
    }

    func refreshProfile() {
        nmpApp.dispatch(action: .refreshProfile)
    }

    func closeProfile() {
        nmpApp.dispatch(action: .closeProfile)
    }

    func toggleProfileFollow() {
        nmpApp.dispatch(action: .toggleProfileFollow)
    }

    func openArticleReader(pubkeyHex: String, dTag: String, seed: ArticleRecord?) {
        let pubkey = pubkeyHex.trimmingCharacters(in: .whitespacesAndNewlines)
        let identifier = dTag.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !pubkey.isEmpty, !identifier.isEmpty else { return }
        nmpApp.dispatch(
            action: .openArticleReader(
                pubkeyHex: pubkey,
                dTag: identifier,
                seed: seed
            )
        )
    }

    func refreshArticleReader() {
        nmpApp.dispatch(action: .refreshArticleReader)
    }

    func closeArticleReader() {
        nmpApp.dispatch(action: .closeArticleReader)
    }

    func publishArticleHighlight(quote: String, context: String, note: String) {
        let trimmedQuote = quote.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedQuote.isEmpty else { return }
        nmpApp.dispatch(
            action: .publishArticleHighlight(
                quote: trimmedQuote,
                context: context,
                note: note.trimmingCharacters(in: .whitespacesAndNewlines)
            )
        )
    }

    func setNetworkWifiOnly(_ enabled: Bool) {
        nmpApp.dispatch(action: .setNetworkWifiOnly(enabled: enabled))
    }

    func reconnectNetwork() {
        nmpApp.dispatch(action: .reconnectNetwork)
    }

    func profile(pubkeyHex: String) -> ProfileMetadata? {
        let key = pubkeyHex.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !key.isEmpty else { return nil }
        return nmpState.profiles.first { $0.pubkeyHex == key || $0.metadata.pubkey == key }?.metadata
    }

    /// Ask Rust-owned app state to resolve web metadata for the current UI.
    func requestWebMetadata(url: String) {
        let trimmed = url.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nmpApp.dispatch(action: .requestWebMetadata(url: trimmed))
    }

    func webMetadata(url: String) -> WebMetadata? {
        let key = url.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !key.isEmpty else { return nil }
        return nmpState.webMetadata.first {
            $0.url == key || (!$0.metadata.url.isEmpty && $0.metadata.url == key)
        }?.metadata
    }

    /// Ask Rust-owned app state to resolve an ISBN preview for the current UI.
    /// `isbn` must be the bare ISBN string (no "isbn:" prefix).
    func requestIsbnPreview(isbn: String) {
        let key = isbn.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !key.isEmpty else { return }
        nmpApp.dispatch(action: .requestIsbnPreview(isbn: key))
    }

    func isbnPreview(isbn: String) -> ArtifactPreview? {
        let key = isbn.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !key.isEmpty else { return nil }
        return nmpState.isbnPreviews.first { $0.isbn == key }?.preview
    }

    /// Persist Rust's bounded Share Extension projection into the App Group
    /// cache. Swift is only the OS handoff writer; Rust owns the payload.
    private func mirrorCommunitiesToAppGroup(_ communities: [HighlighterShareExtensionCommunity]) {
        let snapshot = communities.map {
            SharedCommunitySummary(id: $0.id, name: $0.name, picture: $0.picture)
        }
        SharedCommunitiesCache.save(snapshot)
    }

    // MARK: - Private

    private func registerEventBridge() {
        let bridge = EventBridge(appStore: self)
        nmpApp.setLegacyEventCallback(callback: bridge)
        eventBridge = bridge
    }

    private func dispatchStoredCredential(_ credential: HighlighterSessionCredential) {
        switch credential {
        case .nsec(let nsec):
            nmpApp.dispatch(
                action: .signInNsec(
                    nsec: nsec,
                    persist: false,
                    clearStoredOnFailure: true
                )
            )
        case .bunkerUri(let uri):
            nmpApp.dispatch(
                action: .pairBunker(
                    uri: uri,
                    persist: false,
                    clearStoredOnFailure: true
                )
            )
        }
    }

    fileprivate func openExternalUrl(_ url: String) {
        guard let externalUrlHandler else {
            nmpApp.dispatch(action: .externalUrlOpenFailed(url: url))
            return
        }
        externalUrlHandler(url) { [weak self] accepted in
            guard !accepted else { return }
            Task { @MainActor in
                self?.nmpApp.dispatch(action: .externalUrlOpenFailed(url: url))
            }
        }
    }

    @MainActor
    fileprivate func applyNmpState(_ state: HighlighterAppState) {
        nmpState = state
        syncNetworkPathMonitor(wifiOnlyEnabled: state.network.wifiOnlyEnabled)
        let extensionCommunities = state.shareExtension.communities
        let communityIds = extensionCommunities.map(\.id)
        if communityIds != mirroredCommunityIds {
            mirroredCommunityIds = communityIds
            mirrorCommunitiesToAppGroup(extensionCommunities)
        }
        if let toast = state.toast {
            lastNmpToastMessage = toast.message
            shareToast = toast.message
        } else if let lastNmpToastMessage, shareToast == lastNmpToastMessage {
            self.lastNmpToastMessage = nil
            shareToast = nil
        }
    }

    private func syncNetworkPathMonitor(wifiOnlyEnabled: Bool) {
        if wifiOnlyEnabled {
            guard networkPathMonitor == nil else { return }
            let monitor = NWPathMonitor()
            monitor.pathUpdateHandler = { [weak self] path in
                let isWifi = path.usesInterfaceType(.wifi)
                Task { @MainActor in
                    self?.nmpApp.dispatch(action: .networkPathChanged(isWifi: isWifi))
                }
            }
            monitor.start(queue: DispatchQueue.global(qos: .utility))
            networkPathMonitor = monitor
        } else {
            networkPathMonitor?.cancel()
            networkPathMonitor = nil
        }
    }
}

private final class HighlighterAppStateReconciler: HighlighterAppReconciler, @unchecked Sendable {
    private weak var appStore: HighlighterStore?

    init(appStore: HighlighterStore) {
        self.appStore = appStore
    }

    func onUpdate(update: HighlighterAppUpdate) {
        Task { @MainActor in
            guard let appStore else { return }
            switch update {
            case .fullState(let state):
                appStore.applyNmpState(state)
            case .persistSessionCredential(let credential):
                AppSessionStore.shared.persist(credential)
            case .clearSessionCredentials:
                AppSessionStore.shared.clear()
            case .openExternalUrl(let url):
                appStore.openExternalUrl(url)
            }
        }
    }
}
