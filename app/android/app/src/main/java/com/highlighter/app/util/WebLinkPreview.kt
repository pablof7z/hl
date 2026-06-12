package com.highlighter.app.util

import android.util.Patterns
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.components.CoverShape
import com.highlighter.app.ui.components.RemoteImage
import uniffi.highlighter_core.HighlighterAppAction

/**
 * Renders a web-link preview card for the first http(s) URL found in [text],
 * mirroring iOS `HighlightFeedCardView`'s web-source treatment.
 *
 * On first composition (and whenever the resolved URL changes) it dispatches
 * [HighlighterAppAction.RequestWebMetadata] via [LocalDispatch], then reads the
 * resolved [uniffi.highlighter_core.WebMetadata] from [LocalWebMetadata]. Until
 * the Rust core fills the cache, nothing is rendered — the card simply appears
 * once metadata arrives, so this is non-intrusive on text with no links.
 */
@Composable
fun WebLinkPreview(text: String, modifier: Modifier = Modifier) {
    val url = remember(text) { firstHttpUrlIn(text) } ?: return
    val dispatch = LocalDispatch.current
    LaunchedEffect(url) {
        dispatch(HighlighterAppAction.RequestWebMetadata(url))
    }
    val metadata = LocalWebMetadata.current.webMetadataFor(url) ?: return

    val title = metadata.title.trim().ifEmpty { metadata.url.trim() }
    val site = metadata.siteName.trim().ifEmpty { metadata.author.trim() }
    val image = metadata.image.trim().ifEmpty { metadata.favicon.trim() }

    Surface(
        modifier = modifier
            .fillMaxWidth()
            .padding(top = 6.dp),
        shape = RoundedCornerShape(10.dp),
        color = MaterialTheme.colorScheme.surfaceVariant,
        tonalElevation = 1.dp,
    ) {
        Row(modifier = Modifier.padding(8.dp)) {
            if (image.isNotEmpty()) {
                RemoteImage(
                    url = image,
                    contentDescription = null,
                    modifier = Modifier.size(44.dp),
                    shape = CoverShape,
                )
                Spacer(modifier = Modifier.width(10.dp))
            }
            Column(modifier = Modifier.fillMaxWidth()) {
                Text(
                    text = title,
                    style = MaterialTheme.typography.bodyMedium,
                    fontWeight = FontWeight.Medium,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                if (site.isNotEmpty()) {
                    Spacer(modifier = Modifier.height(2.dp))
                    Text(
                        text = site,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
    }
}

/** First http(s) URL inside [text], or null. Mirrors iOS NSDataDetector. */
private fun firstHttpUrlIn(text: String): String? {
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
