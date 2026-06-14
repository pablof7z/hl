package com.highlighter.app.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.FormatQuote
import androidx.compose.material.icons.filled.Groups
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.highlighter.app.ui.bookmarks.BookmarkLibraryPanel
import com.highlighter.app.ui.components.AvatarButton
import com.highlighter.app.ui.feedback.FeedbackPanel
import com.highlighter.app.ui.home.HighlightDetailScreen
import com.highlighter.app.ui.home.homeFeedItems
import com.highlighter.app.ui.podcast.MiniPlayerBar
import com.highlighter.app.ui.podcast.PodcastListeningScreen
import com.highlighter.app.ui.podcast.rememberPodcastPlayerController
import com.highlighter.app.ui.rooms.CreateRoomPanel
import com.highlighter.app.ui.rooms.RoomExplorerPanel
import com.highlighter.app.ui.search.SearchPanel
import com.highlighter.app.util.statusLabel
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterAppState
import uniffi.highlighter_core.HydratedHighlight

/** The three primary tabs, mirroring the iOS `MainTabView`. */
internal enum class MainTab(val title: String) {
    HIGHLIGHTS("Highlights"),
    ROOMS("Rooms"),
    SEARCH("Search"),
}

/** Local (non-core) destinations layered over the tabbed scaffold. */
private enum class ScaffoldRoute {
    TABS,
    SETTINGS,
    CAPTURE,
    BOOKMARKS,
    FEEDBACK,
    PODCAST,
}

/**
 * The logged-in, onboarded home: a Material3 tabbed scaffold with a top app
 * bar (avatar -> profile, gear -> settings) and three tabs whose enter/leave
 * dispatch the matching Open/Close lifecycle actions. A "+" FAB on the
 * Highlights tab opens the full-screen Capture destination.
 *
 * Core-state-driven overlays (comments, room detail, reader, profile) are
 * handled one level up in [RootScene]; this scaffold only owns the local
 * navigation between tabs, settings, capture, bookmarks, and feedback.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun MainScaffold(
    state: HighlighterAppState,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    var route by rememberSaveable { mutableStateOf(ScaffoldRoute.TABS) }
    var selectedTab by rememberSaveable { mutableStateOf(MainTab.HIGHLIGHTS) }
    // Controls the "New room" create-room modal sheet on the Rooms tab.
    var createRoomOpen by rememberSaveable { mutableStateOf(false) }

    // Process-wide podcast player (the same singleton any Play affordance
    // resolves via rememberPodcastPlayerController), so the mini player and
    // full screen drive one engine.
    val podcastPlayer = rememberPodcastPlayerController()
    val podcastState by podcastPlayer.state.collectAsStateWithLifecycle()

    when (route) {
        ScaffoldRoute.PODCAST -> {
            BackHandler { route = ScaffoldRoute.TABS }
            DestinationScaffold(title = "Now Playing", onBack = { route = ScaffoldRoute.TABS }) { padding ->
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(padding),
                ) {
                    PodcastListeningScreen(
                        state = podcastState,
                        onToggle = podcastPlayer::toggle,
                        onSeek = podcastPlayer::seekTo,
                        onSkip = podcastPlayer::skip,
                        onSetSpeed = podcastPlayer::setSpeed,
                    )
                }
            }
            return
        }
        ScaffoldRoute.SETTINGS -> {
            BackHandler { route = ScaffoldRoute.TABS }
            SettingsScreen(
                state = state,
                onBack = { route = ScaffoldRoute.TABS },
                onOpenBookmarks = { route = ScaffoldRoute.BOOKMARKS },
                onOpenFeedback = { route = ScaffoldRoute.FEEDBACK },
                dispatch = dispatch,
            )
            return
        }
        ScaffoldRoute.CAPTURE -> {
            BackHandler { route = ScaffoldRoute.TABS }
            DestinationScaffold(title = "Capture", onBack = { route = ScaffoldRoute.TABS }) { padding ->
                LazyColumn(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(padding),
                    contentPadding = PaddingValues(18.dp),
                ) {
                    item {
                        com.highlighter.app.ui.capture.CapturePanel(
                            capture = state.capture,
                            bookPicker = state.bookPicker,
                            communities = state.chrome.joinedCommunities,
                            dispatch = dispatch,
                        )
                    }
                }
            }
            return
        }
        ScaffoldRoute.BOOKMARKS -> {
            BackHandler { route = ScaffoldRoute.TABS }
            DisposableEffect(Unit) {
                dispatch(HighlighterAppAction.OpenBookmarks)
                onDispose { dispatch(HighlighterAppAction.CloseBookmarks) }
            }
            DestinationScaffold(title = "Bookmarks", onBack = { route = ScaffoldRoute.TABS }) { padding ->
                LazyColumn(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(padding),
                    contentPadding = PaddingValues(18.dp),
                ) {
                    item {
                        BookmarkLibraryPanel(bookmarks = state.bookmarks, dispatch = dispatch)
                    }
                }
            }
            return
        }
        ScaffoldRoute.FEEDBACK -> {
            BackHandler { route = ScaffoldRoute.TABS }
            DisposableEffect(Unit) {
                dispatch(HighlighterAppAction.OpenFeedback(com.highlighter.app.FEEDBACK_PROJECT_COORDINATE))
                onDispose { dispatch(HighlighterAppAction.CloseFeedback) }
            }
            DestinationScaffold(title = "Feedback", onBack = { route = ScaffoldRoute.TABS }) { padding ->
                LazyColumn(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(padding),
                    contentPadding = PaddingValues(18.dp),
                ) {
                    item {
                        FeedbackPanel(feedback = state.feedback, dispatch = dispatch)
                    }
                }
            }
            return
        }
        ScaffoldRoute.TABS -> Unit
    }

    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text(
                            text = selectedTab.title,
                            style = MaterialTheme.typography.titleLarge,
                            fontWeight = FontWeight.SemiBold,
                        )
                        Text(
                            text = state.chrome.connectionState.statusLabel(state.isBootstrapping),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                },
                actions = {
                    AvatarButton(
                        profile = state.chrome.currentUserProfile,
                        fallbackName = state.chrome.currentUser?.npub ?: "",
                        onClick = {
                            state.chrome.currentUser?.pubkey?.let {
                                dispatch(HighlighterAppAction.OpenProfile(it))
                            }
                        },
                    )
                    IconButton(onClick = { route = ScaffoldRoute.SETTINGS }) {
                        Icon(
                            imageVector = Icons.Outlined.Settings,
                            contentDescription = "Settings",
                        )
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background,
                    titleContentColor = MaterialTheme.colorScheme.onBackground,
                    actionIconContentColor = MaterialTheme.colorScheme.onBackground,
                ),
            )
        },
        bottomBar = {
            Column {
                // Mini player sits directly above the nav bar when an episode
                // is loaded; tapping it opens the full listening screen.
                MiniPlayerBar(
                    state = podcastState,
                    onOpen = { route = ScaffoldRoute.PODCAST },
                    onToggle = podcastPlayer::toggle,
                )
                NavigationBar(containerColor = MaterialTheme.colorScheme.surface) {
                    NavigationBarItem(
                        selected = selectedTab == MainTab.HIGHLIGHTS,
                        onClick = { selectedTab = MainTab.HIGHLIGHTS },
                        icon = { Icon(Icons.Filled.FormatQuote, contentDescription = null) },
                        label = { Text("Highlights") },
                    )
                    NavigationBarItem(
                        selected = selectedTab == MainTab.ROOMS,
                        onClick = { selectedTab = MainTab.ROOMS },
                        icon = { Icon(Icons.Filled.Groups, contentDescription = null) },
                        label = { Text("Rooms") },
                    )
                    NavigationBarItem(
                        selected = selectedTab == MainTab.SEARCH,
                        onClick = { selectedTab = MainTab.SEARCH },
                        icon = { Icon(Icons.Filled.Search, contentDescription = null) },
                        label = { Text("Search") },
                    )
                }
            }
        },
        floatingActionButton = {
            when (selectedTab) {
                MainTab.HIGHLIGHTS -> {
                    FloatingActionButton(onClick = { route = ScaffoldRoute.CAPTURE }) {
                        Icon(Icons.Filled.Add, contentDescription = "Capture highlight")
                    }
                }
                MainTab.ROOMS -> {
                    // "New room" FAB — presents the create-room sheet as a modal.
                    FloatingActionButton(
                        onClick = { createRoomOpen = true },
                        modifier = Modifier.testTag("create_room_fab"),
                    ) {
                        Icon(Icons.Filled.Add, contentDescription = "New room")
                    }
                }
                else -> Unit
            }
        },
    ) { padding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
        ) {
            when (selectedTab) {
                MainTab.HIGHLIGHTS -> HighlightsTab(state, dispatch)
                MainTab.ROOMS -> RoomsTab(state, dispatch)
                MainTab.SEARCH -> SearchTab(state, dispatch)
            }
        }
    }

    // "New room" modal — presented from the Rooms-tab FAB. Lives outside the
    // Scaffold composable so the sheet isn't clipped by the scaffold padding.
    if (createRoomOpen) {
        CreateRoomSheet(
            state = state,
            dispatch = dispatch,
            onDismiss = { createRoomOpen = false },
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun HighlightsTab(
    state: HighlighterAppState,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    // Host-side detail navigation: no core state involved — the HydratedHighlight
    // is already present in the feed item so no round-trip is needed. When non-null,
    // the full-screen detail view is rendered in place of (on top of) the feed.
    var selectedHighlight by remember { mutableStateOf<HydratedHighlight?>(null) }

    DisposableEffect(Unit) {
        dispatch(HighlighterAppAction.OpenHomeFeed)
        onDispose { dispatch(HighlighterAppAction.CloseHomeFeed) }
    }

    // If a highlight is selected, show its detail screen instead of the feed.
    if (selectedHighlight != null) {
        val highlight = selectedHighlight!!
        BackHandler { selectedHighlight = null }
        DestinationScaffold(
            title = "Highlight",
            onBack = { selectedHighlight = null },
        ) { _ ->
            HighlightDetailScreen(
                item = highlight,
                dispatch = dispatch,
            )
        }
        return
    }

    PullToRefreshBox(
        isRefreshing = state.homeFeed.isLoading,
        onRefresh = { dispatch(HighlighterAppAction.RefreshHomeFeed) },
        modifier = Modifier.fillMaxSize(),
    ) {
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .testTag("feed_item_list"),
            contentPadding = PaddingValues(18.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            homeFeedItems(
                feed = state.homeFeed,
                dispatch = dispatch,
                onOpenHighlightDetail = { hydratedHighlight ->
                    selectedHighlight = hydratedHighlight
                },
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun RoomsTab(
    state: HighlighterAppState,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    // Open the explorer on enter; do NOT dispatch CloseRoom on leave — the room
    // overlay's own lifecycle (RootScene Overlays / RoomDetailPanel) owns CloseRoom.
    // Dispatching CloseRoom here caused "opening a room does nothing" because any
    // tab switch or recomposition would tear down the just-opened room overlay.
    DisposableEffect(Unit) {
        dispatch(HighlighterAppAction.OpenRoomExplorer)
        onDispose { /* explorer teardown is handled by OpenRoomExplorer on re-entry */ }
    }
    PullToRefreshBox(
        isRefreshing = false,
        onRefresh = { dispatch(HighlighterAppAction.RefreshRoomExplorer) },
        modifier = Modifier.fillMaxSize(),
    ) {
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .testTag("room_explorer_list"),
            contentPadding = PaddingValues(18.dp),
        ) {
            // The create-room form was previously rendered here as the first item,
            // which caused it to appear above the explorer on every Rooms visit.
            // It is now presented as a modal sheet via the "New room" FAB.
            item {
                // Memoize the ID set so each recomposition (triggered by the
                // coalesced state flow) doesn't allocate a fresh Set and List.
                val joinedRoomIds by remember(state.chrome.joinedCommunities) {
                    derivedStateOf { state.chrome.joinedCommunities.map { it.id }.toSet() }
                }
                RoomExplorerPanel(
                    explorer = state.roomExplorer,
                    joinedRoomIds = joinedRoomIds,
                    dispatch = dispatch,
                )
            }
        }
    }
}

/**
 * Modal bottom sheet containing [CreateRoomPanel]. Mirrors iOS `CreateRoomSheet`
 * which is presented modally from the `+` toolbar button in `RoomExplorerView`.
 * On successful creation (`createRoom.createdGroupId` non-blank) automatically
 * routes to the invite/welcome screen and dismisses the sheet.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun CreateRoomSheet(
    state: HighlighterAppState,
    dispatch: (HighlighterAppAction) -> Unit,
    onDismiss: () -> Unit,
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    // When the core reports a created group, open the invite screen and dismiss
    // the sheet — mirroring iOS `CreateRoomSheet` which routes to `RoomInviteView`
    // on success.
    LaunchedEffect(state.createRoom.createdGroupId) {
        val groupId = state.createRoom.createdGroupId
        if (!groupId.isNullOrBlank()) {
            dispatch(HighlighterAppAction.OpenRoomInvite(groupId))
            dispatch(HighlighterAppAction.ClearCreateRoomResult)
            onDismiss()
        }
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surface,
    ) {
        CreateRoomPanel(
            createRoom = state.createRoom,
            dispatch = dispatch,
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SearchTab(
    state: HighlighterAppState,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    DisposableEffect(Unit) {
        dispatch(HighlighterAppAction.SearchOpened)
        onDispose { dispatch(HighlighterAppAction.SearchClosed) }
    }
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(18.dp),
    ) {
        item { SearchPanel(search = state.search, dispatch = dispatch) }
    }
}
