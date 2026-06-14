package com.highlighter.app.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import coil3.compose.LocalPlatformContext
import coil3.request.ImageRequest

/** Default rounded-corner shape for cover / artwork imagery (8.dp). */
internal val CoverShape: Shape = RoundedCornerShape(8.dp)

/**
 * Remote image wrapper around Coil's [AsyncImage]. A subtle `surfaceVariant`
 * box sits behind the image, so it doubles as the placeholder while loading,
 * on error, and whenever the URL is null/blank — mirroring how iOS leans on
 * Kingfisher placeholders.
 *
 * When [targetSize] is supplied (in dp) Coil will down-sample the source image
 * to exactly that pixel dimensions, preventing a 2 000 px cover from being
 * decoded full-resolution into a 56 dp thumbnail slot.
 */
@Composable
internal fun RemoteImage(
    url: String?,
    contentDescription: String?,
    modifier: Modifier = Modifier,
    shape: Shape? = null,
    contentScale: ContentScale = ContentScale.Crop,
    targetSize: Dp? = null,
) {
    val shaped = if (shape != null) modifier.clip(shape) else modifier
    Box(
        modifier = shaped.background(MaterialTheme.colorScheme.surfaceVariant),
        contentAlignment = Alignment.Center,
    ) {
        val trimmed = url?.trim()
        if (!trimmed.isNullOrEmpty()) {
            // Convert dp → px for the explicit size hint so Coil can skip
            // decoding pixels that will never be displayed.
            val density = LocalDensity.current
            val pxSize = targetSize?.let { with(density) { it.roundToPx() } }

            val request = ImageRequest.Builder(LocalPlatformContext.current)
                .data(trimmed)
                .apply { if (pxSize != null) size(pxSize) }
                .build()

            AsyncImage(
                model = request,
                contentDescription = contentDescription,
                modifier = Modifier.fillMaxSize(),
                contentScale = contentScale,
            )
        }
    }
}

/**
 * Circular avatar that falls back to a monogram (first letter of [name]) on a
 * `surfaceVariant` background when [url] is missing or the image fails to load
 * — matching the iOS author/relay rows that show an initial when no picture is
 * available.
 *
 * The avatar slot size is always known at call-site (the [size] parameter), so
 * Coil is told to decode and cache exactly that many pixels — no full-resolution
 * decode for a tiny avatar chip.
 */
@Composable
internal fun AvatarImage(
    url: String?,
    name: String,
    size: Dp,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .size(size)
            .clip(CircleShape)
            .background(MaterialTheme.colorScheme.surfaceVariant),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = name.trim().firstOrNull()?.uppercase() ?: "?",
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            fontWeight = FontWeight.SemiBold,
            style = MaterialTheme.typography.titleMedium,
        )
        val trimmed = url?.trim()
        if (!trimmed.isNullOrEmpty()) {
            // Convert the known dp size to px once so Coil's decoder limits
            // the Bitmap to exactly the rendered dimensions.
            val density = LocalDensity.current
            val pxSize = with(density) { size.roundToPx() }

            AsyncImage(
                model = ImageRequest.Builder(LocalPlatformContext.current)
                    .data(trimmed)
                    .size(pxSize)
                    .build(),
                contentDescription = name.ifBlank { null },
                modifier = Modifier.fillMaxSize(),
                contentScale = ContentScale.Crop,
            )
        }
    }
}
