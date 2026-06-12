package com.highlighter.app.ui.rooms

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.components.CoverShape
import com.highlighter.app.ui.components.Panel
import com.highlighter.app.ui.components.RemoteImage
import com.highlighter.app.ui.components.SectionHeader
import com.highlighter.app.ui.podcast.rememberPodcastPlayerController
import com.highlighter.app.ui.podcast.selectAudioUrl
import com.highlighter.app.ui.search.SearchGroupHeader
import com.highlighter.app.ui.search.SearchResultRow
import uniffi.highlighter_core.ArtifactRecord
import uniffi.highlighter_core.ChatMessageRecord
import uniffi.highlighter_core.DiscussionRecord
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterRoomDetailSnapshot
import uniffi.highlighter_core.HydratedHighlight

@Composable
internal fun RoomDetailPanel(
    room: HighlighterRoomDetailSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    var discussionTitle by remember(room.groupId) { mutableStateOf("") }
    var discussionBody by remember(room.groupId) { mutableStateOf("") }
    var chatBody by remember(room.groupId) { mutableStateOf("") }
    Panel {
        SectionHeader("Room", room.groupId.take(12))
        Spacer(modifier = Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(
                onClick = { dispatch(HighlighterAppAction.RefreshRoom) },
                shape = RoundedCornerShape(8.dp),
            ) {
                Text(if (room.isLoading) "Loading" else "Refresh")
            }
            TextButton(onClick = { dispatch(HighlighterAppAction.CloseRoom) }) {
                Text("Close")
            }
        }
        room.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.tertiary)
        }
        if (room.artifacts.isNotEmpty()) {
            SearchGroupHeader("Artifacts", room.artifactCount.toString())
            room.artifacts.take(4).forEach { ArtifactSummaryRow(it) }
        }
        if (room.highlights.isNotEmpty()) {
            SearchGroupHeader("Highlights", room.highlightCount.toString())
            room.highlights.take(4).forEach { hydrated ->
                HydratedHighlightRow(hydrated) {
                    dispatch(HighlighterAppAction.OpenComments("e", hydrated.highlight.eventId, 9802u))
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
                dispatch(
                    HighlighterAppAction.PublishRoomDiscussion(
                        discussionTitle.trim(),
                        discussionBody.trim(),
                        null,
                    ),
                )
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
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.tertiary)
        }
        room.discussions.take(5).forEach { discussion ->
            DiscussionRow(discussion) {
                dispatch(HighlighterAppAction.OpenComments("e", discussion.eventId, 11u))
            }
        }
        SearchGroupHeader("Chat", room.chatMessageCount.toString())
        room.chatMessages.takeLast(6).forEach { ChatRow(it) }
        if (room.chatHasMore) {
            TextButton(onClick = { dispatch(HighlighterAppAction.LoadMoreRoomChat) }) {
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
                dispatch(HighlighterAppAction.PublishRoomChatMessage(chatBody.trim(), null))
                chatBody = ""
            },
            shape = RoundedCornerShape(8.dp),
            enabled = chatBody.isNotBlank() && !room.isSendingChatMessage,
        ) {
            Text(if (room.isSendingChatMessage) "Sending" else "Send")
        }
        room.chatErrorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.tertiary)
        }
    }
}

@Composable
private fun ArtifactSummaryRow(record: ArtifactRecord) {
    // A podcast artifact is anything that carries a playable audio URL.
    val playableAudio = selectAudioUrl(record.preview.audioUrl, record.preview.audioPreviewUrl)
    val player = rememberPodcastPlayerController()

    Row(verticalAlignment = Alignment.CenterVertically) {
        Row(modifier = Modifier.weight(1f)) {
            SearchResultRow(
                title = record.preview.title.ifBlank { record.preview.url.ifBlank { "Untitled" } },
                subtitle = record.note.ifBlank { record.preview.author.ifBlank { record.preview.domain } },
                leading = record.preview.image.takeIf { it.isNotBlank() }?.let { image ->
                    {
                        RemoteImage(
                            url = image,
                            contentDescription = null,
                            modifier = Modifier.size(40.dp),
                            shape = CoverShape,
                        )
                    }
                },
            )
        }
        if (playableAudio != null) {
            FilledTonalIconButton(onClick = { player.load(record) }) {
                Icon(
                    imageVector = Icons.Filled.PlayArrow,
                    contentDescription = "Play ${record.preview.title.ifBlank { "episode" }}",
                )
            }
        }
    }
}

@Composable
private fun HydratedHighlightRow(item: HydratedHighlight, onComments: () -> Unit) {
    val artwork = item.highlight.imageUrl.takeIf { it.isNotBlank() }
        ?: item.artifact?.preview?.image?.takeIf { it.isNotBlank() }
    Row(modifier = Modifier.padding(vertical = 7.dp)) {
        if (artwork != null) {
            RemoteImage(
                url = artwork,
                contentDescription = null,
                modifier = Modifier.size(40.dp),
                shape = CoverShape,
            )
            Spacer(modifier = Modifier.width(12.dp))
        }
        Column {
            Text(
                text = item.highlight.quote.ifBlank { "Untitled highlight" },
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
                fontWeight = FontWeight.Medium,
                maxLines = 3,
                overflow = TextOverflow.Ellipsis,
            )
            TextButton(onClick = onComments) {
                Text("Comments")
            }
        }
    }
}

@Composable
private fun DiscussionRow(discussion: DiscussionRecord, onComments: () -> Unit) {
    Column(modifier = Modifier.padding(vertical = 7.dp)) {
        Text(
            text = discussion.title.ifBlank { discussion.summary.ifBlank { discussion.eventId.take(12) } },
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurface,
            fontWeight = FontWeight.Medium,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
        if (discussion.body.isNotBlank()) {
            Text(
                text = discussion.body,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
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
