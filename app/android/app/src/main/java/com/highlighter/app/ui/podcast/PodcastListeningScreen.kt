package com.highlighter.app.ui.podcast

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Replay
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.components.RemoteImage
import uniffi.highlighter_core.Chapter

private const val SKIP_SECONDS = 15.0

/**
 * Full-screen podcast player: artwork, show/episode titles, a draggable
 * scrubber bound to position/duration, ±15s skip, a speed selector, and a
 * chapter list (when the artifact carries chapters). Waveform + transcript
 * views are intentionally deferred (see KNOWN LIMITATIONS in the PR notes) —
 * v1 focuses on solid playback transport.
 *
 * Mirrors iOS PodcastListeningView's transport, sized for a phone column.
 */
@Composable
internal fun PodcastListeningScreen(
    state: PodcastPlaybackState,
    onToggle: () -> Unit,
    onSeek: (Double) -> Unit,
    onSkip: (Double) -> Unit,
    onSetSpeed: (Float) -> Unit,
) {
    if (!state.isLoaded) {
        Box(modifier = Modifier.fillMaxWidth().padding(32.dp), contentAlignment = Alignment.Center) {
            Text(
                text = "Nothing playing",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        return
    }

    LazyColumn(
        modifier = Modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        item {
            RemoteImage(
                url = state.imageUrl,
                contentDescription = null,
                modifier = Modifier
                    .fillMaxWidth(0.7f)
                    .aspectRatio(1f)
                    .padding(top = 16.dp),
                shape = RoundedCornerShape(16.dp),
            )
        }

        item {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 24.dp, vertical = 16.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                if (state.showTitle.isNotBlank()) {
                    Text(
                        text = state.showTitle,
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Spacer(modifier = Modifier.height(4.dp))
                }
                Text(
                    text = state.title,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Bold,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 3,
                    overflow = TextOverflow.Ellipsis,
                )
                durationLabel(state.metadataDurationSeconds, state.durationSeconds)?.let { label ->
                    Spacer(modifier = Modifier.height(4.dp))
                    Text(
                        text = label,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }

        item { Scrubber(state = state, onSeek = onSeek) }

        item {
            Spacer(modifier = Modifier.height(8.dp))
            TransportRow(state = state, onToggle = onToggle, onSkip = onSkip)
        }

        item {
            Spacer(modifier = Modifier.height(16.dp))
            SpeedSelector(current = state.speed, onSetSpeed = onSetSpeed)
        }

        state.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            item {
                Spacer(modifier = Modifier.height(12.dp))
                Text(
                    text = message,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(horizontal = 24.dp),
                )
            }
        }

        if (state.chapters.isNotEmpty()) {
            item {
                Spacer(modifier = Modifier.height(24.dp))
                Text(
                    text = "Chapters",
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface,
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 24.dp),
                )
                Spacer(modifier = Modifier.height(8.dp))
            }
            items(state.chapters) { chapter ->
                ChapterRow(
                    chapter = chapter,
                    isActive = isChapterActive(state, chapter),
                    onSeek = { onSeek(chapter.startSeconds) },
                )
            }
        }

        item { Spacer(modifier = Modifier.height(32.dp)) }
    }
}

@Composable
private fun Scrubber(state: PodcastPlaybackState, onSeek: (Double) -> Unit) {
    val duration = state.effectiveDurationSeconds
    // While the user is dragging we show the local value so the thumb doesn't
    // fight the 1Hz position poll; on release we commit the seek.
    var dragValue by remember { mutableStateOf<Float?>(null) }
    val sliderValue = dragValue ?: state.progressFraction

    Column(modifier = Modifier.padding(horizontal = 24.dp)) {
        Slider(
            value = sliderValue.coerceIn(0f, 1f),
            onValueChange = { dragValue = it },
            onValueChangeFinished = {
                dragValue?.let { fraction ->
                    if (duration > 0) onSeek(fraction * duration)
                }
                dragValue = null
            },
            enabled = duration > 0,
            modifier = Modifier
                .fillMaxWidth()
                .semantics { contentDescription = "Playback position" },
        )
        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            val shown = dragValue?.let { it * duration } ?: state.positionSeconds
            Text(
                text = formatPlaybackTime(shown),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                text = formatPlaybackTime(duration),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun TransportRow(
    state: PodcastPlaybackState,
    onToggle: () -> Unit,
    onSkip: (Double) -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = { onSkip(-SKIP_SECONDS) }) {
            Icon(
                imageVector = Icons.Filled.Replay,
                contentDescription = "Skip back 15 seconds",
                tint = MaterialTheme.colorScheme.onSurface,
            )
        }
        Spacer(modifier = Modifier.width(24.dp))
        Box(
            modifier = Modifier
                .size(64.dp)
                .background(MaterialTheme.colorScheme.primary, RoundedCornerShape(32.dp))
                .clickable(onClick = onToggle)
                .semantics { contentDescription = if (state.isPlaying) "Pause" else "Play" },
            contentAlignment = Alignment.Center,
        ) {
            if (state.isBuffering) {
                CircularProgressIndicator(
                    modifier = Modifier.size(28.dp),
                    strokeWidth = 3.dp,
                    color = MaterialTheme.colorScheme.onPrimary,
                )
            } else {
                Icon(
                    imageVector = if (state.isPlaying) Icons.Filled.Pause else Icons.Filled.PlayArrow,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onPrimary,
                    modifier = Modifier.size(32.dp),
                )
            }
        }
        Spacer(modifier = Modifier.width(24.dp))
        // Mirror the replay glyph for "skip forward 15".
        IconButton(onClick = { onSkip(SKIP_SECONDS) }) {
            Icon(
                imageVector = Icons.Filled.Replay,
                contentDescription = "Skip forward 15 seconds",
                tint = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.graphicsLayer(scaleX = -1f),
            )
        }
    }
}

@Composable
private fun SpeedSelector(current: Float, onSetSpeed: (Float) -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
    ) {
        PODCAST_SPEEDS.forEach { speed ->
            FilterChip(
                selected = current == speed,
                onClick = { onSetSpeed(speed) },
                label = { Text(formatSpeed(speed)) },
            )
        }
    }
}

@Composable
private fun ChapterRow(chapter: Chapter, isActive: Boolean, onSeek: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onSeek)
            .background(
                if (isActive) MaterialTheme.colorScheme.surfaceVariant else MaterialTheme.colorScheme.surface,
            )
            .padding(horizontal = 24.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = formatPlaybackTime(chapter.startSeconds),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.width(64.dp),
        )
        Text(
            text = chapter.title.ifBlank { "Chapter" },
            style = MaterialTheme.typography.bodyMedium,
            fontWeight = if (isActive) FontWeight.SemiBold else FontWeight.Normal,
            color = MaterialTheme.colorScheme.onSurface,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

private fun isChapterActive(state: PodcastPlaybackState, chapter: Chapter): Boolean {
    val sorted = state.chapters.sortedBy { it.startSeconds }
    val idx = sorted.indexOfFirst { it.startSeconds == chapter.startSeconds && it.title == chapter.title }
    if (idx < 0) return false
    val start = sorted[idx].startSeconds
    val end = sorted.getOrNull(idx + 1)?.startSeconds ?: Double.MAX_VALUE
    return state.positionSeconds >= start && state.positionSeconds < end
}

private fun formatSpeed(speed: Float): String {
    return if (speed % 1f == 0f) "${speed.toInt()}x" else "${speed}x"
}
