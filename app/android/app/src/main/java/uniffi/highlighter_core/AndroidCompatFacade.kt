@file:Suppress("unused", "MemberVisibilityCanBePrivate")

package uniffi.highlighter_core

import android.util.Log
import java.util.concurrent.atomic.AtomicBoolean
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

private const val TAG = "HlCompatFacade"

enum class HighlighterConnectionState {
    CONNECTING,
    ONLINE,
    OFFLINE,
}

data class WebMetadata(
    val title: String = "",
    val url: String = "",
    val siteName: String = "",
    val author: String = "",
    val image: String = "",
    val favicon: String = "",
)

data class HighlighterProfileViewSnapshot(
    val pubkeyHex: String = "",
    val profile: ProfileMetadata? = null,
    val isFollowing: Boolean = false,
    val communities: List<CommunitySummary> = emptyList(),
    val viewerPubkeyHex: String? = null,
    val isOwnProfile: Boolean = false,
    val isMutatingFollow: Boolean = false,
    val isLoading: Boolean = false,
    val errorMessage: String? = null,
    val articles: List<ArticleRecord> = emptyList(),
    val highlights: List<HighlightRecord> = emptyList(),
) {
    val articleCount: ULong get() = articles.size.toULong()
    val highlightCount: ULong get() = highlights.size.toULong()
    val communityCount: ULong get() = communities.size.toULong()
}

data class HighlighterArticleReaderSnapshot(
    val address: String = "",
    val article: ArticleRecord? = null,
    val authorProfile: ProfileMetadata? = null,
    val highlights: List<HighlightRecord> = emptyList(),
    val isLoading: Boolean = false,
    val errorMessage: String? = null,
    val isPublishingHighlight: Boolean = false,
) {
    val highlightCount: ULong get() = highlights.size.toULong()
}

data class HighlighterShareComposerSnapshot(
    val isPublishing: Boolean = false,
    val publishedGroupId: String? = null,
    val errorMessage: String? = null,
)

data class HighlighterWhatsNewEntry(
    val shippedAt: String,
    val shippedAtUnix: ULong,
    val lines: List<String>,
)

data class HighlighterWhatsNewSnapshot(
    val entries: List<HighlighterWhatsNewEntry> = emptyList(),
    val shouldPresent: Boolean = false,
)

data class HighlighterAppConfig(
    val dataDir: String,
    val visibleLimit: UInt = 250u,
    val emitHz: UInt = 30u,
)

sealed class HighlighterSessionCredential {
    data class Nsec(val nsec: String) : HighlighterSessionCredential()
    data class BunkerUri(val uri: String) : HighlighterSessionCredential()
    data class Nip55SignerPackage(val signerPackage: String) : HighlighterSessionCredential()
}

sealed class HighlighterSignerRequestDrain {
    data class Request(val requestJson: String) : HighlighterSignerRequestDrain()
    data object Idle : HighlighterSignerRequestDrain()
    data object Closed : HighlighterSignerRequestDrain()
}

fun initPlatformLogging() = Unit

interface HighlighterAppReconciler {
    fun onState(state: HighlighterAppState)
    fun onPersistSessionCredential(credential: HighlighterSessionCredential) {}
    fun onClearSessionCredentials() {}
    fun onOpenExternalUrl(url: String) {}
}

data class HighlighterProfile(
    val pubkeyHex: String,
    val metadata: ProfileMetadata,
)

data class HighlighterWebMetadata(
    val url: String,
    val metadata: WebMetadata,
)

data class HighlighterIsbnPreview(
    val isbn: String,
    val preview: ArtifactPreview,
)

enum class HighlighterUsernameStatus {
    IDLE,
    CHECKING,
    AVAILABLE,
    TAKEN,
    INVALID,
    ERROR,
}

data class HighlighterAuthSnapshot(
    val isSigningIn: Boolean = false,
    val errorMessage: String? = null,
    val nostrconnectUri: String? = null,
)

data class HighlighterCreateAccountSnapshot(
    val displayName: String = "",
    val username: String = "",
    val usernameIdentifier: String = "",
    val usernameStatus: HighlighterUsernameStatus = HighlighterUsernameStatus.IDLE,
    val canSubmit: Boolean = false,
    val isCreating: Boolean = false,
    val errorMessage: String? = null,
)

data class HighlighterOnboardingSnapshot(
    val isComplete: Boolean = true,
    val interests: List<HighlighterOnboardingInterest> = emptyList(),
    val minimumSelectionCount: UByte = 0u,
    val remainingSelectionCount: UByte = 0u,
    val canFinish: Boolean = true,
    val isFinishing: Boolean = false,
)

data class HighlighterOnboardingInterest(
    val id: String,
    val emoji: String,
    val label: String,
    val selected: Boolean = false,
)

data class HighlighterChromeSnapshot(
    val currentUser: CurrentUser? = null,
    val currentUserProfile: ProfileMetadata? = null,
    val joinedCommunities: List<CommunitySummary> = emptyList(),
    val joinedCommunitiesTotal: ULong = 0u,
    val bookmarkedArticleAddressCount: ULong = 0u,
    val connectionState: HighlighterConnectionState = HighlighterConnectionState.OFFLINE,
)

data class HighlighterNetworkRelayRow(
    val url: String,
    val read: Boolean = true,
    val write: Boolean = true,
    val indexer: Boolean = false,
    val rooms: Boolean = false,
    val state: RelayStatus? = null,
)

data class HighlighterNetworkSnapshot(
    val wifiOnlyEnabled: Boolean = false,
    val currentPathIsWifi: Boolean? = null,
    val isSaving: Boolean = false,
    val errorMessage: String? = null,
    val relays: List<RelayConfig> = emptyList(),
    val autoConnectedRelays: List<RelayConfig> = emptyList(),
    val diagnostics: List<HighlighterNetworkRelayRow> = emptyList(),
) {
    val relayCount: ULong get() = relays.size.toULong()
    val autoConnectedRelayCount: ULong get() = autoConnectedRelays.size.toULong()
}

data class HighlighterMediaSettingsSnapshot(
    val wifiOnlyPlayback: Boolean = false,
    val isSaving: Boolean = false,
    val errorMessage: String? = null,
    val blossomServers: List<String> = emptyList(),
) {
    val blossomServerCount: ULong get() = blossomServers.size.toULong()
}

data class HighlighterCreateRoomSnapshot(
    val isCreating: Boolean = false,
    val errorMessage: String? = null,
    val createdGroupId: String? = null,
)

data class HighlighterRoomInviteCandidate(
    val pubkeyHex: String,
    val profile: ProfileMetadata? = null,
    val isSelected: Boolean = false,
)

enum class HighlighterRoomInviteCandidateSource {
    FOLLOW,
    PASTE,
}

data class HighlighterRoomInviteSnapshot(
    val groupId: String = "",
    val query: String = "",
    val visibleFollows: List<String> = emptyList(),
    val selected: List<HighlighterRoomInviteCandidate> = emptyList(),
    val pastedCandidate: HighlighterRoomInviteCandidate? = null,
    val inviteUrl: String? = null,
    val isMintingInviteLink: Boolean = false,
    val isAddingMembers: Boolean = false,
    val addErrorMessage: String? = null,
    val inviteLinkErrorMessage: String? = null,
    val toastMessage: String? = null,
    val errorMessage: String? = null,
)

data class HighlighterCommentInteraction(
    val eventId: String = "",
    val likeCount: ULong = 0u,
    val isLiked: Boolean = false,
    val isBookmarked: Boolean = false,
)

data class HighlighterCommentChildLinks(
    val eventId: String,
    val childEventIds: List<String>,
)

data class HighlighterCommentDraft(
    val parentEventId: String?,
    val body: String,
)

data class HighlighterCommentsSnapshot(
    val rootTagValue: String = "",
    val records: List<CommentRecord> = emptyList(),
    val recordCount: ULong = 0u,
    val topLevelEventIds: List<String> = emptyList(),
    val childLinks: List<HighlighterCommentChildLinks> = emptyList(),
    val interactions: List<HighlighterCommentInteraction> = emptyList(),
    val drafts: List<HighlighterCommentDraft> = emptyList(),
    val isPublishing: Boolean = false,
    val errorMessage: String? = null,
    val publishErrorMessage: String? = null,
)

data class HighlighterFeedbackSnapshot(
    val threads: List<FeedbackThreadRecord> = emptyList(),
    val threadCount: ULong = 0u,
    val selectedRootEventId: String? = null,
    val selectedEvents: List<FeedbackEventRecord> = emptyList(),
    val selectedEventCount: ULong = 0u,
    val newThreadDraft: String = "",
    val replyDraft: String = "",
    val isLoadingThreads: Boolean = false,
    val isPublishingNewThread: Boolean = false,
    val isPublishingReply: Boolean = false,
    val publishErrorMessage: String? = null,
)

data class HighlighterCurationMenuSnapshot(
    val articleAddress: String = "",
    val curationSets: List<BookmarkSetRecord> = emptyList(),
    val isLoading: Boolean = false,
    val isSaving: Boolean = false,
    val errorMessage: String? = null,
)

data class HighlighterEditProfileSnapshot(
    val profile: ProfileMetadata? = null,
    val displayName: String = "",
    val name: String = "",
    val about: String = "",
    val picture: String = "",
    val banner: String = "",
    val nip05: String = "",
    val lud16: String = "",
    val website: String = "",
    val isSaving: Boolean = false,
    val isPictureUploading: Boolean = false,
    val isBannerUploading: Boolean = false,
    val errorMessage: String? = null,
    val savedProfile: ProfileMetadata? = null,
)

enum class HighlighterEditProfileImageTarget {
    PICTURE,
    BANNER,
}

data class HighlighterBookPickerSnapshot(
    val recentBooks: List<ArtifactRecord> = emptyList(),
    val searchResults: List<ArtifactRecord> = emptyList(),
    val isLoadingRecents: Boolean = false,
    val isSearching: Boolean = false,
)

data class HighlighterCaptureUploadSnapshot(
    val url: String = "",
    val sha256Hex: String = "",
    val mime: String = "",
    val sizeBytes: ULong = 0u,
    val imageUrl: String? = null,
    val width: UInt = 0u,
    val height: UInt = 0u,
)

data class HighlightDraft(
    val quote: String = "",
    val context: String = "",
    val note: String = "",
    val clipStartSeconds: Double? = null,
    val clipEndSeconds: Double? = null,
    val clipSpeaker: String = "",
    val clipTranscriptSegmentIds: List<String> = emptyList(),
    val image: Any? = null,
)

sealed class HighlighterCaptureArtifact {
    data class Existing(val record: ArtifactRecord) : HighlighterCaptureArtifact() {
        val shareEventId: String get() = record.shareEventId
        val preview: ArtifactPreview get() = record.preview
    }

    data class Preview(val preview: ArtifactPreview) : HighlighterCaptureArtifact()
    data class Pending(val preview: ArtifactPreview) : HighlighterCaptureArtifact()
}

fun normalizeIsbn(input: String): String? =
    input.filter { it.isDigit() || it == 'X' || it == 'x' }
        .uppercase()
        .takeIf { it.length == 10 || it.length == 13 }

data class HighlighterCaptureSnapshot(
    val errorMessage: String? = null,
    val isPublishing: Boolean = false,
    val publishedEventId: String? = null,
    val isUploading: Boolean = false,
    val uploadErrorMessage: String? = null,
    val upload: HighlighterCaptureUploadSnapshot? = null,
)

data class HighlighterHomeFeedSnapshot(
    val items: List<HighlighterHomeFeedItem> = emptyList(),
    val artifactPreviews: List<ArtifactRecord> = emptyList(),
    val isLoading: Boolean = false,
    val errorMessage: String? = null,
) {
    val itemCount: ULong get() = items.size.toULong()
}

enum class HighlighterHomeFeedItemKind {
    HIGHLIGHTS,
    READ,
}

data class HighlighterHomeFeedItem(
    val stableId: String,
    val sortKey: ULong,
    val highlights: List<HydratedHighlight> = emptyList(),
    val read: HighlighterHomeReadItem? = null,
) {
    val kind: HighlighterHomeFeedItemKind
        get() = if (read != null) HighlighterHomeFeedItemKind.READ else HighlighterHomeFeedItemKind.HIGHLIGHTS

    val highlightCount: ULong get() = highlights.size.toULong()
}

data class HighlighterHomeReadItem(
    val pubkey: String,
    val identifier: String,
    val title: String,
    val summary: String,
    val image: String,
    val authorFollowed: Boolean,
    val interactorPubkeys: List<String>,
)

data class HighlighterSearchSnapshot(
    val query: String = "",
    val recentQueries: List<String> = emptyList(),
    val recentQueryCount: ULong = 0u,
    val isLocalLoading: Boolean = false,
    val isRelayLoading: Boolean = false,
    val searchRelays: List<String> = emptyList(),
    val articles: List<ArticleRecord> = emptyList(),
    val highlights: List<HighlightRecord> = emptyList(),
    val communities: List<CommunitySummary> = emptyList(),
    val profiles: List<ProfileMetadata> = emptyList(),
) {
    val articleCount: ULong get() = articles.size.toULong()
    val highlightCount: ULong get() = highlights.size.toULong()
    val communityCount: ULong get() = communities.size.toULong()
    val profileCount: ULong get() = profiles.size.toULong()
}

data class HighlighterBookmarksSnapshot(
    val articles: List<ArticleRecord> = emptyList(),
    val webBookmarks: List<WebBookmarkRecord> = emptyList(),
    val myBookmarkSets: List<BookmarkSetRecord> = emptyList(),
    val myCurationSets: List<BookmarkSetRecord> = emptyList(),
    val followingCurationSets: List<BookmarkSetRecord> = emptyList(),
    val isLoading: Boolean = false,
    val errorMessage: String? = null,
) {
    val articleCount: ULong get() = articles.size.toULong()
    val webBookmarkCount: ULong get() = webBookmarks.size.toULong()
    val myBookmarkSetCount: ULong get() = myBookmarkSets.size.toULong()
    val myCurationSetCount: ULong get() = myCurationSets.size.toULong()
    val followingCurationSetCount: ULong get() = followingCurationSets.size.toULong()
}

data class HighlighterAppState(
    val auth: HighlighterAuthSnapshot = HighlighterAuthSnapshot(),
    val createAccount: HighlighterCreateAccountSnapshot = HighlighterCreateAccountSnapshot(),
    val onboarding: HighlighterOnboardingSnapshot = HighlighterOnboardingSnapshot(),
    val chrome: HighlighterChromeSnapshot = HighlighterChromeSnapshot(),
    val network: HighlighterNetworkSnapshot = HighlighterNetworkSnapshot(),
    val mediaSettings: HighlighterMediaSettingsSnapshot = HighlighterMediaSettingsSnapshot(),
    val profileView: HighlighterProfileViewSnapshot = HighlighterProfileViewSnapshot(),
    val comments: HighlighterCommentsSnapshot = HighlighterCommentsSnapshot(),
    val roomInvite: HighlighterRoomInviteSnapshot = HighlighterRoomInviteSnapshot(),
    val roomDetail: HighlighterRoomDetailSnapshot = HighlighterRoomDetailSnapshot(),
    val articleReader: HighlighterArticleReaderSnapshot = HighlighterArticleReaderSnapshot(),
    val feedback: HighlighterFeedbackSnapshot = HighlighterFeedbackSnapshot(),
    val shareComposer: HighlighterShareComposerSnapshot = HighlighterShareComposerSnapshot(),
    val toast: ToastSnapshot? = null,
    val whatsNew: HighlighterWhatsNewSnapshot = HighlighterWhatsNewSnapshot(),
    val createRoom: HighlighterCreateRoomSnapshot = HighlighterCreateRoomSnapshot(),
    val curationMenu: HighlighterCurationMenuSnapshot = HighlighterCurationMenuSnapshot(),
    val editProfile: HighlighterEditProfileSnapshot = HighlighterEditProfileSnapshot(),
    val bookPicker: HighlighterBookPickerSnapshot = HighlighterBookPickerSnapshot(),
    val bookmarks: HighlighterBookmarksSnapshot = HighlighterBookmarksSnapshot(),
    val capture: HighlighterCaptureSnapshot = HighlighterCaptureSnapshot(),
    val homeFeed: HighlighterHomeFeedSnapshot = HighlighterHomeFeedSnapshot(),
    val roomExplorer: HighlighterRoomExplorerSnapshot = HighlighterRoomExplorerSnapshot(),
    val search: HighlighterSearchSnapshot = HighlighterSearchSnapshot(),
    val profiles: List<HighlighterProfile> = emptyList(),
    val webMetadata: List<HighlighterWebMetadata> = emptyList(),
    val isbnPreviews: List<HighlighterIsbnPreview> = emptyList(),
    val isBootstrapping: Boolean = false,
)

data class HighlighterRoomExplorerSnapshot(
    val featured: List<CommunitySummary> = emptyList(),
    val newNoteworthy: List<CommunitySummary> = emptyList(),
    val friendsShelf: List<RoomRecommendation> = emptyList(),
    val authorsShelf: List<RoomRecommendation> = emptyList(),
    val allRooms: List<CommunitySummary> = emptyList(),
    val isLoading: Boolean = false,
    val isBrowseLoading: Boolean = false,
    val errorMessage: String? = null,
) {
    val featuredCount: ULong get() = featured.size.toULong()
    val newNoteworthyCount: ULong get() = newNoteworthy.size.toULong()
    val friendsShelfCount: ULong get() = friendsShelf.size.toULong()
    val authorsShelfCount: ULong get() = authorsShelf.size.toULong()
    val allRoomCount: ULong get() = allRooms.size.toULong()
}

data class HighlighterRoomDetailSnapshot(
    val groupId: String = "",
    val hostRelayUrl: String = "",
    val name: String? = null,
    val picture: String? = null,
    val about: String? = null,
    val memberCount: UInt = 0u,
    val isLoading: Boolean = false,
    val artifacts: List<ArtifactRecord> = emptyList(),
    val highlights: List<HydratedHighlight> = emptyList(),
    val discussions: List<DiscussionRecord> = emptyList(),
    val chatMessages: List<ChatMessageRecord> = emptyList(),
    val chatHasMore: Boolean = false,
    val isChatLoadingMore: Boolean = false,
    val chatErrorMessage: String? = null,
    val isSendingChatMessage: Boolean = false,
    val lastPublishedDiscussionId: String? = null,
    val discussionErrorMessage: String? = null,
    val isPublishingDiscussion: Boolean = false,
) {
    val artifactCount: ULong get() = artifacts.size.toULong()
    val highlightCount: ULong get() = highlights.size.toULong()
    val discussionCount: ULong get() = discussions.size.toULong()
    val chatMessageCount: ULong get() = chatMessages.size.toULong()
}

enum class RoomVisibility {
    PUBLIC,
    PRIVATE,
}

enum class RoomAccess {
    OPEN,
    CLOSED,
}

sealed class HighlighterAppAction {
    data object Bootstrap : HighlighterAppAction()
    data object AppForegrounded : HighlighterAppAction()
    data object Logout : HighlighterAppAction()
    data class SignInNsec(val nsec: String, val persist: Boolean = true, val clearStoredOnFailure: Boolean = false) : HighlighterAppAction()
    data class PairBunker(val uri: String, val persist: Boolean = true, val clearStoredOnFailure: Boolean = false) : HighlighterAppAction()
    data class SignInNip55(val signerPackage: String? = null, val persist: Boolean = true, val clearStoredOnFailure: Boolean = false) : HighlighterAppAction()
    data class StartNostrConnect(val callback: String? = null) : HighlighterAppAction()
    data class DeliverExternalSignerResponse(val responseJson: String) : HighlighterAppAction()
    data object CompleteOnboarding : HighlighterAppAction()
    data class ToggleOnboardingInterest(val interestId: String) : HighlighterAppAction()
    data object OpenMediaSettings : HighlighterAppAction()
    data object CloseMediaSettings : HighlighterAppAction()
    data object OpenNetworkSettings : HighlighterAppAction()
    data object CloseNetworkSettings : HighlighterAppAction()
    data object OpenHomeFeed : HighlighterAppAction()
    data object CloseHomeFeed : HighlighterAppAction()
    data object RefreshHomeFeed : HighlighterAppAction()
    data object OpenRoomExplorer : HighlighterAppAction()
    data object RefreshRoomExplorer : HighlighterAppAction()
    data object RefreshRoomBrowseAll : HighlighterAppAction()
    data object OpenBookmarks : HighlighterAppAction()
    data object CloseBookmarks : HighlighterAppAction()
    data class OpenFeedback(val root: String) : HighlighterAppAction()
    data object CloseFeedback : HighlighterAppAction()
    data class OpenFeedbackThread(val rootEventId: String) : HighlighterAppAction()
    data object CloseFeedbackThread : HighlighterAppAction()
    data object PublishFeedbackNewThread : HighlighterAppAction()
    data object PublishFeedbackReply : HighlighterAppAction()
    data class SetFeedbackNewThreadDraft(val body: String) : HighlighterAppAction()
    data class SetFeedbackReplyDraft(val body: String) : HighlighterAppAction()
    data class OpenProfile(val pubkey: String) : HighlighterAppAction()
    data object CloseProfile : HighlighterAppAction()
    data class RequestProfile(val pubkey: String) : HighlighterAppAction()
    data object ToggleProfileFollow : HighlighterAppAction()
    data class OpenRoom(val groupId: String) : HighlighterAppAction()
    data object CloseRoom : HighlighterAppAction()
    data class OpenRoomInvite(val groupId: String) : HighlighterAppAction()
    data object CloseRoomInvite : HighlighterAppAction()
    data class OpenComments(val rootTagName: String, val rootTagValue: String, val rootKind: UShort) : HighlighterAppAction()
    data object CloseComments : HighlighterAppAction()
    data class SetCommentDraft(val replyingToEventId: String?, val body: String) : HighlighterAppAction()
    data class PublishComment(val replyingToEventId: String?) : HighlighterAppAction()
    data class ToggleCommentLike(val eventId: String) : HighlighterAppAction()
    data class ToggleCommentBookmark(val eventId: String) : HighlighterAppAction()
    data class OpenArticleReader(val address: String, val eventId: String? = null, val article: ArticleRecord? = null) : HighlighterAppAction()
    data object CloseArticleReader : HighlighterAppAction()
    data class PublishArticleHighlight(val quote: String, val context: String = "", val note: String) : HighlighterAppAction()
    data object SearchOpened : HighlighterAppAction()
    data object SearchClosed : HighlighterAppAction()
    data class SetSearchQuery(val query: String) : HighlighterAppAction()
    data class SubmitSearch(val query: String) : HighlighterAppAction()
    data object ClearSearch : HighlighterAppAction()
    data object ClearRecentSearches : HighlighterAppAction()
    data object ClearToast : HighlighterAppAction()
    data object DismissWhatsNew : HighlighterAppAction()
    data class RequestWebMetadata(val url: String) : HighlighterAppAction()
    data class RequestIsbnPreview(val isbn: String) : HighlighterAppAction()
    data class OpenEditProfile(val profile: ProfileMetadata?) : HighlighterAppAction()
    data object CloseEditProfile : HighlighterAppAction()
    data object ClearEditProfileResult : HighlighterAppAction()
    data object ClearEditProfileError : HighlighterAppAction()
    data class SetEditProfileDisplayName(val value: String) : HighlighterAppAction()
    data class SetEditProfileName(val value: String) : HighlighterAppAction()
    data class SetEditProfileAbout(val value: String) : HighlighterAppAction()
    data class SetEditProfilePicture(val value: String) : HighlighterAppAction()
    data class SetEditProfileBanner(val value: String) : HighlighterAppAction()
    data class SetEditProfileNip05(val value: String) : HighlighterAppAction()
    data class SetEditProfileLud16(val value: String) : HighlighterAppAction()
    data class SetEditProfileWebsite(val value: String) : HighlighterAppAction()
    data object SubmitEditProfile : HighlighterAppAction()
    class UploadEditProfileImage(
        val target: HighlighterEditProfileImageTarget,
        val bytes: ByteArray,
        val mime: String,
        val width: UInt,
        val height: UInt,
        val altText: String,
    ) : HighlighterAppAction() {
        constructor(uri: String, kind: String) : this(
            if (kind.equals("banner", ignoreCase = true)) HighlighterEditProfileImageTarget.BANNER else HighlighterEditProfileImageTarget.PICTURE,
            ByteArray(0),
            "",
            0u,
            0u,
            uri,
        )
    }
    data class EditProfileCapabilityFailed(val message: String) : HighlighterAppAction()
    data class SetCreateAccountDisplayName(val value: String) : HighlighterAppAction()
    data class SetCreateAccountUsername(val value: String) : HighlighterAppAction()
    data object SubmitCreateAccount : HighlighterAppAction()
    class SubmitCreateRoom(
        val groupId: String,
        val hostRelayUrl: String,
        val name: String,
        val about: String?,
        val visibility: RoomVisibility = RoomVisibility.PUBLIC,
        val access: RoomAccess = RoomAccess.OPEN,
    ) : HighlighterAppAction() {
        constructor(name: String, about: String, visibility: RoomVisibility, access: RoomAccess) : this(
            name.lowercase().replace(Regex("[^a-z0-9-]+"), "-").trim('-').ifBlank { "room" },
            "",
            name,
            about.ifBlank { null },
            visibility,
            access,
        )
    }
    data object ClearCreateRoomError : HighlighterAppAction()
    data object ClearCreateRoomResult : HighlighterAppAction()
    data class SetRoomInviteQuery(val query: String) : HighlighterAppAction()
    data object AcceptRoomInvitePastedCandidate : HighlighterAppAction()
    data class ToggleRoomInviteCandidate(val pubkey: String, val source: HighlighterRoomInviteCandidateSource = HighlighterRoomInviteCandidateSource.FOLLOW) : HighlighterAppAction()
    data class RemoveRoomInviteCandidate(val pubkey: String) : HighlighterAppAction()
    data object MintRoomInviteLink : HighlighterAppAction()
    data object SubmitRoomInviteMembers : HighlighterAppAction()
    data class RequestJoinRoom(val groupId: String, val name: String? = null) : HighlighterAppAction()
    data class PublishRoomDiscussion(val title: String, val body: String, val attachmentUrl: String?) : HighlighterAppAction()
    data object LoadMoreRoomChat : HighlighterAppAction()
    data class PublishRoomChatMessage(val body: String, val replyToEventId: String?) : HighlighterAppAction()
    data object ClearShareComposerError : HighlighterAppAction()
    data object ClearShareComposerResult : HighlighterAppAction()
    data class PublishUrlShare(val url: String, val note: String?, val groupId: String) : HighlighterAppAction()
    data class OpenCurationMenu(val articleAddress: String) : HighlighterAppAction()
    data object CloseCurationMenu : HighlighterAppAction()
    data class SetAddressInCurationSet(val setId: String, val articleAddress: String, val enabled: Boolean) : HighlighterAppAction()
    data class CreateCurationSetAndAdd(val title: String, val articleAddress: String) : HighlighterAppAction()
    data class ToggleArticleBookmark(val address: String) : HighlighterAppAction()
    data class NetworkPathChanged(val isWifi: Boolean) : HighlighterAppAction()
    data class SetNetworkWifiOnly(val enabled: Boolean) : HighlighterAppAction()
    data class UpsertNetworkRelay(val config: RelayConfig) : HighlighterAppAction()
    data class RemoveNetworkRelay(val url: String) : HighlighterAppAction()
    data object ReconnectNetwork : HighlighterAppAction()
    data class AddBlossomServer(val url: String) : HighlighterAppAction()
    data class RemoveBlossomServer(val url: String) : HighlighterAppAction()
    data class ExternalUrlOpenFailed(val url: String) : HighlighterAppAction()
    data object RefreshAppChrome : HighlighterAppAction()
    data class UploadCapturePhoto(
        val bytes: ByteArray,
        val mime: String,
        val width: UInt,
        val height: UInt,
        val altText: String,
    ) : HighlighterAppAction()
    data object ClearCaptureUpload : HighlighterAppAction()
    data object ClearCaptureError : HighlighterAppAction()
    data object ClearCaptureResult : HighlighterAppAction()
    data class RequestBookPickerRecents(val limit: UInt) : HighlighterAppAction()
    data class SearchBookPickerArtifacts(val query: String, val limit: UInt) : HighlighterAppAction()
    data object ClearBookPickerSearch : HighlighterAppAction()
    class PublishCaptureHighlight(
        val artifact: HighlighterCaptureArtifact?,
        val groupId: String?,
        val draft: HighlightDraft,
    ) : HighlighterAppAction() {
        constructor(quote: String, context: String, note: String, groupId: String?) : this(
            null,
            groupId,
            HighlightDraft(quote = quote, context = context, note = note),
        )
    }

    class PublishCapturePicture(
        val artifact: HighlighterCaptureArtifact?,
        val groupId: String?,
        val upload: HighlighterCaptureUploadSnapshot?,
        val note: String,
    ) : HighlighterAppAction() {
        constructor(imageHandle: String, note: String, groupId: String?) : this(
            null,
            groupId,
            HighlighterCaptureUploadSnapshot(url = imageHandle, imageUrl = imageHandle),
            note,
        )
    }
}

class HighlighterNmpApp(config: HighlighterAppConfig) {
    private val app = HighlighterApp(AppConfig(config.dataDir))
    private var reconciler: HighlighterAppReconciler? = null
    private val closed = AtomicBoolean(false)
    private var aggregate = HighlighterAppState()
    private val observer = object : HighlighterObserver {
        override fun onSnapshot(viewId: ViewId, snapshot: ViewSnapshot) {
            aggregate = aggregate.reducing(viewId, snapshot)
            reconciler?.onState(aggregate)
        }

        override fun onCapabilityRequest(request: CapabilityRequest) {
            Log.d(TAG, "capability request requires Android bridge: $request")
        }
    }

    init {
        app.setObserver(observer)
        openView(ViewId.AppRoot, ViewRoute.AppRoot)
        openView(ViewId.RootShell, ViewRoute.RootShell)
        openView(ViewId.Communities, ViewRoute.Communities)
        openView(ViewId.NetworkSettings, ViewRoute.NetworkSettings)
        openView(ViewId.SharePublish, ViewRoute.SharePublish)
        openView(ViewId.WhatsNew, ViewRoute.WhatsNew)
        openView(ViewId.PodcastListening, ViewRoute.PodcastListening)
    }

    fun listenForUpdates(reconciler: HighlighterAppReconciler) {
        this.reconciler = reconciler
        reconciler.onState(aggregate)
    }

    fun state(): HighlighterAppState = aggregate

    fun dispatch(action: HighlighterAppAction) {
        when (action) {
            HighlighterAppAction.Bootstrap -> dispatchEnvelope("hl.auth.restore_session")
            HighlighterAppAction.AppForegrounded -> app.resume()
            HighlighterAppAction.Logout -> dispatchEnvelope("hl.auth.logout")
            is HighlighterAppAction.SignInNsec -> dispatchEnvelope("hl.auth.sign_in_nsec", obj("nsec" to action.nsec))
            is HighlighterAppAction.PairBunker -> dispatchEnvelope("hl.auth.pair_bunker", obj("uri" to action.uri))
            is HighlighterAppAction.SignInNip55 -> dispatchEnvelope("hl.auth.sign_in_nip55")
            is HighlighterAppAction.StartNostrConnect -> dispatchEnvelope("hl.auth.start_nostr_connect")
            is HighlighterAppAction.CompleteOnboarding -> dispatchEnvelope("hl.route.complete_onboarding")
            is HighlighterAppAction.OpenProfile -> openView(ViewId.Profile(action.pubkey), ViewRoute.Profile(action.pubkey))
            HighlighterAppAction.CloseProfile -> closeView(aggregate.profileView.pubkeyHex.takeIf { it.isNotBlank() }?.let { ViewId.Profile(it) })
            is HighlighterAppAction.RequestProfile -> openView(ViewId.Profile(action.pubkey), ViewRoute.Profile(action.pubkey))
            HighlighterAppAction.OpenHomeFeed -> openView(ViewId.HomeFeed, ViewRoute.HomeFeed)
            HighlighterAppAction.CloseHomeFeed -> closeView(ViewId.HomeFeed)
            HighlighterAppAction.RefreshHomeFeed -> dispatchEnvelope("hl.highlight.drain_feed")
            HighlighterAppAction.OpenRoomExplorer -> openView(ViewId.RoomExplorer, ViewRoute.RoomExplorer)
            HighlighterAppAction.RefreshRoomExplorer, HighlighterAppAction.RefreshRoomBrowseAll -> dispatchEnvelope("hl.room.start_discovery")
            HighlighterAppAction.OpenBookmarks -> openView(ViewId.Bookmarks, ViewRoute.Bookmarks)
            HighlighterAppAction.CloseBookmarks -> closeView(ViewId.Bookmarks)
            is HighlighterAppAction.OpenRoom -> {
                openView(ViewId.RoomHome(action.groupId), ViewRoute.RoomHome(action.groupId))
                openView(ViewId.RoomDiscussions(action.groupId), ViewRoute.RoomDiscussions(action.groupId))
                openView(ViewId.RoomChat(action.groupId), ViewRoute.RoomChat(action.groupId))
            }
            HighlighterAppAction.CloseRoom -> closeView(aggregate.roomDetail.groupId.takeIf { it.isNotBlank() }?.let { ViewId.RoomHome(it) })
            is HighlighterAppAction.OpenArticleReader -> openView(ViewId.ArticleReader(action.address), ViewRoute.ArticleReader(action.address))
            HighlighterAppAction.CloseArticleReader -> closeView(aggregate.articleReader.address.takeIf { it.isNotBlank() }?.let { ViewId.ArticleReader(it) })
            HighlighterAppAction.SearchOpened -> openView(ViewId.Search, ViewRoute.Search)
            HighlighterAppAction.SearchClosed -> closeView(ViewId.Search)
            is HighlighterAppAction.SubmitSearch -> dispatchEnvelope("hl.search.omnibox", obj("query" to action.query))
            is HighlighterAppAction.SetSearchQuery -> aggregate = aggregate.copy(search = aggregate.search.copy(query = action.query))
            HighlighterAppAction.ClearSearch -> aggregate = aggregate.copy(search = HighlighterSearchSnapshot())
            is HighlighterAppAction.OpenComments -> openView(
                ViewId.CommentThread(action.rootTagValue),
                ViewRoute.CommentThread(action.rootTagValue),
            )
            HighlighterAppAction.CloseComments -> closeView(aggregate.comments.rootTagValue.takeIf { it.isNotBlank() }?.let { ViewId.CommentThread(it) })
            is HighlighterAppAction.NetworkPathChanged -> aggregate = aggregate.copy(network = aggregate.network.copy(currentPathIsWifi = action.isWifi))
            is HighlighterAppAction.SetNetworkWifiOnly -> dispatchEnvelope("hl.network.apply_path", obj("wifi_only_enabled" to action.enabled))
            HighlighterAppAction.DismissWhatsNew -> aggregate.whatsNew.entries.firstOrNull()?.let {
                dispatchEnvelope("hl.whats_new.mark_seen", obj("shipped_at_unix" to it.shippedAtUnix))
            }
            HighlighterAppAction.ClearToast -> dispatchEnvelope("hl.toast.clear")
            is HighlighterAppAction.PublishRoomChatMessage -> dispatchEnvelope(
                "hl.chat.post",
                obj("group_id" to aggregate.roomDetail.groupId, "host_relay_url" to aggregate.roomDetail.hostRelayUrl, "body" to action.body),
            )
            is HighlighterAppAction.PublishFeedbackNewThread -> dispatchEnvelope("hl.feedback.post_root", obj("body" to aggregate.feedback.newThreadDraft))
            is HighlighterAppAction.PublishFeedbackReply -> dispatchEnvelope("hl.feedback.post_reply", obj("body" to aggregate.feedback.replyDraft))
            is HighlighterAppAction.SetFeedbackNewThreadDraft -> aggregate = aggregate.copy(feedback = aggregate.feedback.copy(newThreadDraft = action.body))
            is HighlighterAppAction.SetFeedbackReplyDraft -> aggregate = aggregate.copy(feedback = aggregate.feedback.copy(replyDraft = action.body))
            is HighlighterAppAction.PublishUrlShare -> dispatchEnvelope(
                "hl.share.publish_url",
                obj("url" to action.url, "group_id" to action.groupId, "note" to action.note.orEmpty()),
            )
            is HighlighterAppAction.PublishCaptureHighlight -> dispatchEnvelope(
                "hl.capture.publish_highlight",
                obj(
                    "quote" to action.draft.quote,
                    "context" to action.draft.context,
                    "note" to action.draft.note,
                    "group_id" to action.groupId.orEmpty(),
                ),
            )
            is HighlighterAppAction.PublishCapturePicture -> dispatchEnvelope(
                "hl.capture.publish_picture",
                obj(
                    "image_url" to (action.upload?.url ?: action.upload?.imageUrl).orEmpty(),
                    "note" to action.note,
                    "group_id" to action.groupId.orEmpty(),
                ),
            )
            else -> Log.d(TAG, "compat action not yet mapped: $action")
        }
        reconciler?.onState(aggregate)
    }

    fun nextSignerRequest(): HighlighterSignerRequestDrain = HighlighterSignerRequestDrain.Idle
    fun decodeNostrEntity(token: String): NostrEntityRef = error("NIP-19 decode not exposed on HighlighterApp")
    fun setCoreEventCallback(callback: EventCallback) = Unit
    fun clearCoreEventCallback() = Unit
    fun close() {
        if (closed.compareAndSet(false, true)) app.shutdown()
    }

    private fun openView(id: ViewId, route: ViewRoute) {
        app.openView(id, route)
        app.currentSnapshot(id)?.let { aggregate = aggregate.reducing(id, it) }
    }

    private fun closeView(id: ViewId?) {
        if (id != null) app.closeView(id)
    }

    private fun dispatchEnvelope(namespace: String, json: JsonObject = JsonObject(emptyMap())) {
        app.dispatchAction(AppActionEnvelope(namespace, Json.encodeToString(json)))
    }
}

private fun HighlighterAppState.reducing(viewId: ViewId, snapshot: ViewSnapshot): HighlighterAppState =
    when (snapshot) {
        is ViewSnapshot.AppRoot -> copy(
            auth = auth.copy(
                errorMessage = snapshot.v1.authError,
                nostrconnectUri = snapshot.v1.nostrconnectUri,
            ),
            onboarding = onboarding.copy(isComplete = snapshot.v1.onboardingComplete),
            chrome = chrome.copy(
                currentUser = snapshot.v1.activePubkeyHex?.let { CurrentUser(it, snapshot.v1.activePubkeyNpub.orEmpty()) },
            ),
        )
        is ViewSnapshot.RootShell -> copy(toast = snapshot.v1.toast)
        is ViewSnapshot.NetworkSettings -> copy(network = network.copy(relays = snapshot.v1.relays.map { it.toRelayConfig() }))
        is ViewSnapshot.RelayDiagnostics -> copy(network = network.copy(diagnostics = snapshot.v1.relays.map { it.compatRelayRow() }))
        is ViewSnapshot.Communities -> copy(chrome = chrome.copy(
            joinedCommunities = snapshot.v1.groups.map { it.toCommunitySummary() },
            joinedCommunitiesTotal = snapshot.v1.groups.size.toULong(),
        ))
        is ViewSnapshot.Profile -> copy(profileView = snapshot.v1.toCompatProfile())
        is ViewSnapshot.RoomHome -> copy(roomDetail = snapshot.v1.toCompatRoomDetail())
        is ViewSnapshot.RoomExplorer -> copy(roomExplorer = snapshot.v1.toCompatRoomExplorer())
        is ViewSnapshot.Bookmarks -> copy(bookmarks = HighlighterBookmarksSnapshot(
            articles = snapshot.v1.articlePreviews.map { it.toArticleRecord() },
            webBookmarks = snapshot.v1.myWebBookmarks.map { it.toWebBookmarkRecord() },
            myBookmarkSets = snapshot.v1.myBookmarkSets.map { it.toBookmarkSetRecord() },
            myCurationSets = snapshot.v1.myCurationSets.map { it.toBookmarkSetRecord() },
            followingCurationSets = snapshot.v1.followingCurationSets.map { it.toBookmarkSetRecord() },
        ))
        is ViewSnapshot.ArticleReader -> copy(articleReader = snapshot.v1.toCompatArticleReader())
        is ViewSnapshot.Search -> copy(search = search.copy(articles = snapshot.v1.hits.map { it.toArticleRecord() }))
        is ViewSnapshot.HomeFeed -> copy(homeFeed = HighlighterHomeFeedSnapshot(
            items = snapshot.v1.rows.map { it.toCompatHomeFeedItem(snapshot.v1.artifactPreviews) },
        ))
        is ViewSnapshot.WhatsNew -> copy(whatsNew = HighlighterWhatsNewSnapshot(
            entries = snapshot.v1.entries.map { HighlighterWhatsNewEntry(it.shippedAtIso, it.shippedAtUnix, it.lines) },
            shouldPresent = snapshot.v1.shouldPresent,
        ))
        is ViewSnapshot.SharePublish -> copy(shareComposer = HighlighterShareComposerSnapshot(
            isPublishing = snapshot.v1.publishing,
            publishedGroupId = snapshot.v1.inviteCodes.firstOrNull(),
            errorMessage = snapshot.v1.errorMessage,
        ))
        is ViewSnapshot.CommentThread -> copy(comments = HighlighterCommentsSnapshot(
            rootTagValue = snapshot.v1.rootTagValue,
            records = snapshot.v1.records.map { it.toCommentRecord() },
            recordCount = snapshot.v1.commentCount.toULong(),
            topLevelEventIds = snapshot.v1.records.filter { it.isTopLevel }.map { it.eventId },
            childLinks = snapshot.v1.records
                .groupBy { it.parentTagValue }
                .map { (parent, children) ->
                    HighlighterCommentChildLinks(parent, children.filter { !it.isTopLevel }.map { it.eventId })
                },
        ))
        is ViewSnapshot.FeedbackThreads -> copy(feedback = feedback.copy(
            threads = snapshot.v1.threads.map { it.toFeedbackThreadRecord() },
            threadCount = snapshot.v1.threads.size.toULong(),
            isPublishingNewThread = snapshot.v1.isPublishing,
            publishErrorMessage = snapshot.v1.error,
        ))
        is ViewSnapshot.FeedbackThread -> copy(feedback = feedback.copy(
            selectedRootEventId = snapshot.v1.rootEventId,
            selectedEvents = snapshot.v1.rows.map { it.toFeedbackEventRecord() },
            selectedEventCount = snapshot.v1.rows.size.toULong(),
            isPublishingReply = snapshot.v1.isPublishing,
            publishErrorMessage = snapshot.v1.error,
        ))
        else -> this
    }

private fun RelayDiagRow.compatRelayRow(): HighlighterNetworkRelayRow =
    HighlighterNetworkRelayRow(
        url = relayUrl,
        state = when (connectionState) {
            RelayConnectionState.CONNECTED -> RelayStatus.CONNECTED
            RelayConnectionState.RECONNECTING -> RelayStatus.CONNECTING
            RelayConnectionState.ERROR -> RelayStatus.DISCONNECTED
            RelayConnectionState.UNKNOWN -> RelayStatus.DISCONNECTED
        },
    )

private fun RelayDiagRow.toRelayConfig(): RelayConfig =
    RelayConfig(
        url = relayUrl,
        read = true,
        write = true,
        rooms = false,
        indexer = false,
    )

private fun CommunityRow.toCommunitySummary() = CommunitySummary(
    id = groupId,
    name = name.orEmpty(),
    about = about.orEmpty(),
    picture = picture.orEmpty(),
    access = if (open) "open" else "closed",
    visibility = if (public) "public" else "private",
    adminPubkeys = emptyList(),
    memberCount = memberCount.toULong(),
    relayUrl = hostRelayUrl,
    metadataEventId = "",
    createdAt = null,
)

private fun ProfileSnapshot.toCompatProfile() = HighlighterProfileViewSnapshot(
    pubkeyHex = pubkey,
    profile = ProfileMetadata(
        pubkey = pubkey,
        name = name.orEmpty(),
        displayName = displayName.orEmpty(),
        about = about,
        picture = pictureUrl.orEmpty(),
        banner = banner.orEmpty(),
        nip05 = nip05,
        website = website.orEmpty(),
        lud16 = lud16.orEmpty(),
        createdAt = null,
    ),
    isFollowing = isFollowing,
    communities = communities.map { it.toCommunitySummary() },
)

private fun ArtifactPreviewRow.toArticleRecord() = ArticleRecord(
    eventId = coordinate,
    address = coordinate,
    pubkey = authorPubkey.orEmpty(),
    identifier = coordinate.substringAfterLast(':', coordinate),
    title = title.orEmpty(),
    summary = summary.orEmpty(),
    image = imageUrl.orEmpty(),
    content = "",
    hashtags = emptyList(),
    publishedAt = null,
    createdAt = null,
)

private fun KernelSearchHitRow.toArticleRecord() = ArticleRecord(
    eventId = id,
    address = firstTag("a").orEmpty(),
    pubkey = author,
    identifier = firstTag("d") ?: id,
    title = firstTag("title") ?: content.lineSequence().firstOrNull().orEmpty(),
    summary = firstTag("summary").orEmpty(),
    image = firstTag("image").orEmpty(),
    content = "",
    hashtags = emptyList(),
    publishedAt = null,
    createdAt = createdAt,
)

private fun BookmarkSetRow.toBookmarkSetRecord() = BookmarkSetRecord(
    id = dTag,
    pubkey = pubkey,
    kind = kind,
    title = title.orEmpty(),
    description = description.orEmpty(),
    image = image.orEmpty(),
    articleAddresses = articleAddresses,
    noteIds = noteIds,
    rRefs = rRefs,
    topics = topics,
    createdAt = createdAt,
)

private fun WebBookmarkRow.toWebBookmarkRecord() = WebBookmarkRecord(
    url = url,
    pubkey = pubkey,
    title = title.orEmpty(),
    description = description.orEmpty(),
    topics = topics,
    publishedAt = publishedAt,
    createdAt = createdAt,
)

private fun KernelRoomHomeSnapshot.toCompatRoomDetail() = HighlighterRoomDetailSnapshot(
    groupId = groupId,
    hostRelayUrl = hostRelayUrl,
    name = name,
    picture = picture,
    about = about,
    memberCount = memberCount,
    artifacts = artifactLibrary.map { it.artifactRecord },
    highlights = highlights.map { it.toHydratedHighlight() },
    discussions = assembledLanes.mapNotNull { it.toDiscussionRecord(groupId) },
)

private fun KernelArticleReaderSnapshot.toCompatArticleReader() = HighlighterArticleReaderSnapshot(
    address = address,
    article = ArticleRecord(
        eventId = id,
        address = address,
        pubkey = authorPubkey,
        identifier = dTag,
        title = title.orEmpty(),
        summary = summary.orEmpty(),
        image = heroImageUrl.orEmpty(),
        content = contentTreeJson,
        hashtags = emptyList(),
        publishedAt = createdAt,
        createdAt = createdAt,
    ),
    authorProfile = ProfileMetadata(
        pubkey = authorPubkey,
        name = authorDisplayName.orEmpty(),
        displayName = authorDisplayName.orEmpty(),
        about = "",
        picture = authorPictureUrl.orEmpty(),
        banner = "",
        nip05 = "",
        website = "",
        lud16 = "",
        createdAt = null,
    ),
    highlights = highlights.map { it.toHighlightRecord() },
)

private fun KernelRoomExplorerSnapshot.toCompatRoomExplorer() = HighlighterRoomExplorerSnapshot(
    featured = featured.map { it.toCommunitySummary() },
    newNoteworthy = newNoteworthy.map { it.toCommunitySummary() },
    friendsShelf = friendsShelf.map { it.toRoomRecommendation(RoomRecommendationReason.FRIENDS) },
    authorsShelf = authorsShelf.map { it.toRoomRecommendation(RoomRecommendationReason.AUTHORS) },
    allRooms = (featured.map { it.toCommunitySummary() } + newNoteworthy.map { it.toCommunitySummary() }),
)

private fun DiscoveredRow.toCommunitySummary() = CommunitySummary(
    id = groupId,
    name = name.orEmpty(),
    about = about.orEmpty(),
    picture = picture.orEmpty(),
    access = if (open) "open" else "closed",
    visibility = if (public) "public" else "private",
    adminPubkeys = emptyList(),
    memberCount = memberCount.toULong(),
    relayUrl = hostRelayUrl,
    metadataEventId = "",
    createdAt = null,
)

private fun RecommendationRow.toRoomRecommendation(reason: RoomRecommendationReason) = RoomRecommendation(
    summary = CommunitySummary(
        id = groupId,
        name = name.orEmpty(),
        about = about.orEmpty(),
        picture = picture.orEmpty(),
        access = "open",
        visibility = "public",
        adminPubkeys = emptyList(),
        memberCount = totalReasonCount.toULong(),
        relayUrl = hostRelayUrl,
        metadataEventId = "",
        createdAt = null,
    ),
    reasonPubkeys = reasonPubkeys,
    reasonKind = reason,
)

private fun HighlightRow.toHighlightRecord() = HighlightRecord(
    eventId = eventId,
    pubkey = authorPubkey,
    quote = content,
    context = context,
    note = note.orEmpty(),
    artifactAddress = artifactAddress,
    eventReference = eventReference,
    externalReference = externalReference,
    sourceUrl = sourceUrl,
    sourceReferenceKey = sourceReferenceKey,
    clipStartSeconds = clipStartSeconds,
    clipEndSeconds = clipEndSeconds,
    clipSpeaker = clipSpeaker,
    clipTranscriptSegmentIds = clipTranscriptSegmentIds,
    imageUrl = imageUrl,
    createdAt = createdAt,
)

private fun HighlightRow.toHydratedHighlight() = HydratedHighlight(
    highlight = toHighlightRecord(),
    artifact = null,
    sharedByEventId = null,
    sharedByPubkey = null,
)

private fun KernelHomeFeedRow.toCompatHomeFeedItem(previews: List<ArtifactPreviewRow>) =
    when (kind) {
        KernelHomeFeedRowKind.ARTICLE -> HighlighterHomeFeedItem(
            stableId = stableId,
            sortKey = sortKey,
            read = HighlighterHomeReadItem(
                pubkey = articleAuthorPubkey.orEmpty(),
                identifier = articleAddress.orEmpty(),
                title = previews.firstOrNull { it.coordinate == articleAddress }?.title.orEmpty(),
                summary = previews.firstOrNull { it.coordinate == articleAddress }?.summary.orEmpty(),
                image = previews.firstOrNull { it.coordinate == articleAddress }?.imageUrl.orEmpty(),
                authorFollowed = authorFollowed,
                interactorPubkeys = interactorPubkeys,
            ),
        )
        KernelHomeFeedRowKind.HIGHLIGHT -> HighlighterHomeFeedItem(
            stableId = stableId,
            sortKey = sortKey,
            highlights = highlights.map { it.toHydratedHighlight() },
        )
    }

private fun CommentRecordRow.toCommentRecord() = CommentRecord(
    eventId = eventId,
    pubkey = authorPubkey,
    body = body,
    rootTagName = rootTagName,
    rootTagValue = rootTagValue,
    parentTagName = parentTagName,
    parentTagValue = parentTagValue,
    rootKind = rootKind,
    createdAt = createdAt,
)

private fun FeedbackThreadRow.toFeedbackThreadRecord() = FeedbackThreadRecord(
    rootEventId = rootEventId,
    authorPubkey = authorPubkey,
    createdAt = createdAt,
    lastActivityAt = lastActivityAt,
    title = title,
    summary = summary,
    statusLabel = statusLabel,
    preview = preview,
)

private fun FeedbackMessageRow.toFeedbackEventRecord() = FeedbackEventRecord(
    eventId = eventId,
    rootEventId = rootEventId,
    authorPubkey = authorPubkey,
    createdAt = createdAt,
    content = content,
)

private fun KernelRoomLane.toDiscussionRecord(groupId: String): DiscussionRecord? {
    val eventId = shareEventId.takeIf { it.isNotBlank() } ?: return null
    return DiscussionRecord(
        id = eventId,
        eventId = eventId,
        groupId = groupId,
        pubkey = artifactRecord.pubkey,
        title = artifactRecord.preview.title,
        body = artifactRecord.note,
        summary = artifactRecord.preview.description,
        createdAt = artifactRecord.createdAt,
        attachment = null,
    )
}

private fun KernelSearchHitRow.firstTag(name: String): String? =
    tags.firstOrNull { it.firstOrNull() == name }?.getOrNull(1)

private fun ArticleRecord.toArtifactPreview() = ArtifactPreview(
    id = eventId,
    url = "",
    title = title,
    author = pubkey,
    image = image,
    description = summary,
    source = "article",
    domain = "",
    catalogId = address,
    catalogKind = "article",
    podcastGuid = "",
    podcastItemGuid = "",
    podcastShowTitle = "",
    audioUrl = "",
    audioPreviewUrl = "",
    transcriptUrl = "",
    feedUrl = "",
    publishedAt = "",
    durationSeconds = null,
    referenceTagName = "a",
    referenceTagValue = address,
    referenceKind = "30023",
    highlightTagName = "a",
    highlightTagValue = address,
    highlightReferenceKey = address,
    chapters = emptyList(),
)

private fun obj(vararg pairs: Pair<String, Any?>): JsonObject = buildJsonObject {
    for ((key, value) in pairs) {
        when (value) {
            null -> put(key, JsonPrimitive(null))
            is String -> put(key, value)
            is Boolean -> put(key, value)
            is Int -> put(key, value)
            is UInt -> put(key, value.toInt())
            is ULong -> put(key, value.toLong())
            is UShort -> put(key, value.toInt())
            else -> put(key, value.toString())
        }
    }
}

private fun emptyProfileSnapshot() = ProfileSnapshot(
    pubkey = "",
    displayName = null,
    name = null,
    rawDisplayName = null,
    pictureUrl = null,
    banner = null,
    website = null,
    nip05 = "",
    about = "",
    lud16 = null,
    isFollowing = false,
    communities = emptyList(),
)

private fun emptyRoomHomeSnapshot() = KernelRoomHomeSnapshot(
    groupId = "",
    hostRelayUrl = "",
    name = null,
    picture = null,
    about = null,
    memberCount = 0u,
    public = true,
    open = true,
    isAdmin = false,
    laneIds = emptyList(),
    inviteLinkBase = "",
    lanes = emptyList(),
    artifactLibrary = emptyList(),
    highlights = emptyList(),
    highlightsByReference = emptyList(),
    commentsByReference = emptyList(),
    assembledLanes = emptyList(),
)

private fun emptyArticleReaderSnapshot() = KernelArticleReaderSnapshot(
    address = "",
    id = "",
    authorPubkey = "",
    authorDisplayName = null,
    authorPictureUrl = null,
    title = null,
    summary = null,
    heroImageUrl = null,
    dTag = "",
    createdAt = 0u,
    contentTreeBytes = ByteArray(0),
    contentTreeJson = "",
    highlights = emptyList(),
)

private fun emptySharePublishSnapshot() = SharePublishSnapshot(
    publishing = false,
    didPublish = false,
    errorMessage = null,
    inviteCodes = emptyList(),
)
