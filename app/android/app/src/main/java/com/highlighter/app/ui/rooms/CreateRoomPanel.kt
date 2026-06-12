package com.highlighter.app.ui.rooms

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.components.Panel
import com.highlighter.app.ui.components.SectionHeader
import com.highlighter.app.ui.components.ToggleButton
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterCreateRoomSnapshot
import uniffi.highlighter_core.RoomAccess
import uniffi.highlighter_core.RoomVisibility

@Composable
internal fun CreateRoomPanel(
    createRoom: HighlighterCreateRoomSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
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
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.tertiary)
            TextButton(onClick = { dispatch(HighlighterAppAction.ClearCreateRoomError) }) {
                Text("Dismiss")
            }
        }
        createRoom.createdGroupId?.takeIf { it.isNotBlank() }?.let { groupId ->
            Spacer(modifier = Modifier.height(10.dp))
            Text(
                text = "Created ${groupId.take(12)}",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TextButton(onClick = { dispatch(HighlighterAppAction.OpenRoom(groupId)) }) {
                    Text("Open")
                }
                TextButton(onClick = { dispatch(HighlighterAppAction.OpenRoomInvite(groupId)) }) {
                    Text("Invite")
                }
                TextButton(onClick = { dispatch(HighlighterAppAction.ClearCreateRoomResult) }) {
                    Text("Clear")
                }
            }
        }
        Spacer(modifier = Modifier.height(10.dp))
        Button(
            onClick = {
                val cleanName = name.trim()
                if (cleanName.isNotEmpty()) {
                    dispatch(
                        HighlighterAppAction.SubmitCreateRoom(
                            cleanName,
                            about.trim(),
                            visibility,
                            access,
                        ),
                    )
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
