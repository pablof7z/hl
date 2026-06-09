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

    nonisolated func getOnboardingInterestProjection(selectedIds: [String]) -> OnboardingInterestProjection {
        core.getOnboardingInterestProjection(selectedIds: selectedIds)
    }

    nonisolated func toggleOnboardingInterestSelection(
        selectedIds: [String],
        interestId: String
    ) -> [String] {
        core.toggleOnboardingInterestSelection(selectedIds: selectedIds, interestId: interestId)
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

    nonisolated func getPodcastClipComposerProjection(
        segments: [TranscriptSegment],
        transcriptAvailable: Bool,
        clipStartSeconds: Double,
        clipEndSeconds: Double,
        durationSeconds: Double,
        selectedGroupId: String?,
        joinedCommunities: [CommunitySummary]
    ) -> PodcastClipComposerProjection {
        core.getPodcastClipComposerProjection(
            input: PodcastClipComposerInput(
                segments: segments,
                transcriptAvailable: transcriptAvailable,
                clipStartSeconds: clipStartSeconds,
                clipEndSeconds: clipEndSeconds,
                durationSeconds: durationSeconds,
                selectedGroupId: selectedGroupId,
                joinedCommunities: joinedCommunities
            )
        )
    }

    nonisolated func getPodcastListeningProjection(
        input: PodcastListeningProjectionInput
    ) -> PodcastListeningProjection {
        core.getPodcastListeningProjection(input: input)
    }

    nonisolated func getPodcastNowPlayingProjection(
        input: PodcastNowPlayingProjectionInput
    ) -> PodcastNowPlayingProjection {
        core.getPodcastNowPlayingProjection(input: input)
    }

    nonisolated func getPodcastClipReference(artifact: ArtifactRecord) -> PodcastClipReference {
        core.getPodcastClipReference(artifact: artifact)
    }

    nonisolated func clearPodcastClipSelection() -> PodcastClipSelection {
        core.clearPodcastClipSelection()
    }

    nonisolated func markPodcastClipIn(
        selection: PodcastClipSelection,
        currentTime: Double
    ) -> PodcastClipSelection {
        core.markPodcastClipIn(
            selection: selection,
            currentTime: currentTime
        )
    }

    nonisolated func markPodcastClipOut(
        selection: PodcastClipSelection,
        currentTime: Double
    ) -> PodcastClipSelection {
        core.markPodcastClipOut(
            selection: selection,
            currentTime: currentTime
        )
    }

    nonisolated func extendPodcastClipToSegment(
        selection: PodcastClipSelection,
        segment: TranscriptSegment
    ) -> PodcastClipSelection {
        core.extendPodcastClipToSegment(
            selection: selection,
            segment: segment
        )
    }

    nonisolated func setPodcastClipStart(
        selection: PodcastClipSelection,
        value: Double
    ) -> PodcastClipSelection {
        core.setPodcastClipStart(
            selection: selection,
            value: value
        )
    }

    nonisolated func setPodcastClipEnd(
        selection: PodcastClipSelection,
        value: Double,
        durationSeconds: Double
    ) -> PodcastClipSelection {
        core.setPodcastClipEnd(
            selection: selection,
            value: value,
            durationSeconds: durationSeconds
        )
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

    func getRoomHomeSnapshot(groupId: String) async -> RoomHomeSnapshot {
        await core.getRoomHomeSnapshot(groupId: groupId)
    }

    func getRecentBooks(limit: UInt32 = 24) async -> ArtifactListOutcome {
        await core.getRecentBooks(limit: limit)
    }

    func searchArtifacts(query: String, limit: UInt32 = 20) async -> ArtifactListOutcome {
        await core.searchArtifacts(query: query, limit: limit)
    }

    // MARK: - Search (local ndb + NIP-50 relay)

    nonisolated func projectSearchQuery(
        input: SearchQueryProjectionInput
    ) -> SearchQueryProjection {
        core.projectSearchQuery(input: input)
    }

    nonisolated func projectSearchSuggestions(
        input: SearchSuggestionsProjectionInput
    ) -> SearchSuggestionsProjection {
        core.projectSearchSuggestions(input: input)
    }

    nonisolated func projectSearchHighlightRow(
        input: SearchHighlightRowProjectionInput
    ) -> SearchHighlightRowProjection {
        core.projectSearchHighlightRow(input: input)
    }

    nonisolated func projectSearchCommunityRow(
        input: SearchCommunityRowProjectionInput
    ) -> SearchCommunityRowProjection {
        core.projectSearchCommunityRow(input: input)
    }

    nonisolated func projectSearchTextMatches(
        input: SearchTextMatchesProjectionInput
    ) -> SearchTextMatchesProjection {
        core.projectSearchTextMatches(input: input)
    }

    func getSearchResultsSnapshot(query: String) async -> SearchResultsSnapshot {
        await core.getSearchResultsSnapshot(query: query)
    }

    func getSearchArticleResultsSnapshot(query: String) async -> SearchArticleResultsSnapshot {
        await core.getSearchArticleResultsSnapshot(query: query)
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

    nonisolated func projectArticleBookmarkState(
        input: ArticleBookmarkStateProjectionInput
    ) -> ArticleBookmarkStateProjection {
        core.projectArticleBookmarkState(input: input)
    }

    nonisolated func projectArticleBookmarkChrome(
        input: ArticleBookmarkChromeProjectionInput
    ) -> ArticleBookmarkChromeProjection {
        core.projectArticleBookmarkChrome(input: input)
    }

    nonisolated func projectEventBookmarkState(
        input: EventBookmarkStateProjectionInput
    ) -> EventBookmarkStateProjection {
        core.projectEventBookmarkState(input: input)
    }

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

    nonisolated func projectCommentLikeState(
        input: CommentLikeStateProjectionInput
    ) -> CommentLikeStateProjection {
        core.projectCommentLikeState(input: input)
    }

    func toggleCommentLike(eventId: String, authorPubkeyHex: String) async -> BoolOutcome {
        await core.toggleCommentLike(eventId: eventId, authorPubkeyHex: authorPubkeyHex)
    }

    // MARK: - Event bookmarks (kind:10003 note bookmarks)

    func toggleEventBookmark(eventIdHex: String) async -> BoolOutcome {
        await core.toggleEventBookmark(eventIdHex: eventIdHex)
    }

    // MARK: - Bookmark sets (kind:30003/30004) + NIP-B0 (kind:39701)

    func getBookmarkSetArticles(record: BookmarkSetRecord) async -> ArticleListOutcome {
        await core.getBookmarkSetArticles(record: record)
    }

    func getBookmarkLibrarySnapshot() async -> BookmarkLibrarySnapshot {
        await core.getBookmarkLibrarySnapshot()
    }

    func getCurationMenuItems(address: String) async -> CurationMenuItemListOutcome {
        await core.getCurationMenuItems(address: address)
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

    nonisolated func projectBookmarkedArticleRow(
        input: BookmarkedArticleRowProjectionInput
    ) -> BookmarkedArticleRowProjection {
        core.projectBookmarkedArticleRow(input: input)
    }

    nonisolated func projectBookmarkLibrary(
        input: BookmarkLibraryProjectionInput
    ) -> BookmarkLibraryProjection {
        core.projectBookmarkLibrary(input: input)
    }

    nonisolated func projectBookmarkSetRow(
        input: BookmarkSetRowProjectionInput
    ) -> BookmarkSetRowProjection {
        core.projectBookmarkSetRow(input: input)
    }

    nonisolated func projectBookmarkSetDetail(
        input: BookmarkSetDetailProjectionInput
    ) -> BookmarkSetDetailProjection {
        core.projectBookmarkSetDetail(input: input)
    }

    nonisolated func projectCurationSetCreate(
        input: CurationSetCreateProjectionInput
    ) -> CurationSetCreateProjection {
        core.projectCurationSetCreate(input: input)
    }

    nonisolated func projectWebBookmarkRow(
        input: WebBookmarkRowProjectionInput
    ) -> WebBookmarkRowProjection {
        core.projectWebBookmarkRow(input: input)
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

    nonisolated func projectBookPickerQuery(
        input: BookPickerQueryProjectionInput
    ) -> BookPickerQueryProjection {
        core.projectBookPickerQuery(input: input)
    }

    nonisolated func projectIsbnManualPreview(
        input: IsbnManualPreviewProjectionInput
    ) -> IsbnManualPreviewProjection {
        core.projectIsbnManualPreview(input: input)
    }

    nonisolated func projectIsbnPreviewRequest(
        input: IsbnPreviewRequestProjectionInput
    ) -> IsbnPreviewRequestProjection {
        core.projectIsbnPreviewRequest(input: input)
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

    nonisolated func projectCaptureBookDisplay(
        input: CaptureBookDisplayProjectionInput
    ) -> CaptureBookDisplayProjection {
        core.projectCaptureBookDisplay(input: input)
    }

    nonisolated func projectCaptureCommunitySelection(
        input: CaptureCommunitySelectionProjectionInput
    ) -> CaptureCommunitySelectionProjection {
        core.projectCaptureCommunitySelection(input: input)
    }

    nonisolated func projectCaptureStash(
        input: CaptureStashProjectionInput
    ) -> CaptureStashProjection {
        core.projectCaptureStash(input: input)
    }

    nonisolated func projectCapturePublish(
        input: CapturePublishProjectionInput
    ) -> CapturePublishProjection {
        core.projectCapturePublish(input: input)
    }

    func publishCapture(input: CapturePublishInput) async -> StringOutcome {
        await core.publishCapture(input: input)
    }

    func buildPreviewFromUrl(_ url: String) async -> ArtifactPreviewOutcome {
        await core.buildPreviewFromUrl(url: url)
    }

    nonisolated func projectWebMetadataRequest(
        input: WebMetadataRequestProjectionInput
    ) -> WebMetadataRequestProjection {
        core.projectWebMetadataRequest(input: input)
    }

    func getWebMetadata(url: String) async -> WebMetadataOutcome {
        await core.getWebMetadata(url: url)
    }

    func getRoomDiscussionSnapshot(groupId: String) async -> RoomDiscussionSnapshot {
        await core.getRoomDiscussionSnapshot(groupId: groupId)
    }

    // MARK: - Chat (NIP-29 kind:9)

    func getChatPresenceSnapshot(groupId: String) async -> ChatPresenceSnapshot {
        await core.getChatPresenceSnapshot(groupId: groupId)
    }

    func getChatSnapshot(groupId: String, pageCount: UInt32) async -> ChatSnapshot {
        await core.getChatSnapshot(groupId: groupId, pageCount: pageCount)
    }

    nonisolated func projectChatComposer(
        input: ChatComposerProjectionInput
    ) -> ChatComposerProjection {
        core.projectChatComposer(input: input)
    }

    func publishChatMessageSnapshot(
        groupId: String,
        content: String,
        replyToEventId: String? = nil,
        pageCount: UInt32
    ) async -> ChatPublishSnapshotOutcome {
        await core.publishChatMessageSnapshot(
            groupId: groupId,
            content: content,
            replyToEventId: replyToEventId,
            pageCount: pageCount
        )
    }

    func subscribeRoomChat(groupId: String) async -> SubscriptionOutcome {
        await core.subscribeRoomChat(groupId: groupId)
    }

    // MARK: - Feedback (shake-to-share)

    func getFeedbackThreadsSnapshot(coordinate: String) async -> FeedbackThreadsSnapshot {
        await core.getFeedbackThreadsSnapshot(coordinate: coordinate)
    }

    func getFeedbackThreadSnapshot(rootEventId: String) async -> FeedbackThreadSnapshot {
        await core.getFeedbackThreadSnapshot(rootEventId: rootEventId)
    }

    nonisolated func projectFeedbackComposer(
        input: FeedbackComposerProjectionInput
    ) -> FeedbackComposerProjection {
        core.projectFeedbackComposer(input: input)
    }

    nonisolated func projectFeedbackThreadPresentation(
        thread: FeedbackThreadRecord
    ) -> FeedbackThreadPresentationProjection {
        core.projectFeedbackThreadPresentation(thread: thread)
    }

    nonisolated func projectFeedbackMessagePresentation(
        input: FeedbackMessagePresentationInput
    ) -> FeedbackMessagePresentationProjection {
        core.projectFeedbackMessagePresentation(input: input)
    }

    func publishFeedbackRootNoteSnapshot(
        coordinate: String,
        body: String
    ) async -> FeedbackRootPublishSnapshotOutcome {
        await core.publishFeedbackRootNoteSnapshot(
            coordinate: coordinate,
            body: body
        )
    }

    func publishFeedbackThreadReplySnapshot(
        coordinate: String,
        parentEventId: String,
        body: String
    ) async -> FeedbackReplyPublishSnapshotOutcome {
        await core.publishFeedbackThreadReplySnapshot(
            coordinate: coordinate,
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

    func getProfilePageSnapshot(pubkeyHex: String) async -> ProfilePageSnapshot {
        await core.getProfilePageSnapshot(pubkeyHex: pubkeyHex)
    }

    nonisolated func projectPublicKeyDisplay(
        input: PublicKeyDisplayProjectionInput
    ) -> PublicKeyDisplayProjection {
        core.projectPublicKeyDisplay(input: input)
    }

    nonisolated func projectSecretKeyDisplay(
        input: SecretKeyDisplayProjectionInput
    ) -> SecretKeyDisplayProjection {
        core.projectSecretKeyDisplay(input: input)
    }

    nonisolated func projectRelativeTimeLabel(
        input: RelativeTimeLabelInput
    ) -> RelativeTimeLabelProjection {
        core.projectRelativeTimeLabel(input: input)
    }

    nonisolated func projectProfileDisplay(
        input: ProfileDisplayProjectionInput
    ) -> ProfileDisplayProjection {
        core.projectProfileDisplay(input: input)
    }

    nonisolated func projectProfileDisplayWithLabel(
        input: ProfileDisplayWithLabelProjectionInput
    ) -> ProfileDisplayProjection {
        core.projectProfileDisplayWithLabel(input: input)
    }

    nonisolated func projectProfileHandle(
        input: ProfileDisplayProjectionInput
    ) -> ProfileDisplayProjection {
        core.projectProfileHandle(input: input)
    }

    nonisolated func projectProfileIdentity(
        input: ProfileIdentityProjectionInput
    ) -> ProfileIdentityProjection {
        core.projectProfileIdentity(input: input)
    }

    nonisolated func projectProfileRelationship(
        input: ProfileRelationshipProjectionInput
    ) -> ProfileRelationshipProjection {
        core.projectProfileRelationship(input: input)
    }

    nonisolated func projectProfileUpdate(
        input: ProfileUpdateProjectionInput
    ) -> ProfileUpdateProjection {
        core.projectProfileUpdate(input: input)
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

    nonisolated func projectNostrEntityArticleCard(
        input: NostrEntityArticleCardProjectionInput
    ) -> NostrEntityArticleCardProjection {
        core.projectNostrEntityArticleCard(input: input)
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

    func updateProfile(draft: ProfileUpdateDraft) async -> ProfileOutcome {
        await core.updateProfile(draft: draft)
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
        return await updateProfile(draft: draft)
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

    nonisolated func projectOnboardingCreateAccount(
        input: OnboardingCreateAccountProjectionInput
    ) -> OnboardingCreateAccountProjection {
        core.projectOnboardingCreateAccount(input: input)
    }

    nonisolated func projectOnboardingUsernameCheck(
        username: String
    ) -> OnboardingUsernameCheckProjection {
        core.projectOnboardingUsernameCheck(username: username)
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

    func getArticleReaderSnapshot(pubkeyHex: String, dTag: String) async -> ArticleReaderSnapshot {
        await core.getArticleReaderSnapshot(pubkeyHex: pubkeyHex, dTag: dTag)
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

    nonisolated func projectArticleReaderHeader(
        input: ArticleReaderHeaderProjectionInput
    ) -> ArticleReaderHeaderProjection {
        core.projectArticleReaderHeader(input: input)
    }

    nonisolated func projectArticleProfileCard(
        input: ArticleProfileCardProjectionInput
    ) -> ArticleProfileCardProjection {
        core.projectArticleProfileCard(input: input)
    }

    nonisolated func projectShareArticleTarget(
        input: ShareArticleTargetProjectionInput
    ) -> ShareArtifactTargetProjection {
        core.projectShareArticleTarget(input: input)
    }

    nonisolated func projectShareArtifactTarget(
        input: ShareArtifactTargetProjectionInput
    ) -> ShareArtifactTargetProjection {
        core.projectShareArtifactTarget(input: input)
    }

    nonisolated func projectShareWebReaderTarget(
        input: ShareWebReaderTargetProjectionInput
    ) -> ShareArtifactTargetProjection {
        core.projectShareWebReaderTarget(input: input)
    }

    nonisolated func projectShareHighlightTarget(
        input: ShareHighlightTargetProjectionInput
    ) -> ShareHighlightTargetProjection {
        core.projectShareHighlightTarget(input: input)
    }

    nonisolated func projectShareHighlightArticleTarget(
        input: ShareHighlightArticleTargetProjectionInput
    ) -> ShareArtifactTargetProjection? {
        core.projectShareHighlightArticleTarget(input: input)
    }

    nonisolated func projectShareQueueDrain(
        input: ShareQueueDrainProjectionInput
    ) -> ShareQueueDrainProjection {
        core.projectShareQueueDrain(input: input)
    }

    nonisolated func projectCommunityRow(
        input: CommunityRowProjectionInput
    ) -> CommunityRowProjection {
        core.projectCommunityRow(input: input)
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


    nonisolated func countArtifactComments(
        artifact: ArtifactRecord,
        commentsByReference: [CommentReferenceBucket]
    ) -> UInt32 {
        core.countArtifactComments(
            artifact: artifact,
            commentsByReference: commentsByReference
        )
    }

    nonisolated func projectDiscussionAttachment(
        input: DiscussionAttachmentProjectionInput
    ) -> DiscussionAttachmentProjection {
        core.projectDiscussionAttachment(input: input)
    }

    nonisolated func projectDiscussionComposer(
        input: DiscussionComposerProjectionInput
    ) -> DiscussionComposerProjection {
        core.projectDiscussionComposer(input: input)
    }

    nonisolated func projectCommentComposer(
        input: CommentComposerProjectionInput
    ) -> CommentComposerProjection {
        core.projectCommentComposer(input: input)
    }

    nonisolated func projectCommentThreadView(
        input: CommentThreadViewProjectionInput
    ) -> CommentThreadViewProjection {
        core.projectCommentThreadView(input: input)
    }

    nonisolated func projectCommentNodeChrome(
        input: CommentNodeChromeProjectionInput
    ) -> CommentNodeChromeProjection {
        core.projectCommentNodeChrome(input: input)
    }

    nonisolated func projectCommentToolbar(
        input: CommentToolbarProjectionInput
    ) -> CommentToolbarProjection {
        core.projectCommentToolbar(input: input)
    }

    nonisolated func projectCommentActionChrome(
        input: CommentActionChromeProjectionInput
    ) -> CommentActionChromeProjection {
        core.projectCommentActionChrome(input: input)
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

    func getCommentInteractionSnapshot(records: [CommentRecord]) async -> CommentInteractionSnapshot {
        await core.getCommentInteractionSnapshot(records: records)
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

    func startRoomExplorerFeaturedRooms() async -> MutationOutcome {
        await core.startRoomExplorerFeaturedRooms()
    }

    func getRoomExplorerSnapshot(joined: [CommunitySummary]) async -> RoomExplorerSnapshot {
        await core.getRoomExplorerSnapshot(joined: joined)
    }

    func getAllRooms(limit: UInt32 = 120) async -> CommunityListOutcome {
        await core.getAllRooms(limit: limit)
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

    nonisolated func projectCreateRoom(input: CreateRoomProjectionInput) -> CreateRoomProjection {
        core.projectCreateRoom(input: input)
    }

    nonisolated func projectRoomAvatar(input: RoomAvatarProjectionInput) -> RoomAvatarProjection {
        core.projectRoomAvatar(input: input)
    }

    nonisolated func projectRoomCoverCard(
        input: RoomCoverCardProjectionInput
    ) -> RoomCoverCardProjection {
        core.projectRoomCoverCard(input: input)
    }

    nonisolated func projectRoomRecommendationCard(
        input: RoomRecommendationCardProjectionInput
    ) -> RoomRecommendationCardProjection {
        core.projectRoomRecommendationCard(input: input)
    }

    nonisolated func projectRoomPreviewArtifacts(
        input: RoomPreviewArtifactsProjectionInput
    ) -> RoomPreviewArtifactsProjection {
        core.projectRoomPreviewArtifacts(input: input)
    }

    nonisolated func projectRoomPreviewHeader(
        input: RoomPreviewHeaderProjectionInput
    ) -> RoomPreviewHeaderProjection {
        core.projectRoomPreviewHeader(input: input)
    }

    nonisolated func projectRoomPreviewAction(
        input: RoomPreviewActionProjectionInput
    ) -> RoomPreviewActionProjection {
        core.projectRoomPreviewAction(input: input)
    }

    nonisolated func projectRoomLibraryArticleCard(
        input: RoomLibraryArticleCardProjectionInput
    ) -> RoomLibraryArticleCardProjection {
        core.projectRoomLibraryArticleCard(input: input)
    }

    nonisolated func projectRoomLibraryCardKind(
        input: RoomLibraryCardKindProjectionInput
    ) -> RoomLibraryCardKindProjection {
        core.projectRoomLibraryCardKind(input: input)
    }

    nonisolated func projectRoomLibraryBookCard(
        input: RoomLibraryBookCardProjectionInput
    ) -> RoomLibraryBookCardProjection {
        core.projectRoomLibraryBookCard(input: input)
    }

    nonisolated func projectRoomLibraryPodcastCard(
        input: RoomLibraryPodcastCardProjectionInput
    ) -> RoomLibraryPodcastCardProjection {
        core.projectRoomLibraryPodcastCard(input: input)
    }

    nonisolated func projectRoomLibraryGenericCard(
        input: RoomLibraryGenericCardProjectionInput
    ) -> RoomLibraryGenericCardProjection {
        core.projectRoomLibraryGenericCard(input: input)
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

    nonisolated func getRoomInviteProjection(
        input: RoomInviteProjectionInput
    ) -> RoomInviteProjection {
        core.getRoomInviteProjection(input: input)
    }

    nonisolated func getRoomInviteAvatarProjection(
        input: RoomInviteAvatarProjectionInput
    ) -> RoomInviteAvatarProjection {
        core.getRoomInviteAvatarProjection(input: input)
    }

    nonisolated func projectRoomInviteSelection(
        input: RoomInviteSelectionInput
    ) -> RoomInviteSelectionProjection {
        core.projectRoomInviteSelection(input: input)
    }

    nonisolated func projectRoomInviteSelectionChrome(
        input: RoomInviteSelectionChromeInput
    ) -> RoomInviteSelectionChromeProjection {
        core.projectRoomInviteSelectionChrome(input: input)
    }

    nonisolated func getRoomInviteSendResult(
        selected: [RoomInviteCandidate],
        failedPubkeys: [String]
    ) -> RoomInviteSendResultProjection {
        core.getRoomInviteSendResult(
            selected: selected,
            failedPubkeys: failedPubkeys
        )
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

    nonisolated func projectReadingFeedCard(
        input: ReadingFeedCardProjectionInput
    ) -> ReadingFeedCardProjection {
        core.projectReadingFeedCard(input: input)
    }

    // MARK: - Following Highlights

    func getFollowingHighlights(limit: UInt32 = 120) async -> HydratedHighlightListOutcome {
        await core.getFollowingHighlights(limit: limit)
    }

    nonisolated func projectHighlightGroupCard(
        input: HighlightGroupCardProjectionInput
    ) -> HighlightGroupCardProjection {
        core.projectHighlightGroupCard(input: input)
    }

    nonisolated func projectHighlightResourceHeader(
        input: HighlightResourceHeaderProjectionInput
    ) -> HighlightResourceHeaderProjection {
        core.projectHighlightResourceHeader(input: input)
    }

    nonisolated func projectHighlightDetailResource(
        input: HighlightDetailResourceProjectionInput
    ) -> HighlightDetailResourceProjection {
        core.projectHighlightDetailResource(input: input)
    }

    nonisolated func projectHighlightFeedContent(
        input: HighlightFeedContentProjectionInput
    ) -> HighlightFeedContentProjection {
        core.projectHighlightFeedContent(input: input)
    }

    nonisolated func projectHighlightDetailContent(
        input: HighlightDetailContentProjectionInput
    ) -> HighlightDetailContentProjection {
        core.projectHighlightDetailContent(input: input)
    }

    nonisolated func projectArticleReaderSelection(
        input: ArticleReaderSelectionProjectionInput
    ) -> ArticleReaderSelectionProjection {
        core.projectArticleReaderSelection(input: input)
    }

    nonisolated func projectArticleHighlightPublish(
        input: ArticleHighlightPublishProjectionInput
    ) -> ArticleHighlightPublishProjection {
        core.projectArticleHighlightPublish(input: input)
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

    func publishDiscussionFromComposer(
        input: DiscussionComposerPublishInput
    ) async -> DiscussionOutcome {
        await core.publishDiscussionFromComposer(input: input)
    }

    func publishPodcastClipHighlight(
        input: PodcastClipPublishInput
    ) async -> HighlightOutcome {
        await core.publishPodcastClipHighlight(input: input)
    }

    func publishPodcastComposerClip(
        input: PodcastClipComposerPublishInput
    ) async -> HighlightOutcome {
        await core.publishPodcastComposerClip(input: input)
    }

    func publishArticleReaderHighlight(
        article: ArticleRecord?,
        quote: String,
        note: String,
        context: String
    ) async -> HighlightOutcome {
        await core.publishArticleReaderHighlight(
            article: article,
            quote: quote,
            note: note,
            context: context
        )
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

    nonisolated func projectBlossomServerEntry(
        input: BlossomServerEntryProjectionInput
    ) -> BlossomServerEntryProjection {
        core.projectBlossomServerEntry(input: input)
    }

    nonisolated func projectBlossomServerList(
        input: BlossomServerListProjectionInput
    ) -> BlossomServerListProjection {
        core.projectBlossomServerList(input: input)
    }

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

    nonisolated func defaultAddRelayConfig() -> RelayConfig {
        core.defaultAddRelayConfig()
    }

    nonisolated func projectRelaySettings(
        configuredRelays: [RelayConfig],
        diagnostics: [RelayDiagnostic]
    ) -> RelaySettingsProjection {
        core.projectRelaySettings(
            configuredRelays: configuredRelays,
            diagnostics: diagnostics
        )
    }

    nonisolated func projectRelayRow(input: RelayRowProjectionInput) -> RelayRowProjection {
        core.projectRelayRow(input: input)
    }

    nonisolated func projectRelayDetail(input: RelayDetailProjectionInput) -> RelayDetailProjection {
        core.projectRelayDetail(input: input)
    }

    nonisolated func projectRelayRemove(input: RelayRemoveProjectionInput) -> RelayRemoveProjection {
        core.projectRelayRemove(input: input)
    }

    nonisolated func projectAddRelaySheet(input: AddRelaySheetProjectionInput) -> AddRelaySheetProjection {
        core.projectAddRelaySheet(input: input)
    }

    nonisolated func planRelayNip11Probes(input: RelayNip11ProbePlanInput) -> RelayNip11ProbePlan {
        core.planRelayNip11Probes(input: input)
    }

    nonisolated func finishRelayNip11Probe(inFlightUrls: [String], url: String) -> [String] {
        core.finishRelayNip11Probe(inFlightUrls: inFlightUrls, url: url)
    }

    nonisolated func defaultImportRelaySelection(relays: [RelayConfig]) -> [String] {
        core.defaultImportRelaySelection(relays: relays)
    }

    nonisolated func toggleImportRelaySelection(
        fetched: [RelayConfig],
        selectedUrls: [String],
        url: String
    ) -> [String] {
        core.toggleImportRelaySelection(
            fetched: fetched,
            selectedUrls: selectedUrls,
            url: url
        )
    }

    nonisolated func projectImportRelaysSource(
        input: ImportRelaysSourceProjectionInput
    ) -> ImportRelaysSourceProjection {
        core.projectImportRelaysSource(input: input)
    }

    nonisolated func projectImportRelays(input: ImportRelaysProjectionInput) -> ImportRelaysProjection {
        core.projectImportRelays(input: input)
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
