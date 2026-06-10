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

    func loginNsec(_ nsec: String) -> AuthSessionSnapshot {
        core.loginNsec(nsec: nsec)
    }

    nonisolated func classifyLoginInput(_ input: String) -> LoginInputAction {
        core.classifyLoginInput(input: input)
    }

    func startDefaultNostrConnect(callback: String) async -> NostrConnectStartSnapshot {
        await core.startDefaultNostrConnect(callback: callback)
    }

    func restoreSessionSnapshot(nsec: String?, bunkerUri: String?) async -> AuthSessionRestoreSnapshot {
        await core.restoreSessionSnapshot(nsec: nsec, bunkerUri: bunkerUri)
    }

    func pairBunker(_ uri: String) async -> AuthSessionSnapshot {
        await core.pairBunker(uri: uri)
    }

    func generateAccount() -> AccountGenerationSnapshot {
        core.generateAccount()
    }

    func currentUser() -> CurrentUser? {
        core.currentUser()
    }

    func isOnboardingComplete() -> Bool {
        core.isOnboardingComplete()
    }

    func setOnboardingComplete(_ complete: Bool) -> MutationSnapshot {
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

    func completeOnboardingInterests(selectedIds: [String]) async -> MutationSnapshot {
        await core.completeOnboardingInterests(selectedIds: selectedIds)
    }

    func setWifiOnlyEnabled(_ enabled: Bool) async -> NetworkWifiOnlyPreferenceSnapshot {
        await core.setWifiOnlyEnabled(enabled: enabled)
    }

    nonisolated func getNetworkWifiOnlyPreferenceSnapshot() -> NetworkWifiOnlyPreferenceSnapshot {
        core.getNetworkWifiOnlyPreferenceSnapshot()
    }

    nonisolated func planPodcastPlaybackSession(
        input: PodcastPlaybackSessionInput
    ) -> PodcastPlaybackSessionPlan {
        core.planPodcastPlaybackSession(input: input)
    }

    func recordPodcastPlaybackPosition(
        artifact: ArtifactRecord,
        positionSeconds: Double
    ) -> MutationSnapshot {
        core.recordPodcastPlaybackPosition(
            input: PodcastPlaybackPositionInput(
                artifact: artifact,
                positionSeconds: positionSeconds
            )
        )
    }

    nonisolated func projectPodcastPlaybackSeek(
        input: PodcastPlaybackSeekInput
    ) -> PodcastPlaybackSeekProjection {
        core.projectPodcastPlaybackSeek(input: input)
    }

    nonisolated func projectPodcastPlaybackTick(
        input: PodcastPlaybackTickInput
    ) -> PodcastPlaybackTickProjection {
        core.projectPodcastPlaybackTick(input: input)
    }

    func getPodcastPlaybackRehydrationSnapshot(
        hasCurrentArtifact: Bool
    ) -> PodcastPlaybackRehydrationSnapshot {
        core.getPodcastPlaybackRehydrationSnapshot(hasCurrentArtifact: hasCurrentArtifact)
    }

    func loadPodcastTranscript(url: String) async -> PodcastTranscriptLoadSnapshot {
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

    nonisolated func projectWaveformCacheKey(
        input: WaveformCacheKeyProjectionInput
    ) -> WaveformCacheKeyProjection {
        core.projectWaveformCacheKey(input: input)
    }

    nonisolated func planWaveformPeaks(
        input: WaveformPeaksPlanInput
    ) -> WaveformPeaksPlan {
        core.planWaveformPeaks(input: input)
    }

    func getPodcastListeningClipsSnapshot(
        artifact: ArtifactRecord?,
        limit: UInt32 = 128
    ) async -> PodcastListeningClipsSnapshot {
        await core.getPodcastListeningClipsSnapshot(artifact: artifact, limit: limit)
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

    func downloadPodcastArtwork(url: String) async -> Data? {
        await core.downloadPodcastArtwork(url: url)
    }

    func prepareWhatsNew() async -> WhatsNewPresentationSnapshot {
        await core.prepareWhatsNew()
    }

    func markWhatsNewSeen(shippedAtUnixSeconds: UInt64) async -> MutationSnapshot {
        await core.markWhatsNewSeen(shippedAtUnixSeconds: shippedAtUnixSeconds)
    }

    // MARK: - Reads

    func getJoinedCommunities() async -> JoinedCommunitiesSnapshot {
        await core.getJoinedCommunities()
    }

    func getRelayHostedRoomsSnapshot(hostedOnRelay url: String) async -> RelayHostedRoomsSnapshot {
        await core.getRelayHostedRoomsSnapshot(url: url)
    }

    func getRoomHomeSnapshot(groupId: String) async -> RoomHomeSnapshot {
        await core.getRoomHomeSnapshot(groupId: groupId)
    }

    func getBookPickerSnapshot(
        query: String,
        recentLimit: UInt32 = 24,
        searchLimit: UInt32 = 20
    ) async -> BookPickerSnapshot {
        await core.getBookPickerSnapshot(
            query: query,
            recentLimit: recentLimit,
            searchLimit: searchLimit
        )
    }

    // MARK: - Search (local ndb + NIP-50 relay)

    nonisolated func projectSearchQuery(
        input: SearchQueryProjectionInput
    ) -> SearchQueryProjection {
        core.projectSearchQuery(input: input)
    }

    nonisolated func projectSearchSchedule(
        input: SearchScheduleInput
    ) -> SearchScheduleProjection {
        core.projectSearchSchedule(input: input)
    }

    nonisolated func projectSearchResultsApply(
        input: SearchResultsApplyInput
    ) -> SearchResultsApplyProjection {
        core.projectSearchResultsApply(input: input)
    }

    nonisolated func projectSearchRelayRefresh(
        input: SearchRelayRefreshInput
    ) -> SearchRelayRefreshProjection {
        core.projectSearchRelayRefresh(input: input)
    }

    nonisolated func projectSearchRelayStartResult(
        input: SearchRelayStartResultInput
    ) -> SearchRelayStartResultProjection {
        core.projectSearchRelayStartResult(input: input)
    }

    nonisolated func projectSearchRelayUpdate(
        input: SearchRelayUpdateInput
    ) -> SearchRelayUpdateProjection {
        core.projectSearchRelayUpdate(input: input)
    }

    nonisolated func projectSearchRelayArticlesApply(
        input: SearchRelayArticlesApplyInput
    ) -> SearchRelayArticlesApplyProjection {
        core.projectSearchRelayArticlesApply(input: input)
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

    func getSearchChromeSnapshot() async -> SearchChromeSnapshot {
        await core.getSearchChromeSnapshot()
    }

    func recordRecentSearchSnapshot(_ query: String) async -> SearchChromeSnapshot {
        await core.recordRecentSearchSnapshot(query: query)
    }

    func clearRecentSearchesSnapshot() async -> SearchChromeSnapshot {
        await core.clearRecentSearchesSnapshot()
    }

    func subscribeArticleSearch(query: String) async -> SubscriptionStartSnapshot {
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

    func getArticleBookmarksSnapshot() async -> ArticleBookmarksSnapshot {
        await core.getArticleBookmarksSnapshot()
    }

    func toggleArticleBookmarkSnapshot(address: String) async -> ArticleBookmarksSnapshot {
        await core.toggleArticleBookmarkSnapshot(address: address)
    }

    func subscribeBookmarks() async -> SubscriptionStartSnapshot {
        await core.subscribeBookmarks()
    }

    // MARK: - Comment interactions

    func toggleCommentLikeSnapshot(
        records: [CommentRecord],
        eventId: String,
        authorPubkeyHex: String
    ) async -> CommentInteractionMutationSnapshot {
        await core.toggleCommentLikeSnapshot(
            records: records,
            eventId: eventId,
            authorPubkeyHex: authorPubkeyHex
        )
    }

    func toggleCommentBookmarkSnapshot(
        records: [CommentRecord],
        eventIdHex: String
    ) async -> CommentInteractionMutationSnapshot {
        await core.toggleCommentBookmarkSnapshot(records: records, eventIdHex: eventIdHex)
    }

    // MARK: - Bookmark sets (kind:30003/30004) + NIP-B0 (kind:39701)

    func getBookmarkSetDetailSnapshot(record: BookmarkSetRecord) async -> BookmarkSetDetailSnapshot {
        await core.getBookmarkSetDetailSnapshot(record: record)
    }

    func getBookmarkLibrarySnapshot() async -> BookmarkLibrarySnapshot {
        await core.getBookmarkLibrarySnapshot()
    }

    func getCurationMenuSnapshot(address: String) async -> CurationMenuSnapshot {
        await core.getCurationMenuSnapshot(address: address)
    }

    func createCurationSetWithAddressSnapshot(
        title: String,
        address: String
    ) async -> CurationMenuSnapshot {
        await core.createCurationSetWithAddressSnapshot(title: title, address: address)
    }

    func toggleCurationMenuItemSnapshot(
        dTag: String,
        address: String
    ) async -> CurationMenuSnapshot {
        await core.toggleCurationMenuItemSnapshot(dTag: dTag, address: address)
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

    func subscribeBookmarkSets() async -> SubscriptionStartSnapshot {
        await core.subscribeBookmarkSets()
    }

    func subscribeFollowingCurationSets() async -> SubscriptionStartSnapshot {
        await core.subscribeFollowingCurationSets()
    }

    func subscribeWebBookmarks() async -> SubscriptionStartSnapshot {
        await core.subscribeWebBookmarks()
    }

    func lookupIsbn(_ isbn: String) async -> IsbnPreviewLookupSnapshot {
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
    ) -> EditedBookPreviewProjection {
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

    nonisolated func projectCaptureUpload(
        input: CaptureUploadProjectionInput
    ) -> CaptureUploadProjection {
        core.projectCaptureUpload(input: input)
    }

    func publishCapture(input: CapturePublishInput) async -> CapturePublishSnapshot {
        await core.publishCapture(input: input)
    }

    func publishShareQueueItem(_ item: ShareQueueItem) async -> ShareQueueAttempt {
        await core.publishShareQueueItem(item: item)
    }

    nonisolated func projectWebMetadataRequest(
        input: WebMetadataRequestProjectionInput
    ) -> WebMetadataRequestProjection {
        core.projectWebMetadataRequest(input: input)
    }

    func getWebMetadata(url: String) async -> WebMetadata? {
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

    nonisolated func projectChatLoadMore(
        input: ChatLoadMoreProjectionInput
    ) -> ChatLoadMoreProjection {
        core.projectChatLoadMore(input: input)
    }

    nonisolated func projectChatActivityReload(
        input: ChatActivityReloadProjectionInput
    ) -> ChatActivityReloadProjection {
        core.projectChatActivityReload(input: input)
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
    ) async -> ChatPublishSnapshot {
        await core.publishChatMessageSnapshot(
            groupId: groupId,
            content: content,
            replyToEventId: replyToEventId,
            pageCount: pageCount
        )
    }

    func subscribeRoomChat(groupId: String) async -> SubscriptionStartSnapshot {
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
    ) async -> FeedbackRootPublishSnapshot {
        await core.publishFeedbackRootNoteSnapshot(
            coordinate: coordinate,
            body: body
        )
    }

    func publishFeedbackThreadReplySnapshot(
        coordinate: String,
        parentEventId: String,
        body: String
    ) async -> FeedbackReplyPublishSnapshot {
        await core.publishFeedbackThreadReplySnapshot(
            coordinate: coordinate,
            parentEventId: parentEventId,
            body: body
        )
    }

    func subscribeFeedbackThreads(coordinate: String) async -> SubscriptionStartSnapshot {
        await core.subscribeFeedbackThreads(coordinate: coordinate)
    }

    func subscribeFeedbackThread(rootEventId: String) async -> SubscriptionStartSnapshot {
        await core.subscribeFeedbackThread(rootEventId: rootEventId)
    }

    // MARK: - Profile reads

    func getUserProfile(pubkeyHex: String) async -> ProfileMetadata? {
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

    nonisolated func projectSessionStorageWrite(
        input: SessionStorageWriteInput
    ) -> SessionStorageWriteSnapshot {
        core.projectSessionStorageWrite(input: input)
    }

    nonisolated func currentSecretKeySettingsSnapshot(
        isRevealed: Bool
    ) -> SecretKeySettingsSnapshot {
        core.currentSecretKeySettingsSnapshot(isRevealed: isRevealed)
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

    nonisolated func projectProfileFollowAction(
        relationship: ProfileRelationshipProjection,
        input: ProfileFollowActionInput
    ) -> ProfileFollowActionProjection {
        core.projectProfileFollowAction(relationship: relationship, input: input)
    }

    func applyProfileFollowMutation(
        input: ProfileFollowMutationInput
    ) async -> ProfileFollowMutationSnapshot {
        await core.applyProfileFollowMutation(input: input)
    }

    nonisolated func projectProfileUpdate(
        input: ProfileUpdateProjectionInput
    ) -> ProfileUpdateProjection {
        core.projectProfileUpdate(input: input)
    }

    nonisolated func decodeNostrEntity(_ input: String) -> NostrEntityRefSnapshot {
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

    /// Project the public highlight share URL. Relay hints and route format
    /// are Rust-owned policy, not native view input.
    func getHighlightShareUrlSnapshot(
        eventIdHex: String,
        authorPubkeyHex: String
    ) -> HighlightShareUrlSnapshot {
        core.getHighlightShareUrlSnapshot(
            eventIdHex: eventIdHex,
            authorPubkeyHex: authorPubkeyHex
        )
    }

    func resolveNostrEntity(_ entity: NostrEntityRef) async -> NostrEntityResolutionSnapshot {
        await core.resolveNostrEntity(entity: entity)
    }

    func subscribeNostrEntity(_ entity: NostrEntityRef) async -> SubscriptionStartSnapshot {
        await core.subscribeNostrEntity(entity: entity)
    }

    func updateProfile(draft: ProfileUpdateDraft) async -> ProfileUpdateSnapshot {
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
    ) async -> ProfileUpdateSnapshot {
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

    func checkNip05Availability(name: String) async -> Nip05AvailabilitySnapshot {
        await core.checkNip05Availability(name: name)
    }

    func registerNip05(name: String, domain: String) async -> Nip05RegistrationSnapshot {
        await core.registerNip05(name: name, domain: domain)
    }

    func getArticleReaderSnapshot(pubkeyHex: String, dTag: String) async -> ArticleReaderSnapshot {
        await core.getArticleReaderSnapshot(pubkeyHex: pubkeyHex, dTag: dTag)
    }

    nonisolated func projectArticleReaderSnapshot(
        input: ArticleReaderSnapshotApplyInput
    ) -> ArticleReaderSnapshotProjection {
        core.projectArticleReaderSnapshot(input: input)
    }

    nonisolated func projectArticleReaderPublishResult(
        input: ArticleReaderPublishResultInput
    ) -> ArticleReaderPublishResultProjection {
        core.projectArticleReaderPublishResult(input: input)
    }

    func getArticleByAddress(address: String) async -> ArticleRecord? {
        await core.getArticleByAddress(address: address)
    }

    func getArticleAddressAuthor(address: String) async -> String? {
        await core.getArticleAddressAuthor(address: address)
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

    func buildWebReaderShareTarget(url: String) async -> ShareWebReaderTargetSnapshot {
        await core.buildWebReaderShareTarget(url: url)
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

    nonisolated func getBookRoute(catalogId: String) -> BookRoute? {
        core.getBookRoute(catalogId: catalogId)
    }

    nonisolated func getHighlightBookRoute(externalReference: String, artifactAddress: String) -> BookRoute? {
        core.getHighlightBookRoute(externalReference: externalReference, artifactAddress: artifactAddress)
    }

    func getBookDetailSnapshot(catalogId: String, limit: UInt32 = 64) async -> BookDetailSnapshot {
        await core.getBookDetailSnapshot(catalogId: catalogId, limit: limit)
    }

    nonisolated func getProfileUpdateAction(kind: UInt32) -> ProfileUpdateAction {
        core.getProfileUpdateAction(kind: kind)
    }

    nonisolated func getArticleCommentScope(address: String) -> CommentScopeSnapshot {
        core.getArticleCommentScope(address: address)
    }

    nonisolated func getArtifactCommentScope(preview: ArtifactPreview) -> CommentScopeSnapshot {
        core.getArtifactCommentScope(preview: preview)
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

    nonisolated func getHighlightCommentScope(eventIdHex: String) -> CommentScopeSnapshot {
        core.getHighlightCommentScope(eventIdHex: eventIdHex)
    }

    nonisolated func getDiscussionCommentScope(eventIdHex: String) -> CommentScopeSnapshot {
        core.getDiscussionCommentScope(eventIdHex: eventIdHex)
    }

    nonisolated func getWebCommentScope(url: String) -> CommentScopeSnapshot {
        core.getWebCommentScope(url: url)
    }

    func getCommentThreadSnapshot(
        scope: CommentScope,
        limit: UInt32 = 256
    ) async -> CommentThreadSnapshot {
        await core.getCommentThreadSnapshot(scope: scope, limit: limit)
    }

    func publishCommentForScopeSnapshot(
        scope: CommentScope,
        parentEventId: String? = nil,
        content: String,
        limit: UInt32 = 256
    ) async -> CommentPublishSnapshot {
        await core.publishCommentForScopeSnapshot(
            scope: scope,
            parentEventId: parentEventId,
            content: content,
            limit: limit
        )
    }

    // MARK: - Rooms explorer

    func startRoomDiscovery() async {
        await core.startRoomDiscovery()
    }

    func startFriendsRoomsDiscovery() async -> MutationSnapshot {
        await core.startFriendsRoomsDiscovery()
    }

    func startRoomExplorerFeaturedRooms() async -> MutationSnapshot {
        await core.startRoomExplorerFeaturedRooms()
    }

    func getRoomExplorerSnapshot(joined: [CommunitySummary]) async -> RoomExplorerSnapshot {
        await core.getRoomExplorerSnapshot(joined: joined)
    }

    func getRoomBrowseSnapshot(query: String, limit: UInt32 = 120) async -> RoomBrowseSnapshot {
        await core.getRoomBrowseSnapshot(query: query, limit: limit)
    }

    func requestJoinRoom(groupId: String, roomName: String) async -> JoinRoomRequestSnapshot {
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
    ) async -> CreateRoomPublishSnapshot {
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

    func getRoomShareLinkSnapshot(groupId: String) async -> RoomShareLinkSnapshot {
        await core.getRoomShareLinkSnapshot(groupId: groupId)
    }

    func getRoomInviteSnapshot(
        input: RoomInviteSnapshotInput
    ) async -> RoomInviteSnapshot {
        await core.getRoomInviteSnapshot(input: input)
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

    func sendRoomInvites(
        groupId: String,
        selected: [RoomInviteCandidate],
    ) async -> RoomInviteSendResultProjection {
        await core.sendRoomInvites(groupId: groupId, selected: selected)
    }

    // MARK: - Home Feed

    func getHomeFeedSnapshot(
        highlightLimit: UInt32 = 120,
        readLimit: UInt32 = 40
    ) async -> HomeFeedSnapshot {
        await core.getHomeFeedSnapshot(highlightLimit: highlightLimit, readLimit: readLimit)
    }

    nonisolated func projectReadingFeedCard(
        input: ReadingFeedCardProjectionInput
    ) -> ReadingFeedCardProjection {
        core.projectReadingFeedCard(input: input)
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

    // MARK: - Subscriptions

    func subscribeFollowingReads() async -> SubscriptionStartSnapshot {
        await core.subscribeFollowingReads()
    }

    func subscribeFollowingHighlights() async -> SubscriptionStartSnapshot {
        await core.subscribeFollowingHighlights()
    }

    func subscribeJoinedCommunities() async -> SubscriptionStartSnapshot {
        await core.subscribeJoinedCommunities()
    }

    func subscribeRoom(groupId: String) async -> SubscriptionStartSnapshot {
        await core.subscribeRoom(groupId: groupId)
    }

    func subscribeRoomDiscussions(groupId: String) async -> SubscriptionStartSnapshot {
        await core.subscribeRoomDiscussions(groupId: groupId)
    }

    func subscribeUserProfile(pubkeyHex: String) async -> SubscriptionStartSnapshot {
        await core.subscribeUserProfile(pubkeyHex: pubkeyHex)
    }

    func subscribeArticle(pubkeyHex: String, dTag: String) async -> SubscriptionStartSnapshot {
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
    ) async -> ArtifactPublishSnapshot {
        await core.publishArtifact(preview: preview, groupId: groupId, note: note)
    }

    func publishDiscussion(
        groupId: String,
        title: String,
        body: String,
        attachment: ArtifactPreview?
    ) async -> DiscussionPublishSnapshot {
        await core.publishDiscussion(
            groupId: groupId,
            title: title,
            body: body,
            attachment: attachment
        )
    }

    func publishDiscussionFromComposer(
        input: DiscussionComposerPublishInput
    ) async -> DiscussionPublishSnapshot {
        await core.publishDiscussionFromComposer(input: input)
    }

    func publishPodcastClipHighlight(
        input: PodcastClipPublishInput
    ) async -> PodcastClipPublishSnapshot {
        await core.publishPodcastClipHighlight(input: input)
    }

    func publishPodcastComposerClip(
        input: PodcastClipComposerPublishInput
    ) async -> PodcastClipPublishSnapshot {
        await core.publishPodcastComposerClip(input: input)
    }

    func publishArticleReaderHighlightSnapshot(
        pubkeyHex: String,
        dTag: String,
        article: ArticleRecord?,
        quote: String,
        note: String,
        context: String
    ) async -> ArticleReaderHighlightPublishSnapshot {
        await core.publishArticleReaderHighlightSnapshot(
            pubkeyHex: pubkeyHex,
            dTag: dTag,
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
    ) async -> MutationSnapshot {
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

    func getBlossomServerSettingsSnapshot() async -> BlossomServerSettingsSnapshot {
        await core.getBlossomServerSettingsSnapshot()
    }

    func setBlossomServerSettings(_ servers: [String]) async -> BlossomServerSettingsMutationSnapshot {
        await core.setBlossomServerSettings(servers: servers)
    }

    func initDefaultBlossomServers() async -> MutationSnapshot {
        await core.initDefaultBlossomServers()
    }

    // MARK: - Capture (Blossom upload + kind:20 picture)

    func uploadPhoto(
        bytes: Data,
        mime: String,
        width: UInt32,
        height: UInt32,
        alt: String
    ) async -> BlossomUploadSnapshot {
        await core.uploadPhoto(
            bytes: bytes,
            mime: mime,
            width: width,
            height: height,
            alt: alt
        )
    }

    // MARK: - Relay config (NIP-65 read/write + NIP-78 rooms/indexer)

    func getNetworkSettingsSnapshot(previousRelays: [RelayConfig]) async -> NetworkSettingsSnapshot {
        await core.getNetworkSettingsSnapshot(previousRelays: previousRelays)
    }

    func upsertRelay(_ cfg: RelayConfig) async -> NetworkSettingsMutationSnapshot {
        await core.upsertRelay(cfg: cfg)
    }

    func removeRelay(_ url: String) async -> NetworkSettingsMutationSnapshot {
        await core.removeRelay(url: url)
    }

    func setRelayRoles(
        url: String,
        read: Bool,
        write: Bool,
        rooms: Bool,
        indexer: Bool
    ) async -> NetworkSettingsMutationSnapshot {
        await core.setRelayRoles(
            url: url,
            read: read,
            write: write,
            rooms: rooms,
            indexer: indexer
        )
    }

    // MARK: - Relay telemetry (PR 4)

    nonisolated func projectNetworkDiagnosticsSnapshot(
        configuredRelays: [RelayConfig],
        diagnostics: [RelayDiagnostic]
    ) -> NetworkDiagnosticsSnapshot {
        core.projectNetworkDiagnosticsSnapshot(
            configuredRelays: configuredRelays,
            diagnostics: diagnostics
        )
    }

    func autoConnectedRelayConfig(url: String) -> RelayConfig {
        core.autoConnectedRelayConfig(url: url)
    }

    nonisolated func defaultAddRelayConfig() -> RelayConfig {
        core.defaultAddRelayConfig()
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

    func subscribeRelayStatus() async -> SubscriptionStartSnapshot {
        await core.subscribeRelayStatus()
    }

    func reconnectAll() async -> NetworkSettingsMutationSnapshot {
        await core.reconnectAll()
    }

    func disconnectAll() async -> NetworkSettingsMutationSnapshot {
        await core.disconnectAll()
    }

    func refreshRelayConnectionsForForeground() async -> NetworkSettingsMutationSnapshot {
        await core.refreshRelayConnectionsForForeground()
    }

    func applyNetworkPathStatus(isWifi: Bool) async -> NetworkPathPolicySnapshot {
        await core.applyNetworkPathStatus(isWifi: isWifi)
    }

    func probeRelayNip11Snapshot(_ url: String) async -> RelayNip11ProbeSnapshot {
        await core.probeRelayNip11Snapshot(url: url)
    }

    func importRelaysFromNpubSnapshot(_ npub: String) async -> ImportRelaysFetchSnapshot {
        await core.importRelaysFromNpubSnapshot(npub: npub)
    }

    func getNetworkCacheStatsSnapshot() async -> NetworkCacheStatsSnapshot {
        await core.getNetworkCacheStatsSnapshot()
    }
}
