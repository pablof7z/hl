package com.highlighter.app.ui

import androidx.activity.compose.BackHandler
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
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.highlighter.app.ui.bookmarks.BookmarkLibraryPanel
import com.highlighter.app.ui.components.AvatarButton
import com.highlighter.app.ui.feedback.FeedbackPanel
import com.highlighter.app.ui.home.HomeFeedPanel
import com.highlighter.app.ui.podcast.MiniPlayerBar
import com.highlighter.app.ui.podcast.PodcastListeningScreen
import com.highlighter.app.ui.podcast.rememberPodcastPlayerController
import com.highlighter.app.ui.rooms.CreateRoomPanel
import com.highlighter.app.ui.rooms.RoomExplorerPanel
import com.highlighter.app.ui.search.SearchPanel
import com.highlighter.app.util.statusLabel
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterAppState

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
            if (selectedTab == MainTab.HIGHLIGHTS) {
                FloatingActionButton(onClick = { route = ScaffoldRoute.CAPTURE }) {
                    Icon(Icons.Filled.Add, contentDescription = "Capture highlight")
                }
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
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun HighlightsTab(
    state: HighlighterAppState,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    DisposableEffect(Unit) {
        dispatch(HighlighterAppAction.OpenHomeFeed)
        onDispose { dispatch(HighlighterAppAction.CloseHomeFeed) }
    }
    PullToRefreshBox(
        isRefreshing = state.homeFeed.isLoading,
        onRefresh = { dispatch(HighlighterAppAction.RefreshHomeFeed) },
        modifier = Modifier.fillMaxSize(),
    ) {
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(18.dp),
        ) {
            item { HomeFeedPanel(feed = state.homeFeed, dispatch = dispatch) }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun RoomsTab(
    state: HighlighterAppState,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    DisposableEffect(Unit) {
        dispatch(HighlighterAppAction.OpenRoomExplorer)
        onDispose { dispatch(HighlighterAppAction.CloseRoom) }
    }
    PullToRefreshBox(
        isRefreshing = false,
        onRefresh = { dispatch(HighlighterAppAction.RefreshRoomExplorer) },
        modifier = Modifier.fillMaxSize(),
    ) {
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(18.dp),
        ) {
            item {
                CreateRoomPanel(createRoom = state.createRoom, dispatch = dispatch)
            }
            item {
                RoomExplorerPanel(
                    explorer = state.roomExplorer,
                    joinedRoomIds = state.chrome.joinedCommunities.map { it.id }.toSet(),
                    dispatch = dispatch,
                )
            }
        }
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
