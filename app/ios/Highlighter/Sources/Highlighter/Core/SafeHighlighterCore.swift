import Foundation

/// Actor-isolated wrapper around the UniFFI-generated `HighlighterCore` so
/// Swift call sites get serialized access without worrying about FFI thread
/// safety. Mirrors TENEX's `SafeTenexCore`.
actor SafeHighlighterCore {
    private let core: HighlighterCore

    init(core: HighlighterCore) {
        self.core = core
    }

    // MARK: - Auth

    func loginNsec(_ nsec: String) -> CurrentUserOutcome {
        core.loginNsec(nsec: nsec)
    }

    func startNostrConnect(_ options: NostrConnectOptions) async -> StringOutcome {
        await core.startNostrConnect(options: options)
    }

    func pairBunker(_ uri: String) async -> CurrentUserOutcome {
        await core.pairBunker(uri: uri)
    }

    func generateAccount() -> GeneratedAccountOutcome {
        core.generateAccount()
    }

    func currentUser() -> CurrentUser? {
        core.currentUser()
    }

    func isOnboardingComplete() -> Bool {
        core.isOnboardingComplete()
    }

    func setOnboardingComplete(_ complete: Bool) -> MutationOutcome {
        core.setOnboardingComplete(complete: complete)
    }

    func isWifiOnlyEnabled() -> Bool {
        core.isWifiOnlyEnabled()
    }

    func setWifiOnlyEnabled(_ enabled: Bool) -> MutationOutcome {
        core.setWifiOnlyEnabled(enabled: enabled)
    }

    func getPodcastPosition() -> PodcastPositionRecord? {
        core.getPodcastPosition()
    }

    func getPodcastPositionSeconds(guid: String) -> Double? {
        core.getPodcastPositionSeconds(guid: guid)
    }

    func savePodcastPosition(
        guid: String,
        positionSeconds: Double,
        artifact: ArtifactRecord
    ) -> MutationOutcome {
        core.savePodcastPosition(
            guid: guid,
            positionSeconds: positionSeconds,
            artifact: artifact
        )
    }

    func loadPodcastTranscript(url: String) async -> TranscriptSegmentListOutcome {
        await core.loadPodcastTranscript(url: url)
    }

    func downloadPodcastArtwork(url: String) async -> DataOutcome {
        await core.downloadPodcastArtwork(url: url)
    }

    func prepareWhatsNew() async -> WhatsNewEntriesOutcome {
        await core.prepareWhatsNew()
    }

    func markWhatsNewSeen(shippedAtUnixSeconds: UInt64) async -> MutationOutcome {
        await core.markWhatsNewSeen(shippedAtUnixSeconds: shippedAtUnixSeconds)
    }

    // MARK: - Reads

    func getJoinedCommunities() async throws -> [CommunitySummary] {
        try await core.getJoinedCommunities()
    }

    func getArtifacts(groupId: String, limit: UInt32 = 32) async throws -> [ArtifactRecord] {
        try await core.getArtifacts(groupId: groupId, limit: limit)
    }

    func getHighlights(groupId: String, limit: UInt32 = 64) async throws -> [HydratedHighlight] {
        try await core.getHighlights(groupId: groupId, limit: limit)
    }

    func getRecentBooks(limit: UInt32 = 24) async throws -> [ArtifactRecord] {
        try await core.getRecentBooks(limit: limit)
    }

    func searchArtifacts(query: String, limit: UInt32 = 20) async throws -> [ArtifactRecord] {
        try await core.searchArtifacts(query: query, limit: limit)
    }

    // MARK: - Search (local ndb + NIP-50 relay)

    func searchHighlights(query: String, limit: UInt32 = 20) async throws -> [HighlightRecord] {
        try await core.searchHighlights(query: query, limit: limit)
    }

    func searchArticles(query: String, limit: UInt32 = 20) async throws -> [ArticleRecord] {
        try await core.searchArticles(query: query, limit: limit)
    }

    func searchCommunities(query: String, limit: UInt32 = 20) async throws -> [CommunitySummary] {
        let candidates = try await core.searchCommunities(query: query, limit: publicRoomCandidateLimit(limit))
        return Array(candidates.filter(\.isPublicOpenRoom).prefix(Int(limit)))
    }

    func searchProfiles(query: String, limit: UInt32 = 20) async throws -> [ProfileMetadata] {
        try await core.searchProfiles(query: query, limit: limit)
    }

    func getSearchRelays() async -> StringListOutcome {
        await core.getSearchRelays()
    }

    func getRecentSearches() async -> StringListOutcome {
        await core.getRecentSearches()
    }

    func recordRecentSearch(_ query: String) async -> StringListOutcome {
        await core.recordRecentSearch(query: query)
    }

    func clearRecentSearches() async -> StringListOutcome {
        await core.clearRecentSearches()
    }

    func subscribeArticleSearch(query: String) async -> SubscriptionOutcome {
        await core.subscribeArticleSearch(query: query)
    }

    // MARK: - Bookmarks (NIP-51 kind:10003)

    func getBookmarkedArticleAddresses() async -> StringListOutcome {
        await core.getBookmarkedArticleAddresses()
    }

    func isArticleBookmarked(address: String) async -> BoolOutcome {
        await core.isArticleBookmarked(address: address)
    }

    func toggleArticleBookmark(address: String) async -> BoolOutcome {
        await core.toggleArticleBookmark(address: address)
    }

    func subscribeBookmarks() async -> SubscriptionOutcome {
        await core.subscribeBookmarks()
    }

    // MARK: - Reactions (kind:7)

    func getReactionsForEvent(targetEventId: String, limit: UInt32) async throws -> [ReactionRecord] {
        try await core.getReactionsForEvent(targetEventId: targetEventId, limit: limit)
    }

    func publishReaction(eventId: String, authorPubkeyHex: String, targetKind: UInt16, content: String) async throws -> ReactionRecord {
        try await core.publishReaction(eventId: eventId, authorPubkeyHex: authorPubkeyHex, targetKind: targetKind, content: content)
    }

    func unpublishReaction(reactionEventId: String) async throws -> String {
        try await core.unpublishReaction(reactionEventId: reactionEventId)
    }

    // MARK: - Event bookmarks (kind:10003 note bookmarks)

    func isEventBookmarked(eventIdHex: String) async -> BoolOutcome {
        await core.isEventBookmarked(eventIdHex: eventIdHex)
    }

    func toggleEventBookmark(eventIdHex: String) async -> BoolOutcome {
        await core.toggleEventBookmark(eventIdHex: eventIdHex)
    }

    // MARK: - Bookmark sets (kind:30003/30004) + NIP-B0 (kind:39701)

    func getMyBookmarkSets() async -> BookmarkSetListOutcome {
        await core.getMyBookmarkSets()
    }

    func getMyCurationSets() async -> BookmarkSetListOutcome {
        await core.getMyCurationSets()
    }

    func getFollowingCurationSets() async -> BookmarkSetListOutcome {
        await core.getFollowingCurationSets()
    }

    func createCurationSet(title: String) async -> BookmarkSetOutcome {
        await core.createCurationSet(title: title)
    }

    @discardableResult
    func setAddressInCurationSet(
        dTag: String,
        address: String,
        member: Bool
    ) async -> BoolOutcome {
        await core.setAddressInCurationSet(dTag: dTag, address: address, member: member)
    }

    func getMyWebBookmarks() async -> WebBookmarkListOutcome {
        await core.getMyWebBookmarks()
    }

    func subscribeBookmarkSets() async -> SubscriptionOutcome {
        await core.subscribeBookmarkSets()
    }

    func subscribeFollowingCurationSets() async -> SubscriptionOutcome {
        await core.subscribeFollowingCurationSets()
    }

    func subscribeWebBookmarks() async -> SubscriptionOutcome {
        await core.subscribeWebBookmarks()
    }

    func lookupIsbn(_ isbn: String) async throws -> ArtifactPreview {
        try await core.lookupIsbn(isbn: isbn)
    }

    func buildPreviewFromUrl(_ url: String) async throws -> ArtifactPreview {
        try await core.buildPreviewFromUrl(url: url)
    }

    func getWebMetadata(url: String) async throws -> WebMetadata {
        try await core.getWebMetadata(url: url)
    }

    func getDiscussions(groupId: String, limit: UInt32 = 64) async throws -> [DiscussionRecord] {
        try await core.getDiscussions(groupId: groupId, limit: limit)
    }

    // MARK: - Chat (NIP-29 kind:9)

    func getChatMessages(groupId: String, limit: UInt32 = 200) async throws -> [ChatMessageRecord] {
        try await core.getChatMessages(groupId: groupId, limit: limit)
    }

    func publishChatMessage(
        groupId: String,
        content: String,
        replyToEventId: String? = nil
    ) async -> ChatMessageOutcome {
        await core.publishChatMessage(
            groupId: groupId,
            content: content,
            replyToEventId: replyToEventId
        )
    }

    func subscribeRoomChat(groupId: String) async -> SubscriptionOutcome {
        await core.subscribeRoomChat(groupId: groupId)
    }

    // MARK: - Feedback (shake-to-share)

    func getFeedbackThreads(coordinate: String) async throws -> [FeedbackThreadRecord] {
        try await core.getFeedbackThreads(coordinate: coordinate)
    }

    func getFeedbackThreadEvents(rootEventId: String) async throws -> [FeedbackEventRecord] {
        try await core.getFeedbackThreadEvents(rootEventId: rootEventId)
    }

    func getProjectFirstAgentPubkey(coordinate: String) async throws -> String? {
        try await core.getProjectFirstAgentPubkey(coordinate: coordinate)
    }

    func publishFeedbackNote(
        coordinate: String,
        agentPubkey: String?,
        parentEventId: String?,
        body: String
    ) async -> FeedbackEventOutcome {
        await core.publishFeedbackNote(
            coordinate: coordinate,
            agentPubkey: agentPubkey,
            parentEventId: parentEventId,
            body: body
        )
    }

    func subscribeFeedbackThreads(coordinate: String) async -> SubscriptionOutcome {
        await core.subscribeFeedbackThreads(coordinate: coordinate)
    }

    func subscribeFeedbackThread(rootEventId: String) async -> SubscriptionOutcome {
        await core.subscribeFeedbackThread(rootEventId: rootEventId)
    }

    // MARK: - Profile reads

    func getUserProfile(pubkeyHex: String) async throws -> ProfileMetadata? {
        try await core.getUserProfile(pubkeyHex: pubkeyHex)
    }

    func decodeNostrEntity(_ input: String) throws -> NostrEntityRef {
        try core.decodeNostrEntity(input: input)
    }

    /// Mint a NIP-19 `nevent` for a highlight share URL. Relay hints are
    /// Rust-owned policy, not native view input.
    func encodeHighlightShareNevent(
        eventIdHex: String,
        authorPubkeyHex: String
    ) throws -> String {
        try core.encodeHighlightShareNevent(
            eventIdHex: eventIdHex,
            authorPubkeyHex: authorPubkeyHex
        )
    }

    func resolveNostrEntity(_ entity: NostrEntityRef) async throws -> NostrEntityEvent? {
        try await core.resolveNostrEntity(entity: entity)
    }

    func subscribeNostrEntity(_ entity: NostrEntityRef) async -> SubscriptionOutcome {
        await core.subscribeNostrEntity(entity: entity)
    }

    func updateProfile(
        name: String,
        displayName: String,
        about: String,
        picture: String,
        banner: String,
        nip05: String,
        website: String,
        lud16: String
    ) async throws -> ProfileMetadata {
        try await core.updateProfile(
            name: name,
            displayName: displayName,
            about: about,
            picture: picture,
            banner: banner,
            nip05: nip05,
            website: website,
            lud16: lud16
        )
    }

    nonisolated func normalizeNip05Username(_ input: String) -> String {
        core.normalizeNip05Username(input: input)
    }

    nonisolated func suggestNip05Username(displayName: String) -> String {
        core.suggestNip05Username(displayName: displayName)
    }

    nonisolated func isNip05UsernameValid(_ input: String) -> Bool {
        core.isNip05UsernameValid(input: input)
    }

    func checkNip05Availability(name: String) async throws -> Nip05Availability {
        try await core.checkNip05Availability(name: name)
    }

    func registerNip05(name: String, domain: String) async throws -> String {
        try await core.registerNip05(name: name, domain: domain)
    }

    func getUserArticles(pubkeyHex: String, limit: UInt32 = 32) async throws -> [ArticleRecord] {
        try await core.getUserArticles(pubkeyHex: pubkeyHex, limit: limit)
    }

    func getArticle(pubkeyHex: String, dTag: String) async throws -> ArticleRecord? {
        try await core.getArticle(pubkeyHex: pubkeyHex, dTag: dTag)
    }

    func getHighlightsForArticle(address: String, limit: UInt32 = 128) async throws -> [HighlightRecord] {
        try await core.getHighlightsForArticle(address: address, limit: limit)
    }

    func getHighlightsForReference(
        tagName: String,
        tagValue: String,
        limit: UInt32 = 128
    ) async throws -> [HighlightRecord] {
        try await core.getHighlightsForReference(tagName: tagName, tagValue: tagValue, limit: limit)
    }

    func getCommentsForReference(
        tagName: String,
        tagValue: String,
        limit: UInt32 = 128
    ) async throws -> [CommentRecord] {
        try await core.getCommentsForReference(tagName: tagName, tagValue: tagValue, limit: limit)
    }

    func publishComment(
        rootTagName: String,
        rootTagValue: String,
        rootKind: UInt16,
        parentEventId: String? = nil,
        content: String
    ) async -> CommentOutcome {
        await core.publishComment(rootTagName: rootTagName, rootTagValue: rootTagValue, rootKind: rootKind, parentEventId: parentEventId, content: content)
    }

    func getUserHighlights(pubkeyHex: String, limit: UInt32 = 64) async throws -> [HighlightRecord] {
        try await core.getUserHighlights(pubkeyHex: pubkeyHex, limit: limit)
    }

    func getUserCommunities(pubkeyHex: String) async throws -> [CommunitySummary] {
        try await core.getUserCommunities(pubkeyHex: pubkeyHex)
    }

    // MARK: - Rooms explorer

    func startRoomDiscovery() async {
        await core.startRoomDiscovery()
    }

    func startFriendsRoomsDiscovery() async throws {
        try await core.startFriendsRoomsDiscovery()
    }

    func startFeaturedRooms(curatorPubkeyHex: String) async throws {
        try await core.startFeaturedRooms(curatorPubkeyHex: curatorPubkeyHex)
    }

    func getRoomExplorerCuratorPubkey() async throws -> String {
        try await core.getRoomExplorerCuratorPubkey()
    }

    func startRoomExplorerFeaturedRooms() async throws {
        try await core.startRoomExplorerFeaturedRooms()
    }

    func getFeaturedRooms(curatorPubkeyHex: String) async throws -> [CommunitySummary] {
        try await core.getFeaturedRooms(curatorPubkeyHex: curatorPubkeyHex).filter(\.isPublicOpenRoom)
    }

    func getAllRooms(limit: UInt32 = 120) async throws -> [CommunitySummary] {
        let candidates = try await core.getAllRooms(limit: publicRoomCandidateLimit(limit))
        return Array(candidates.filter(\.isPublicOpenRoom).prefix(Int(limit)))
    }

    func getNewRooms(limit: UInt32 = 24) async throws -> [CommunitySummary] {
        let candidates = try await core.getNewRooms(limit: publicRoomCandidateLimit(limit))
        return Array(candidates.filter(\.isPublicOpenRoom).prefix(Int(limit)))
    }

    func getRoomsWithFriends(limit: UInt32 = 16) async throws -> [RoomRecommendation] {
        let candidates = try await core.getRoomsWithFriends(limit: publicRoomCandidateLimit(limit))
        return Array(candidates.filter { $0.summary.isPublicOpenRoom }.prefix(Int(limit)))
    }

    func getRoomsFromReadAuthors(limit: UInt32 = 16) async throws -> [RoomRecommendation] {
        let candidates = try await core.getRoomsFromReadAuthors(limit: publicRoomCandidateLimit(limit))
        return Array(candidates.filter { $0.summary.isPublicOpenRoom }.prefix(Int(limit)))
    }

    func requestJoinRoom(groupId: String) async throws -> String {
        try await core.requestJoinRoom(groupId: groupId)
    }

    func createRoom(
        name: String,
        about: String,
        picture: String,
        visibility: RoomVisibility,
        access: RoomAccess
    ) async throws -> String {
        try await core.createRoom(
            name: name,
            about: about,
            picture: picture,
            visibility: visibility,
            access: access
        )
    }

    func addRoomMember(groupId: String, pubkeyHex: String) async throws -> String {
        try await core.addRoomMember(groupId: groupId, pubkeyHex: pubkeyHex)
    }

    func createRoomInviteCodes(groupId: String, count: UInt32) async throws -> [String] {
        try await core.createRoomInviteCodes(groupId: groupId, count: count)
    }

    func getFollows() async throws -> [String] {
        try await core.getFollows()
    }

    func decodeNpub(_ input: String) throws -> String {
        try core.decodeNpub(input: input)
    }

    func isFollowing(targetPubkeyHex: String) async throws -> Bool {
        try await core.isFollowing(targetPubkeyHex: targetPubkeyHex)
    }

    func setFollow(targetPubkeyHex: String, follow: Bool) async throws -> String? {
        try await core.setFollow(targetPubkeyHex: targetPubkeyHex, follow: follow)
    }

    // MARK: - Following Reads

    func getFollowingReads(limit: UInt32 = 40) async throws -> [ReadingFeedItem] {
        try await core.getFollowingReads(limit: limit)
    }

    // MARK: - Following Highlights

    func getFollowingHighlights(limit: UInt32 = 120) async throws -> [HydratedHighlight] {
        try await core.getFollowingHighlights(limit: limit)
    }

    // MARK: - Subscriptions

    func subscribeFollowingReads() async -> SubscriptionOutcome {
        await core.subscribeFollowingReads()
    }

    func subscribeFollowingHighlights() async -> SubscriptionOutcome {
        await core.subscribeFollowingHighlights()
    }

    func subscribeJoinedCommunities() async -> SubscriptionOutcome {
        await core.subscribeJoinedCommunities()
    }

    func subscribeRoom(groupId: String) async -> SubscriptionOutcome {
        await core.subscribeRoom(groupId: groupId)
    }

    func subscribeRoomDiscussions(groupId: String) async -> SubscriptionOutcome {
        await core.subscribeRoomDiscussions(groupId: groupId)
    }

    func subscribeUserProfile(pubkeyHex: String) async -> SubscriptionOutcome {
        await core.subscribeUserProfile(pubkeyHex: pubkeyHex)
    }

    func subscribeArticle(pubkeyHex: String, dTag: String) async -> SubscriptionOutcome {
        await core.subscribeArticle(pubkeyHex: pubkeyHex, dTag: dTag)
    }

    func unsubscribe(_ handle: UInt64) {
        core.unsubscribe(handle: handle)
    }

    // MARK: - Writes

    func publishArtifact(
        preview: ArtifactPreview,
        groupId: String,
        note: String?
    ) async -> ArtifactOutcome {
        await core.publishArtifact(preview: preview, groupId: groupId, note: note)
    }

    func publishDiscussion(
        groupId: String,
        title: String,
        body: String,
        attachment: ArtifactPreview?
    ) async -> DiscussionOutcome {
        await core.publishDiscussion(
            groupId: groupId,
            title: title,
            body: body,
            attachment: attachment
        )
    }

    func publishHighlightsAndShare(
        artifact: ArtifactRecord,
        drafts: [HighlightDraft],
        targetGroupId: String
    ) async -> HighlightListOutcome {
        await core.publishHighlightsAndShare(
            artifact: artifact,
            drafts: drafts,
            targetGroupId: targetGroupId
        )
    }

    func publishHighlight(
        draft: HighlightDraft,
        artifact: ArtifactRecord
    ) async -> HighlightOutcome {
        await core.publishHighlight(draft: draft, artifact: artifact)
    }

    /// Re-share an existing highlight into a room as a kind:16 repost.
    /// `relayHint` may be empty — the core falls back to the Highlighter
    /// relay for the e-tag hint when so.
    func shareHighlightToRoom(
        highlightId: String,
        highlightAuthorPubkeyHex: String,
        highlightRelayUrl: String,
        targetGroupId: String
    ) async -> MutationOutcome {
        await core.shareHighlightToRoom(
            highlightId: highlightId,
            highlightAuthorPubkeyHex: highlightAuthorPubkeyHex,
            highlightRelayUrl: highlightRelayUrl,
            targetGroupId: targetGroupId
        )
    }

    // MARK: - Blossom (BUD-03, kind:10063)

    func getBlossomServers() async throws -> [String] {
        try await core.getBlossomServers()
    }

    func setBlossomServers(_ servers: [String]) async throws -> String {
        try await core.setBlossomServers(servers: servers)
    }

    func initDefaultBlossomServers() async throws {
        try await core.initDefaultBlossomServers()
    }

    // MARK: - Capture (Blossom upload + kind:20 picture)

    func uploadPhoto(
        bytes: Data,
        mime: String,
        width: UInt32,
        height: UInt32,
        alt: String
    ) async -> BlossomUploadOutcome {
        await core.uploadPhoto(
            bytes: bytes,
            mime: mime,
            width: width,
            height: height,
            alt: alt
        )
    }

    func publishPicture(_ draft: PictureDraft) async -> PictureOutcome {
        await core.publishPicture(draft: draft)
    }

    // MARK: - Relay config (NIP-65 read/write + NIP-78 rooms/indexer)

    func getRelays() async throws -> [RelayConfig] {
        try await core.getRelays()
    }

    func upsertRelay(_ cfg: RelayConfig) async throws {
        try await core.upsertRelay(cfg: cfg)
    }

    func removeRelay(_ url: String) async throws {
        try await core.removeRelay(url: url)
    }

    func setRelayRoles(
        url: String,
        read: Bool,
        write: Bool,
        rooms: Bool,
        indexer: Bool
    ) async throws {
        try await core.setRelayRoles(
            url: url,
            read: read,
            write: write,
            rooms: rooms,
            indexer: indexer
        )
    }

    // MARK: - Relay telemetry (PR 4)

    func getRelayDiagnostics() async throws -> [RelayDiagnostic] {
        try await core.getRelayDiagnostics()
    }

    func autoConnectedRelayConfig(url: String) -> RelayConfig {
        core.autoConnectedRelayConfig(url: url)
    }

    func subscribeRelayStatus() async -> SubscriptionOutcome {
        await core.subscribeRelayStatus()
    }

    func reconnectAll() async throws {
        try await core.reconnectAll()
    }

    func disconnectAll() async throws {
        try await core.disconnectAll()
    }

    func probeRelayNip11(_ url: String) async throws -> Nip11Document {
        try await core.probeRelayNip11(url: url)
    }

    func importRelaysFromNpub(_ npub: String) async throws -> [RelayConfig] {
        try await core.importRelaysFromNpub(npub: npub)
    }

    func getCacheStats() async throws -> CacheStats {
        try await core.getCacheStats()
    }

    private func publicRoomCandidateLimit(_ limit: UInt32) -> UInt32 {
        let expanded = limit > UInt32.max / 4 ? UInt32.max : limit * 4
        return max(limit, min(expanded, 512))
    }
}

private extension CommunitySummary {
    var isPublicOpenRoom: Bool {
        visibility == "public" && access == "open"
    }
}
