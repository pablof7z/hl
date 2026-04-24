import Foundation

/// Actor-isolated wrapper around the UniFFI-generated `HighlighterCore` so
/// Swift call sites get a clean `async throws` API without worrying about
/// FFI thread safety. Mirrors TENEX's `SafeTenexCore`.
actor SafeHighlighterCore {
    private let core: HighlighterCore

    init(core: HighlighterCore) {
        self.core = core
    }

    // MARK: - Auth

    func loginNsec(_ nsec: String) throws -> CurrentUser {
        try core.loginNsec(nsec: nsec)
    }

    func startNostrConnect(_ options: NostrConnectOptions) async throws -> String {
        try await core.startNostrConnect(options: options)
    }

    func pairBunker(_ uri: String) async throws -> CurrentUser {
        try await core.pairBunker(uri: uri)
    }

    func currentUser() -> CurrentUser? {
        core.currentUser()
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
        try await core.searchCommunities(query: query, limit: limit)
    }

    func searchProfiles(query: String, limit: UInt32 = 20) async throws -> [ProfileMetadata] {
        try await core.searchProfiles(query: query, limit: limit)
    }

    func getSearchRelays() async throws -> [String] {
        try await core.getSearchRelays()
    }

    func subscribeArticleSearch(query: String) async throws -> UInt64 {
        try await core.subscribeArticleSearch(query: query)
    }

    func lookupIsbn(_ isbn: String) async throws -> ArtifactPreview {
        try await core.lookupIsbn(isbn: isbn)
    }

    func buildPreviewFromUrl(_ url: String) async throws -> ArtifactPreview {
        try await core.buildPreviewFromUrl(url: url)
    }

    func getDiscussions(groupId: String, limit: UInt32 = 64) async throws -> [DiscussionRecord] {
        try await core.getDiscussions(groupId: groupId, limit: limit)
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
    ) async throws -> FeedbackEventRecord {
        try await core.publishFeedbackNote(
            coordinate: coordinate,
            agentPubkey: agentPubkey,
            parentEventId: parentEventId,
            body: body
        )
    }

    func subscribeFeedbackThreads(coordinate: String) async throws -> UInt64 {
        try await core.subscribeFeedbackThreads(coordinate: coordinate)
    }

    func subscribeFeedbackThread(rootEventId: String) async throws -> UInt64 {
        try await core.subscribeFeedbackThread(rootEventId: rootEventId)
    }

    // MARK: - Profile reads

    func getUserProfile(pubkeyHex: String) async throws -> ProfileMetadata? {
        try await core.getUserProfile(pubkeyHex: pubkeyHex)
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

    func getFeaturedRooms(curatorPubkeyHex: String) async throws -> [CommunitySummary] {
        try await core.getFeaturedRooms(curatorPubkeyHex: curatorPubkeyHex)
    }

    func getAllRooms(limit: UInt32 = 120) async throws -> [CommunitySummary] {
        try await core.getAllRooms(limit: limit)
    }

    func getNewRooms(limit: UInt32 = 24) async throws -> [CommunitySummary] {
        try await core.getNewRooms(limit: limit)
    }

    func getRoomsWithFriends(limit: UInt32 = 16) async throws -> [RoomRecommendation] {
        try await core.getRoomsWithFriends(limit: limit)
    }

    func getRoomsFromReadAuthors(limit: UInt32 = 16) async throws -> [RoomRecommendation] {
        try await core.getRoomsFromReadAuthors(limit: limit)
    }

    func requestJoinRoom(groupId: String) async throws -> String {
        try await core.requestJoinRoom(groupId: groupId)
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

    func subscribeFollowingReads() async throws -> UInt64 {
        try await core.subscribeFollowingReads()
    }

    func subscribeFollowingHighlights() async throws -> UInt64 {
        try await core.subscribeFollowingHighlights()
    }

    func subscribeJoinedCommunities() async throws -> UInt64 {
        try await core.subscribeJoinedCommunities()
    }

    func subscribeRoom(groupId: String) async throws -> UInt64 {
        try await core.subscribeRoom(groupId: groupId)
    }

    func subscribeRoomDiscussions(groupId: String) async throws -> UInt64 {
        try await core.subscribeRoomDiscussions(groupId: groupId)
    }

    func subscribeUserProfile(pubkeyHex: String) async throws -> UInt64 {
        try await core.subscribeUserProfile(pubkeyHex: pubkeyHex)
    }

    func subscribeArticle(pubkeyHex: String, dTag: String) async throws -> UInt64 {
        try await core.subscribeArticle(pubkeyHex: pubkeyHex, dTag: dTag)
    }

    func unsubscribe(_ handle: UInt64) {
        core.unsubscribe(handle: handle)
    }

    // MARK: - Writes

    func publishArtifact(
        preview: ArtifactPreview,
        groupId: String,
        note: String?
    ) async throws -> ArtifactRecord {
        try await core.publishArtifact(preview: preview, groupId: groupId, note: note)
    }

    func publishDiscussion(
        groupId: String,
        title: String,
        body: String,
        attachment: ArtifactPreview?
    ) async throws -> DiscussionRecord {
        try await core.publishDiscussion(
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
    ) async throws -> [HighlightRecord] {
        try await core.publishHighlightsAndShare(
            artifact: artifact,
            drafts: drafts,
            targetGroupId: targetGroupId
        )
    }

    func publishHighlight(
        draft: HighlightDraft,
        artifact: ArtifactRecord
    ) async throws -> HighlightRecord {
        try await core.publishHighlight(draft: draft, artifact: artifact)
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

    func signNip98Auth(url: String, method: String, payloadHash: String?) async throws -> String {
        try await core.signNip98Auth(url: url, method: method, payloadHash: payloadHash)
    }

    // MARK: - Capture (Blossom upload + kind:20 picture)

    func uploadPhoto(
        bytes: Data,
        mime: String,
        width: UInt32,
        height: UInt32,
        alt: String
    ) async throws -> BlossomUpload {
        try await core.uploadPhoto(
            bytes: bytes,
            mime: mime,
            width: width,
            height: height,
            alt: alt
        )
    }

    func publishPicture(_ draft: PictureDraft) async throws -> PictureRecord {
        try await core.publishPicture(draft: draft)
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

    func subscribeRelayStatus() async throws -> UInt64 {
        try await core.subscribeRelayStatus()
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
}
