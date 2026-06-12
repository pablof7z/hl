package com.highlighter.app.ui.feedback

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.components.Panel
import com.highlighter.app.ui.components.SectionHeader
import com.highlighter.app.ui.search.SearchGroupHeader
import com.highlighter.app.ui.search.SearchResultRow
import uniffi.highlighter_core.FeedbackEventRecord
import uniffi.highlighter_core.FeedbackThreadRecord
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterFeedbackSnapshot

@Composable
internal fun FeedbackPanel(
    feedback: HighlighterFeedbackSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    Panel {
        SectionHeader("Feedback", feedback.threadCount.toString())
        Spacer(modifier = Modifier.height(8.dp))
        OutlinedTextField(
            value = feedback.newThreadDraft,
            onValueChange = { dispatch(HighlighterAppAction.SetFeedbackNewThreadDraft(it)) },
            modifier = Modifier.fillMaxWidth(),
            minLines = 2,
            maxLines = 5,
            label = { Text("New feedback") },
        )
        Spacer(modifier = Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = { dispatch(HighlighterAppAction.PublishFeedbackNewThread) },
                shape = RoundedCornerShape(8.dp),
                enabled = feedback.newThreadDraft.isNotBlank() && !feedback.isPublishingNewThread,
            ) {
                Text(if (feedback.isPublishingNewThread) "Sending" else "Send")
            }
            OutlinedButton(
                onClick = { dispatch(HighlighterAppAction.RefreshFeedbackThreads) },
                shape = RoundedCornerShape(8.dp),
            ) {
                Text(if (feedback.isLoadingThreads) "Loading" else "Refresh")
            }
        }
        feedback.publishErrorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.tertiary)
        }
        if (feedback.threads.isNotEmpty()) {
            SearchGroupHeader("Threads", feedback.threadCount.toString())
            feedback.threads.take(5).forEach { thread ->
                FeedbackThreadRow(thread) { rootId ->
                    dispatch(HighlighterAppAction.OpenFeedbackThread(rootId))
                }
            }
        } else if (feedback.isLoadingThreads) {
            Spacer(modifier = Modifier.height(8.dp))
            Text(text = "Loading feedback", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        if (feedback.selectedRootEventId != null) {
            SearchGroupHeader("Conversation", feedback.selectedEventCount.toString())
            feedback.selectedEvents.takeLast(8).forEach { event ->
                FeedbackEventRow(event)
            }
            OutlinedTextField(
                value = feedback.replyDraft,
                onValueChange = { dispatch(HighlighterAppAction.SetFeedbackReplyDraft(it)) },
                modifier = Modifier.fillMaxWidth(),
                minLines = 1,
                maxLines = 4,
                label = { Text("Reply") },
            )
            Spacer(modifier = Modifier.height(8.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(
                    onClick = { dispatch(HighlighterAppAction.PublishFeedbackReply) },
                    shape = RoundedCornerShape(8.dp),
                    enabled = feedback.replyDraft.isNotBlank() && !feedback.isPublishingReply,
                ) {
                    Text(if (feedback.isPublishingReply) "Sending" else "Reply")
                }
                OutlinedButton(
                    onClick = { dispatch(HighlighterAppAction.RefreshFeedbackThread) },
                    shape = RoundedCornerShape(8.dp),
                ) {
                    Text(if (feedback.isLoadingThread) "Loading" else "Refresh")
                }
                TextButton(onClick = { dispatch(HighlighterAppAction.CloseFeedbackThread) }) {
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
