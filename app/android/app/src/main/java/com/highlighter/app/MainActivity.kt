package com.highlighter.app

import android.app.Application
import android.content.Context
import android.content.Intent
import android.graphics.BitmapFactory
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.Uri
import android.os.Bundle
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import androidx.lifecycle.viewmodel.compose.viewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.highlighter_core.ArticleRecord
import uniffi.highlighter_core.ArtifactPreview
import uniffi.highlighter_core.ArtifactRecord
import uniffi.highlighter_core.BookmarkSetRecord
import uniffi.highlighter_core.BlossomUpload
import uniffi.highlighter_core.ChatMessageRecord
import uniffi.highlighter_core.CommentRecord
import uniffi.highlighter_core.CommunitySummary
import uniffi.highlighter_core.DiscussionRecord
import uniffi.highlighter_core.FeedbackEventRecord
import uniffi.highlighter_core.FeedbackThreadRecord
import uniffi.highlighter_core.HighlightRecord
import uniffi.highlighter_core.HighlightDraft
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterAppConfig
import uniffi.highlighter_core.HighlighterAppReconciler
import uniffi.highlighter_core.HighlighterAppState
import uniffi.highlighter_core.HighlighterArticleReaderSnapshot
import uniffi.highlighter_core.HighlighterAuthSnapshot
import uniffi.highlighter_core.HighlighterBookPickerSnapshot
import uniffi.highlighter_core.HighlighterBookmarksSnapshot
import uniffi.highlighter_core.HighlighterCaptureArtifact
import uniffi.highlighter_core.HighlighterCaptureSnapshot
import uniffi.highlighter_core.HighlighterChromeSnapshot
import uniffi.highlighter_core.HighlighterCommentsSnapshot
import uniffi.highlighter_core.HighlighterConnectionState
import uniffi.highlighter_core.HighlighterCreateAccountSnapshot
import uniffi.highlighter_core.HighlighterCreateRoomSnapshot
import uniffi.highlighter_core.HighlighterFeedbackSnapshot
import uniffi.highlighter_core.HighlighterHomeFeedItem
import uniffi.highlighter_core.HighlighterHomeFeedItemKind
import uniffi.highlighter_core.HighlighterHomeFeedSnapshot
import uniffi.highlighter_core.HighlighterMediaSettingsSnapshot
import uniffi.highlighter_core.HighlighterNmpApp
import uniffi.highlighter_core.HighlighterNetworkSnapshot
import uniffi.highlighter_core.HighlighterOnboardingInterest
import uniffi.highlighter_core.HighlighterOnboardingSnapshot
import uniffi.highlighter_core.HighlighterProfileViewSnapshot
import uniffi.highlighter_core.HighlighterRoomDetailSnapshot
import uniffi.highlighter_core.HighlighterRoomExplorerSnapshot
import uniffi.highlighter_core.HighlighterRoomInviteCandidateSource
import uniffi.highlighter_core.HighlighterRoomInviteSnapshot
import uniffi.highlighter_core.HighlighterSearchSnapshot
import uniffi.highlighter_core.HighlighterSessionCredential
import uniffi.highlighter_core.HighlighterUsernameStatus
import uniffi.highlighter_core.HydratedHighlight
import uniffi.highlighter_core.ProfileMetadata
import uniffi.highlighter_core.RelayConfig
import uniffi.highlighter_core.RoomAccess
import uniffi.highlighter_core.RoomRecommendation
import uniffi.highlighter_core.RoomVisibility
import uniffi.highlighter_core.WebBookmarkRecord
import java.io.File

private val Paper = Color(0xFFF8F7F2)
private val Ink = Color(0xFF16211D)
private val Muted = Color(0xFF69736D)
private val Line = Color(0xFFE2DED2)
private val Moss = Color(0xFF315C4D)
private val Gold = Color(0xFFC58B2B)
private val Clay = Color(0xFF8E5141)
private const val FEEDBACK_PROJECT_COORDINATE =
    "31933:09d48a1a5dbe13404a729634f1d6ba722d40513468dd713c8ea38ca9b7b6f2c7:highlighter"

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            HighlighterTheme {
                val viewModel: HighlighterViewModel = viewModel()
                DisposableEffect(viewModel) {
                    viewModel.bootstrap()
                    viewModel.dispatch(HighlighterAppAction.SearchOpened)
                    viewModel.dispatch(HighlighterAppAction.OpenHomeFeed)
                    viewModel.dispatch(HighlighterAppAction.OpenBookmarks)
                    viewModel.dispatch(HighlighterAppAction.OpenRoomExplorer)
                    viewModel.dispatch(HighlighterAppAction.OpenMediaSettings)
                    viewModel.dispatch(HighlighterAppAction.OpenNetworkSettings)
                    viewModel.dispatch(HighlighterAppAction.OpenFeedback(FEEDBACK_PROJECT_COORDINATE))
                    viewModel.dispatch(HighlighterAppAction.RequestBookPickerRecents(12u))
                    onDispose {
                        viewModel.dispatch(HighlighterAppAction.CloseArticleReader)
                        viewModel.dispatch(HighlighterAppAction.CloseBookmarks)
                        viewModel.dispatch(HighlighterAppAction.CloseHomeFeed)
                        viewModel.dispatch(HighlighterAppAction.CloseMediaSettings)
                        viewModel.dispatch(HighlighterAppAction.CloseNetworkSettings)
                        viewModel.dispatch(HighlighterAppAction.CloseFeedback)
                        viewModel.dispatch(HighlighterAppAction.SearchClosed)
                    }
                }
                LifecycleEventEffect(Lifecycle.Event.ON_RESUME) {
                    viewModel.appForegrounded()
                }
                val state by viewModel.state.collectAsState()
                val currentUserPubkey = state.chrome.currentUser?.pubkey
                DisposableEffect(viewModel, currentUserPubkey) {
                    if (currentUserPubkey != null) {
                        viewModel.dispatch(HighlighterAppAction.OpenProfile(currentUserPubkey))
                    }
                    onDispose {
                        viewModel.dispatch(HighlighterAppAction.CloseProfile)
                    }
                }
                HighlighterAppScreen(
                    state = state,
                    onRefresh = { viewModel.dispatch(HighlighterAppAction.RefreshAppChrome) },
                    onSignIn = { nsec ->
                        viewModel.dispatch(HighlighterAppAction.SignInNsec(nsec, false, false))
                    },
                    onStartNostrConnect = {
                        viewModel.dispatch(HighlighterAppAction.StartNostrConnect("highlighter://nip46"))
                    },
                    onSetCreateAccountDisplayName = { displayName ->
                        viewModel.dispatch(HighlighterAppAction.SetCreateAccountDisplayName(displayName))
                    },
                    onSetCreateAccountUsername = { username ->
                        viewModel.dispatch(HighlighterAppAction.SetCreateAccountUsername(username))
                    },
                    onSubmitCreateAccount = {
                        viewModel.dispatch(HighlighterAppAction.SubmitCreateAccount)
                    },
                    onLogout = { viewModel.dispatch(HighlighterAppAction.Logout) },
                    onClearToast = { viewModel.dispatch(HighlighterAppAction.ClearToast) },
                    onToggleOnboardingInterest = { id ->
                        viewModel.dispatch(HighlighterAppAction.ToggleOnboardingInterest(id))
                    },
                    onCompleteOnboarding = {
                        viewModel.dispatch(HighlighterAppAction.CompleteOnboarding)
                    },
                    onSearchQueryChange = { query ->
                        viewModel.dispatch(HighlighterAppAction.SetSearchQuery(query))
                    },
                    onSearchSubmit = { query ->
                        viewModel.dispatch(HighlighterAppAction.SubmitSearch(query))
                    },
                    onClearSearch = {
                        viewModel.dispatch(HighlighterAppAction.ClearSearch)
                    },
                    onClearRecentSearches = {
                        viewModel.dispatch(HighlighterAppAction.ClearRecentSearches)
                    },
                    onRefreshHomeFeed = {
                        viewModel.dispatch(HighlighterAppAction.RefreshHomeFeed)
                    },
                    onRefreshBookmarks = {
                        viewModel.dispatch(HighlighterAppAction.RefreshBookmarks)
                    },
                    onRefreshRoomExplorer = {
                        viewModel.dispatch(HighlighterAppAction.RefreshRoomExplorer)
                    },
                    onSetNetworkWifiOnly = { enabled ->
                        viewModel.dispatch(HighlighterAppAction.SetNetworkWifiOnly(enabled))
                    },
                    onReconnectNetwork = {
                        viewModel.dispatch(HighlighterAppAction.ReconnectNetwork)
                    },
                    onRefreshProfile = {
                        viewModel.dispatch(HighlighterAppAction.RefreshProfile)
                    },
                    onToggleProfileFollow = {
                        viewModel.dispatch(HighlighterAppAction.ToggleProfileFollow)
                    },
                    onRefreshRoomBrowseAll = {
                        viewModel.dispatch(HighlighterAppAction.RefreshRoomBrowseAll)
                    },
                    onSubmitCreateRoom = { name, about, visibility, access ->
                        viewModel.dispatch(
                            HighlighterAppAction.SubmitCreateRoom(
                                name,
                                about,
                                visibility,
                                access,
                            ),
                        )
                    },
                    onClearCreateRoomResult = {
                        viewModel.dispatch(HighlighterAppAction.ClearCreateRoomResult)
                    },
                    onClearCreateRoomError = {
                        viewModel.dispatch(HighlighterAppAction.ClearCreateRoomError)
                    },
                    onRequestJoinRoom = { groupId, roomName ->
                        viewModel.dispatch(HighlighterAppAction.RequestJoinRoom(groupId, roomName))
                    },
                    onOpenRoomInvite = { groupId ->
                        viewModel.dispatch(HighlighterAppAction.OpenRoomInvite(groupId))
                    },
                    onRefreshRoomInvite = {
                        viewModel.dispatch(HighlighterAppAction.RefreshRoomInvite)
                    },
                    onSetRoomInviteQuery = { query ->
                        viewModel.dispatch(HighlighterAppAction.SetRoomInviteQuery(query))
                    },
                    onToggleRoomInviteCandidate = { pubkey, source ->
                        viewModel.dispatch(HighlighterAppAction.ToggleRoomInviteCandidate(pubkey, source))
                    },
                    onRemoveRoomInviteCandidate = { pubkey ->
                        viewModel.dispatch(HighlighterAppAction.RemoveRoomInviteCandidate(pubkey))
                    },
                    onAcceptRoomInvitePastedCandidate = {
                        viewModel.dispatch(HighlighterAppAction.AcceptRoomInvitePastedCandidate)
                    },
                    onMintRoomInviteLink = {
                        viewModel.dispatch(HighlighterAppAction.MintRoomInviteLink)
                    },
                    onSubmitRoomInviteMembers = {
                        viewModel.dispatch(HighlighterAppAction.SubmitRoomInviteMembers)
                    },
                    onCloseRoomInvite = {
                        viewModel.dispatch(HighlighterAppAction.CloseRoomInvite)
                    },
                    onOpenRoom = { groupId ->
                        viewModel.dispatch(HighlighterAppAction.OpenRoom(groupId))
                    },
                    onRefreshRoom = {
                        viewModel.dispatch(HighlighterAppAction.RefreshRoom)
                    },
                    onPublishRoomDiscussion = { title, body ->
                        viewModel.dispatch(HighlighterAppAction.PublishRoomDiscussion(title, body, null))
                    },
                    onPublishRoomChatMessage = { body ->
                        viewModel.dispatch(HighlighterAppAction.PublishRoomChatMessage(body, null))
                    },
                    onLoadMoreRoomChat = {
                        viewModel.dispatch(HighlighterAppAction.LoadMoreRoomChat)
                    },
                    onCloseRoom = {
                        viewModel.dispatch(HighlighterAppAction.CloseRoom)
                    },
                    onOpenComments = { rootTagName, rootTagValue, rootKind ->
                        viewModel.dispatch(
                            HighlighterAppAction.OpenComments(
                                rootTagName,
                                rootTagValue,
                                rootKind,
                            ),
                        )
                    },
                    onSetCommentDraft = { parentEventId, body ->
                        viewModel.dispatch(HighlighterAppAction.SetCommentDraft(parentEventId, body))
                    },
                    onPublishComment = { parentEventId ->
                        viewModel.dispatch(HighlighterAppAction.PublishComment(parentEventId))
                    },
                    onToggleCommentLike = { eventId ->
                        viewModel.dispatch(HighlighterAppAction.ToggleCommentLike(eventId))
                    },
                    onToggleCommentBookmark = { eventId ->
                        viewModel.dispatch(HighlighterAppAction.ToggleCommentBookmark(eventId))
                    },
                    onRefreshComments = {
                        viewModel.dispatch(HighlighterAppAction.RefreshComments)
                    },
                    onCloseComments = {
                        viewModel.dispatch(HighlighterAppAction.CloseComments)
                    },
                    onRefreshFeedback = {
                        viewModel.dispatch(HighlighterAppAction.RefreshFeedbackThreads)
                    },
                    onSetFeedbackNewThreadDraft = { body ->
                        viewModel.dispatch(HighlighterAppAction.SetFeedbackNewThreadDraft(body))
                    },
                    onPublishFeedbackNewThread = {
                        viewModel.dispatch(HighlighterAppAction.PublishFeedbackNewThread)
                    },
                    onOpenFeedbackThread = { rootId ->
                        viewModel.dispatch(HighlighterAppAction.OpenFeedbackThread(rootId))
                    },
                    onSetFeedbackReplyDraft = { body ->
                        viewModel.dispatch(HighlighterAppAction.SetFeedbackReplyDraft(body))
                    },
                    onPublishFeedbackReply = {
                        viewModel.dispatch(HighlighterAppAction.PublishFeedbackReply)
                    },
                    onRefreshFeedbackThread = {
                        viewModel.dispatch(HighlighterAppAction.RefreshFeedbackThread)
                    },
                    onCloseFeedbackThread = {
                        viewModel.dispatch(HighlighterAppAction.CloseFeedbackThread)
                    },
                    onRefreshMediaSettings = {
                        viewModel.dispatch(HighlighterAppAction.RefreshMediaSettings)
                    },
                    onAddBlossomServer = { url ->
                        viewModel.dispatch(HighlighterAppAction.AddBlossomServer(url))
                    },
                    onRemoveBlossomServer = { url ->
                        viewModel.dispatch(HighlighterAppAction.RemoveBlossomServer(url))
                    },
                    onSearchBookPickerArtifacts = { query ->
                        viewModel.dispatch(HighlighterAppAction.SearchBookPickerArtifacts(query, 20u))
                    },
                    onClearBookPickerSearch = {
                        viewModel.dispatch(HighlighterAppAction.ClearBookPickerSearch)
                    },
                    onUploadCapturePhoto = { bytes, mime, width, height, alt ->
                        viewModel.dispatch(
                            HighlighterAppAction.UploadCapturePhoto(bytes, mime, width, height, alt),
                        )
                    },
                    onPublishCaptureHighlight = { selection, targetGroupId, draft ->
                        viewModel.dispatch(
                            HighlighterAppAction.PublishCaptureHighlight(
                                selection,
                                targetGroupId,
                                draft,
                            ),
                        )
                    },
                    onPublishCapturePicture = { selection, targetGroupId, image, note ->
                        viewModel.dispatch(
                            HighlighterAppAction.PublishCapturePicture(
                                selection,
                                targetGroupId,
                                image,
                                note,
                            ),
                        )
                    },
                    onClearCaptureResult = {
                        viewModel.dispatch(HighlighterAppAction.ClearCaptureResult)
                    },
                    onClearCaptureError = {
                        viewModel.dispatch(HighlighterAppAction.ClearCaptureError)
                    },
                    onOpenArticle = { article ->
                        viewModel.dispatch(
                            HighlighterAppAction.OpenArticleReader(
                                article.pubkey,
                                article.identifier,
                                article,
                            ),
                        )
                    },
                    onOpenReadArticle = { pubkey, identifier ->
                        viewModel.dispatch(
                            HighlighterAppAction.OpenArticleReader(
                                pubkey,
                                identifier,
                                null,
                            ),
                        )
                    },
                    onRefreshArticleReader = {
                        viewModel.dispatch(HighlighterAppAction.RefreshArticleReader)
                    },
                    onCloseArticleReader = {
                        viewModel.dispatch(HighlighterAppAction.CloseArticleReader)
                    },
                    onPublishArticleHighlight = { quote, note ->
                        viewModel.dispatch(
                            HighlighterAppAction.PublishArticleHighlight(
                                quote,
                                "",
                                note,
                            ),
                        )
                    },
                )
            }
        }
    }
}

class HighlighterViewModel(application: Application) :
    AndroidViewModel(application),
    HighlighterAppReconciler {
    private val connectivityManager =
        application.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
    private var networkCallback: ConnectivityManager.NetworkCallback? = null
    private val app = HighlighterNmpApp(
        HighlighterAppConfig(
            dataDir = File(application.filesDir, "highlighter-core").absolutePath,
            visibleLimit = 250u,
            emitHz = 30u,
        ),
    )
    private val _state = MutableStateFlow(app.state())
    val state: StateFlow<HighlighterAppState> = _state.asStateFlow()

    init {
        app.listenForUpdates(this)
        syncNetworkCallback(_state.value.network.wifiOnlyEnabled)
    }

    fun bootstrap() {
        app.dispatch(HighlighterAppAction.Bootstrap)
    }

    fun appForegrounded() {
        app.dispatch(HighlighterAppAction.AppForegrounded)
    }

    fun dispatch(action: HighlighterAppAction) {
        app.dispatch(action)
    }

    override fun onState(state: HighlighterAppState) {
        _state.value = state
        syncNetworkCallback(state.network.wifiOnlyEnabled)
    }

    override fun onPersistSessionCredential(credential: HighlighterSessionCredential) {
    }

    override fun onClearSessionCredentials() {
    }

    override fun onOpenExternalUrl(url: String) {
        openExternalUrl(url)
    }

    private fun openExternalUrl(url: String) {
        val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url))
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        val context = getApplication<Application>()
        val accepted = intent.resolveActivity(context.packageManager) != null &&
            runCatching { context.startActivity(intent) }.isSuccess
        if (!accepted) {
            app.dispatch(HighlighterAppAction.ExternalUrlOpenFailed(url))
        }
    }

    override fun onCleared() {
        syncNetworkCallback(false)
        app.close()
    }

    private fun syncNetworkCallback(wifiOnlyEnabled: Boolean) {
        if (wifiOnlyEnabled) {
            if (networkCallback != null) return
            val callback = object : ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: Network) {
                    reportActiveNetworkWifi()
                }

                override fun onCapabilitiesChanged(
                    network: Network,
                    networkCapabilities: NetworkCapabilities,
                ) {
                    reportActiveNetworkWifi()
                }

                override fun onLost(network: Network) {
                    reportActiveNetworkWifi()
                }
            }
            connectivityManager.registerNetworkCallback(
                NetworkRequest.Builder()
                    .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
                    .build(),
                callback,
            )
            networkCallback = callback
            reportActiveNetworkWifi()
        } else {
            networkCallback?.let { callback ->
                runCatching { connectivityManager.unregisterNetworkCallback(callback) }
            }
            networkCallback = null
        }
    }

    private fun reportActiveNetworkWifi() {
        val active = connectivityManager.activeNetwork
        val caps = active?.let { connectivityManager.getNetworkCapabilities(it) }
        val isWifi = caps?.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) == true
        app.dispatch(HighlighterAppAction.NetworkPathChanged(isWifi))
    }
}

@Composable
private fun HighlighterTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = lightColorScheme(
            primary = Moss,
            onPrimary = Color.White,
            secondary = Gold,
            tertiary = Clay,
            background = Paper,
            surface = Color(0xFFFFFCF5),
            onBackground = Ink,
            onSurface = Ink,
            outline = Line,
        ),
        typography = MaterialTheme.typography,
        content = content,
    )
}

@Composable
private fun HighlighterAppScreen(
    state: HighlighterAppState,
    onRefresh: () -> Unit,
    onSignIn: (String) -> Unit,
    onStartNostrConnect: () -> Unit,
    onSetCreateAccountDisplayName: (String) -> Unit,
    onSetCreateAccountUsername: (String) -> Unit,
    onSubmitCreateAccount: () -> Unit,
    onLogout: () -> Unit,
    onClearToast: () -> Unit,
    onToggleOnboardingInterest: (String) -> Unit,
    onCompleteOnboarding: () -> Unit,
    onSearchQueryChange: (String) -> Unit,
    onSearchSubmit: (String) -> Unit,
    onClearSearch: () -> Unit,
    onClearRecentSearches: () -> Unit,
    onRefreshHomeFeed: () -> Unit,
    onRefreshBookmarks: () -> Unit,
    onRefreshRoomExplorer: () -> Unit,
    onSetNetworkWifiOnly: (Boolean) -> Unit,
    onReconnectNetwork: () -> Unit,
    onRefreshProfile: () -> Unit,
    onToggleProfileFollow: () -> Unit,
    onRefreshRoomBrowseAll: () -> Unit,
    onSubmitCreateRoom: (String, String, RoomVisibility, RoomAccess) -> Unit,
    onClearCreateRoomResult: () -> Unit,
    onClearCreateRoomError: () -> Unit,
    onRequestJoinRoom: (String, String) -> Unit,
    onOpenRoomInvite: (String) -> Unit,
    onRefreshRoomInvite: () -> Unit,
    onSetRoomInviteQuery: (String) -> Unit,
    onToggleRoomInviteCandidate: (String, HighlighterRoomInviteCandidateSource) -> Unit,
    onRemoveRoomInviteCandidate: (String) -> Unit,
    onAcceptRoomInvitePastedCandidate: () -> Unit,
    onMintRoomInviteLink: () -> Unit,
    onSubmitRoomInviteMembers: () -> Unit,
    onCloseRoomInvite: () -> Unit,
    onOpenRoom: (String) -> Unit,
    onRefreshRoom: () -> Unit,
    onPublishRoomDiscussion: (String, String) -> Unit,
    onPublishRoomChatMessage: (String) -> Unit,
    onLoadMoreRoomChat: () -> Unit,
    onCloseRoom: () -> Unit,
    onOpenComments: (String, String, UShort) -> Unit,
    onSetCommentDraft: (String?, String) -> Unit,
    onPublishComment: (String?) -> Unit,
    onToggleCommentLike: (String) -> Unit,
    onToggleCommentBookmark: (String) -> Unit,
    onRefreshComments: () -> Unit,
    onCloseComments: () -> Unit,
    onRefreshFeedback: () -> Unit,
    onSetFeedbackNewThreadDraft: (String) -> Unit,
    onPublishFeedbackNewThread: () -> Unit,
    onOpenFeedbackThread: (String) -> Unit,
    onSetFeedbackReplyDraft: (String) -> Unit,
    onPublishFeedbackReply: () -> Unit,
    onRefreshFeedbackThread: () -> Unit,
    onCloseFeedbackThread: () -> Unit,
    onRefreshMediaSettings: () -> Unit,
    onAddBlossomServer: (String) -> Unit,
    onRemoveBlossomServer: (String) -> Unit,
    onSearchBookPickerArtifacts: (String) -> Unit,
    onClearBookPickerSearch: () -> Unit,
    onUploadCapturePhoto: (ByteArray, String, UInt, UInt, String) -> Unit,
    onPublishCaptureHighlight: (HighlighterCaptureArtifact, String?, HighlightDraft) -> Unit,
    onPublishCapturePicture: (HighlighterCaptureArtifact?, String?, BlossomUpload, String) -> Unit,
    onClearCaptureResult: () -> Unit,
    onClearCaptureError: () -> Unit,
    onOpenArticle: (ArticleRecord) -> Unit,
    onOpenReadArticle: (String, String) -> Unit,
    onRefreshArticleReader: () -> Unit,
    onCloseArticleReader: () -> Unit,
    onPublishArticleHighlight: (String, String) -> Unit,
) {
    Scaffold(
        containerColor = Paper,
        topBar = {
            TopBar(
                chrome = state.chrome,
                isBootstrapping = state.isBootstrapping,
                onRefresh = onRefresh,
            )
        },
    ) { padding ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .background(Paper),
            verticalArrangement = Arrangement.spacedBy(14.dp),
            contentPadding = androidx.compose.foundation.layout.PaddingValues(18.dp),
        ) {
            item {
                AccountPanel(
                    auth = state.auth,
                    chrome = state.chrome,
                    createAccount = state.createAccount,
                    onSignIn = onSignIn,
                    onStartNostrConnect = onStartNostrConnect,
                    onSetCreateAccountDisplayName = onSetCreateAccountDisplayName,
                    onSetCreateAccountUsername = onSetCreateAccountUsername,
                    onSubmitCreateAccount = onSubmitCreateAccount,
                    onLogout = onLogout,
                )
            }
            state.toast?.let { toast ->
                item {
                    ToastBanner(message = toast.message, onClearToast = onClearToast)
                }
            }
            if (!state.onboarding.isComplete) {
                item {
                    OnboardingInterestsPanel(
                        onboarding = state.onboarding,
                        onToggleInterest = onToggleOnboardingInterest,
                        onComplete = onCompleteOnboarding,
                    )
                }
            } else {
                item {
                    MetricRow(chrome = state.chrome)
                }
                item {
                    NetworkPanel(
                        network = state.network,
                        onSetWifiOnly = onSetNetworkWifiOnly,
                        onReconnect = onReconnectNetwork,
                    )
                }
                item {
                    MediaSettingsPanel(
                        media = state.mediaSettings,
                        onRefresh = onRefreshMediaSettings,
                        onAddServer = onAddBlossomServer,
                        onRemoveServer = onRemoveBlossomServer,
                    )
                }
                item {
                    CreateRoomPanel(
                        createRoom = state.createRoom,
                        onSubmit = onSubmitCreateRoom,
                        onOpenRoom = onOpenRoom,
                        onOpenInvite = onOpenRoomInvite,
                        onClearResult = onClearCreateRoomResult,
                        onClearError = onClearCreateRoomError,
                    )
                }
                if (state.roomInvite.groupId.isNotBlank()) {
                    item {
                        RoomInvitePanel(
                            invite = state.roomInvite,
                            onRefresh = onRefreshRoomInvite,
                            onQueryChange = onSetRoomInviteQuery,
                            onToggleCandidate = onToggleRoomInviteCandidate,
                            onRemoveCandidate = onRemoveRoomInviteCandidate,
                            onAcceptPastedCandidate = onAcceptRoomInvitePastedCandidate,
                            onMintInviteLink = onMintRoomInviteLink,
                            onSubmitMembers = onSubmitRoomInviteMembers,
                            onClose = onCloseRoomInvite,
                        )
                    }
                }
                item {
                    CapturePanel(
                        capture = state.capture,
                        bookPicker = state.bookPicker,
                        communities = state.chrome.joinedCommunities,
                        onSearch = onSearchBookPickerArtifacts,
                        onClearSearch = onClearBookPickerSearch,
                        onUploadPhoto = onUploadCapturePhoto,
                        onPublishHighlight = onPublishCaptureHighlight,
                        onPublishPicture = onPublishCapturePicture,
                        onClearResult = onClearCaptureResult,
                        onClearError = onClearCaptureError,
                    )
                }
                item {
                    FeedbackPanel(
                        feedback = state.feedback,
                        onRefreshThreads = onRefreshFeedback,
                        onSetNewThreadDraft = onSetFeedbackNewThreadDraft,
                        onPublishNewThread = onPublishFeedbackNewThread,
                        onOpenThread = onOpenFeedbackThread,
                        onSetReplyDraft = onSetFeedbackReplyDraft,
                        onPublishReply = onPublishFeedbackReply,
                        onRefreshThread = onRefreshFeedbackThread,
                        onCloseThread = onCloseFeedbackThread,
                    )
                }
                if (state.profileView.pubkeyHex.isNotBlank()) {
                    item {
                        ProfilePanel(
                            profile = state.profileView,
                            onRefresh = onRefreshProfile,
                            onToggleFollow = onToggleProfileFollow,
                            onOpenArticle = onOpenArticle,
                        )
                    }
                }
                item {
                    SearchPanel(
                        search = state.search,
                        onQueryChange = onSearchQueryChange,
                        onSubmit = onSearchSubmit,
                        onClear = onClearSearch,
                        onClearRecentSearches = onClearRecentSearches,
                        onOpenArticle = onOpenArticle,
                    )
                }
                if (state.articleReader.address.isNotBlank()) {
                    item {
                        ArticleReaderPanel(
                            snapshot = state.articleReader,
                            onRefresh = onRefreshArticleReader,
                            onClose = onCloseArticleReader,
                            onPublishHighlight = onPublishArticleHighlight,
                        )
                    }
                }
                item {
                    HomeFeedPanel(
                        feed = state.homeFeed,
                        onRefresh = onRefreshHomeFeed,
                        onOpenReadArticle = onOpenReadArticle,
                    )
                }
                item {
                    BookmarkLibraryPanel(
                        bookmarks = state.bookmarks,
                        onRefresh = onRefreshBookmarks,
                        onOpenArticle = onOpenArticle,
                    )
                }
                item {
                    RoomExplorerPanel(
                        explorer = state.roomExplorer,
                        joinedRoomIds = state.chrome.joinedCommunities.map { it.id }.toSet(),
                        onRefresh = onRefreshRoomExplorer,
                        onBrowseAll = onRefreshRoomBrowseAll,
                        onJoin = onRequestJoinRoom,
                        onOpenRoom = onOpenRoom,
                    )
                }
                if (state.roomDetail.groupId.isNotBlank()) {
                    item {
                        RoomDetailPanel(
                            room = state.roomDetail,
                            onRefresh = onRefreshRoom,
                            onClose = onCloseRoom,
                            onPublishDiscussion = onPublishRoomDiscussion,
                            onPublishChat = onPublishRoomChatMessage,
                            onLoadMoreChat = onLoadMoreRoomChat,
                            onOpenComments = onOpenComments,
                        )
                    }
                }
                if (state.comments.rootTagValue.isNotBlank()) {
                    item {
                        CommentsPanel(
                            comments = state.comments,
                            onRefresh = onRefreshComments,
                            onSetDraft = onSetCommentDraft,
                            onPublish = onPublishComment,
                            onToggleLike = onToggleCommentLike,
                            onToggleBookmark = onToggleCommentBookmark,
                            onClose = onCloseComments,
                        )
                    }
                }
                item {
                    SectionHeader("Communities", state.chrome.joinedCommunitiesTotal.toString())
                }
                if (state.chrome.joinedCommunities.isEmpty()) {
                    item {
                        EmptyPanel("No joined communities")
                    }
                } else {
                    items(state.chrome.joinedCommunities, key = { it.id }) { community ->
                        CommunityRow(community = community, onOpenRoom = onOpenRoom)
                    }
                }
            }
        }
    }
}

@Composable
private fun TopBar(
    chrome: HighlighterChromeSnapshot,
    isBootstrapping: Boolean,
    onRefresh: () -> Unit,
) {
    Surface(color = Paper) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 18.dp, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = "Highlighter",
                    style = MaterialTheme.typography.titleLarge,
                    fontWeight = FontWeight.SemiBold,
                    color = Ink,
                )
                Text(
                    text = chrome.connectionState.statusLabel(isBootstrapping),
                    style = MaterialTheme.typography.bodySmall,
                    color = Muted,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Spacer(modifier = Modifier.width(12.dp))
            OutlinedButton(
                onClick = onRefresh,
                shape = RoundedCornerShape(8.dp),
                border = BorderStroke(1.dp, Line),
            ) {
                Text("Refresh")
            }
        }
    }
}

@Composable
private fun AccountPanel(
    auth: HighlighterAuthSnapshot,
    chrome: HighlighterChromeSnapshot,
    createAccount: HighlighterCreateAccountSnapshot,
    onSignIn: (String) -> Unit,
    onStartNostrConnect: () -> Unit,
    onSetCreateAccountDisplayName: (String) -> Unit,
    onSetCreateAccountUsername: (String) -> Unit,
    onSubmitCreateAccount: () -> Unit,
    onLogout: () -> Unit,
) {
    var nsec by remember { mutableStateOf("") }
    val user = chrome.currentUser
    Panel {
        if (user == null) {
            Text(
                text = "Sign in",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
            )
            Spacer(modifier = Modifier.height(10.dp))
            CreateAccountForm(
                createAccount = createAccount,
                onSetDisplayName = onSetCreateAccountDisplayName,
                onSetUsername = onSetCreateAccountUsername,
                onSubmit = onSubmitCreateAccount,
            )
            Spacer(modifier = Modifier.height(18.dp))
            OutlinedButton(
                onClick = onStartNostrConnect,
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(8.dp),
                enabled = !auth.isSigningIn,
            ) {
                Text("Continue with signer")
            }
            Spacer(modifier = Modifier.height(10.dp))
            OutlinedTextField(
                value = nsec,
                onValueChange = { nsec = it.trim() },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                label = { Text("nsec") },
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(
                    capitalization = KeyboardCapitalization.None,
                    keyboardType = KeyboardType.Password,
                ),
            )
            Spacer(modifier = Modifier.height(10.dp))
            Button(
                onClick = {
                    val value = nsec.trim()
                    if (value.isNotEmpty()) {
                        onSignIn(value)
                        nsec = ""
                    }
                },
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(8.dp),
                enabled = !auth.isSigningIn,
            ) {
                Text(if (auth.isSigningIn) "Signing in..." else "Sign in")
            }
        } else {
            Text(
                text = chrome.currentUserProfile?.displayName
                    ?.takeIf { it.isNotBlank() }
                    ?: chrome.currentUserProfile?.name
                        ?.takeIf { it.isNotBlank() }
                    ?: user.npub,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Spacer(modifier = Modifier.height(4.dp))
            Text(
                text = user.npub,
                style = MaterialTheme.typography.bodySmall,
                color = Muted,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Spacer(modifier = Modifier.height(12.dp))
            OutlinedButton(
                onClick = onLogout,
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(8.dp),
            ) {
                Text("Sign out")
            }
        }
    }
}

@Composable
private fun CreateAccountForm(
    createAccount: HighlighterCreateAccountSnapshot,
    onSetDisplayName: (String) -> Unit,
    onSetUsername: (String) -> Unit,
    onSubmit: () -> Unit,
) {
    Text(
        text = "Create account",
        style = MaterialTheme.typography.labelLarge,
        color = Muted,
    )
    Spacer(modifier = Modifier.height(8.dp))
    OutlinedTextField(
        value = createAccount.displayName,
        onValueChange = onSetDisplayName,
        modifier = Modifier.fillMaxWidth(),
        singleLine = true,
        label = { Text("Display name") },
        keyboardOptions = KeyboardOptions(
            capitalization = KeyboardCapitalization.Words,
            keyboardType = KeyboardType.Text,
            imeAction = ImeAction.Next,
        ),
    )
    Spacer(modifier = Modifier.height(8.dp))
    OutlinedTextField(
        value = createAccount.username,
        onValueChange = onSetUsername,
        modifier = Modifier.fillMaxWidth(),
        singleLine = true,
        label = { Text("Username") },
        supportingText = { CreateAccountUsernameStatus(createAccount) },
        isError = createAccount.usernameStatus == HighlighterUsernameStatus.TAKEN ||
            createAccount.usernameStatus == HighlighterUsernameStatus.INVALID ||
            createAccount.usernameStatus == HighlighterUsernameStatus.ERROR,
        keyboardOptions = KeyboardOptions(
            capitalization = KeyboardCapitalization.None,
            keyboardType = KeyboardType.Ascii,
            imeAction = ImeAction.Done,
        ),
        keyboardActions = KeyboardActions(
            onDone = {
                if (createAccount.canSubmit && !createAccount.isCreating) {
                    onSubmit()
                }
            },
        ),
    )
    createAccount.errorMessage?.let { message ->
        Spacer(modifier = Modifier.height(6.dp))
        Text(
            text = message,
            style = MaterialTheme.typography.bodySmall,
            color = Clay,
        )
    }
    Spacer(modifier = Modifier.height(10.dp))
    Button(
        onClick = onSubmit,
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(8.dp),
        enabled = createAccount.canSubmit && !createAccount.isCreating &&
            createAccount.usernameStatus != HighlighterUsernameStatus.CHECKING,
    ) {
        Text(if (createAccount.isCreating) "Creating..." else "Create account")
    }
}

@Composable
private fun CreateAccountUsernameStatus(createAccount: HighlighterCreateAccountSnapshot) {
    val text = when (createAccount.usernameStatus) {
        HighlighterUsernameStatus.CHECKING -> "Checking availability"
        HighlighterUsernameStatus.AVAILABLE -> createAccount.usernameIdentifier
        HighlighterUsernameStatus.TAKEN -> "Already taken"
        HighlighterUsernameStatus.INVALID -> "Only letters, numbers, - and _"
        HighlighterUsernameStatus.ERROR -> createAccount.errorMessage ?: "Could not check username"
        HighlighterUsernameStatus.IDLE -> if (createAccount.usernameIdentifier.isNotBlank()) {
            createAccount.usernameIdentifier
        } else {
            "Optional Nostr username"
        }
    }
    Text(text = text, color = Muted)
}

@Composable
@OptIn(ExperimentalLayoutApi::class)
private fun OnboardingInterestsPanel(
    onboarding: HighlighterOnboardingSnapshot,
    onToggleInterest: (String) -> Unit,
    onComplete: () -> Unit,
) {
    Panel {
        Text(
            text = "What do you read?",
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
            color = Ink,
        )
        Spacer(modifier = Modifier.height(6.dp))
        Text(
            text = "Pick at least ${onboarding.minimumSelectionCount} to pre-fill your feed with highlights from readers like you.",
            style = MaterialTheme.typography.bodyMedium,
            color = Muted,
        )
        Spacer(modifier = Modifier.height(14.dp))
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            onboarding.interests.forEach { interest ->
                OnboardingInterestChip(
                    interest = interest,
                    onToggleInterest = onToggleInterest,
                )
            }
        }
        Spacer(modifier = Modifier.height(14.dp))
        if (onboarding.remainingSelectionCount > 0u) {
            Text(
                text = "Choose ${onboarding.remainingSelectionCount} more",
                style = MaterialTheme.typography.labelMedium,
                color = Muted,
            )
            Spacer(modifier = Modifier.height(8.dp))
        }
        Button(
            onClick = onComplete,
            enabled = onboarding.canFinish && !onboarding.isFinishing,
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(8.dp),
        ) {
            Text(if (onboarding.isFinishing) "Finishing..." else "Start exploring")
        }
    }
}

@Composable
private fun OnboardingInterestChip(
    interest: HighlighterOnboardingInterest,
    onToggleInterest: (String) -> Unit,
) {
    OutlinedButton(
        onClick = { onToggleInterest(interest.id) },
        shape = RoundedCornerShape(8.dp),
        border = BorderStroke(1.dp, if (interest.selected) Moss else Line),
        colors = ButtonDefaults.outlinedButtonColors(
            containerColor = if (interest.selected) Moss else Color(0xFFFFFCF5),
            contentColor = if (interest.selected) Color.White else Ink,
        ),
    ) {
        Text(
            text = "${interest.emoji} ${interest.label}",
            style = MaterialTheme.typography.labelLarge,
            fontWeight = if (interest.selected) FontWeight.SemiBold else FontWeight.Normal,
        )
    }
}

@Composable
private fun ProfilePanel(
    profile: HighlighterProfileViewSnapshot,
    onRefresh: () -> Unit,
    onToggleFollow: () -> Unit,
    onOpenArticle: (ArticleRecord) -> Unit,
) {
    Panel {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            SectionHeader("Profile", profile.pubkeyHex.take(8))
        }
        Spacer(modifier = Modifier.height(10.dp))
        Row(verticalAlignment = Alignment.Top) {
            Box(
                modifier = Modifier
                    .size(56.dp)
                    .clip(CircleShape)
                    .background(Moss.copy(alpha = 0.14f)),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = profile.displayName().firstOrNull()?.uppercase() ?: "?",
                    color = Moss,
                    fontWeight = FontWeight.Bold,
                )
            }
            Spacer(modifier = Modifier.width(12.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = profile.displayName(),
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = Ink,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                profile.profile?.nip05?.takeIf { it.isNotBlank() }?.let { nip05 ->
                    Spacer(modifier = Modifier.height(3.dp))
                    Text(
                        text = nip05.removePrefix("_@"),
                        style = MaterialTheme.typography.bodySmall,
                        color = Muted,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                profile.profile?.about?.takeIf { it.isNotBlank() }?.let { about ->
                    Spacer(modifier = Modifier.height(6.dp))
                    Text(
                        text = about,
                        style = MaterialTheme.typography.bodyMedium,
                        color = Muted,
                        maxLines = 4,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
        Spacer(modifier = Modifier.height(12.dp))
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            ProfileStat("Writing", profile.articleCount.toString(), Modifier.weight(1f))
            ProfileStat("Highlights", profile.highlightCount.toString(), Modifier.weight(1f))
            ProfileStat("Rooms", profile.communityCount.toString(), Modifier.weight(1f))
        }
        Spacer(modifier = Modifier.height(12.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(
                onClick = onRefresh,
                shape = RoundedCornerShape(8.dp),
                border = BorderStroke(1.dp, Line),
                enabled = !profile.isLoading && !profile.isMutatingFollow,
            ) {
                Text(if (profile.isLoading) "Refreshing" else "Refresh")
            }
            if (!profile.isOwnProfile && profile.viewerPubkeyHex != null) {
                Button(
                    onClick = onToggleFollow,
                    shape = RoundedCornerShape(8.dp),
                    enabled = !profile.isMutatingFollow,
                ) {
                    Text(
                        when {
                            profile.isMutatingFollow -> "Saving"
                            profile.isFollowing -> "Following"
                            else -> "Follow"
                        },
                    )
                }
            }
        }
        profile.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = message,
                style = MaterialTheme.typography.bodySmall,
                color = Clay,
            )
        }
        if (profile.articles.isNotEmpty()) {
            SearchGroupHeader("Writing", profile.articleCount.toString())
            profile.articles.take(3).forEach { article ->
                ArticleSearchRow(article, onOpenArticle)
            }
        }
        if (profile.highlights.isNotEmpty()) {
            SearchGroupHeader("Highlights", profile.highlightCount.toString())
            profile.highlights.take(3).forEach { highlight ->
                HighlightSearchRow(highlight)
            }
        }
        if (profile.communities.isNotEmpty()) {
            SearchGroupHeader("Communities", profile.communityCount.toString())
            profile.communities.take(3).forEach { community ->
                CommunitySearchRow(community)
            }
        }
        if (profile.isLoading &&
            profile.articles.isEmpty() &&
            profile.highlights.isEmpty() &&
            profile.communities.isEmpty()
        ) {
            Spacer(modifier = Modifier.height(10.dp))
            Text(
                text = "Loading profile",
                style = MaterialTheme.typography.bodyMedium,
                color = Muted,
            )
        }
    }
}

@Composable
private fun ProfileStat(label: String, value: String, modifier: Modifier = Modifier) {
    Column(modifier = modifier) {
        Text(text = value, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold)
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = Muted,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
private fun SearchPanel(
    search: HighlighterSearchSnapshot,
    onQueryChange: (String) -> Unit,
    onSubmit: (String) -> Unit,
    onClear: () -> Unit,
    onClearRecentSearches: () -> Unit,
    onOpenArticle: (ArticleRecord) -> Unit,
) {
    val hasResults = search.highlights.isNotEmpty() ||
        search.articles.isNotEmpty() ||
        search.communities.isNotEmpty() ||
        search.profiles.isNotEmpty()
    Panel {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = "Search",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
            )
            if (search.query.isNotBlank()) {
                TextButton(onClick = onClear) {
                    Text("Clear")
                }
            }
        }
        Spacer(modifier = Modifier.height(10.dp))
        OutlinedTextField(
            value = search.query,
            onValueChange = onQueryChange,
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text("Quotes, essays, people, rooms") },
            keyboardOptions = KeyboardOptions(
                capitalization = KeyboardCapitalization.None,
                keyboardType = KeyboardType.Text,
                imeAction = ImeAction.Search,
            ),
            keyboardActions = KeyboardActions(
                onSearch = { onSubmit(search.query) },
            ),
        )
        Spacer(modifier = Modifier.height(12.dp))
        when {
            search.query.isBlank() -> SearchHint(
                search = search,
                onSubmit = onSubmit,
                onClearRecentSearches = onClearRecentSearches,
            )
            search.isLocalLoading && !hasResults -> Text(
                text = "Searching...",
                style = MaterialTheme.typography.bodyMedium,
                color = Muted,
            )
            !hasResults && !search.isRelayLoading -> Text(
                text = "No results yet",
                style = MaterialTheme.typography.bodyMedium,
                color = Muted,
            )
            else -> SearchResults(search = search, onOpenArticle = onOpenArticle)
        }
    }
}

@Composable
private fun SearchHint(
    search: HighlighterSearchSnapshot,
    onSubmit: (String) -> Unit,
    onClearRecentSearches: () -> Unit,
) {
    if (search.recentQueries.isNotEmpty()) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = "Recent",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
                color = Ink,
            )
            Text(
                text = search.recentQueryCount.toString(),
                style = MaterialTheme.typography.labelLarge,
                color = Muted,
            )
            TextButton(onClick = onClearRecentSearches) {
                Text("Clear")
            }
        }
        search.recentQueries.forEach { query ->
            TextButton(onClick = { onSubmit(query) }) {
                Text(
                    text = query,
                    modifier = Modifier.fillMaxWidth(),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        Spacer(modifier = Modifier.height(10.dp))
    }
    Text(
        text = if (search.searchRelays.isEmpty()) {
            "Search your local Highlighter library."
        } else {
            "Search your local library and configured NIP-50 relays."
        },
        style = MaterialTheme.typography.bodyMedium,
        color = Muted,
    )
}

@Composable
private fun SearchResults(
    search: HighlighterSearchSnapshot,
    onOpenArticle: (ArticleRecord) -> Unit,
) {
    if (search.highlights.isNotEmpty()) {
        SearchGroupHeader("Highlights", search.highlightCount.toString())
        search.highlights.take(3).forEach { highlight ->
            HighlightSearchRow(highlight)
        }
    }
    if (search.articles.isNotEmpty()) {
        SearchGroupHeader("Articles", search.articleCount.toString())
        search.articles.take(3).forEach { article ->
            ArticleSearchRow(article, onOpenArticle)
        }
    }
    if (search.communities.isNotEmpty()) {
        SearchGroupHeader("Communities", search.communityCount.toString())
        search.communities.take(3).forEach { community ->
            CommunitySearchRow(community)
        }
    }
    if (search.profiles.isNotEmpty()) {
        SearchGroupHeader("People", search.profileCount.toString())
        search.profiles.take(3).forEach { profile ->
            ProfileSearchRow(profile)
        }
    }
    if (search.isRelayLoading) {
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "Checking relays...",
            style = MaterialTheme.typography.bodySmall,
            color = Muted,
        )
    }
}

@Composable
private fun SearchGroupHeader(title: String, count: String) {
    Spacer(modifier = Modifier.height(12.dp))
    SectionHeader(title, count)
}

@Composable
private fun HighlightSearchRow(highlight: HighlightRecord) {
    SearchResultRow(
        title = highlight.quote,
        subtitle = highlight.note.ifBlank { highlight.sourceUrl },
    )
}

@Composable
private fun ArticleSearchRow(article: ArticleRecord, onOpenArticle: (ArticleRecord) -> Unit) {
    SearchResultRow(
        title = article.title.ifBlank { article.identifier },
        subtitle = article.summary,
        onClick = { onOpenArticle(article) },
    )
}

@Composable
private fun CommunitySearchRow(community: CommunitySummary) {
    SearchResultRow(
        title = community.name.ifBlank { community.id },
        subtitle = community.about,
    )
}

@Composable
private fun ProfileSearchRow(profile: ProfileMetadata) {
    SearchResultRow(
        title = profile.displayName.ifBlank { profile.name.ifBlank { profile.pubkey } },
        subtitle = profile.about.ifBlank { profile.nip05 },
    )
}

@Composable
private fun SearchResultRow(title: String, subtitle: String, onClick: (() -> Unit)? = null) {
    val modifier = if (onClick == null) {
        Modifier.padding(vertical = 7.dp)
    } else {
        Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(vertical = 7.dp)
    }
    Column(modifier = modifier) {
        Text(
            text = title.ifBlank { "Untitled" },
            style = MaterialTheme.typography.bodyMedium,
            color = Ink,
            fontWeight = FontWeight.Medium,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
        if (subtitle.isNotBlank()) {
            Spacer(modifier = Modifier.height(3.dp))
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodySmall,
                color = Muted,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
private fun ArticleReaderPanel(
    snapshot: HighlighterArticleReaderSnapshot,
    onRefresh: () -> Unit,
    onClose: () -> Unit,
    onPublishHighlight: (String, String) -> Unit,
) {
    val article = snapshot.article
    var quote by remember(snapshot.address) { mutableStateOf("") }
    var note by remember(snapshot.address) { mutableStateOf("") }

    Panel {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            SectionHeader("Reader", snapshot.highlightCount.toString())
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(
                onClick = onRefresh,
                shape = RoundedCornerShape(8.dp),
                border = BorderStroke(1.dp, Line),
                enabled = !snapshot.isLoading,
            ) {
                Text(if (snapshot.isLoading) "Refreshing" else "Refresh")
            }
            TextButton(onClick = onClose) {
                Text("Close")
            }
        }
        snapshot.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = message,
                style = MaterialTheme.typography.bodySmall,
                color = Clay,
            )
        }
        when {
            snapshot.isLoading && article == null -> {
                Spacer(modifier = Modifier.height(10.dp))
                EmptyPanel("Loading article")
            }
            article == null -> {
                Spacer(modifier = Modifier.height(10.dp))
                EmptyPanel("Article unavailable")
            }
            else -> {
                Spacer(modifier = Modifier.height(12.dp))
                Text(
                    text = article.title.ifBlank { article.identifier },
                    style = MaterialTheme.typography.headlineSmall,
                    color = Ink,
                    fontWeight = FontWeight.SemiBold,
                )
                val authorName = snapshot.authorProfile?.displayName?.takeIf { it.isNotBlank() }
                    ?: snapshot.authorProfile?.name?.takeIf { it.isNotBlank() }
                    ?: article.pubkey.take(12)
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = authorName,
                    style = MaterialTheme.typography.bodySmall,
                    color = Muted,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (article.summary.isNotBlank()) {
                    Spacer(modifier = Modifier.height(10.dp))
                    Text(
                        text = article.summary,
                        style = MaterialTheme.typography.bodyLarge,
                        color = Muted,
                    )
                }
                Spacer(modifier = Modifier.height(14.dp))
                SelectionContainer {
                    Text(
                        text = article.content,
                        style = MaterialTheme.typography.bodyMedium,
                        color = Ink,
                    )
                }
                if (snapshot.highlights.isNotEmpty()) {
                    Spacer(modifier = Modifier.height(14.dp))
                    SearchGroupHeader("Highlights", snapshot.highlightCount.toString())
                    snapshot.highlights.take(8).forEach { highlight ->
                        HighlightSearchRow(highlight)
                    }
                }
                Spacer(modifier = Modifier.height(14.dp))
                OutlinedTextField(
                    value = quote,
                    onValueChange = { quote = it },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text("Quote") },
                    minLines = 2,
                    maxLines = 5,
                    keyboardOptions = KeyboardOptions(
                        capitalization = KeyboardCapitalization.Sentences,
                        keyboardType = KeyboardType.Text,
                    ),
                )
                Spacer(modifier = Modifier.height(8.dp))
                OutlinedTextField(
                    value = note,
                    onValueChange = { note = it },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text("Note") },
                    minLines = 1,
                    maxLines = 4,
                    keyboardOptions = KeyboardOptions(
                        capitalization = KeyboardCapitalization.Sentences,
                        keyboardType = KeyboardType.Text,
                    ),
                )
                Spacer(modifier = Modifier.height(8.dp))
                Button(
                    onClick = {
                        val cleanQuote = quote.trim()
                        val cleanNote = note.trim()
                        if (cleanQuote.isNotEmpty()) {
                            onPublishHighlight(cleanQuote, cleanNote)
                            quote = ""
                            note = ""
                        }
                    },
                    shape = RoundedCornerShape(8.dp),
                    enabled = quote.isNotBlank() && !snapshot.isPublishingHighlight,
                ) {
                    Text(if (snapshot.isPublishingHighlight) "Saving" else "Highlight")
                }
            }
        }
    }
}

@Composable
private fun HomeFeedPanel(
    feed: HighlighterHomeFeedSnapshot,
    onRefresh: () -> Unit,
    onOpenReadArticle: (String, String) -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            SectionHeader("Highlights", feed.itemCount.toString())
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(
                onClick = onRefresh,
                shape = RoundedCornerShape(8.dp),
                border = BorderStroke(1.dp, Line),
                enabled = !feed.isLoading,
            ) {
                Text(if (feed.isLoading) "Refreshing" else "Refresh")
            }
        }
        feed.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Text(
                text = message,
                style = MaterialTheme.typography.bodySmall,
                color = Clay,
            )
        }
        when {
            feed.isLoading && feed.items.isEmpty() -> EmptyPanel("Loading highlights")
            feed.items.isEmpty() -> EmptyPanel("No highlights yet")
            else -> feed.items.take(8).forEach { item ->
                HomeFeedRow(item, onOpenReadArticle)
            }
        }
    }
}

@Composable
private fun HomeFeedRow(
    item: HighlighterHomeFeedItem,
    onOpenReadArticle: (String, String) -> Unit,
) {
    when (item.kind) {
        HighlighterHomeFeedItemKind.HIGHLIGHTS -> {
            val lead = item.highlights.firstOrNull()?.highlight
            if (lead != null) {
                Surface(
                    modifier = Modifier.fillMaxWidth(),
                    color = Color(0xFFFFFCF5),
                    shape = RoundedCornerShape(8.dp),
                    border = BorderStroke(1.dp, Line),
                ) {
                    Column(modifier = Modifier.padding(12.dp)) {
                        Text(
                            text = lead.quote.ifBlank { "Untitled highlight" },
                            style = MaterialTheme.typography.bodyLarge,
                            color = Ink,
                            fontWeight = FontWeight.Medium,
                            maxLines = 3,
                            overflow = TextOverflow.Ellipsis,
                        )
                        Spacer(modifier = Modifier.height(6.dp))
                        Text(
                            text = item.highlightCount.feedCountLabel("highlight"),
                            style = MaterialTheme.typography.bodySmall,
                            color = Muted,
                        )
                    }
                }
            }
        }
        HighlighterHomeFeedItemKind.READ -> {
            val read = item.read ?: return
            Surface(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { onOpenReadArticle(read.pubkey, read.identifier) },
                color = Color(0xFFFFFCF5),
                shape = RoundedCornerShape(8.dp),
                border = BorderStroke(1.dp, Line),
            ) {
                Column(modifier = Modifier.padding(12.dp)) {
                    Text(
                        text = read.title.ifBlank { read.identifier },
                        style = MaterialTheme.typography.bodyLarge,
                        color = Ink,
                        fontWeight = FontWeight.Medium,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                    if (read.summary.isNotBlank()) {
                        Spacer(modifier = Modifier.height(5.dp))
                        Text(
                            text = read.summary,
                            style = MaterialTheme.typography.bodySmall,
                            color = Muted,
                            maxLines = 2,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun BookmarkLibraryPanel(
    bookmarks: HighlighterBookmarksSnapshot,
    onRefresh: () -> Unit,
    onOpenArticle: (ArticleRecord) -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        SectionHeader("Bookmarks", bookmarks.articleCount.toString())
        OutlinedButton(
            onClick = onRefresh,
            shape = RoundedCornerShape(8.dp),
            border = BorderStroke(1.dp, Line),
            enabled = !bookmarks.isLoading,
        ) {
            Text(if (bookmarks.isLoading) "Refreshing" else "Refresh")
        }
        bookmarks.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Text(
                text = message,
                style = MaterialTheme.typography.bodySmall,
                color = Clay,
            )
        }
        when {
            bookmarks.isLoading && bookmarks.articles.isEmpty() -> EmptyPanel("Loading bookmarks")
            bookmarks.articles.isEmpty() -> EmptyPanel("No bookmarked articles")
            else -> {
                SearchGroupHeader("Articles", bookmarks.articleCount.toString())
                bookmarks.articles.take(5).forEach { article ->
                    ArticleBookmarkRow(article, onOpenArticle)
                }
            }
        }
        val myCollections = bookmarks.myBookmarkSets + bookmarks.myCurationSets
        if (myCollections.isNotEmpty()) {
            SearchGroupHeader(
                "Collections",
                (bookmarks.myBookmarkSetCount + bookmarks.myCurationSetCount).toString(),
            )
            myCollections.take(5).forEach { collection ->
                BookmarkCollectionRow(collection)
            }
        }
        if (bookmarks.webBookmarks.isNotEmpty()) {
            SearchGroupHeader("Web", bookmarks.webBookmarkCount.toString())
            bookmarks.webBookmarks.take(5).forEach { bookmark ->
                WebBookmarkRow(bookmark)
            }
        }
        if (bookmarks.followingCurationSets.isNotEmpty()) {
            SearchGroupHeader("Explore", bookmarks.followingCurationSetCount.toString())
            bookmarks.followingCurationSets.take(5).forEach { collection ->
                BookmarkCollectionRow(collection)
            }
        }
    }
}

@Composable
private fun ArticleBookmarkRow(article: ArticleRecord, onOpenArticle: (ArticleRecord) -> Unit) {
    SearchResultRow(
        title = article.title.ifBlank { article.identifier },
        subtitle = article.summary.ifBlank { article.pubkey },
        onClick = { onOpenArticle(article) },
    )
}

@Composable
private fun BookmarkCollectionRow(record: BookmarkSetRecord) {
    val title = record.title.ifBlank { record.id.ifBlank { "Untitled" } }
    val itemCount = record.articleAddresses.size + record.noteIds.size
    SearchResultRow(
        title = title,
        subtitle = when {
            record.description.isNotBlank() -> record.description
            itemCount == 1 -> "1 item"
            else -> "$itemCount items"
        },
    )
}

@Composable
private fun WebBookmarkRow(bookmark: WebBookmarkRecord) {
    SearchResultRow(
        title = bookmark.title.ifBlank { bookmark.url },
        subtitle = bookmark.description.ifBlank { bookmark.url },
    )
}

@Composable
private fun RoomExplorerPanel(
    explorer: HighlighterRoomExplorerSnapshot,
    joinedRoomIds: Set<String>,
    onRefresh: () -> Unit,
    onBrowseAll: () -> Unit,
    onJoin: (String, String) -> Unit,
    onOpenRoom: (String) -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            SectionHeader(
                title = "Rooms",
                count = (
                    explorer.featuredCount +
                        explorer.newNoteworthyCount +
                        explorer.friendsShelfCount +
                        explorer.authorsShelfCount
                    ).toString(),
            )
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(
                onClick = onRefresh,
                shape = RoundedCornerShape(8.dp),
                border = BorderStroke(1.dp, Line),
                enabled = !explorer.isLoading,
            ) {
                Text(if (explorer.isLoading) "Refreshing" else "Refresh")
            }
            OutlinedButton(
                onClick = onBrowseAll,
                shape = RoundedCornerShape(8.dp),
                border = BorderStroke(1.dp, Line),
                enabled = !explorer.isBrowseLoading,
            ) {
                Text(if (explorer.isBrowseLoading) "Loading" else "Browse all")
            }
        }
        explorer.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Text(
                text = message,
                style = MaterialTheme.typography.bodySmall,
                color = Clay,
            )
        }
        if (explorer.isLoading &&
            explorer.featured.isEmpty() &&
            explorer.newNoteworthy.isEmpty() &&
            explorer.friendsShelf.isEmpty() &&
            explorer.authorsShelf.isEmpty()
        ) {
            EmptyPanel("Loading rooms")
        }
        RoomShelf(
            title = "Featured",
            count = explorer.featuredCount,
            rooms = explorer.featured,
            joinedRoomIds = joinedRoomIds,
            onJoin = onJoin,
            onOpenRoom = onOpenRoom,
        )
        RecommendationShelf(
            title = "Friends are here",
            count = explorer.friendsShelfCount,
            recommendations = explorer.friendsShelf,
            joinedRoomIds = joinedRoomIds,
            onJoin = onJoin,
            onOpenRoom = onOpenRoom,
        )
        RecommendationShelf(
            title = "Writers you read",
            count = explorer.authorsShelfCount,
            recommendations = explorer.authorsShelf,
            joinedRoomIds = joinedRoomIds,
            onJoin = onJoin,
            onOpenRoom = onOpenRoom,
        )
        RoomShelf(
            title = "New & noteworthy",
            count = explorer.newNoteworthyCount,
            rooms = explorer.newNoteworthy,
            joinedRoomIds = joinedRoomIds,
            onJoin = onJoin,
            onOpenRoom = onOpenRoom,
        )
        if (explorer.allRooms.isNotEmpty()) {
            RoomShelf(
                title = "Browse all",
                count = explorer.allRoomCount,
                rooms = explorer.allRooms.take(12),
                joinedRoomIds = joinedRoomIds,
                onJoin = onJoin,
                onOpenRoom = onOpenRoom,
            )
        }
    }
}

@Composable
private fun RoomShelf(
    title: String,
    count: ULong,
    rooms: List<CommunitySummary>,
    joinedRoomIds: Set<String>,
    onJoin: (String, String) -> Unit,
    onOpenRoom: (String) -> Unit,
) {
    if (rooms.isEmpty()) {
        return
    }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        SectionHeader(title, count.toString())
        LazyRow(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            items(rooms, key = { it.id }) { room ->
                RoomTile(
                    room = room,
                    subtitle = room.about,
                    isJoined = joinedRoomIds.contains(room.id),
                    onJoin = onJoin,
                    onOpenRoom = onOpenRoom,
                )
            }
        }
    }
}

@Composable
private fun RecommendationShelf(
    title: String,
    count: ULong,
    recommendations: List<RoomRecommendation>,
    joinedRoomIds: Set<String>,
    onJoin: (String, String) -> Unit,
    onOpenRoom: (String) -> Unit,
) {
    if (recommendations.isEmpty()) {
        return
    }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        SectionHeader(title, count.toString())
        LazyRow(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            items(recommendations, key = { it.summary.id }) { recommendation ->
                RoomTile(
                    room = recommendation.summary,
                    subtitle = recommendation.signalLabel(),
                    isJoined = joinedRoomIds.contains(recommendation.summary.id),
                    onJoin = onJoin,
                    onOpenRoom = onOpenRoom,
                )
            }
        }
    }
}

@Composable
private fun RoomTile(
    room: CommunitySummary,
    subtitle: String,
    isJoined: Boolean,
    onJoin: (String, String) -> Unit,
    onOpenRoom: (String) -> Unit,
) {
    Surface(
        modifier = Modifier
            .width(220.dp)
            .clickable { onOpenRoom(room.id) },
        color = Color(0xFFFFFCF5),
        shape = RoundedCornerShape(8.dp),
        border = BorderStroke(1.dp, Line),
    ) {
        Column(modifier = Modifier.padding(12.dp)) {
            Text(
                text = room.name.ifBlank { room.id },
                style = MaterialTheme.typography.titleSmall,
                fontWeight = FontWeight.SemiBold,
                color = Ink,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Spacer(modifier = Modifier.height(5.dp))
            Text(
                text = subtitle.ifBlank { room.id },
                style = MaterialTheme.typography.bodySmall,
                color = Muted,
                minLines = 2,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            Spacer(modifier = Modifier.height(10.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Chip(room.access)
                TextButton(
                    onClick = { onJoin(room.id, room.name) },
                    enabled = !isJoined,
                ) {
                    Text(if (isJoined) "Joined" else "Join")
                }
            }
        }
    }
}

@Composable
private fun CreateRoomPanel(
    createRoom: HighlighterCreateRoomSnapshot,
    onSubmit: (String, String, RoomVisibility, RoomAccess) -> Unit,
    onOpenRoom: (String) -> Unit,
    onOpenInvite: (String) -> Unit,
    onClearResult: () -> Unit,
    onClearError: () -> Unit,
) {
    var name by remember { mutableStateOf("") }
    var about by remember { mutableStateOf("") }
    var visibility by remember { mutableStateOf(RoomVisibility.PUBLIC) }
    var access by remember { mutableStateOf(RoomAccess.OPEN) }
    Panel {
        SectionHeader("Create room", if (createRoom.isCreating) "Saving" else "NIP-29")
        Spacer(modifier = Modifier.height(10.dp))
        OutlinedTextField(
            value = name,
            onValueChange = { name = it },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text("Name") },
        )
        Spacer(modifier = Modifier.height(8.dp))
        OutlinedTextField(
            value = about,
            onValueChange = { about = it },
            modifier = Modifier.fillMaxWidth(),
            minLines = 2,
            maxLines = 4,
            label = { Text("About") },
        )
        Spacer(modifier = Modifier.height(10.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            ToggleButton("Public", visibility == RoomVisibility.PUBLIC) {
                visibility = RoomVisibility.PUBLIC
            }
            ToggleButton("Private", visibility == RoomVisibility.PRIVATE) {
                visibility = RoomVisibility.PRIVATE
            }
        }
        Spacer(modifier = Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            ToggleButton("Open", access == RoomAccess.OPEN) {
                access = RoomAccess.OPEN
            }
            ToggleButton("Closed", access == RoomAccess.CLOSED) {
                access = RoomAccess.CLOSED
            }
        }
        createRoom.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(8.dp))
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = Clay)
            TextButton(onClick = onClearError) {
                Text("Dismiss")
            }
        }
        createRoom.createdGroupId?.takeIf { it.isNotBlank() }?.let { groupId ->
            Spacer(modifier = Modifier.height(10.dp))
            Text(
                text = "Created ${groupId.take(12)}",
                style = MaterialTheme.typography.bodySmall,
                color = Muted,
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TextButton(onClick = { onOpenRoom(groupId) }) {
                    Text("Open")
                }
                TextButton(onClick = { onOpenInvite(groupId) }) {
                    Text("Invite")
                }
                TextButton(onClick = onClearResult) {
                    Text("Clear")
                }
            }
        }
        Spacer(modifier = Modifier.height(10.dp))
        Button(
            onClick = {
                val cleanName = name.trim()
                if (cleanName.isNotEmpty()) {
                    onSubmit(cleanName, about.trim(), visibility, access)
                }
            },
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(8.dp),
            enabled = name.isNotBlank() && !createRoom.isCreating,
        ) {
            Text(if (createRoom.isCreating) "Creating" else "Create")
        }
    }
}

@Composable
private fun RoomInvitePanel(
    invite: HighlighterRoomInviteSnapshot,
    onRefresh: () -> Unit,
    onQueryChange: (String) -> Unit,
    onToggleCandidate: (String, HighlighterRoomInviteCandidateSource) -> Unit,
    onRemoveCandidate: (String) -> Unit,
    onAcceptPastedCandidate: () -> Unit,
    onMintInviteLink: () -> Unit,
    onSubmitMembers: () -> Unit,
    onClose: () -> Unit,
) {
    Panel {
        SectionHeader("Invites", invite.selected.size.toString())
        Spacer(modifier = Modifier.height(8.dp))
        OutlinedTextField(
            value = invite.query,
            onValueChange = onQueryChange,
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text("Search follows or paste npub") },
        )
        invite.pastedCandidate?.let { candidate ->
            Spacer(modifier = Modifier.height(8.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = candidate.pubkeyHex.take(16),
                    modifier = Modifier.weight(1f),
                    style = MaterialTheme.typography.bodySmall,
                    color = Muted,
                )
                TextButton(onClick = onAcceptPastedCandidate) {
                    Text("Add")
                }
            }
        }
        if (invite.visibleFollows.isNotEmpty()) {
            Spacer(modifier = Modifier.height(8.dp))
            invite.visibleFollows.take(8).forEach { pubkey ->
                val selected = invite.selected.any { it.pubkeyHex == pubkey }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        text = pubkey.take(18),
                        modifier = Modifier.weight(1f),
                        style = MaterialTheme.typography.bodyMedium,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    TextButton(
                        onClick = {
                            onToggleCandidate(pubkey, HighlighterRoomInviteCandidateSource.FOLLOW)
                        },
                    ) {
                        Text(if (selected) "Remove" else "Select")
                    }
                }
            }
        }
        if (invite.selected.isNotEmpty()) {
            SearchGroupHeader("Selected", invite.selected.size.toString())
            invite.selected.forEach { candidate ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        text = candidate.pubkeyHex.take(18),
                        modifier = Modifier.weight(1f),
                        style = MaterialTheme.typography.bodySmall,
                        color = Muted,
                    )
                    TextButton(onClick = { onRemoveCandidate(candidate.pubkeyHex) }) {
                        Text("Remove")
                    }
                }
            }
        }
        invite.inviteUrl?.takeIf { it.isNotBlank() }?.let { url ->
            Spacer(modifier = Modifier.height(8.dp))
            SelectionContainer {
                Text(text = url, style = MaterialTheme.typography.bodySmall, color = Muted)
            }
        }
        listOfNotNull(invite.addErrorMessage, invite.inviteLinkErrorMessage, invite.toastMessage)
            .filter { it.isNotBlank() }
            .forEach { message ->
                Spacer(modifier = Modifier.height(6.dp))
                Text(text = message, style = MaterialTheme.typography.bodySmall, color = Clay)
            }
        Spacer(modifier = Modifier.height(10.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = onRefresh, shape = RoundedCornerShape(8.dp)) {
                Text(if (invite.isLoadingFollows) "Loading" else "Refresh")
            }
            OutlinedButton(
                onClick = onMintInviteLink,
                shape = RoundedCornerShape(8.dp),
                enabled = !invite.isMintingInviteLink,
            ) {
                Text(if (invite.isMintingInviteLink) "Minting" else "Link")
            }
            Button(
                onClick = onSubmitMembers,
                shape = RoundedCornerShape(8.dp),
                enabled = invite.selected.isNotEmpty() && !invite.isAddingMembers,
            ) {
                Text(if (invite.isAddingMembers) "Adding" else "Add")
            }
            TextButton(onClick = onClose) {
                Text("Close")
            }
        }
    }
}

@Composable
private fun CapturePanel(
    capture: HighlighterCaptureSnapshot,
    bookPicker: HighlighterBookPickerSnapshot,
    communities: List<CommunitySummary>,
    onSearch: (String) -> Unit,
    onClearSearch: () -> Unit,
    onUploadPhoto: (ByteArray, String, UInt, UInt, String) -> Unit,
    onPublishHighlight: (HighlighterCaptureArtifact, String?, HighlightDraft) -> Unit,
    onPublishPicture: (HighlighterCaptureArtifact?, String?, BlossomUpload, String) -> Unit,
    onClearResult: () -> Unit,
    onClearError: () -> Unit,
) {
    val context = LocalContext.current
    var query by remember { mutableStateOf("") }
    var quote by remember { mutableStateOf("") }
    var note by remember { mutableStateOf("") }
    var selectedArtifact by remember { mutableStateOf<ArtifactRecord?>(null) }
    var selectedGroupId by remember { mutableStateOf<String?>(null) }
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        if (uri != null) {
            readPickedImage(context, uri)?.let { image ->
                onUploadPhoto(image.bytes, image.mime, image.width, image.height, note.trim())
            }
        }
    }
    Panel {
        SectionHeader("Capture", if (capture.isPublishing) "Publishing" else "Highlight")
        Spacer(modifier = Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(
                onClick = {
                    picker.launch(
                        PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly),
                    )
                },
                shape = RoundedCornerShape(8.dp),
                enabled = !capture.isUploading,
            ) {
                Text(if (capture.isUploading) "Uploading" else "Photo")
            }
            capture.upload?.let { upload ->
                Chip("${upload.width}x${upload.height}")
            }
        }
        Spacer(modifier = Modifier.height(8.dp))
        OutlinedTextField(
            value = query,
            onValueChange = {
                query = it
                if (it.trim().length >= 2) {
                    onSearch(it.trim())
                }
            },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text("Find artifact") },
        )
        if (query.isNotBlank()) {
            TextButton(onClick = {
                query = ""
                onClearSearch()
            }) {
                Text("Clear search")
            }
        }
        val artifactRows = if (bookPicker.searchQuery.isNotBlank()) {
            bookPicker.searchResults
        } else {
            bookPicker.recentBooks
        }
        artifactRows.take(5).forEach { record ->
            ArtifactPickerRow(
                record = record,
                selected = selectedArtifact?.shareEventId == record.shareEventId,
                onSelect = { selectedArtifact = record },
            )
        }
        if (communities.isNotEmpty()) {
            SearchGroupHeader("Community", communities.size.toString())
            LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                items(communities, key = { it.id }) { community ->
                    ToggleButton(
                        label = community.name.ifBlank { community.id }.take(18),
                        selected = selectedGroupId == community.id,
                    ) {
                        selectedGroupId = if (selectedGroupId == community.id) null else community.id
                    }
                }
            }
        }
        Spacer(modifier = Modifier.height(8.dp))
        OutlinedTextField(
            value = quote,
            onValueChange = { quote = it },
            modifier = Modifier.fillMaxWidth(),
            minLines = 2,
            maxLines = 5,
            label = { Text("Quote") },
        )
        Spacer(modifier = Modifier.height(8.dp))
        OutlinedTextField(
            value = note,
            onValueChange = { note = it },
            modifier = Modifier.fillMaxWidth(),
            minLines = 1,
            maxLines = 4,
            label = { Text("Note / alt text") },
        )
        capture.uploadErrorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = Clay)
        }
        capture.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = Clay)
            TextButton(onClick = onClearError) {
                Text("Dismiss")
            }
        }
        capture.publishedEventId?.takeIf { it.isNotBlank() }?.let { id ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = "Published ${id.take(12)}", style = MaterialTheme.typography.bodySmall, color = Muted)
            TextButton(onClick = onClearResult) {
                Text("Clear")
            }
        }
        Spacer(modifier = Modifier.height(10.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = {
                    val artifact = selectedArtifact ?: return@Button
                    onPublishHighlight(
                        HighlighterCaptureArtifact.Existing(artifact),
                        selectedGroupId,
                        HighlightDraft(
                            quote.trim(),
                            "",
                            note.trim(),
                            null,
                            null,
                            "",
                            emptyList(),
                            capture.upload,
                        ),
                    )
                },
                shape = RoundedCornerShape(8.dp),
                enabled = selectedArtifact != null && quote.isNotBlank() && !capture.isPublishing,
            ) {
                Text(if (capture.isPublishing) "Saving" else "Highlight")
            }
            OutlinedButton(
                onClick = {
                    val upload = capture.upload ?: return@OutlinedButton
                    onPublishPicture(
                        selectedArtifact?.let { HighlighterCaptureArtifact.Existing(it) },
                        selectedGroupId,
                        upload,
                        note.trim(),
                    )
                },
                shape = RoundedCornerShape(8.dp),
                enabled = capture.upload != null && !capture.isPublishing,
            ) {
                Text("Picture")
            }
        }
    }
}

@Composable
private fun ArtifactPickerRow(
    record: ArtifactRecord,
    selected: Boolean,
    onSelect: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onSelect)
            .padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = record.preview.title.ifBlank { record.preview.url.ifBlank { "Untitled" } },
                style = MaterialTheme.typography.bodyMedium,
                color = Ink,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = record.preview.author.ifBlank { record.preview.domain },
                style = MaterialTheme.typography.bodySmall,
                color = Muted,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        TextButton(onClick = onSelect) {
            Text(if (selected) "Selected" else "Use")
        }
    }
}

@Composable
private fun FeedbackPanel(
    feedback: HighlighterFeedbackSnapshot,
    onRefreshThreads: () -> Unit,
    onSetNewThreadDraft: (String) -> Unit,
    onPublishNewThread: () -> Unit,
    onOpenThread: (String) -> Unit,
    onSetReplyDraft: (String) -> Unit,
    onPublishReply: () -> Unit,
    onRefreshThread: () -> Unit,
    onCloseThread: () -> Unit,
) {
    Panel {
        SectionHeader("Feedback", feedback.threadCount.toString())
        Spacer(modifier = Modifier.height(8.dp))
        OutlinedTextField(
            value = feedback.newThreadDraft,
            onValueChange = onSetNewThreadDraft,
            modifier = Modifier.fillMaxWidth(),
            minLines = 2,
            maxLines = 5,
            label = { Text("New feedback") },
        )
        Spacer(modifier = Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = onPublishNewThread,
                shape = RoundedCornerShape(8.dp),
                enabled = feedback.newThreadDraft.isNotBlank() && !feedback.isPublishingNewThread,
            ) {
                Text(if (feedback.isPublishingNewThread) "Sending" else "Send")
            }
            OutlinedButton(onClick = onRefreshThreads, shape = RoundedCornerShape(8.dp)) {
                Text(if (feedback.isLoadingThreads) "Loading" else "Refresh")
            }
        }
        feedback.publishErrorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = Clay)
        }
        if (feedback.threads.isNotEmpty()) {
            SearchGroupHeader("Threads", feedback.threadCount.toString())
            feedback.threads.take(5).forEach { thread ->
                FeedbackThreadRow(thread, onOpenThread)
            }
        } else if (feedback.isLoadingThreads) {
            Spacer(modifier = Modifier.height(8.dp))
            Text(text = "Loading feedback", style = MaterialTheme.typography.bodyMedium, color = Muted)
        }
        if (feedback.selectedRootEventId != null) {
            SearchGroupHeader("Conversation", feedback.selectedEventCount.toString())
            feedback.selectedEvents.takeLast(8).forEach { event ->
                FeedbackEventRow(event)
            }
            OutlinedTextField(
                value = feedback.replyDraft,
                onValueChange = onSetReplyDraft,
                modifier = Modifier.fillMaxWidth(),
                minLines = 1,
                maxLines = 4,
                label = { Text("Reply") },
            )
            Spacer(modifier = Modifier.height(8.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(
                    onClick = onPublishReply,
                    shape = RoundedCornerShape(8.dp),
                    enabled = feedback.replyDraft.isNotBlank() && !feedback.isPublishingReply,
                ) {
                    Text(if (feedback.isPublishingReply) "Sending" else "Reply")
                }
                OutlinedButton(onClick = onRefreshThread, shape = RoundedCornerShape(8.dp)) {
                    Text(if (feedback.isLoadingThread) "Loading" else "Refresh")
                }
                TextButton(onClick = onCloseThread) {
                    Text("Close")
                }
            }
        }
    }
}

@Composable
private fun FeedbackThreadRow(thread: FeedbackThreadRecord, onOpen: (String) -> Unit) {
    SearchResultRow(
        title = thread.title ?: thread.preview.ifBlank { thread.rootEventId.take(12) },
        subtitle = thread.summary ?: thread.statusLabel ?: thread.authorPubkey.take(12),
        onClick = { onOpen(thread.rootEventId) },
    )
}

@Composable
private fun FeedbackEventRow(event: FeedbackEventRecord) {
    SearchResultRow(
        title = event.content,
        subtitle = event.authorPubkey.take(12),
    )
}

@Composable
private fun RoomDetailPanel(
    room: HighlighterRoomDetailSnapshot,
    onRefresh: () -> Unit,
    onClose: () -> Unit,
    onPublishDiscussion: (String, String) -> Unit,
    onPublishChat: (String) -> Unit,
    onLoadMoreChat: () -> Unit,
    onOpenComments: (String, String, UShort) -> Unit,
) {
    var discussionTitle by remember(room.groupId) { mutableStateOf("") }
    var discussionBody by remember(room.groupId) { mutableStateOf("") }
    var chatBody by remember(room.groupId) { mutableStateOf("") }
    Panel {
        SectionHeader("Room", room.groupId.take(12))
        Spacer(modifier = Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = onRefresh, shape = RoundedCornerShape(8.dp)) {
                Text(if (room.isLoading) "Loading" else "Refresh")
            }
            TextButton(onClick = onClose) {
                Text("Close")
            }
        }
        room.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = Clay)
        }
        if (room.artifacts.isNotEmpty()) {
            SearchGroupHeader("Artifacts", room.artifactCount.toString())
            room.artifacts.take(4).forEach { ArtifactSummaryRow(it) }
        }
        if (room.highlights.isNotEmpty()) {
            SearchGroupHeader("Highlights", room.highlightCount.toString())
            room.highlights.take(4).forEach { hydrated ->
                HydratedHighlightRow(hydrated) {
                    onOpenComments("e", hydrated.highlight.eventId, 9802u)
                }
            }
        }
        SearchGroupHeader("Discuss", room.discussionCount.toString())
        OutlinedTextField(
            value = discussionTitle,
            onValueChange = { discussionTitle = it },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text("Title") },
        )
        Spacer(modifier = Modifier.height(8.dp))
        OutlinedTextField(
            value = discussionBody,
            onValueChange = { discussionBody = it },
            modifier = Modifier.fillMaxWidth(),
            minLines = 2,
            maxLines = 5,
            label = { Text("Body") },
        )
        Spacer(modifier = Modifier.height(8.dp))
        Button(
            onClick = {
                onPublishDiscussion(discussionTitle.trim(), discussionBody.trim())
                discussionTitle = ""
                discussionBody = ""
            },
            shape = RoundedCornerShape(8.dp),
            enabled = discussionTitle.isNotBlank() && !room.isPublishingDiscussion,
        ) {
            Text(if (room.isPublishingDiscussion) "Posting" else "Post discussion")
        }
        room.discussionErrorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = Clay)
        }
        room.discussions.take(5).forEach { discussion ->
            DiscussionRow(discussion) {
                onOpenComments("e", discussion.eventId, 11u)
            }
        }
        SearchGroupHeader("Chat", room.chatMessageCount.toString())
        room.chatMessages.takeLast(6).forEach { ChatRow(it) }
        if (room.chatHasMore) {
            TextButton(onClick = onLoadMoreChat) {
                Text(if (room.isChatLoadingMore) "Loading" else "Load more")
            }
        }
        OutlinedTextField(
            value = chatBody,
            onValueChange = { chatBody = it },
            modifier = Modifier.fillMaxWidth(),
            minLines = 1,
            maxLines = 3,
            label = { Text("Message") },
        )
        Spacer(modifier = Modifier.height(8.dp))
        Button(
            onClick = {
                onPublishChat(chatBody.trim())
                chatBody = ""
            },
            shape = RoundedCornerShape(8.dp),
            enabled = chatBody.isNotBlank() && !room.isSendingChatMessage,
        ) {
            Text(if (room.isSendingChatMessage) "Sending" else "Send")
        }
        room.chatErrorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = Clay)
        }
    }
}

@Composable
private fun ArtifactSummaryRow(record: ArtifactRecord) {
    SearchResultRow(
        title = record.preview.title.ifBlank { record.preview.url.ifBlank { "Untitled" } },
        subtitle = record.note.ifBlank { record.preview.author.ifBlank { record.preview.domain } },
    )
}

@Composable
private fun HydratedHighlightRow(item: HydratedHighlight, onComments: () -> Unit) {
    Column(modifier = Modifier.padding(vertical = 7.dp)) {
        Text(
            text = item.highlight.quote.ifBlank { "Untitled highlight" },
            style = MaterialTheme.typography.bodyMedium,
            color = Ink,
            fontWeight = FontWeight.Medium,
            maxLines = 3,
            overflow = TextOverflow.Ellipsis,
        )
        TextButton(onClick = onComments) {
            Text("Comments")
        }
    }
}

@Composable
private fun DiscussionRow(discussion: DiscussionRecord, onComments: () -> Unit) {
    Column(modifier = Modifier.padding(vertical = 7.dp)) {
        Text(
            text = discussion.title.ifBlank { discussion.summary.ifBlank { discussion.eventId.take(12) } },
            style = MaterialTheme.typography.bodyMedium,
            color = Ink,
            fontWeight = FontWeight.Medium,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
        if (discussion.body.isNotBlank()) {
            Text(
                text = discussion.body,
                style = MaterialTheme.typography.bodySmall,
                color = Muted,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
        TextButton(onClick = onComments) {
            Text("Comments")
        }
    }
}

@Composable
private fun ChatRow(message: ChatMessageRecord) {
    SearchResultRow(
        title = message.content,
        subtitle = message.authorPubkey.take(12),
    )
}

@Composable
private fun CommentsPanel(
    comments: HighlighterCommentsSnapshot,
    onRefresh: () -> Unit,
    onSetDraft: (String?, String) -> Unit,
    onPublish: (String?) -> Unit,
    onToggleLike: (String) -> Unit,
    onToggleBookmark: (String) -> Unit,
    onClose: () -> Unit,
) {
    val topDraft = comments.drafts.firstOrNull { it.parentEventId == null }?.body ?: ""
    Panel {
        SectionHeader("Comments", comments.recordCount.toString())
        Spacer(modifier = Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = onRefresh, shape = RoundedCornerShape(8.dp)) {
                Text(if (comments.isLoading) "Loading" else "Refresh")
            }
            TextButton(onClick = onClose) {
                Text("Close")
            }
        }
        comments.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = Clay)
        }
        OutlinedTextField(
            value = topDraft,
            onValueChange = { onSetDraft(null, it) },
            modifier = Modifier.fillMaxWidth(),
            minLines = 2,
            maxLines = 5,
            label = { Text("Add comment") },
        )
        Spacer(modifier = Modifier.height(8.dp))
        Button(
            onClick = { onPublish(null) },
            shape = RoundedCornerShape(8.dp),
            enabled = topDraft.isNotBlank() && !comments.isPublishing,
        ) {
            Text(if (comments.isPublishing) "Posting" else "Post")
        }
        comments.publishErrorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = Clay)
        }
        comments.records.take(12).forEach { comment ->
            CommentRow(
                comment = comment,
                likeCount = comments.interactions
                    .firstOrNull { it.eventId == comment.eventId }
                    ?.likeCount ?: 0uL,
                bookmarked = comments.interactions
                    .firstOrNull { it.eventId == comment.eventId }
                    ?.isBookmarked == true,
                onLike = { onToggleLike(comment.eventId) },
                onBookmark = { onToggleBookmark(comment.eventId) },
            )
        }
    }
}

@Composable
private fun CommentRow(
    comment: CommentRecord,
    likeCount: ULong,
    bookmarked: Boolean,
    onLike: () -> Unit,
    onBookmark: () -> Unit,
) {
    Column(modifier = Modifier.padding(vertical = 8.dp)) {
        Text(text = comment.body, style = MaterialTheme.typography.bodyMedium, color = Ink)
        Text(
            text = comment.pubkey.take(12),
            style = MaterialTheme.typography.bodySmall,
            color = Muted,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            TextButton(onClick = onLike) {
                Text("Like $likeCount")
            }
            TextButton(onClick = onBookmark) {
                Text(if (bookmarked) "Saved" else "Save")
            }
        }
    }
}

@Composable
private fun MediaSettingsPanel(
    media: HighlighterMediaSettingsSnapshot,
    onRefresh: () -> Unit,
    onAddServer: (String) -> Unit,
    onRemoveServer: (String) -> Unit,
) {
    var serverUrl by remember { mutableStateOf("") }
    Panel {
        SectionHeader("Media", media.blossomServerCount.toString())
        Spacer(modifier = Modifier.height(8.dp))
        OutlinedTextField(
            value = serverUrl,
            onValueChange = { serverUrl = it },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text("Blossom server") },
        )
        Spacer(modifier = Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = {
                    val url = serverUrl.trim()
                    if (url.isNotEmpty()) {
                        onAddServer(url)
                        serverUrl = ""
                    }
                },
                shape = RoundedCornerShape(8.dp),
                enabled = serverUrl.isNotBlank() && !media.isSaving,
            ) {
                Text(if (media.isSaving) "Saving" else "Add")
            }
            OutlinedButton(onClick = onRefresh, shape = RoundedCornerShape(8.dp)) {
                Text(if (media.isLoading) "Loading" else "Refresh")
            }
        }
        media.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = Clay)
        }
        media.blossomServers.take(6).forEach { url ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = url,
                    modifier = Modifier.weight(1f),
                    style = MaterialTheme.typography.bodySmall,
                    color = Muted,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                TextButton(onClick = { onRemoveServer(url) }) {
                    Text("Remove")
                }
            }
        }
    }
}

@Composable
private fun MetricRow(chrome: HighlighterChromeSnapshot) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        MetricTile(
            label = "Rooms",
            value = chrome.joinedCommunitiesTotal.toString(),
            modifier = Modifier.weight(1f),
        )
        MetricTile(
            label = "Bookmarks",
            value = chrome.bookmarkedArticleAddressCount.toString(),
            modifier = Modifier.weight(1f),
        )
        MetricTile(
            label = "Session",
            value = if (chrome.currentUser == null) "Out" else "In",
            modifier = Modifier.weight(1f),
        )
    }
}

@Composable
private fun MetricTile(label: String, value: String, modifier: Modifier = Modifier) {
    Surface(
        modifier = modifier,
        color = Color(0xFFFFFCF5),
        shape = RoundedCornerShape(8.dp),
        border = BorderStroke(1.dp, Line),
    ) {
        Column(modifier = Modifier.padding(12.dp)) {
            Text(text = label, style = MaterialTheme.typography.labelMedium, color = Muted)
            Spacer(modifier = Modifier.height(4.dp))
            Text(
                text = value,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
                color = Ink,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
private fun SectionHeader(title: String, count: String) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(
            text = title,
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
            color = Ink,
        )
        Text(
            text = count,
            style = MaterialTheme.typography.labelLarge,
            color = Muted,
        )
    }
}

@Composable
@OptIn(ExperimentalLayoutApi::class)
private fun CommunityRow(community: CommunitySummary, onOpenRoom: (String) -> Unit) {
    Panel {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable { onOpenRoom(community.id) },
            verticalAlignment = Alignment.Top,
        ) {
            Box(
                modifier = Modifier
                    .size(40.dp)
                    .clip(CircleShape)
                    .background(Moss.copy(alpha = 0.14f)),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = community.name.firstOrNull()?.uppercase() ?: "#",
                    color = Moss,
                    fontWeight = FontWeight.Bold,
                )
            }
            Spacer(modifier = Modifier.width(12.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = community.name.ifBlank { community.id },
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (community.about.isNotBlank()) {
                    Spacer(modifier = Modifier.height(3.dp))
                    Text(
                        text = community.about,
                        style = MaterialTheme.typography.bodySmall,
                        color = Muted,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                Spacer(modifier = Modifier.height(8.dp))
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Chip(community.access)
                    Chip(community.visibility)
                    community.memberCount?.let { Chip("$it members") }
                }
            }
        }
    }
}

@Composable
private fun NetworkPanel(
    network: HighlighterNetworkSnapshot,
    onSetWifiOnly: (Boolean) -> Unit,
    onReconnect: () -> Unit,
) {
    val pathLabel = when (network.currentPathIsWifi) {
        true -> "Wi-Fi path active"
        false -> "Not on Wi-Fi"
        null -> "Path pending"
    }
    Panel {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = "Network",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = Ink,
                )
                Text(
                    text = if (network.wifiOnlyEnabled) pathLabel else "Relay connections allowed",
                    style = MaterialTheme.typography.bodySmall,
                    color = Muted,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Switch(
                checked = network.wifiOnlyEnabled,
                onCheckedChange = onSetWifiOnly,
            )
        }
        Spacer(modifier = Modifier.height(10.dp))
        OutlinedButton(
            onClick = onReconnect,
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(8.dp),
            border = BorderStroke(1.dp, Line),
        ) {
            Text("Reconnect All")
        }
    }
}

@Composable
private fun EmptyPanel(message: String) {
    Surface(
        modifier = Modifier.fillMaxWidth(),
        color = Color.Transparent,
        shape = RoundedCornerShape(8.dp),
        border = BorderStroke(1.dp, Line),
    ) {
        Text(
            text = message,
            modifier = Modifier.padding(16.dp),
            style = MaterialTheme.typography.bodyMedium,
            color = Muted,
        )
    }
}

@Composable
private fun ToastBanner(message: String, onClearToast: () -> Unit) {
    Surface(
        modifier = Modifier.fillMaxWidth(),
        color = Color(0xFFEAF2EE),
        shape = RoundedCornerShape(8.dp),
        border = BorderStroke(1.dp, Color(0xFFD2E2DA)),
    ) {
        Row(
            modifier = Modifier.padding(start = 14.dp, top = 10.dp, end = 8.dp, bottom = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = message,
                modifier = Modifier.weight(1f),
                style = MaterialTheme.typography.bodyMedium,
                color = Ink,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            TextButton(onClick = onClearToast) {
                Text("Dismiss")
            }
        }
    }
}

@Composable
private fun Chip(label: String) {
    Surface(
        color = Color(0xFFF1EFE6),
        shape = RoundedCornerShape(8.dp),
    ) {
        Text(
            text = label.replaceFirstChar { it.uppercase() },
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
            style = MaterialTheme.typography.labelSmall,
            color = Muted,
            maxLines = 1,
        )
    }
}

@Composable
private fun Panel(content: @Composable ColumnScope.() -> Unit) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = Color(0xFFFFFCF5)),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
        shape = RoundedCornerShape(8.dp),
        border = BorderStroke(1.dp, Line),
    ) {
        Column(
            modifier = Modifier.padding(14.dp),
            content = content,
        )
    }
}

@Composable
private fun ToggleButton(label: String, selected: Boolean, onClick: () -> Unit) {
    OutlinedButton(
        onClick = onClick,
        shape = RoundedCornerShape(8.dp),
        border = BorderStroke(1.dp, if (selected) Moss else Line),
        colors = ButtonDefaults.outlinedButtonColors(
            containerColor = if (selected) Moss else Color.Transparent,
            contentColor = if (selected) Color.White else Ink,
        ),
    ) {
        Text(
            text = label,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

private data class PickedImage(
    val bytes: ByteArray,
    val mime: String,
    val width: UInt,
    val height: UInt,
)

private fun readPickedImage(context: Context, uri: Uri): PickedImage? =
    runCatching {
        val bytes = context.contentResolver.openInputStream(uri)?.use { it.readBytes() }
            ?: return null
        val options = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options)
        PickedImage(
            bytes = bytes,
            mime = context.contentResolver.getType(uri) ?: "image/jpeg",
            width = options.outWidth.coerceAtLeast(0).toUInt(),
            height = options.outHeight.coerceAtLeast(0).toUInt(),
        )
    }.getOrNull()

private fun HighlighterConnectionState.statusLabel(isBootstrapping: Boolean): String =
    when {
        isBootstrapping -> "Syncing"
        this == HighlighterConnectionState.CONNECTING -> "Connecting"
        this == HighlighterConnectionState.ONLINE -> "Online"
        this == HighlighterConnectionState.OFFLINE -> "Offline"
        else -> "Ready"
    }

private fun RoomRecommendation.signalLabel(): String =
    when (val count = reasonPubkeys.size) {
        0 -> summary.about
        1 -> "1 matching reader"
        else -> "$count matching readers"
    }

private fun HighlighterProfileViewSnapshot.displayName(): String =
    profile?.displayName?.takeIf { it.isNotBlank() }
        ?: profile?.name?.takeIf { it.isNotBlank() }
        ?: pubkeyHex.take(12)

private fun ULong.feedCountLabel(noun: String): String =
    when (this) {
        1uL -> "1 $noun"
        else -> "$this ${noun}s"
    }
