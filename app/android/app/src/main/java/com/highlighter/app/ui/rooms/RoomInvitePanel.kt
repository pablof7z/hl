package com.highlighter.app.ui.rooms

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.components.Panel
import com.highlighter.app.ui.components.SectionHeader
import com.highlighter.app.ui.search.SearchGroupHeader
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterRoomInviteCandidateSource
import uniffi.highlighter_core.HighlighterRoomInviteSnapshot

@Composable
internal fun RoomInvitePanel(
    invite: HighlighterRoomInviteSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    Panel {
        SectionHeader("Invites", invite.selected.size.toString())
        Spacer(modifier = Modifier.height(8.dp))
        OutlinedTextField(
            value = invite.query,
            onValueChange = { dispatch(HighlighterAppAction.SetRoomInviteQuery(it)) },
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
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                TextButton(onClick = { dispatch(HighlighterAppAction.AcceptRoomInvitePastedCandidate) }) {
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
                            dispatch(
                                HighlighterAppAction.ToggleRoomInviteCandidate(
                                    pubkey,
                                    HighlighterRoomInviteCandidateSource.FOLLOW,
                                ),
                            )
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
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    TextButton(
                        onClick = {
                            dispatch(HighlighterAppAction.RemoveRoomInviteCandidate(candidate.pubkeyHex))
                        },
                    ) {
                        Text("Remove")
                    }
                }
            }
        }
        invite.inviteUrl?.takeIf { it.isNotBlank() }?.let { url ->
            Spacer(modifier = Modifier.height(8.dp))
            SelectionContainer {
                Text(text = url, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        listOfNotNull(invite.addErrorMessage, invite.inviteLinkErrorMessage, invite.toastMessage)
            .filter { it.isNotBlank() }
            .forEach { message ->
                Spacer(modifier = Modifier.height(6.dp))
                Text(text = message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.tertiary)
            }
        Spacer(modifier = Modifier.height(10.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(
                onClick = { dispatch(HighlighterAppAction.RefreshRoomInvite) },
                shape = RoundedCornerShape(8.dp),
            ) {
                Text(if (invite.isLoadingFollows) "Loading" else "Refresh")
            }
            OutlinedButton(
                onClick = { dispatch(HighlighterAppAction.MintRoomInviteLink) },
                shape = RoundedCornerShape(8.dp),
                enabled = !invite.isMintingInviteLink,
            ) {
                Text(if (invite.isMintingInviteLink) "Minting" else "Link")
            }
            Button(
                onClick = { dispatch(HighlighterAppAction.SubmitRoomInviteMembers) },
                shape = RoundedCornerShape(8.dp),
                enabled = invite.selected.isNotEmpty() && !invite.isAddingMembers,
            ) {
                Text(if (invite.isAddingMembers) "Adding" else "Add")
            }
            TextButton(onClick = { dispatch(HighlighterAppAction.CloseRoomInvite) }) {
                Text("Close")
            }
        }
    }
}
