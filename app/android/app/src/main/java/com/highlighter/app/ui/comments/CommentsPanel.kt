package com.highlighter.app.ui.comments

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
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
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
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
import uniffi.highlighter_core.HighlighterCommentChildLinks
import uniffi.highlighter_core.HighlighterCommentInteraction
import uniffi.highlighter_core.HighlighterCommentsSnapshot

// ---------------------------------------------------------------------------
// Data model (mirrors iOS CommentTreeBuilder / CommentNode)
// ---------------------------------------------------------------------------

/**
 * One node of a NIP-22 comment thread — a [CommentRecord] plus its
 * recursively-built replies, sorted as the core delivers them.
 * Mirrors `CommentNode` in iOS `CommentTreeBuilder.swift`.
 */
private data class CommentNode(
    val record: CommentRecord,
    val children: List<CommentNode>,
)

/**
 * Build a nested display forest from the Rust-owned thread links.
 * Rust owns NIP-22 parentage, orphan promotion, and ordering;
 * we only assemble ids into view nodes — exactly as iOS does.
 * Mirrors `CommentTreeBuilder.build(snapshot:)`.
 */
private fun buildCommentTree(snapshot: HighlighterCommentsSnapshot): List<CommentNode> {
    val recordsById: Map<String, CommentRecord> =
        snapshot.records.associateBy { it.eventId }
    val childrenById: Map<String, List<String>> =
        snapshot.childLinks.associate { it.eventId to it.childEventIds }

    fun node(id: String): CommentNode? {
        val record = recordsById[id] ?: return null
        val children = (childrenById[id] ?: emptyList()).mapNotNull { node(it) }
        return CommentNode(record = record, children = children)
    }

    return snapshot.topLevelEventIds.mapNotNull { node(it) }
}

// ---------------------------------------------------------------------------
// CommentsPanel
// ---------------------------------------------------------------------------

@Composable
internal fun CommentsPanel(
    comments: HighlighterCommentsSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    // eventId being replied to; null = top-level draft
    var replyingToEventId by remember { mutableStateOf<String?>(null) }

    val draftBody: String = comments.drafts
        .firstOrNull { it.parentEventId == replyingToEventId }?.body ?: ""

    val tree = remember(comments.records, comments.topLevelEventIds, comments.childLinks) {
        buildCommentTree(comments)
    }

    Panel {
        SectionHeader("Comments", comments.recordCount.toString())
        Spacer(modifier = Modifier.height(8.dp))
        TextButton(onClick = { dispatch(HighlighterAppAction.CloseComments) }) {
            Text("Close")
        }

        comments.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(
                text = message,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.tertiary,
            )
        }

        // Reply context banner -----------------------------------------------
        val replyingToRecord = replyingToEventId?.let { id ->
            comments.records.firstOrNull { it.eventId == id }
        }
        if (replyingToRecord != null) {
            ReplyContextBanner(
                replyingTo = replyingToRecord,
                onCancel = {
                    replyingToEventId = null
                    // reset draft for the old parentEventId, keep top-level draft intact
                    dispatch(HighlighterAppAction.SetCommentDraft(null, draftBody))
                },
            )
        }

        // Composer -----------------------------------------------------------
        OutlinedTextField(
            value = draftBody,
            onValueChange = { dispatch(HighlighterAppAction.SetCommentDraft(replyingToEventId, it)) },
            modifier = Modifier.fillMaxWidth(),
            minLines = 2,
            maxLines = 5,
            label = {
                Text(if (replyingToEventId == null) "Add comment" else "Add reply")
            },
        )
        Spacer(modifier = Modifier.height(8.dp))
        Button(
            onClick = { dispatch(HighlighterAppAction.PublishComment(replyingToEventId)) },
            shape = RoundedCornerShape(8.dp),
            enabled = draftBody.isNotBlank() && !comments.isPublishing,
        ) {
            Text(if (comments.isPublishing) "Posting" else "Post")
        }
        comments.publishErrorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(
                text = message,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.tertiary,
            )
        }

        Spacer(modifier = Modifier.height(12.dp))

        // Thread-tree --------------------------------------------------------
        tree.forEach { node ->
            CommentRow(
                comment = node.record,
                depth = 0,
                likeCount = comments.interactions.likeCountFor(node.record.eventId),
                bookmarked = comments.interactions.isBookmarkedFor(node.record.eventId),
                onLike = { dispatch(HighlighterAppAction.ToggleCommentLike(node.record.eventId)) },
                onBookmark = { dispatch(HighlighterAppAction.ToggleCommentBookmark(node.record.eventId)) },
                onReply = { replyingToEventId = node.record.eventId },
                dispatch = dispatch,
            )
            // Inline reply preview (first child, like iOS inlineReplyPreview)
            node.children.firstOrNull()?.let { firstChild ->
                CommentRow(
                    comment = firstChild.record,
                    depth = 1,
                    likeCount = comments.interactions.likeCountFor(firstChild.record.eventId),
                    bookmarked = comments.interactions.isBookmarkedFor(firstChild.record.eventId),
                    onLike = {
                        dispatch(HighlighterAppAction.ToggleCommentLike(firstChild.record.eventId))
                    },
                    onBookmark = {
                        dispatch(
                            HighlighterAppAction.ToggleCommentBookmark(firstChild.record.eventId),
                        )
                    },
                    onReply = { replyingToEventId = firstChild.record.eventId },
                    dispatch = dispatch,
                )
                if (node.children.size > 1) {
                    val extra = node.children.size - 1
                    TextButton(
                        onClick = { replyingToEventId = node.record.eventId },
                        modifier = Modifier.padding(start = 42.dp),
                    ) {
                        Text(
                            text = "View $extra more ${if (extra == 1) "reply" else "replies"}",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.primary,
                        )
                    }
                }
            }
            HorizontalDivider(modifier = Modifier.padding(vertical = 2.dp))
        }
    }
}

// ---------------------------------------------------------------------------
// ReplyContextBanner
// ---------------------------------------------------------------------------

@Composable
private fun ReplyContextBanner(
    replyingTo: CommentRecord,
    onCancel: () -> Unit,
) {
    val profiles = LocalProfiles.current
    val profile = profiles.profileFor(replyingTo.pubkey)
    val name = profile.displayNameOr(replyingTo.pubkey)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(
                color = MaterialTheme.colorScheme.secondaryContainer,
                shape = RoundedCornerShape(8.dp),
            )
            .padding(horizontal = 12.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = "Replying to $name",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSecondaryContainer,
            modifier = Modifier.weight(1f),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        TextButton(onClick = onCancel) {
            Text(
                text = "Cancel",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSecondaryContainer,
            )
        }
    }
    Spacer(modifier = Modifier.height(6.dp))
}

// ---------------------------------------------------------------------------
// CommentRow
// ---------------------------------------------------------------------------

@Composable
private fun CommentRow(
    comment: CommentRecord,
    depth: Int,
    likeCount: ULong,
    bookmarked: Boolean,
    onLike: () -> Unit,
    onBookmark: () -> Unit,
    onReply: () -> Unit,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    val profiles = LocalProfiles.current
    val profile = profiles.profileFor(comment.pubkey)
    val authorName = profile.displayNameOr(comment.pubkey)

    // Request profile enrichment (de-duped by Rust)
    LaunchedEffect(comment.pubkey) {
        dispatch(HighlighterAppAction.RequestProfile(comment.pubkey))
    }

    val indentStart = if (depth > 0) 42.dp else 0.dp
    Row(
        modifier = Modifier
            .padding(start = indentStart, top = 8.dp, end = 0.dp, bottom = 8.dp)
            .testTag(if (depth == 0) "comment_row" else "comment_reply_row"),
    ) {
        // Thread rail for depth-1 replies
        if (depth > 0) {
            Box(
                modifier = Modifier
                    .padding(end = 8.dp)
                    .width(2.dp)
                    .height(48.dp)
                    .background(
                        color = MaterialTheme.colorScheme.primary.copy(alpha = 0.35f),
                        shape = RoundedCornerShape(1.dp),
                    ),
            )
        }
        AvatarImage(
            url = profile.avatarUrl(),
            name = authorName,
            size = if (depth == 0) 36.dp else 28.dp,
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
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                TextButton(onClick = onLike) {
                    Text(
                        text = if (likeCount > 0uL) "Like $likeCount" else "Like",
                        style = MaterialTheme.typography.labelMedium,
                    )
                }
                TextButton(onClick = onBookmark) {
                    Text(
                        text = if (bookmarked) "Saved" else "Save",
                        style = MaterialTheme.typography.labelMedium,
                    )
                }
                TextButton(
                    onClick = onReply,
                    modifier = Modifier.testTag("comment_reply_button"),
                ) {
                    Text(
                        text = "Reply",
                        style = MaterialTheme.typography.labelMedium,
                    )
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Interaction helpers
// ---------------------------------------------------------------------------

private fun List<HighlighterCommentInteraction>.likeCountFor(eventId: String): ULong =
    firstOrNull { it.eventId == eventId }?.likeCount ?: 0uL

private fun List<HighlighterCommentInteraction>.isBookmarkedFor(eventId: String): Boolean =
    firstOrNull { it.eventId == eventId }?.isBookmarked == true
