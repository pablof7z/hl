import Foundation
import Network
import Observation

/// App-scoped facade over Rust-owned TEA state. Swift renders snapshots and
/// dispatches typed actions; native helpers stay limited to UI routing and OS
/// capability execution.
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
    var whatsNew: HighlighterWhatsNewSnapshot { nmpState.whatsNew }
    var createRoom: HighlighterCreateRoomSnapshot { nmpState.createRoom }
    var roomInvite: HighlighterRoomInviteSnapshot { nmpState.roomInvite }
    var comments: HighlighterCommentsSnapshot { nmpState.comments }
    var feedback: HighlighterFeedbackSnapshot { nmpState.feedback }
    var mediaSettings: HighlighterMediaSettingsSnapshot { nmpState.mediaSettings }
    var editProfile: HighlighterEditProfileSnapshot { nmpState.editProfile }
    var shareComposer: HighlighterShareComposerSnapshot { nmpState.shareComposer }
    var capture: HighlighterCaptureSnapshot { nmpState.capture }
    var bookPicker: HighlighterBookPickerSnapshot { nmpState.bookPicker }

    func highlightShareURL(eventIdHex: String, authorPubkeyHex: String?) -> String? {
        nmpApp.highlightShareUrl(eventIdHex: eventIdHex, authorPubkeyHex: authorPubkeyHex)
    }

    func decodeNostrEntity(_ input: String) -> NostrEntityRef? {
        nmpApp.decodeNostrEntity(input: input)
    }

    func resolveNostrEntity(_ entity: NostrEntityRef) async -> NostrEntityEvent? {
        await nmpApp.resolveNostrEntity(entity: entity)
    }

    func article(pubkeyHex: String, dTag: String) async -> ArticleRecord? {
        await nmpApp.article(pubkeyHex: pubkeyHex, dTag: dTag)
    }

    func dismissWhatsNew() {
        nmpApp.dispatch(action: .dismissWhatsNew)
    }

    func publishArtifactShare(preview: ArtifactPreview, groupId: String, note: String?) {
        nmpApp.dispatch(
            action: .publishArtifactShare(
                preview: preview,
                groupId: groupId,
                note: note
            )
        )
    }

    func publishUrlShare(url: String, groupId: String, note: String?) {
        nmpApp.dispatch(
            action: .publishUrlShare(
                url: url,
                groupId: groupId,
                note: note
            )
        )
    }

    func shareHighlightRepost(
        eventId: String,
        authorPubkeyHex: String,
        relayHint: String,
        targetGroupId: String
    ) {
        nmpApp.dispatch(
            action: .shareHighlightRepost(
                eventId: eventId,
                authorPubkeyHex: authorPubkeyHex,
                relayHint: relayHint,
                targetGroupId: targetGroupId
            )
        )
    }

    func clearShareComposerResult() {
        nmpApp.dispatch(action: .clearShareComposerResult)
    }

    func clearShareComposerError() {
        nmpApp.dispatch(action: .clearShareComposerError)
    }

    func publishQueuedUrlShare(url: String, groupId: String, note: String?) async -> Bool {
        await nmpApp.publishUrlShare(url: url, groupId: groupId, note: note)
    }

    init() {
        // Install the core's tracing subscriber before anything can log.
        // Without this every tracing event in Rust is silently dropped.
        initPlatformLogging()
        let nmpApp = HighlighterNmpApp(
            config: HighlighterAppConfig(dataDir: nil, visibleLimit: 250, emitHz: 30)
        )
        self.nmpApp = nmpApp
        nmpState = nmpApp.state()
        // Surface the MiniPlayer (paused) with whatever episode the user was
        // last listening to, if any. Tapping play wires AVPlayer through the
        // normal `load(artifact:)` path which seeks to the saved position.
        podcastPlayer.rehydrateFromSavedRecord()
        let reconciler = HighlighterAppStateReconciler(appStore: self)
        nmpReconciler = reconciler
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
        nmpApp.clearCoreEventCallback()
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

    func uploadCreateRoomCover(bytes: Data, mime: String, width: UInt32, height: UInt32, alt: String) {
        nmpApp.dispatch(
            action: .uploadCreateRoomCover(
                bytes: bytes,
                mime: mime,
                width: width,
                height: height,
                alt: alt
            )
        )
    }

    func clearCreateRoomCover() {
        nmpApp.dispatch(action: .clearCreateRoomCover)
    }

    func createRoomCapabilityFailed(message: String) {
        nmpApp.dispatch(action: .createRoomCapabilityFailed(message: message))
    }

    func submitCreateRoom(name: String, about: String, visibility: RoomVisibility, access: RoomAccess) {
        nmpApp.dispatch(
            action: .submitCreateRoom(
                name: name,
                about: about,
                visibility: visibility,
                access: access
            )
        )
    }

    func clearCreateRoomResult() {
        nmpApp.dispatch(action: .clearCreateRoomResult)
    }

    func clearCreateRoomError() {
        nmpApp.dispatch(action: .clearCreateRoomError)
    }

    func openRoomInvite(groupId: String) {
        let trimmed = groupId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nmpApp.dispatch(action: .openRoomInvite(groupId: trimmed))
    }

    func refreshRoomInvite() {
        nmpApp.dispatch(action: .refreshRoomInvite)
    }

    func setRoomInviteQuery(_ query: String) {
        nmpApp.dispatch(action: .setRoomInviteQuery(query: query))
    }

    func toggleRoomInviteCandidate(pubkeyHex: String, source: HighlighterRoomInviteCandidateSource) {
        let trimmed = pubkeyHex.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nmpApp.dispatch(action: .toggleRoomInviteCandidate(pubkeyHex: trimmed, source: source))
    }

    func removeRoomInviteCandidate(pubkeyHex: String) {
        let trimmed = pubkeyHex.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nmpApp.dispatch(action: .removeRoomInviteCandidate(pubkeyHex: trimmed))
    }

    func acceptRoomInvitePastedCandidate() {
        nmpApp.dispatch(action: .acceptRoomInvitePastedCandidate)
    }

    func mintRoomInviteLink() {
        nmpApp.dispatch(action: .mintRoomInviteLink)
    }

    func submitRoomInviteMembers() {
        nmpApp.dispatch(action: .submitRoomInviteMembers)
    }

    func clearRoomInviteAddError() {
        nmpApp.dispatch(action: .clearRoomInviteAddError)
    }

    func clearRoomInviteInviteLinkError() {
        nmpApp.dispatch(action: .clearRoomInviteInviteLinkError)
    }

    func clearRoomInviteToast() {
        nmpApp.dispatch(action: .clearRoomInviteToast)
    }

    func closeRoomInvite() {
        nmpApp.dispatch(action: .closeRoomInvite)
    }

    func openComments(rootTagName: String, rootTagValue: String, rootKind: UInt16) {
        let tag = rootTagName.trimmingCharacters(in: .whitespacesAndNewlines)
        let value = rootTagValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !tag.isEmpty, !value.isEmpty else { return }
        nmpApp.dispatch(
            action: .openComments(
                rootTagName: tag,
                rootTagValue: value,
                rootKind: rootKind
            )
        )
    }

    func refreshComments() {
        nmpApp.dispatch(action: .refreshComments)
    }

    func commentDraft(parentEventId: String?) -> String {
        comments.drafts.first { $0.parentEventId == parentEventId }?.body ?? ""
    }

    func setCommentDraft(parentEventId: String?, body: String) {
        nmpApp.dispatch(action: .setCommentDraft(parentEventId: parentEventId, body: body))
    }

    func publishComment(parentEventId: String?) {
        nmpApp.dispatch(action: .publishComment(parentEventId: parentEventId))
    }

    func clearCommentPublishError() {
        nmpApp.dispatch(action: .clearCommentPublishError)
    }

    func toggleCommentLike(eventId: String) {
        let trimmed = eventId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nmpApp.dispatch(action: .toggleCommentLike(eventId: trimmed))
    }

    func toggleCommentBookmark(eventId: String) {
        let trimmed = eventId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nmpApp.dispatch(action: .toggleCommentBookmark(eventId: trimmed))
    }

    func clearCommentInteractionError() {
        nmpApp.dispatch(action: .clearCommentInteractionError)
    }

    func closeComments() {
        nmpApp.dispatch(action: .closeComments)
    }

    func openFeedback(coordinate: String) {
        let trimmed = coordinate.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nmpApp.dispatch(action: .openFeedback(coordinate: trimmed))
    }

    func refreshFeedbackThreads() {
        nmpApp.dispatch(action: .refreshFeedbackThreads)
    }

    func setFeedbackNewThreadDraft(_ body: String) {
        nmpApp.dispatch(action: .setFeedbackNewThreadDraft(body: body))
    }

    func publishFeedbackNewThread() {
        nmpApp.dispatch(action: .publishFeedbackNewThread)
    }

    func openFeedbackThread(rootEventId: String) {
        let trimmed = rootEventId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nmpApp.dispatch(action: .openFeedbackThread(rootEventId: trimmed))
    }

    func refreshFeedbackThread() {
        nmpApp.dispatch(action: .refreshFeedbackThread)
    }

    func setFeedbackReplyDraft(_ body: String) {
        nmpApp.dispatch(action: .setFeedbackReplyDraft(body: body))
    }

    func publishFeedbackReply() {
        nmpApp.dispatch(action: .publishFeedbackReply)
    }

    func clearFeedbackPublishError() {
        nmpApp.dispatch(action: .clearFeedbackPublishError)
    }

    func closeFeedbackThread() {
        nmpApp.dispatch(action: .closeFeedbackThread)
    }

    func closeFeedback() {
        nmpApp.dispatch(action: .closeFeedback)
    }

    func openMediaSettings() {
        nmpApp.dispatch(action: .openMediaSettings)
    }

    func refreshMediaSettings() {
        nmpApp.dispatch(action: .refreshMediaSettings)
    }

    func addBlossomServer(url: String) {
        let trimmed = url.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nmpApp.dispatch(action: .addBlossomServer(url: trimmed))
    }

    func removeBlossomServer(url: String) {
        let trimmed = url.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nmpApp.dispatch(action: .removeBlossomServer(url: trimmed))
    }

    func moveBlossomServers(fromOffsets: IndexSet, toOffset: Int) {
        let indices = fromOffsets.map { UInt32($0) }
        guard !indices.isEmpty, toOffset >= 0 else { return }
        nmpApp.dispatch(
            action: .moveBlossomServers(
                fromIndices: indices,
                toIndex: UInt32(toOffset)
            )
        )
    }

    func clearMediaSettingsError() {
        nmpApp.dispatch(action: .clearMediaSettingsError)
    }

    func closeMediaSettings() {
        nmpApp.dispatch(action: .closeMediaSettings)
    }

    func openEditProfile(seed: ProfileMetadata?) {
        nmpApp.dispatch(action: .openEditProfile(seed: seed))
    }

    func setEditProfileDisplayName(_ value: String) {
        nmpApp.dispatch(action: .setEditProfileDisplayName(value: value))
    }

    func setEditProfileName(_ value: String) {
        nmpApp.dispatch(action: .setEditProfileName(value: value))
    }

    func setEditProfileAbout(_ value: String) {
        nmpApp.dispatch(action: .setEditProfileAbout(value: value))
    }

    func setEditProfilePicture(_ value: String) {
        nmpApp.dispatch(action: .setEditProfilePicture(value: value))
    }

    func setEditProfileBanner(_ value: String) {
        nmpApp.dispatch(action: .setEditProfileBanner(value: value))
    }

    func setEditProfileNip05(_ value: String) {
        nmpApp.dispatch(action: .setEditProfileNip05(value: value))
    }

    func setEditProfileWebsite(_ value: String) {
        nmpApp.dispatch(action: .setEditProfileWebsite(value: value))
    }

    func setEditProfileLud16(_ value: String) {
        nmpApp.dispatch(action: .setEditProfileLud16(value: value))
    }

    func uploadEditProfileImage(
        target: HighlighterEditProfileImageTarget,
        bytes: Data,
        mime: String,
        width: UInt32,
        height: UInt32,
        alt: String
    ) {
        nmpApp.dispatch(
            action: .uploadEditProfileImage(
                target: target,
                bytes: bytes,
                mime: mime,
                width: width,
                height: height,
                alt: alt
            )
        )
    }

    func editProfileCapabilityFailed(message: String) {
        nmpApp.dispatch(action: .editProfileCapabilityFailed(message: message))
    }

    func submitEditProfile() {
        nmpApp.dispatch(action: .submitEditProfile)
    }

    func clearEditProfileError() {
        nmpApp.dispatch(action: .clearEditProfileError)
    }

    func clearEditProfileResult() {
        nmpApp.dispatch(action: .clearEditProfileResult)
    }

    func closeEditProfile() {
        nmpApp.dispatch(action: .closeEditProfile)
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

    var roomDetail: HighlighterRoomDetailSnapshot {
        nmpState.roomDetail
    }

    var network: HighlighterNetworkSnapshot {
        nmpState.network
    }

    func networkRemovalImpact(url: String) -> HighlighterRelayRemovalImpact? {
        nmpApp.networkRemovalImpact(url: url)
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

    func openNetworkSettings() {
        nmpApp.dispatch(action: .openNetworkSettings)
    }

    func refreshNetworkSettings() {
        nmpApp.dispatch(action: .refreshNetworkSettings)
    }

    func closeNetworkSettings() {
        nmpApp.dispatch(action: .closeNetworkSettings)
    }

    func upsertNetworkRelay(_ config: RelayConfig) {
        nmpApp.dispatch(action: .upsertNetworkRelay(config: config))
    }

    func removeNetworkRelay(url: String) {
        let trimmed = url.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nmpApp.dispatch(action: .removeNetworkRelay(url: trimmed))
    }

    func setNetworkRelayRoles(
        url: String,
        read: Bool,
        write: Bool,
        rooms: Bool,
        indexer: Bool
    ) {
        let trimmed = url.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nmpApp.dispatch(
            action: .setNetworkRelayRoles(
                url: trimmed,
                read: read,
                write: write,
                rooms: rooms,
                indexer: indexer
            )
        )
    }

    func probeNetworkRelayNip11(url: String) {
        let trimmed = url.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix("wss://") || trimmed.hasPrefix("ws://") else { return }
        nmpApp.dispatch(action: .probeNetworkRelayNip11(url: trimmed))
    }

    func setNetworkImportNpub(_ npub: String) {
        nmpApp.dispatch(action: .setNetworkImportNpub(npub: npub))
    }

    func fetchNetworkImportRelays() {
        nmpApp.dispatch(action: .fetchNetworkImportRelays)
    }

    func toggleNetworkImportRelay(url: String) {
        let trimmed = url.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nmpApp.dispatch(action: .toggleNetworkImportRelay(url: trimmed))
    }

    func applyNetworkImportRelays() {
        nmpApp.dispatch(action: .applyNetworkImportRelays)
    }

    func clearNetworkError() {
        nmpApp.dispatch(action: .clearNetworkError)
    }

    func networkDiagnostic(url: String) -> RelayDiagnostic? {
        network.diagnostics.first { $0.url == url }
    }

    func networkNip11(url: String) -> HighlighterRelayNip11Snapshot? {
        network.nip11.first { $0.url == url }
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
    /// snapshot; Swift never computes bookmark membership locally.
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

    func openRoom(groupId: String) {
        let trimmed = groupId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nmpApp.dispatch(action: .openRoom(groupId: trimmed))
    }

    func refreshRoom() {
        nmpApp.dispatch(action: .refreshRoom)
    }

    func publishRoomDiscussion(title: String, body: String, attachmentUrl: String?) {
        nmpApp.dispatch(
            action: .publishRoomDiscussion(
                title: title,
                body: body,
                attachmentUrl: attachmentUrl
            )
        )
    }

    func clearRoomDiscussionError() {
        nmpApp.dispatch(action: .clearRoomDiscussionError)
    }

    func loadMoreRoomChat() {
        nmpApp.dispatch(action: .loadMoreRoomChat)
    }

    func publishRoomChatMessage(content: String, replyToEventId: String?) {
        let trimmed = content.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nmpApp.dispatch(
            action: .publishRoomChatMessage(
                content: trimmed,
                replyToEventId: replyToEventId
            )
        )
    }

    func clearRoomChatError() {
        nmpApp.dispatch(action: .clearRoomChatError)
    }

    func closeRoom() {
        nmpApp.dispatch(action: .closeRoom)
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

    func requestReferenceHighlights(tagName: String, tagValue: String, limit: UInt32) {
        let cleanTag = tagName.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let cleanValue = tagValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !cleanTag.isEmpty, !cleanValue.isEmpty else { return }
        nmpApp.dispatch(
            action: .requestReferenceHighlights(
                tagName: cleanTag,
                tagValue: cleanValue,
                limit: limit
            )
        )
    }

    func referenceHighlights(tagName: String, tagValue: String) -> [HighlightRecord]? {
        let key = "\(tagName.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()):\(tagValue.trimmingCharacters(in: .whitespacesAndNewlines))"
        return nmpState.referenceHighlights.first { $0.key == key }?.highlights
    }

    func requestBookPickerRecents(limit: UInt32 = 24) {
        nmpApp.dispatch(action: .requestBookPickerRecents(limit: limit))
    }

    func searchBookPickerArtifacts(query: String, limit: UInt32 = 20) {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            nmpApp.dispatch(action: .clearBookPickerSearch)
            return
        }
        nmpApp.dispatch(action: .searchBookPickerArtifacts(query: trimmed, limit: limit))
    }

    func clearBookPickerSearch() {
        nmpApp.dispatch(action: .clearBookPickerSearch)
    }

    func uploadCapturePhoto(
        bytes: Data,
        mime: String,
        width: UInt32,
        height: UInt32,
        alt: String
    ) {
        nmpApp.dispatch(
            action: .uploadCapturePhoto(
                bytes: bytes,
                mime: mime,
                width: width,
                height: height,
                alt: alt
            )
        )
    }

    func clearCaptureUpload() {
        nmpApp.dispatch(action: .clearCaptureUpload)
    }

    func publishCaptureHighlight(
        selection: HighlighterCaptureArtifact,
        targetGroupId: String?,
        draft: HighlightDraft
    ) {
        nmpApp.dispatch(
            action: .publishCaptureHighlight(
                selection: selection,
                targetGroupId: targetGroupId,
                draft: draft
            )
        )
    }

    func publishCapturePicture(
        selection: HighlighterCaptureArtifact?,
        targetGroupId: String?,
        image: BlossomUpload,
        note: String
    ) {
        nmpApp.dispatch(
            action: .publishCapturePicture(
                selection: selection,
                targetGroupId: targetGroupId,
                image: image,
                note: note
            )
        )
    }

    func publishClipHighlight(
        artifact: ArtifactRecord,
        targetGroupId: String?,
        draft: HighlightDraft
    ) {
        nmpApp.dispatch(
            action: .publishClipHighlight(
                artifact: artifact,
                targetGroupId: targetGroupId,
                draft: draft
            )
        )
    }

    func clearCaptureResult() {
        nmpApp.dispatch(action: .clearCaptureResult)
    }

    func clearCaptureError() {
        nmpApp.dispatch(action: .clearCaptureError)
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
        nmpApp.setCoreEventCallback(callback: bridge)
        eventBridge = bridge
    }

    private func dispatchStoredCredential(_ credential: HighlighterSessionCredential) {
        switch credential {
        case let .nsec(nsec):
            nmpApp.dispatch(
                action: .signInNsec(
                    nsec: nsec,
                    persist: false,
                    clearStoredOnFailure: true
                )
            )
        case let .bunkerUri(uri):
            nmpApp.dispatch(
                action: .pairBunker(
                    uri: uri,
                    persist: false,
                    clearStoredOnFailure: true
                )
            )
        case let .nip55SignerPackage(signerPackage):
            // NIP-55 is an Android-only external-signer (Amber) flow. A persisted
            // NIP-55 package only reaches an iOS install when the identity store
            // is shared with Android. Restore it symmetrically with the other
            // credentials: dispatch the stored package and let NMP own the
            // outcome. There is no signer app on iOS, so pairing fails and
            // `clearStoredOnFailure: true` drops the unusable credential (the
            // documented self-cleaning restore contract), after which the user
            // can re-authenticate with an iOS-supported method.
            nmpApp.dispatch(
                action: .signInNip55(
                    signerPackage: signerPackage,
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

    func onState(state: HighlighterAppState) {
        Task { @MainActor in
            guard let appStore else { return }
            appStore.applyNmpState(state)
        }
    }

    func onPersistSessionCredential(credential: HighlighterSessionCredential) {
        Task { @MainActor in
            AppSessionStore.shared.persist(credential)
        }
    }

    func onClearSessionCredentials() {
        Task { @MainActor in
            AppSessionStore.shared.clear()
        }
    }

    func onOpenExternalUrl(url: String) {
        Task { @MainActor in
            guard let appStore else { return }
            appStore.openExternalUrl(url)
        }
    }
}
