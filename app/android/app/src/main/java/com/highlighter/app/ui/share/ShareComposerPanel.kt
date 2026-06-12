package com.highlighter.app.ui.share

import android.util.Patterns
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.highlighter.app.PendingShare
import com.highlighter.app.ui.components.Panel
import com.highlighter.app.ui.components.SectionHeader
import com.highlighter.app.ui.components.ToggleButton
import uniffi.highlighter_core.CommunitySummary
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterShareComposerSnapshot

/**
 * Mirrors the iOS share composer (ShareRootView + ShareToCommunitySheet):
 * preview the inbound URL/text, pick a joined community, optionally add a note,
 * then publish via [HighlighterAppAction.PublishUrlShare]. The publish lifecycle
 * (spinner / error / success) is driven entirely by the Rust
 * [HighlighterShareComposerSnapshot]; the inbound payload itself is host-held
 * presentation state passed down as [share].
 */
@Composable
internal fun ShareComposerPanel(
    share: PendingShare,
    composer: HighlighterShareComposerSnapshot,
    communities: List<CommunitySummary>,
    dispatch: (HighlighterAppAction) -> Unit,
    onClose: () -> Unit,
) {
    var note by remember { mutableStateOf(share.note) }
    var selectedGroupId by remember { mutableStateOf<String?>(null) }

    // A successful publish clears the result flag, surfaces the core toast, and
    // dismisses the composer — matching ShareToCommunitySheet.onChange(publishedGroupId).
    LaunchedEffect(composer.publishedGroupId) {
        if (composer.publishedGroupId != null) {
            dispatch(HighlighterAppAction.ClearShareComposerResult)
            onClose()
        }
    }

    Panel {
        SectionHeader("Share", if (composer.isPublishing) "Publishing" else "To community")
        Spacer(modifier = Modifier.height(8.dp))

        if (share.url != null) {
            Text(
                text = "Link",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                text = share.url,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
                maxLines = 3,
                overflow = TextOverflow.Ellipsis,
            )
        } else {
            Text(
                text = "Shared text",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                text = share.text,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
                maxLines = 4,
                overflow = TextOverflow.Ellipsis,
            )
            Spacer(modifier = Modifier.height(6.dp))
            Text(
                text = "No link detected in the shared text — share a link to publish it.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.tertiary,
            )
        }

        Spacer(modifier = Modifier.height(10.dp))
        OutlinedTextField(
            value = note,
            onValueChange = { note = it },
            modifier = Modifier.fillMaxWidth(),
            minLines = 1,
            maxLines = 4,
            label = { Text("Note (optional)") },
            enabled = !composer.isPublishing,
        )

        Spacer(modifier = Modifier.height(10.dp))
        if (communities.isEmpty()) {
            Text(
                text = "Join a community in the app first, then share again.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        } else {
            SectionHeader("Community", communities.size.toString())
            Spacer(modifier = Modifier.height(6.dp))
            LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                items(communities, key = { it.id }) { community ->
                    ToggleButton(
                        label = community.name.ifBlank { community.id }.take(18),
                        selected = selectedGroupId == community.id,
                    ) {
                        if (!composer.isPublishing) {
                            selectedGroupId =
                                if (selectedGroupId == community.id) null else community.id
                        }
                    }
                }
            }
        }

        composer.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = message,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.tertiary,
            )
            TextButton(onClick = { dispatch(HighlighterAppAction.ClearShareComposerError) }) {
                Text("Dismiss")
            }
        }

        Spacer(modifier = Modifier.height(10.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = {
                    val url = share.url ?: return@Button
                    val groupId = selectedGroupId ?: return@Button
                    val trimmedNote = note.trim()
                    dispatch(
                        HighlighterAppAction.PublishUrlShare(
                            url = url,
                            groupId = groupId,
                            note = trimmedNote.ifBlank { null },
                        ),
                    )
                },
                shape = RoundedCornerShape(8.dp),
                enabled = share.url != null &&
                    selectedGroupId != null &&
                    !composer.isPublishing,
            ) {
                Text(if (composer.isPublishing) "Publishing" else "Publish")
            }
            TextButton(
                onClick = onClose,
                enabled = !composer.isPublishing,
            ) {
                Text("Cancel")
            }
        }
    }
}

/** Returns the first http(s) URL inside [text], or null. Mirrors iOS NSDataDetector. */
internal fun firstUrlIn(text: String): String? {
    val trimmed = text.trim()
    if (trimmed.isEmpty()) return null
    val matcher = Patterns.WEB_URL.matcher(trimmed)
    while (matcher.find()) {
        val candidate = trimmed.substring(matcher.start(), matcher.end())
        if (candidate.startsWith("http://", ignoreCase = true) ||
            candidate.startsWith("https://", ignoreCase = true)
        ) {
            return candidate
        }
    }
    return null
}
