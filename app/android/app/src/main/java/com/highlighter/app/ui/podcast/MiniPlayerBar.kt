package com.highlighter.app.ui.podcast

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.components.RemoteImage

/**
 * Slim now-playing bar shown just above the bottom navigation bar whenever an
 * episode is loaded. Tapping the bar opens the full listening screen; the
 * trailing button toggles play/pause. A 2dp progress line sits along the
 * bottom edge. Mirrors iOS MiniPlayerView.
 */
@Composable
internal fun MiniPlayerBar(
    state: PodcastPlaybackState,
    onOpen: () -> Unit,
    onToggle: () -> Unit,
    modifier: Modifier = Modifier,
) {
    if (!state.isLoaded) return

    Box(
        modifier = modifier
            .fillMaxWidth()
            .height(56.dp)
            .background(MaterialTheme.colorScheme.surfaceVariant)
            .clickable(onClick = onOpen)
            .semantics { contentDescription = "Now playing. Opens the full player." },
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 12.dp)
                .height(56.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            RemoteImage(
                url = state.imageUrl,
                contentDescription = null,
                modifier = Modifier.size(40.dp),
                shape = RoundedCornerShape(6.dp),
                targetSize = 40.dp,
            )
            Spacer(modifier = Modifier.width(12.dp))
            Column(modifier = Modifier.weight(1f)) {
                if (state.showTitle.isNotBlank()) {
                    Text(
                        text = state.showTitle,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                Text(
                    text = state.title,
                    style = MaterialTheme.typography.bodyMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Spacer(modifier = Modifier.width(8.dp))
            PlayPauseButton(
                isPlaying = state.isPlaying,
                isBuffering = state.isBuffering,
                onToggle = onToggle,
            )
        }

        // Progress line pinned to the bottom edge.
        Box(
            modifier = Modifier
                .align(Alignment.BottomStart)
                .fillMaxWidth()
                .height(2.dp)
                .background(MaterialTheme.colorScheme.surface),
        )
        Box(
            modifier = Modifier
                .align(Alignment.BottomStart)
                .fillMaxWidth(state.progressFraction)
                .height(2.dp)
                .background(MaterialTheme.colorScheme.primary),
        )
    }
}

@Composable
internal fun PlayPauseButton(
    isPlaying: Boolean,
    isBuffering: Boolean,
    onToggle: () -> Unit,
    modifier: Modifier = Modifier,
) {
    IconButton(onClick = onToggle, modifier = modifier) {
        if (isBuffering) {
            CircularProgressIndicator(
                modifier = Modifier.size(20.dp),
                strokeWidth = 2.dp,
                color = MaterialTheme.colorScheme.onSurface,
            )
        } else {
            Icon(
                imageVector = if (isPlaying) Icons.Filled.Pause else Icons.Filled.PlayArrow,
                contentDescription = if (isPlaying) "Pause" else "Play",
                tint = MaterialTheme.colorScheme.onSurface,
            )
        }
    }
}
