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

    nonisolated func classifyLoginInput(_ input: String) -> LoginInputAction {
        core.classifyLoginInput(input: input)
    }

    func startDefaultNostrConnect(callback: String) async -> StringOutcome {
        await core.startDefaultNostrConnect(callback: callback)
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

    nonisolated func getOnboardingInterests() -> [OnboardingInterest] {
        core.getOnboardingInterests()
    }

    nonisolated func getOnboardingInterestSelection(selectedIds: [String]) -> OnboardingInterestSelection {
        core.getOnboardingInterestSelection(selectedIds: selectedIds)
    }

    func completeOnboardingInterests(selectedIds: [String]) async -> MutationOutcome {
        await core.completeOnboardingInterests(selectedIds: selectedIds)
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

    func getJoinedCommunities() async -> CommunityListOutcome {
        await core.getJoinedCommunities()
    }

    func joinedRoomNames(hostedOnRelay url: String) async -> StringListOutcome {
        await core.getJoinedRoomNamesForRelay(url: url)
    }

    func getArtifacts(groupId: String, limit: UInt32 = 32) async -> ArtifactListOutcome {
        await core.getArtifacts(groupId: groupId, limit: limit)
    }

    func getHighlights(groupId: String, limit: UInt32 = 64) async -> HydratedHighlightListOutcome {
        await core.getHighlights(groupId: groupId, limit: limit)
    }

    func getRecentBooks(limit: UInt32 = 24) async -> ArtifactListOutcome {
        await core.getRecentBooks(limit: limit)
    }

    func searchArtifacts(query: String, limit: UInt32 = 20) async -> ArtifactListOutcome {
        await core.searchArtifacts(query: query, limit: limit)
    }

    // MARK: - Search (local ndb + NIP-50 relay)

    func searchHighlights(query: String, limit: UInt32 = 20) async -> HighlightListOutcome {
        await core.searchHighlights(query: query, limit: limit)
    }

    func searchArticles(query: String, limit: UInt32 = 20) async -> ArticleListOutcome {
        await core.searchArticles(query: query, limit: limit)
    }

    func searchCommunities(query: String, limit: UInt32 = 20) async -> CommunityListOutcome {
        await core.searchCommunities(query: query, limit: limit)
    }

    func searchProfiles(query: String, limit: UInt32 = 20) async -> ProfileListOutcome {
        await core.searchProfiles(query: query, limit: limit)
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

    func getLikeSummaryForEvent(targetEventId: String, limit: UInt32) async -> ReactionSummaryOutcome {
        await core.getLikeSummaryForEvent(targetEventId: targetEventId, limit: limit)
    }

    func publishCommentLike(eventId: String, authorPubkeyHex: String) async -> ReactionOutcome {
        await core.publishCommentLike(eventId: eventId, authorPubkeyHex: authorPubkeyHex)
    }

    func unpublishReaction(reactionEventId: String) async -> StringOutcome {
        await core.unpublishReaction(reactionEventId: reactionEventId)
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

    func getBookmarkSetArticles(record: BookmarkSetRecord) async -> ArticleListOutcome {
        await core.getBookmarkSetArticles(record: record)
    }

    func getBookmarkedArticles(addresses: [String]) async -> ArticleListOutcome {
        await core.getBookmarkedArticles(addresses: addresses)
    }

    func getMyCurationSets() async -> BookmarkSetListOutcome {
        await core.getMyCurationSets()
    }

    func getCurationMenuItems(address: String) async -> CurationMenuItemListOutcome {
        await core.getCurationMenuItems(address: address)
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

    @discardableResult
    func toggleAddressInCurationSet(dTag: String, address: String) async -> BoolOutcome {
        await core.toggleAddressInCurationSet(dTag: dTag, address: address)
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

    func lookupIsbn(_ isbn: String) async -> ArtifactPreviewOutcome {
        await core.lookupIsbn(isbn: isbn)
    }

    nonisolated func normalizeIsbnInput(_ raw: String) -> String? {
        core.normalizeIsbnInput(raw: raw)
    }

    nonisolated func findExistingBookForIsbn(
        _ isbn: String,
        recents: [ArtifactRecord]
    ) -> ArtifactRecord? {
        core.findExistingBookForIsbn(isbn: isbn, recents: recents)
    }

    nonisolated func reconstructOcrMarkdown(_ lines: [OCRLine]) -> String {
        core.reconstructOcrMarkdown(lines: lines)
    }

    nonisolated func detectOcrActivePage(_ lines: [OCRLine]) -> OcrPageDetection? {
        core.detectOcrActivePage(lines: lines)
    }

    nonisolated func cropOcrLines(_ lines: [OCRLine], to pageRect: OcrRect) -> [OCRLine] {
        core.cropOcrLines(lines: lines, pageRect: pageRect)
    }

    nonisolated func defaultHighlightCropBox(
        highlightBoxes: [OcrRect],
        imageWidth: Double,
        imageHeight: Double,
        marginFraction: Double
    ) -> OcrRect? {
        core.defaultHighlightCropBox(
            highlightBoxes: highlightBoxes,
            imageWidth: imageWidth,
            imageHeight: imageHeight,
            marginFraction: marginFraction
        )
    }

    nonisolated func sanitizeHighlightCropBox(_ cropBox: OcrRect, fallback: OcrRect?) -> OcrRect {
        core.sanitizeHighlightCropBox(cropBox: cropBox, fallback: fallback)
    }

    nonisolated func selectableOcrWords(from lines: [OCRLine]) -> [OCRWord] {
        core.selectableOcrWords(lines: lines)
    }

    nonisolated func joinOcrQuote(_ words: [OCRWord]) -> String {
        core.joinOcrQuote(words: words)
    }

    nonisolated func ocrAltText(from markdown: String) -> String {
        core.ocrAltText(markdown: markdown)
    }

    nonisolated func buildEditedBookPreview(
        isbn: String,
        basePreview: ArtifactPreview?,
        title: String,
        author: String
    ) -> ArtifactPreviewOutcome {
        core.buildEditedBookPreview(
            isbn: isbn,
            basePreview: basePreview,
            title: title,
            author: author
        )
    }

    func buildPreviewFromUrl(_ url: String) async -> ArtifactPreviewOutcome {
        await core.buildPreviewFromUrl(url: url)
    }

    func getWebMetadata(url: String) async -> WebMetadataOutcome {
        await core.getWebMetadata(url: url)
    }

    func getDiscussions(groupId: String, limit: UInt32 = 64) async -> DiscussionListOutcome {
        await core.getDiscussions(groupId: groupId, limit: limit)
    }

    // MARK: - Chat (NIP-29 kind:9)

    func getChatMessages(groupId: String, limit: UInt32 = 200) async -> ChatMessageListOutcome {
        await core.getChatMessages(groupId: groupId, limit: limit)
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

    func getFeedbackThreads(coordinate: String) async -> FeedbackThreadListOutcome {
        await core.getFeedbackThreads(coordinate: coordinate)
    }

    func getFeedbackThreadEvents(rootEventId: String) async -> FeedbackEventListOutcome {
        await core.getFeedbackThreadEvents(rootEventId: rootEventId)
    }

    func getProjectFirstAgentPubkey(coordinate: String) async -> OptionalStringOutcome {
        await core.getProjectFirstAgentPubkey(coordinate: coordinate)
    }

    nonisolated func optimisticallyInsertFeedbackRootThread(
        threads: [FeedbackThreadRecord],
        rootEvent: FeedbackEventRecord
    ) -> [FeedbackThreadRecord] {
        core.optimisticallyInsertFeedbackRootThread(
            threads: threads,
            rootEvent: rootEvent
        )
    }

    nonisolated func upsertFeedbackThreadEvent(
        events: [FeedbackEventRecord],
        event: FeedbackEventRecord
    ) -> [FeedbackEventRecord] {
        core.upsertFeedbackThreadEvent(
            events: events,
            event: event
        )
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

    func getUserProfile(pubkeyHex: String) async -> ProfileOutcome {
        await core.getUserProfile(pubkeyHex: pubkeyHex)
    }

    nonisolated func decodeNostrEntity(_ input: String) -> NostrEntityRefOutcome {
        core.decodeNostrEntity(input: input)
    }

    nonisolated func nostrEntityInlineRender(entity: NostrEntityRef) -> NostrEntityInlineRender {
        core.nostrEntityInlineRender(entity: entity)
    }

    nonisolated func nostrEntityIdentityKey(entity: NostrEntityRef) -> String {
        core.nostrEntityIdentityKey(entity: entity)
    }

    nonisolated func tokenizeNostrContent(_ content: String) -> [NostrContentRun] {
        core.tokenizeNostrContent(content: content)
    }

    nonisolated func tokenizeNostrMarkdownInline(_ content: String) -> [NostrContentRun] {
        core.tokenizeNostrMarkdownInline(content: content)
    }

    nonisolated func standaloneNostrEntity(_ content: String) -> NostrEntityRef? {
        core.standaloneNostrEntity(content: content)
    }

    nonisolated func extractNostrEventRefs(_ content: String) -> [NostrEntityRef] {
        core.extractNostrEventRefs(content: content)
    }

    /// Mint a NIP-19 `nevent` for a highlight share URL. Relay hints are
    /// Rust-owned policy, not native view input.
    func encodeHighlightShareNevent(
        eventIdHex: String,
        authorPubkeyHex: String
    ) -> StringOutcome {
        core.encodeHighlightShareNevent(
            eventIdHex: eventIdHex,
            authorPubkeyHex: authorPubkeyHex
        )
    }

    func resolveNostrEntity(_ entity: NostrEntityRef) async -> NostrEntityEventOutcome {
        await core.resolveNostrEntity(entity: entity)
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
    ) async -> ProfileOutcome {
        let draft = ProfileUpdateDraft(
            name: name,
            displayName: displayName,
            about: about,
            picture: picture,
            banner: banner,
            nip05: nip05,
            website: website,
            lud16: lud16
        )
        return await core.updateProfile(draft: draft)
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

    func checkNip05Availability(name: String) async -> Nip05AvailabilityOutcome {
        await core.checkNip05Availability(name: name)
    }

    func registerNip05(name: String, domain: String) async -> StringOutcome {
        await core.registerNip05(name: name, domain: domain)
    }

    func getUserArticles(pubkeyHex: String, limit: UInt32 = 32) async -> ArticleListOutcome {
        await core.getUserArticles(pubkeyHex: pubkeyHex, limit: limit)
    }

    func getArticle(pubkeyHex: String, dTag: String) async -> ArticleOutcome {
        await core.getArticle(pubkeyHex: pubkeyHex, dTag: dTag)
    }

    func getArticleByAddress(address: String) async -> ArticleOutcome {
        await core.getArticleByAddress(address: address)
    }

    func getArticleAddressAuthor(address: String) async -> OptionalStringOutcome {
        await core.getArticleAddressAuthor(address: address)
    }

    nonisolated func getArticleReaderRoute(address: String) -> ArticleReaderRouteOutcome {
        core.getArticleReaderRoute(address: address)
    }

    nonisolated func getArticleReaderRouteForArticle(pubkeyHex: String, dTag: String) -> ArticleReaderRouteOutcome {
        core.getArticleReaderRouteForArticle(pubkeyHex: pubkeyHex, dTag: dTag)
    }

    nonisolated func getArticleArtifactPreview(article: ArticleRecord) -> ArtifactPreviewOutcome {
        core.getArticleArtifactPreview(article: article)
    }

    nonisolated func getArticleArtifactPreviewForAddress(address: String) -> ArtifactPreviewOutcome {
        core.getArticleArtifactPreviewForAddress(address: address)
    }

    nonisolated func getArticleArtifactRecord(article: ArticleRecord) -> ArtifactOutcome {
        core.getArticleArtifactRecord(article: article)
    }

    nonisolated func getUnpublishedArtifactRecord(preview: ArtifactPreview) -> ArtifactOutcome {
        core.getUnpublishedArtifactRecord(preview: preview)
    }

    nonisolated func getBookRoute(catalogId: String) -> BookRouteOutcome {
        core.getBookRoute(catalogId: catalogId)
    }

    nonisolated func getHighlightBookRoute(externalReference: String, artifactAddress: String) -> BookRouteOutcome {
        core.getHighlightBookRoute(externalReference: externalReference, artifactAddress: artifactAddress)
    }

    func getHighlightsForArticle(address: String, limit: UInt32 = 128) async -> HighlightListOutcome {
        await core.getHighlightsForArticle(address: address, limit: limit)
    }

    func getHighlightsForReference(
        tagName: String,
        tagValue: String,
        limit: UInt32 = 128
    ) async -> HighlightListOutcome {
        await core.getHighlightsForReference(tagName: tagName, tagValue: tagValue, limit: limit)
    }

    func getBookHighlights(catalogId: String, limit: UInt32 = 64) async -> HighlightListOutcome {
        await core.getBookHighlights(catalogId: catalogId, limit: limit)
    }

    nonisolated func getArticleUpdateAction(kind: UInt32) -> ArticleUpdateAction {
        core.getArticleUpdateAction(kind: kind)
    }

    nonisolated func insertUniqueHighlightFront(
        highlights: [HighlightRecord],
        highlight: HighlightRecord
    ) -> [HighlightRecord] {
        core.insertUniqueHighlightFront(
            highlights: highlights,
            highlight: highlight
        )
    }

    nonisolated func getProfileUpdateAction(kind: UInt32) -> ProfileUpdateAction {
        core.getProfileUpdateAction(kind: kind)
    }

    nonisolated func getArticleCommentScope(address: String) -> CommentScopeOutcome {
        core.getArticleCommentScope(address: address)
    }

    nonisolated func getArtifactCommentScope(preview: ArtifactPreview) -> CommentScopeOutcome {
        core.getArtifactCommentScope(preview: preview)
    }

    nonisolated func getArtifactReferenceTarget(
        artifact: ArtifactRecord
    ) -> ArtifactReferenceTarget? {
        core.getArtifactReferenceTarget(artifact: artifact)
    }

    nonisolated func getHighlightReferenceTarget(
        highlight: HighlightRecord
    ) -> HighlightReferenceTarget? {
        core.getHighlightReferenceTarget(highlight: highlight)
    }

    nonisolated func buildVisibleRoomLanes(
        artifacts: [ArtifactRecord],
        highlights: [HydratedHighlight],
        highlightsByReference: [HighlightReferenceBucket],
        commentsByReference: [CommentReferenceBucket]
    ) -> [RoomLane] {
        core.buildVisibleRoomLanes(
            artifacts: artifacts,
            highlights: highlights,
            highlightsByReference: highlightsByReference,
            commentsByReference: commentsByReference
        )
    }

    nonisolated func buildCommentThread(
        records: [CommentRecord],
        rootTagValue: String
    ) -> [CommentThreadNode] {
        core.buildCommentThread(
            records: records,
            rootTagValue: rootTagValue
        )
    }

    nonisolated func insertCommentAndBuildThread(
        records: [CommentRecord],
        comment: CommentRecord,
        rootTagValue: String
    ) -> CommentThreadProjection {
        core.insertCommentAndBuildThread(
            records: records,
            comment: comment,
            rootTagValue: rootTagValue
        )
    }

    nonisolated func upsertRoomArtifact(
        artifacts: [ArtifactRecord],
        artifact: ArtifactRecord
    ) -> [ArtifactRecord] {
        core.upsertRoomArtifact(
            artifacts: artifacts,
            artifact: artifact
        )
    }

    nonisolated func upsertRoomHighlight(
        highlights: [HydratedHighlight],
        highlight: HydratedHighlight
    ) -> [HydratedHighlight] {
        core.upsertRoomHighlight(
            highlights: highlights,
            highlight: highlight
        )
    }

    nonisolated func upsertHighlightReferenceBucket(
        bucket: [HighlightRecord],
        highlight: HighlightRecord
    ) -> [HighlightRecord] {
        core.upsertHighlightReferenceBucket(
            bucket: bucket,
            highlight: highlight
        )
    }

    nonisolated func countArtifactComments(
        artifact: ArtifactRecord,
        commentsByReference: [CommentReferenceBucket]
    ) -> UInt32 {
        core.countArtifactComments(
            artifact: artifact,
            commentsByReference: commentsByReference
        )
    }

    nonisolated func upsertRoomDiscussion(
        discussions: [DiscussionRecord],
        discussion: DiscussionRecord
    ) -> [DiscussionRecord] {
        core.upsertRoomDiscussion(
            discussions: discussions,
            discussion: discussion
        )
    }

    nonisolated func upsertChatMessage(
        messages: [ChatMessageRecord],
        message: ChatMessageRecord
    ) -> [ChatMessageRecord] {
        core.upsertChatMessage(
            messages: messages,
            message: message
        )
    }

    nonisolated func getHighlightCommentScope(eventIdHex: String) -> CommentScopeOutcome {
        core.getHighlightCommentScope(eventIdHex: eventIdHex)
    }

    nonisolated func getDiscussionCommentScope(eventIdHex: String) -> CommentScopeOutcome {
        core.getDiscussionCommentScope(eventIdHex: eventIdHex)
    }

    nonisolated func getWebCommentScope(url: String) -> CommentScopeOutcome {
        core.getWebCommentScope(url: url)
    }

    func getCommentsForScope(scope: CommentScope, limit: UInt32 = 128) async -> CommentListOutcome {
        await core.getCommentsForScope(scope: scope, limit: limit)
    }

    func publishCommentForScope(
        scope: CommentScope,
        parentEventId: String? = nil,
        content: String
    ) async -> CommentOutcome {
        await core.publishCommentForScope(scope: scope, parentEventId: parentEventId, content: content)
    }

    func getUserHighlights(pubkeyHex: String, limit: UInt32 = 64) async -> HighlightListOutcome {
        await core.getUserHighlights(pubkeyHex: pubkeyHex, limit: limit)
    }

    func getUserCommunities(pubkeyHex: String) async -> CommunityListOutcome {
        await core.getUserCommunities(pubkeyHex: pubkeyHex)
    }

    // MARK: - Rooms explorer

    func startRoomDiscovery() async {
        await core.startRoomDiscovery()
    }

    func startFriendsRoomsDiscovery() async -> MutationOutcome {
        await core.startFriendsRoomsDiscovery()
    }

    func startFeaturedRooms(curatorPubkeyHex: String) async -> MutationOutcome {
        await core.startFeaturedRooms(curatorPubkeyHex: curatorPubkeyHex)
    }

    func getRoomExplorerCuratorPubkey() async -> StringOutcome {
        await core.getRoomExplorerCuratorPubkey()
    }

    func startRoomExplorerFeaturedRooms() async -> MutationOutcome {
        await core.startRoomExplorerFeaturedRooms()
    }

    func getFeaturedRooms(curatorPubkeyHex: String) async -> CommunityListOutcome {
        await core.getFeaturedRooms(curatorPubkeyHex: curatorPubkeyHex)
    }

    func getAllRooms(limit: UInt32 = 120) async -> CommunityListOutcome {
        await core.getAllRooms(limit: limit)
    }

    func getNewRooms(limit: UInt32 = 24) async -> CommunityListOutcome {
        await core.getNewRooms(limit: limit)
    }

    nonisolated func excludeJoinedRooms(
        rooms: [CommunitySummary],
        joined: [CommunitySummary]
    ) -> [CommunitySummary] {
        core.excludeJoinedRooms(
            rooms: rooms,
            joined: joined
        )
    }

    nonisolated func searchRooms(
        rooms: [CommunitySummary],
        query: String
    ) -> [CommunitySummary] {
        core.searchRooms(
            rooms: rooms,
            query: query
        )
    }

    func getRoomsWithFriends(limit: UInt32 = 16) async -> RoomRecommendationListOutcome {
        await core.getRoomsWithFriends(limit: limit)
    }

    func getRoomsFromReadAuthors(limit: UInt32 = 16) async -> RoomRecommendationListOutcome {
        await core.getRoomsFromReadAuthors(limit: limit)
    }

    func requestJoinRoom(groupId: String, roomName: String) async -> StringOutcome {
        await core.requestJoinRoom(groupId: groupId, roomName: roomName)
    }

    func confirmPendingJoin(groupId: String) {
        core.confirmPendingJoin(groupId: groupId)
    }

    func createRoom(
        name: String,
        about: String,
        picture: String,
        visibility: RoomVisibility,
        access: RoomAccess
    ) async -> StringOutcome {
        await core.createRoom(
            name: name,
            about: about,
            picture: picture,
            visibility: visibility,
            access: access
        )
    }

    func addRoomMember(groupId: String, pubkeyHex: String) async -> StringOutcome {
        await core.addRoomMember(groupId: groupId, pubkeyHex: pubkeyHex)
    }

    func createRoomInviteCodes(groupId: String, count: UInt32) async -> StringListOutcome {
        await core.createRoomInviteCodes(groupId: groupId, count: count)
    }

    func getFollows() async -> StringListOutcome {
        await core.getFollows()
    }

    nonisolated func decodeNpub(_ input: String) -> StringOutcome {
        core.decodeNpub(input: input)
    }

    func isFollowing(targetPubkeyHex: String) async -> BoolOutcome {
        await core.isFollowing(targetPubkeyHex: targetPubkeyHex)
    }

    func setFollow(targetPubkeyHex: String, follow: Bool) async -> OptionalStringOutcome {
        await core.setFollow(targetPubkeyHex: targetPubkeyHex, follow: follow)
    }

    // MARK: - Following Reads

    func getFollowingReads(limit: UInt32 = 40) async -> ReadingFeedListOutcome {
        await core.getFollowingReads(limit: limit)
    }

    // MARK: - Following Highlights

    func getFollowingHighlights(limit: UInt32 = 120) async -> HydratedHighlightListOutcome {
        await core.getFollowingHighlights(limit: limit)
    }

    nonisolated func buildHomeFeedItems(
        highlights: [HydratedHighlight],
        reads: [ReadingFeedItem]
    ) -> [HomeFeedItem] {
        core.buildHomeFeedItems(
            highlights: highlights,
            reads: reads
        )
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

    func getBlossomServers() async -> StringListOutcome {
        await core.getBlossomServers()
    }

    func setBlossomServers(_ servers: [String]) async -> StringOutcome {
        await core.setBlossomServers(servers: servers)
    }

    func initDefaultBlossomServers() async -> MutationOutcome {
        await core.initDefaultBlossomServers()
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

    func getRelays() async -> RelayConfigListOutcome {
        await core.getRelays()
    }

    func upsertRelay(_ cfg: RelayConfig) async -> MutationOutcome {
        await core.upsertRelay(cfg: cfg)
    }

    func removeRelay(_ url: String) async -> MutationOutcome {
        await core.removeRelay(url: url)
    }

    func setRelayRoles(
        url: String,
        read: Bool,
        write: Bool,
        rooms: Bool,
        indexer: Bool
    ) async -> MutationOutcome {
        await core.setRelayRoles(
            url: url,
            read: read,
            write: write,
            rooms: rooms,
            indexer: indexer
        )
    }

    // MARK: - Relay telemetry (PR 4)

    func getRelayDiagnostics() async -> RelayDiagnosticListOutcome {
        await core.getRelayDiagnostics()
    }

    func autoConnectedRelayConfig(url: String) -> RelayConfig {
        core.autoConnectedRelayConfig(url: url)
    }

    func subscribeRelayStatus() async -> SubscriptionOutcome {
        await core.subscribeRelayStatus()
    }

    func reconnectAll() async -> MutationOutcome {
        await core.reconnectAll()
    }

    func disconnectAll() async -> MutationOutcome {
        await core.disconnectAll()
    }

    func probeRelayNip11(_ url: String) async -> Nip11DocumentOutcome {
        await core.probeRelayNip11(url: url)
    }

    func importRelaysFromNpub(_ npub: String) async -> RelayConfigListOutcome {
        await core.importRelaysFromNpub(npub: npub)
    }

    func getCacheStats() async -> CacheStatsOutcome {
        await core.getCacheStats()
    }
}
