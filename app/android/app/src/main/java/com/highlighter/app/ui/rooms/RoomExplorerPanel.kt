package com.highlighter.app.ui.rooms

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.components.AvatarImage
import com.highlighter.app.ui.components.Chip
import com.highlighter.app.ui.components.EmptyPanel
import com.highlighter.app.ui.components.SectionHeader
import com.highlighter.app.util.signalLabel
import uniffi.highlighter_core.CommunitySummary
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterRoomExplorerSnapshot
import uniffi.highlighter_core.RoomRecommendation

@Composable
internal fun RoomExplorerPanel(
    explorer: HighlighterRoomExplorerSnapshot,
    joinedRoomIds: Set<String>,
    dispatch: (HighlighterAppAction) -> Unit,
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
                onClick = { dispatch(HighlighterAppAction.RefreshRoomExplorer) },
                shape = RoundedCornerShape(8.dp),
                border = BorderStroke(1.dp, MaterialTheme.colorScheme.outline),
                enabled = !explorer.isLoading,
            ) {
                Text(if (explorer.isLoading) "Refreshing" else "Refresh")
            }
            OutlinedButton(
                onClick = { dispatch(HighlighterAppAction.RefreshRoomBrowseAll) },
                shape = RoundedCornerShape(8.dp),
                border = BorderStroke(1.dp, MaterialTheme.colorScheme.outline),
                enabled = !explorer.isBrowseLoading,
            ) {
                Text(if (explorer.isBrowseLoading) "Loading" else "Browse all")
            }
        }
        explorer.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Text(
                text = message,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.tertiary,
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
            dispatch = dispatch,
        )
        RecommendationShelf(
            title = "Friends are here",
            count = explorer.friendsShelfCount,
            recommendations = explorer.friendsShelf,
            joinedRoomIds = joinedRoomIds,
            dispatch = dispatch,
        )
        RecommendationShelf(
            title = "Writers you read",
            count = explorer.authorsShelfCount,
            recommendations = explorer.authorsShelf,
            joinedRoomIds = joinedRoomIds,
            dispatch = dispatch,
        )
        RoomShelf(
            title = "New & noteworthy",
            count = explorer.newNoteworthyCount,
            rooms = explorer.newNoteworthy,
            joinedRoomIds = joinedRoomIds,
            dispatch = dispatch,
        )
        if (explorer.allRooms.isNotEmpty()) {
            RoomShelf(
                title = "Browse all",
                count = explorer.allRoomCount,
                rooms = explorer.allRooms.take(12),
                joinedRoomIds = joinedRoomIds,
                dispatch = dispatch,
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
    dispatch: (HighlighterAppAction) -> Unit,
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
                    dispatch = dispatch,
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
    dispatch: (HighlighterAppAction) -> Unit,
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
                    dispatch = dispatch,
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
    dispatch: (HighlighterAppAction) -> Unit,
) {
    Surface(
        modifier = Modifier
            .width(220.dp)
            .clickable { dispatch(HighlighterAppAction.OpenRoom(room.id)) },
        color = MaterialTheme.colorScheme.surface,
        shape = RoundedCornerShape(8.dp),
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outline),
    ) {
        Column(modifier = Modifier.padding(12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                AvatarImage(
                    url = room.picture,
                    name = room.name.ifBlank { "#" },
                    size = 40.dp,
                )
                Spacer(modifier = Modifier.width(10.dp))
                Text(
                    text = room.name.ifBlank { room.id },
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Spacer(modifier = Modifier.height(5.dp))
            Text(
                text = subtitle.ifBlank { room.id },
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
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
                    onClick = { dispatch(HighlighterAppAction.RequestJoinRoom(room.id, room.name)) },
                    enabled = !isJoined,
                ) {
                    Text(if (isJoined) "Joined" else "Join")
                }
            }
        }
    }
}
