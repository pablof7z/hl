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

    func lookupIsbn(_ isbn: String) async throws -> ArtifactPreview {
        try await core.lookupIsbn(isbn: isbn)
    }

    func buildPreviewFromUrl(_ url: String) async throws -> ArtifactPreview {
        try await core.buildPreviewFromUrl(url: url)
    }

    func getDiscussions(groupId: String, limit: UInt32 = 64) async throws -> [DiscussionRecord] {
        try await core.getDiscussions(groupId: groupId, limit: limit)
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

    func debugHighlightsReport() async throws -> String {
        try await core.debugHighlightsReport()
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
}
