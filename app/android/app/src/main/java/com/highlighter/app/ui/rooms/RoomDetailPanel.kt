package com.highlighter.app.ui.rooms

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.components.AvatarImage
import com.highlighter.app.ui.components.CoverShape
import com.highlighter.app.ui.components.EmptyPanel
import com.highlighter.app.ui.components.RemoteImage
import com.highlighter.app.ui.podcast.rememberPodcastPlayerController
import com.highlighter.app.ui.podcast.selectAudioUrl
import com.highlighter.app.util.LocalProfiles
import com.highlighter.app.util.avatarUrl
import com.highlighter.app.util.displayNameOr
import com.highlighter.app.util.profileFor
import uniffi.highlighter_core.ArtifactRecord
import uniffi.highlighter_core.ChatMessageRecord
import uniffi.highlighter_core.DiscussionRecord
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterRoomDetailSnapshot
import uniffi.highlighter_core.HydratedHighlight

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

private enum class RoomTab { HOME, LIBRARY, DISCUSSIONS, CHAT }

// ---------------------------------------------------------------------------
// Root screen
// ---------------------------------------------------------------------------

/**
 * Full-screen room detail, matching the iOS RoomHomeView layout:
 *  - Named top-bar with back affordance (dispatches CloseRoom).
 *  - Pill-style tab bar: Home / Library / Discussions / [Chat if messages exist].
 *  - Per-tab content areas rendered below.
 *  - Discussion composer hidden behind a FAB modal sheet (not inline).
 *  - Chat composer inline at the bottom of the Chat tab only.
 *
 * IMPORTANT: CloseRoom is dispatched ONLY from [onBack] (back arrow / system
 * back), not from tab switches or onDispose. This preserves the lifecycle fix
 * established in MainScaffold.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun RoomDetailPanel(
    room: HighlighterRoomDetailSnapshot,
    roomName: String,
    onBack: () -> Unit,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    var selectedTab by rememberSaveable { mutableStateOf(RoomTab.HOME) }
    val hasChatActivity = room.chatMessageCount > 0u
    var composerSheetOpen by rememberSaveable { mutableStateOf(false) }

    // If chat becomes unavailable while on the Chat tab, fall back to Home.
    LaunchedEffect(hasChatActivity) {
        if (!hasChatActivity && selectedTab == RoomTab.CHAT) {
            selectedTab = RoomTab.HOME
        }
    }

    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            TopAppBar(
                title = {
                    Text(
                        text = roomName,
                        modifier = Modifier.testTag("room_detail_name"),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        fontWeight = FontWeight.SemiBold,
                    )
                },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back",
                        )
                    }
                },
                actions = {
                    if (selectedTab == RoomTab.DISCUSSIONS) {
                        IconButton(onClick = { composerSheetOpen = true }) {
                            Icon(
                                imageVector = Icons.Default.Add,
                                contentDescription = "New discussion",
                                modifier = Modifier.testTag("room_new_discussion_fab"),
                            )
                        }
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background,
                    titleContentColor = MaterialTheme.colorScheme.onBackground,
                    navigationIconContentColor = MaterialTheme.colorScheme.onBackground,
                    actionIconContentColor = MaterialTheme.colorScheme.onBackground,
                ),
            )
        },
        bottomBar = {
            PillTabBar(
                selected = selectedTab,
                hasChatActivity = hasChatActivity,
                onSelect = { selectedTab = it },
            )
        },
        floatingActionButton = {
            // FAB on Discussions tab as additional affordance
            if (selectedTab == RoomTab.DISCUSSIONS) {
                FloatingActionButton(
                    onClick = { composerSheetOpen = true },
                    modifier = Modifier.testTag("room_new_discussion_fab"),
                    containerColor = MaterialTheme.colorScheme.primary,
                    contentColor = MaterialTheme.colorScheme.onPrimary,
                ) {
                    Icon(Icons.Default.Add, contentDescription = "New discussion")
                }
            }
        },
    ) { innerPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding),
        ) {
            when (selectedTab) {
                RoomTab.HOME -> HomeTab(room = room, dispatch = dispatch)
                RoomTab.LIBRARY -> LibraryTab(room = room, dispatch = dispatch)
                RoomTab.DISCUSSIONS -> DiscussionsTab(room = room, dispatch = dispatch)
                RoomTab.CHAT -> ChatTab(room = room, dispatch = dispatch)
            }
        }
    }

    // Discussion composer modal sheet
    if (composerSheetOpen) {
        val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
        ModalBottomSheet(
            onDismissRequest = { composerSheetOpen = false },
            sheetState = sheetState,
        ) {
            DiscussionComposerSheet(
                room = room,
                onDismiss = { composerSheetOpen = false },
                dispatch = dispatch,
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Pill tab bar
// ---------------------------------------------------------------------------

@Composable
private fun PillTabBar(
    selected: RoomTab,
    hasChatActivity: Boolean,
    onSelect: (RoomTab) -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        PillSegment(
            label = "Home",
            selected = selected == RoomTab.HOME,
            modifier = Modifier
                .weight(1f)
                .testTag("room_tab_home"),
            onClick = { onSelect(RoomTab.HOME) },
        )
        PillSegment(
            label = "Library",
            selected = selected == RoomTab.LIBRARY,
            modifier = Modifier
                .weight(1f)
                .testTag("room_tab_library"),
            onClick = { onSelect(RoomTab.LIBRARY) },
        )
        PillSegment(
            label = "Discussions",
            selected = selected == RoomTab.DISCUSSIONS,
            modifier = Modifier
                .weight(1f)
                .testTag("room_tab_discussions"),
            onClick = { onSelect(RoomTab.DISCUSSIONS) },
        )
        if (hasChatActivity) {
            PillSegment(
                label = "Chat",
                selected = selected == RoomTab.CHAT,
                modifier = Modifier
                    .weight(1f)
                    .testTag("room_tab_chat"),
                onClick = { onSelect(RoomTab.CHAT) },
            )
        }
    }
}

@Composable
private fun PillSegment(
    label: String,
    selected: Boolean,
    modifier: Modifier = Modifier,
    onClick: () -> Unit,
) {
    Button(
        onClick = onClick,
        modifier = modifier,
        shape = RoundedCornerShape(20.dp),
        contentPadding = PaddingValues(horizontal = 8.dp, vertical = 6.dp),
        colors = ButtonDefaults.buttonColors(
            containerColor = if (selected) MaterialTheme.colorScheme.primary else Color.Transparent,
            contentColor = if (selected) MaterialTheme.colorScheme.onPrimary else MaterialTheme.colorScheme.onSurfaceVariant,
        ),
        border = if (selected) null else BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
        elevation = null,
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelMedium,
            fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

// ---------------------------------------------------------------------------
// Home tab — highlight lanes (grouped by reference)
// ---------------------------------------------------------------------------

@Composable
private fun HomeTab(
    room: HighlighterRoomDetailSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    val profiles = LocalProfiles.current
    // Dedupe profile requests against already-loaded profiles
    val hydratedPubkeys = remember(profiles) { profiles.map { it.pubkeyHex }.toSet() }

    when {
        room.isLoading && room.highlights.isEmpty() -> {
            Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) {
                CircularProgressIndicator()
            }
        }
        room.highlights.isEmpty() -> {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(16.dp),
            ) {
                EmptyPanel("No highlights yet")
            }
        }
        else -> {
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                items(room.highlights, key = { it.highlight.eventId }) { hydrated ->
                    // Dispatch profile requests for unique pubkeys not yet in cache
                    val pk = hydrated.highlight.pubkey
                    LaunchedEffect(pk) {
                        if (pk.isNotBlank() && pk !in hydratedPubkeys) {
                            dispatch(HighlighterAppAction.RequestProfile(pk))
                        }
                    }
                    val profile = profiles.profileFor(pk)
                    val authorName = profile.displayNameOr(pk)
                    val authorAvatar = profile.avatarUrl()

                    RoomHighlightCard(
                        hydrated = hydrated,
                        authorName = authorName,
                        authorAvatarUrl = authorAvatar,
                        onComments = {
                            dispatch(
                                HighlighterAppAction.OpenComments(
                                    "e",
                                    hydrated.highlight.eventId,
                                    9802u,
                                ),
                            )
                        },
                    )
                }
            }
        }
    }
}

@Composable
private fun RoomHighlightCard(
    hydrated: HydratedHighlight,
    authorName: String,
    authorAvatarUrl: String?,
    onComments: () -> Unit,
) {
    val highlight = hydrated.highlight
    val coverUrl = highlight.imageUrl.takeIf { it.isNotBlank() }
        ?: hydrated.artifact?.preview?.image?.takeIf { it.isNotBlank() }
    val sourceTitle = hydrated.artifact?.preview?.title.orEmpty()

    Surface(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onComments),
        color = MaterialTheme.colorScheme.surface,
        shape = RoundedCornerShape(8.dp),
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outline),
    ) {
        Column(modifier = Modifier.padding(12.dp)) {
            // Source header row
            if (coverUrl != null || sourceTitle.isNotBlank()) {
                Row(
                    verticalAlignment = Alignment.Top,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    if (coverUrl != null) {
                        RemoteImage(
                            url = coverUrl,
                            contentDescription = null,
                            modifier = Modifier.size(44.dp),
                            shape = RoundedCornerShape(6.dp),
                            targetSize = 44.dp,
                        )
                        Spacer(modifier = Modifier.width(10.dp))
                    }
                    if (sourceTitle.isNotBlank()) {
                        Text(
                            text = sourceTitle,
                            style = MaterialTheme.typography.bodyMedium,
                            fontWeight = FontWeight.SemiBold,
                            color = MaterialTheme.colorScheme.onSurface,
                            maxLines = 2,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f),
                        )
                    }
                }
                Spacer(modifier = Modifier.height(10.dp))
            }

            // Author byline
            Row(verticalAlignment = Alignment.CenterVertically) {
                AvatarImage(url = authorAvatarUrl, name = authorName, size = 22.dp)
                Spacer(modifier = Modifier.width(8.dp))
                Text(
                    text = authorName,
                    style = MaterialTheme.typography.labelMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Spacer(modifier = Modifier.height(8.dp))

            // Pull-quote with accent rail
            Row(
                verticalAlignment = Alignment.Top,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Surface(
                    modifier = Modifier
                        .width(3.dp)
                        .height(60.dp),
                    color = MaterialTheme.colorScheme.primary,
                    shape = RoundedCornerShape(1.5.dp),
                ) {}
                Text(
                    text = highlight.quote.trim().ifBlank { "Untitled highlight" },
                    style = MaterialTheme.typography.bodyLarge.copy(fontStyle = FontStyle.Italic),
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 4,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Library tab — artifact list
// ---------------------------------------------------------------------------

@Composable
private fun LibraryTab(
    room: HighlighterRoomDetailSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    when {
        room.isLoading && room.artifacts.isEmpty() -> {
            Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) {
                CircularProgressIndicator()
            }
        }
        room.artifacts.isEmpty() -> {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(16.dp),
            ) {
                EmptyPanel("No library items yet")
            }
        }
        else -> {
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                items(room.artifacts, key = { it.shareEventId }) { artifact ->
                    ArtifactCard(artifact = artifact, dispatch = dispatch)
                }
            }
        }
    }
}

@Composable
private fun ArtifactCard(
    artifact: ArtifactRecord,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    val playableAudio = selectAudioUrl(artifact.preview.audioUrl, artifact.preview.audioPreviewUrl)
    val player = rememberPodcastPlayerController(dispatch)

    Surface(
        modifier = Modifier.fillMaxWidth(),
        color = MaterialTheme.colorScheme.surface,
        shape = RoundedCornerShape(8.dp),
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outline),
    ) {
        Row(
            modifier = Modifier.padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            // Cover
            if (artifact.preview.image.isNotBlank()) {
                RemoteImage(
                    url = artifact.preview.image,
                    contentDescription = null,
                    modifier = Modifier.size(48.dp),
                    shape = CoverShape,
                    targetSize = 48.dp,
                )
                Spacer(modifier = Modifier.width(12.dp))
            }
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = artifact.preview.title.ifBlank { artifact.preview.url.ifBlank { "Untitled" } },
                    style = MaterialTheme.typography.bodyMedium,
                    fontWeight = FontWeight.Medium,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                val subtitle = artifact.note.ifBlank {
                    artifact.preview.author.ifBlank { artifact.preview.domain }
                }
                if (subtitle.isNotBlank()) {
                    Text(
                        text = subtitle,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
            if (playableAudio != null) {
                Spacer(modifier = Modifier.width(8.dp))
                FilledTonalIconButton(onClick = { player.load(artifact) }) {
                    Icon(
                        imageVector = Icons.Filled.PlayArrow,
                        contentDescription = "Play ${artifact.preview.title.ifBlank { "episode" }}",
                    )
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Discussions tab — list of kind:11 threads
// ---------------------------------------------------------------------------

@Composable
private fun DiscussionsTab(
    room: HighlighterRoomDetailSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    val profiles = LocalProfiles.current
    val hydratedPubkeys = remember(profiles) { profiles.map { it.pubkeyHex }.toSet() }

    when {
        room.isLoading && room.discussions.isEmpty() -> {
            Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) {
                CircularProgressIndicator()
            }
        }
        room.discussions.isEmpty() -> {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(16.dp),
            ) {
                EmptyPanel("No discussions yet. Start one with the + button.")
            }
        }
        else -> {
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
            ) {
                items(room.discussions, key = { it.eventId }) { discussion ->
                    val pk = discussion.pubkey
                    LaunchedEffect(pk) {
                        if (pk.isNotBlank() && pk !in hydratedPubkeys) {
                            dispatch(HighlighterAppAction.RequestProfile(pk))
                        }
                    }
                    val profile = profiles.profileFor(pk)
                    val authorName = profile.displayNameOr(pk)
                    val authorAvatar = profile.avatarUrl()

                    DiscussionListRow(
                        discussion = discussion,
                        authorName = authorName,
                        authorAvatarUrl = authorAvatar,
                        onOpen = {
                            dispatch(
                                HighlighterAppAction.OpenComments(
                                    "e",
                                    discussion.eventId,
                                    11u,
                                ),
                            )
                        },
                    )
                    // Divider
                    Surface(
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(1.dp)
                            .padding(start = 52.dp),
                        color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.4f),
                    ) {}
                }
            }
        }
    }
}

@Composable
private fun DiscussionListRow(
    discussion: DiscussionRecord,
    authorName: String,
    authorAvatarUrl: String?,
    onOpen: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onOpen)
            .padding(vertical = 14.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.Top,
    ) {
        AvatarImage(url = authorAvatarUrl, name = authorName, size = 36.dp)
        Column(modifier = Modifier.weight(1f)) {
            val title = discussion.title.ifBlank {
                discussion.summary.ifBlank { discussion.eventId.take(12) }
            }
            Text(
                text = title,
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurface,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            if (discussion.body.isNotBlank()) {
                Spacer(modifier = Modifier.height(2.dp))
                Text(
                    text = discussion.body,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 3,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            // Attachment chip
            discussion.attachment?.let { a ->
                val chipLabel = a.title.ifBlank { a.url }
                if (chipLabel.isNotBlank()) {
                    Spacer(modifier = Modifier.height(4.dp))
                    Surface(
                        color = MaterialTheme.colorScheme.primaryContainer,
                        shape = RoundedCornerShape(6.dp),
                    ) {
                        Text(
                            text = chipLabel,
                            modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onPrimaryContainer,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
            Spacer(modifier = Modifier.height(4.dp))
            Text(
                text = authorName,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Discussion composer sheet (modal, not inline)
// ---------------------------------------------------------------------------

@Composable
private fun DiscussionComposerSheet(
    room: HighlighterRoomDetailSnapshot,
    onDismiss: () -> Unit,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    var title by rememberSaveable(room.groupId) { mutableStateOf("") }
    var body by rememberSaveable(room.groupId) { mutableStateOf("") }

    // Auto-dismiss when a new discussion is published successfully
    LaunchedEffect(room.lastPublishedDiscussionId) {
        if (room.lastPublishedDiscussionId != null) {
            onDismiss()
        }
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 20.dp)
            .padding(bottom = 24.dp)
            .imePadding(),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(
            text = "New Discussion",
            style = MaterialTheme.typography.titleLarge,
            fontWeight = FontWeight.SemiBold,
        )
        OutlinedTextField(
            value = title,
            onValueChange = { title = it },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text("Title") },
        )
        OutlinedTextField(
            value = body,
            onValueChange = { body = it },
            modifier = Modifier.fillMaxWidth(),
            minLines = 3,
            maxLines = 6,
            label = { Text("Body (optional)") },
        )
        room.discussionErrorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Text(
                text = message,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error,
            )
        }
        Button(
            onClick = {
                dispatch(
                    HighlighterAppAction.PublishRoomDiscussion(
                        title.trim(),
                        body.trim(),
                        null,
                    ),
                )
            },
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(8.dp),
            enabled = title.isNotBlank() && !room.isPublishingDiscussion,
        ) {
            if (room.isPublishingDiscussion) {
                CircularProgressIndicator(
                    modifier = Modifier.size(16.dp),
                    strokeWidth = 2.dp,
                    color = MaterialTheme.colorScheme.onPrimary,
                )
                Spacer(modifier = Modifier.width(8.dp))
            }
            Text(if (room.isPublishingDiscussion) "Posting…" else "Post Discussion")
        }
    }
}

// ---------------------------------------------------------------------------
// Chat tab — message list + inline composer
// ---------------------------------------------------------------------------

@Composable
private fun ChatTab(
    room: HighlighterRoomDetailSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    val profiles = LocalProfiles.current
    val hydratedPubkeys = remember(profiles) { profiles.map { it.pubkeyHex }.toSet() }
    var draft by rememberSaveable { mutableStateOf("") }

    // Request profiles for all message authors (deduplicated)
    val authorPubkeys = remember(room.chatMessages) {
        room.chatMessages.map { it.authorPubkey }.distinct()
    }
    authorPubkeys.forEach { pk ->
        LaunchedEffect(pk) {
            if (pk.isNotBlank() && pk !in hydratedPubkeys) {
                dispatch(HighlighterAppAction.RequestProfile(pk))
            }
        }
    }

    val listState = rememberLazyListState()
    // Detect scroll-to-top for load-more
    val isAtTop by remember {
        derivedStateOf { listState.firstVisibleItemIndex == 0 && listState.firstVisibleItemScrollOffset == 0 }
    }
    LaunchedEffect(isAtTop) {
        if (isAtTop && room.chatHasMore && !room.isChatLoadingMore) {
            dispatch(HighlighterAppAction.LoadMoreRoomChat)
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .imePadding(),
    ) {
        // Message list
        Box(modifier = Modifier.weight(1f)) {
            when {
                room.isLoading && room.chatMessages.isEmpty() -> {
                    Box(
                        modifier = Modifier.fillMaxSize(),
                        contentAlignment = Alignment.Center,
                    ) {
                        CircularProgressIndicator()
                    }
                }
                room.chatMessages.isEmpty() -> {
                    Box(
                        modifier = Modifier
                            .fillMaxSize()
                            .padding(16.dp),
                    ) {
                        EmptyPanel("No messages yet. Be the first to say something.")
                    }
                }
                else -> {
                    LazyColumn(
                        state = listState,
                        modifier = Modifier.fillMaxSize(),
                        contentPadding = PaddingValues(vertical = 8.dp),
                        reverseLayout = false,
                    ) {
                        // Load-more indicator at top
                        if (room.chatHasMore || room.isChatLoadingMore) {
                            item(key = "load_more") {
                                if (room.isChatLoadingMore) {
                                    Box(
                                        modifier = Modifier
                                            .fillMaxWidth()
                                            .padding(vertical = 12.dp),
                                        contentAlignment = Alignment.Center,
                                    ) {
                                        CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
                                    }
                                } else {
                                    TextButton(
                                        onClick = { dispatch(HighlighterAppAction.LoadMoreRoomChat) },
                                        modifier = Modifier.fillMaxWidth(),
                                    ) {
                                        Text("Load older messages")
                                    }
                                }
                            }
                        }

                        items(room.chatMessages, key = { it.eventId }) { message ->
                            val profile = profiles.profileFor(message.authorPubkey)
                            val authorName = profile.displayNameOr(message.authorPubkey)
                            val authorAvatar = profile.avatarUrl()
                            // Show header when author changes
                            val index = room.chatMessages.indexOf(message)
                            val showHeader = index == 0 ||
                                room.chatMessages[index - 1].authorPubkey != message.authorPubkey

                            ChatMessageRow(
                                message = message,
                                authorName = authorName,
                                authorAvatarUrl = authorAvatar,
                                showHeader = showHeader,
                            )
                        }
                    }
                }
            }
        }

        // Inline composer at the bottom
        Surface(
            modifier = Modifier.fillMaxWidth(),
            shadowElevation = 4.dp,
            color = MaterialTheme.colorScheme.surface,
        ) {
            Row(
                modifier = Modifier
                    .padding(horizontal = 12.dp, vertical = 8.dp),
                verticalAlignment = Alignment.Bottom,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                OutlinedTextField(
                    value = draft,
                    onValueChange = { draft = it },
                    modifier = Modifier.weight(1f),
                    placeholder = { Text("Message") },
                    minLines = 1,
                    maxLines = 4,
                    shape = RoundedCornerShape(20.dp),
                )
                room.chatErrorMessage?.takeIf { it.isNotBlank() }?.let { _ ->
                    // Error is shown briefly; clear it on next send
                }
                IconButton(
                    onClick = {
                        val text = draft.trim()
                        if (text.isNotBlank()) {
                            dispatch(HighlighterAppAction.PublishRoomChatMessage(text, null))
                            draft = ""
                        }
                    },
                    enabled = draft.isNotBlank() && !room.isSendingChatMessage,
                ) {
                    if (room.isSendingChatMessage) {
                        CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
                    } else {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.Send,
                            contentDescription = "Send",
                            tint = if (draft.isNotBlank()) MaterialTheme.colorScheme.primary
                            else MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun ChatMessageRow(
    message: ChatMessageRecord,
    authorName: String,
    authorAvatarUrl: String?,
    showHeader: Boolean,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(
                start = 12.dp,
                end = 12.dp,
                top = if (showHeader) 10.dp else 2.dp,
            ),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.Top,
    ) {
        if (showHeader) {
            AvatarImage(url = authorAvatarUrl, name = authorName, size = 28.dp)
        } else {
            Spacer(modifier = Modifier.size(28.dp))
        }
        Column(modifier = Modifier.weight(1f)) {
            if (showHeader) {
                Text(
                    text = authorName,
                    style = MaterialTheme.typography.labelMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Text(
                text = message.content,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
        }
    }
}
