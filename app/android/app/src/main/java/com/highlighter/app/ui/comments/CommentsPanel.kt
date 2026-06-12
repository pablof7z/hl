package com.highlighter.app.ui.comments

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
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.components.AvatarImage
import com.highlighter.app.ui.components.Panel
import com.highlighter.app.ui.components.SectionHeader
import com.highlighter.app.util.LocalProfiles
import com.highlighter.app.util.WebLinkPreview
import com.highlighter.app.util.avatarUrl
import com.highlighter.app.util.displayNameOr
import com.highlighter.app.util.profileFor
import uniffi.highlighter_core.CommentRecord
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterCommentsSnapshot

@Composable
internal fun CommentsPanel(
    comments: HighlighterCommentsSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    val topDraft = comments.drafts.firstOrNull { it.parentEventId == null }?.body ?: ""
    Panel {
        SectionHeader("Comments", comments.recordCount.toString())
        Spacer(modifier = Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(
                onClick = { dispatch(HighlighterAppAction.RefreshComments) },
                shape = RoundedCornerShape(8.dp),
            ) {
                Text(if (comments.isLoading) "Loading" else "Refresh")
            }
            TextButton(onClick = { dispatch(HighlighterAppAction.CloseComments) }) {
                Text("Close")
            }
        }
        comments.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.tertiary)
        }
        OutlinedTextField(
            value = topDraft,
            onValueChange = { dispatch(HighlighterAppAction.SetCommentDraft(null, it)) },
            modifier = Modifier.fillMaxWidth(),
            minLines = 2,
            maxLines = 5,
            label = { Text("Add comment") },
        )
        Spacer(modifier = Modifier.height(8.dp))
        Button(
            onClick = { dispatch(HighlighterAppAction.PublishComment(null)) },
            shape = RoundedCornerShape(8.dp),
            enabled = topDraft.isNotBlank() && !comments.isPublishing,
        ) {
            Text(if (comments.isPublishing) "Posting" else "Post")
        }
        comments.publishErrorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.tertiary)
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
                onLike = { dispatch(HighlighterAppAction.ToggleCommentLike(comment.eventId)) },
                onBookmark = { dispatch(HighlighterAppAction.ToggleCommentBookmark(comment.eventId)) },
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
    val profile = LocalProfiles.current.profileFor(comment.pubkey)
    val authorName = profile.displayNameOr(comment.pubkey)
    Row(modifier = Modifier.padding(vertical = 8.dp)) {
        AvatarImage(
            url = profile.avatarUrl(),
            name = authorName,
            size = 32.dp,
        )
        Spacer(modifier = Modifier.width(10.dp))
        Column(modifier = Modifier.fillMaxWidth()) {
            Text(
                text = authorName,
                style = MaterialTheme.typography.bodySmall,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = comment.body,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
            WebLinkPreview(text = comment.body)
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
}
