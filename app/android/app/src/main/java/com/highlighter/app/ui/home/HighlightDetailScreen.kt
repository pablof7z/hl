package com.highlighter.app.ui.home

import android.content.Intent
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Comment
import androidx.compose.material.icons.filled.Bookmark
import androidx.compose.material.icons.filled.BookmarkBorder
import androidx.compose.material.icons.filled.IosShare
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.highlighter.app.ui.components.AvatarImage
import com.highlighter.app.ui.components.RemoteImage
import com.highlighter.app.util.LocalIsbnPreviews
import com.highlighter.app.util.LocalProfiles
import com.highlighter.app.util.avatarUrl
import com.highlighter.app.util.displayNameOr
import com.highlighter.app.util.previewForIsbn
import com.highlighter.app.util.profileFor
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HydratedHighlight

/**
 * Full-screen detail view for a single highlight — the Android counterpart of
 * iOS `HighlightDetailView`. Hosted as a local (host-side) destination inside
 * [MainScaffold]'s `ScaffoldRoute`, so the [HydratedHighlight] is passed
 * directly (it is already present in the feed item — no round-trip needed).
 *
 * Layout (mirrors iOS):
 *   1. Resource header (tappable → article / web reader)
 *   2. Author byline (tappable → OpenProfile)
 *   3. Quote block (accent rail + pull-quote italic, or page image + quote)
 *   4. Optional note
 *   5. Action bar: Comment | Share | Bookmark
 */
@Composable
internal fun HighlightDetailScreen(
    item: HydratedHighlight,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    val highlight = item.highlight
    val profiles = LocalProfiles.current
    val isbnPreviews = LocalIsbnPreviews.current
    val context = LocalContext.current

    // ── Hydration dispatches ─────────────────────────────────────────────────
    LaunchedEffect(highlight.pubkey) {
        if (highlight.pubkey.isNotBlank()) {
            dispatch(HighlighterAppAction.RequestProfile(highlight.pubkey))
        }
    }

    // ── Derived resource metadata ────────────────────────────────────────────

    val isbn: String? = run {
        val ext = highlight.externalReference.trim()
        if (ext.startsWith("isbn:")) ext.removePrefix("isbn:")
        else {
            val addr = highlight.artifactAddress.trim()
            if (addr.startsWith("isbn:")) addr.removePrefix("isbn:") else null
        }
    }

    LaunchedEffect(isbn) {
        if (!isbn.isNullOrBlank() && isbnPreviews.previewForIsbn(isbn) == null) {
            dispatch(HighlighterAppAction.RequestIsbnPreview(isbn))
        }
    }

    val artifactPreview = item.artifact?.preview
    val isbnPreview = if (isbn != null) isbnPreviews.previewForIsbn(isbn) else null

    val resourceTitle: String = when {
        !artifactPreview?.title.isNullOrBlank() -> artifactPreview!!.title
        isbnPreview != null && isbnPreview.title.isNotBlank() -> isbnPreview.title
        sourceUrlHost(highlight.sourceUrl) != null -> sourceUrlHost(highlight.sourceUrl)!!
        else -> ""
    }

    val resourceAuthor: String = when {
        !artifactPreview?.author.isNullOrBlank() -> artifactPreview!!.author
        isbnPreview != null && isbnPreview.author.isNotBlank() -> isbnPreview.author
        !artifactPreview?.domain.isNullOrBlank() -> artifactPreview!!.domain
        else -> sourceUrlHost(highlight.sourceUrl) ?: ""
    }

    val resourceKindLabel: String = when {
        isbn != null -> "Book"
        highlight.artifactAddress.trim().startsWith("30023:") -> "Article"
        highlight.sourceUrl.trim().let { it.startsWith("http://") || it.startsWith("https://") } -> "Web"
        else -> "Source"
    }

    val coverUrl: String? = when {
        !artifactPreview?.image.isNullOrBlank() -> artifactPreview!!.image
        isbnPreview != null && isbnPreview.image.isNotBlank() -> isbnPreview.image
        highlight.imageUrl.isNotBlank() -> highlight.imageUrl
        else -> null
    }

    // Article reader target: 30023:<pubkey>:<dTag>
    val articleReaderTarget: Pair<String, String>? = run {
        val addr = highlight.artifactAddress.trim()
        if (!addr.startsWith("30023:")) return@run null
        val parts = addr.split(":", limit = 3)
        if (parts.size == 3 && parts[1].isNotBlank() && parts[2].isNotBlank())
            Pair(parts[1], parts[2])
        else null
    }

    // Bookmark address: only for 30023 NIP-23 articles (same rule as iOS)
    val articleAddressForBookmark: String? = run {
        val addr = highlight.artifactAddress.trim()
        if (addr.startsWith("30023:")) addr else null
    }

    // Page image: NIP-92 imeta scan photo
    val pageImageUrl: String? = highlight.imageUrl.trim().takeIf { it.isNotBlank() }

    // Author profile
    val authorProfile = profiles.profileFor(highlight.pubkey)
    val authorName = authorProfile.displayNameOr(highlight.pubkey)
    val authorAvatarUrl = authorProfile.avatarUrl()

    // Relative timestamp
    val relativeTime: String? = highlight.createdAt?.let { ts ->
        val delta = System.currentTimeMillis() / 1000 - ts.toLong()
        when {
            delta < 60 -> "just now"
            delta < 3600 -> "${delta / 60}m ago"
            delta < 86400 -> "${delta / 3600}h ago"
            delta < 86400 * 7 -> "${delta / 86400}d ago"
            delta < 86400 * 30 -> "${delta / (86400 * 7)}w ago"
            else -> "${delta / (86400 * 30)}mo ago"
        }
    }

    // Share URL (mirrors iOS highlightShareURL)
    val shareUrl = "https://beta.highlighter.com/highlight/${highlight.eventId}"

    // ── Layout ───────────────────────────────────────────────────────────────

    LazyColumn(
        modifier = Modifier
            .fillMaxSize()
            .testTag("highlight_detail"),
        contentPadding = PaddingValues(horizontal = 18.dp, vertical = 16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {

        // 1. Resource header
        if (resourceTitle.isNotBlank()) {
            item {
                ResourceHeader(
                    title = resourceTitle,
                    author = resourceAuthor,
                    kindLabel = resourceKindLabel,
                    coverUrl = coverUrl,
                    tappable = articleReaderTarget != null ||
                        highlight.sourceUrl.trim().let { it.startsWith("http://") || it.startsWith("https://") },
                    onClick = {
                        if (articleReaderTarget != null) {
                            dispatch(
                                HighlighterAppAction.OpenArticleReader(
                                    articleReaderTarget.first,
                                    articleReaderTarget.second,
                                    null,
                                ),
                            )
                        }
                        // Web reader: dispatches OpenExternalUrl via core reconciler
                        // (no dedicated OpenWebReader action in binding — omit for now)
                    },
                )
            }
        }

        // 2. Author byline
        item {
            AuthorByline(
                name = authorName,
                avatarUrl = authorAvatarUrl,
                relativeTime = relativeTime,
                onClick = {
                    if (highlight.pubkey.isNotBlank()) {
                        dispatch(HighlighterAppAction.OpenProfile(highlight.pubkey))
                    }
                },
            )
        }

        // 3. Quote block (page image + quote, or accent-rail + quote)
        item {
            QuoteBlock(
                quote = highlight.quote.trim(),
                pageImageUrl = pageImageUrl,
            )
        }

        // 4. Optional note
        if (highlight.note.trim().isNotBlank()) {
            item {
                NoteBlock(note = highlight.note.trim())
            }
        }

        // 5. Action bar
        item {
            ActionBar(
                articleAddress = articleAddressForBookmark,
                onComment = {
                    dispatch(
                        HighlighterAppAction.OpenComments(
                            rootTagName = "e",
                            rootTagValue = highlight.eventId,
                            rootKind = 9802u,
                        ),
                    )
                },
                onShare = {
                    // System share sheet with the highlight URL (iOS ShareLink equivalent)
                    val intent = Intent(Intent.ACTION_SEND).apply {
                        type = "text/plain"
                        putExtra(Intent.EXTRA_TEXT, shareUrl)
                        putExtra(Intent.EXTRA_SUBJECT, "Highlight")
                    }
                    context.startActivity(Intent.createChooser(intent, "Share highlight"))
                },
                onBookmark = { address ->
                    dispatch(HighlighterAppAction.ToggleArticleBookmark(address))
                },
                dispatch = dispatch,
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Resource header
// ---------------------------------------------------------------------------

@Composable
private fun ResourceHeader(
    title: String,
    author: String,
    kindLabel: String,
    coverUrl: String?,
    tappable: Boolean,
    onClick: () -> Unit,
) {
    val modifier = if (tappable) {
        Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
    } else {
        Modifier.fillMaxWidth()
    }

    Surface(
        modifier = modifier,
        color = MaterialTheme.colorScheme.surfaceVariant,
        shape = RoundedCornerShape(12.dp),
    ) {
        Row(
            modifier = Modifier.padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            if (coverUrl != null) {
                RemoteImage(
                    url = coverUrl,
                    contentDescription = null,
                    modifier = Modifier.size(40.dp),
                    shape = RoundedCornerShape(6.dp),
                )
            }
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = kindLabel.uppercase(),
                    style = MaterialTheme.typography.labelSmall,
                    fontWeight = FontWeight.Bold,
                    letterSpacing = 0.6.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(
                    text = title,
                    style = MaterialTheme.typography.bodyMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                if (author.isNotBlank()) {
                    Text(
                        text = author,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Author byline
// ---------------------------------------------------------------------------

@Composable
private fun AuthorByline(
    name: String,
    avatarUrl: String?,
    relativeTime: String?,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .testTag("highlight_detail_author"),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        AvatarImage(
            url = avatarUrl,
            name = name,
            size = 36.dp,
        )
        Column {
            Text(
                text = name,
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (!relativeTime.isNullOrBlank()) {
                Text(
                    text = relativeTime,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Quote block
// ---------------------------------------------------------------------------

@Composable
private fun QuoteBlock(
    quote: String,
    pageImageUrl: String?,
) {
    if (pageImageUrl != null) {
        Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
            RemoteImage(
                url = pageImageUrl,
                contentDescription = "Page scan",
                modifier = Modifier
                    .fillMaxWidth()
                    .height(220.dp),
                shape = RoundedCornerShape(8.dp),
            )
            QuoteText(quote = quote)
        }
    } else {
        Row(
            horizontalArrangement = Arrangement.spacedBy(14.dp),
            verticalAlignment = Alignment.Top,
        ) {
            Surface(
                modifier = Modifier
                    .width(3.dp)
                    .height(80.dp),
                color = MaterialTheme.colorScheme.primary,
                shape = RoundedCornerShape(1.5.dp),
            ) {}
            QuoteText(quote = quote)
        }
    }
}

@Composable
private fun QuoteText(quote: String) {
    Text(
        text = quote.ifBlank { "Untitled highlight" },
        style = MaterialTheme.typography.bodyLarge.copy(
            fontStyle = FontStyle.Italic,
            lineHeight = 28.sp,
        ),
        fontSize = 21.sp,
        color = MaterialTheme.colorScheme.onSurface,
        modifier = Modifier.fillMaxWidth(),
    )
}

// ---------------------------------------------------------------------------
// Note block
// ---------------------------------------------------------------------------

@Composable
private fun NoteBlock(note: String) {
    Column(
        modifier = Modifier.padding(start = 17.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text(
            text = "NOTE",
            style = MaterialTheme.typography.labelSmall,
            fontWeight = FontWeight.Bold,
            letterSpacing = 0.6.sp,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            text = note,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurface,
        )
    }
}

// ---------------------------------------------------------------------------
// Action bar
// ---------------------------------------------------------------------------

@Composable
private fun ActionBar(
    articleAddress: String?,
    onComment: () -> Unit,
    onShare: () -> Unit,
    onBookmark: (String) -> Unit,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    Column {
        HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = 4.dp),
            horizontalArrangement = Arrangement.spacedBy(4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            // Comment
            IconButton(
                onClick = onComment,
                modifier = Modifier.testTag("highlight_detail_comment"),
            ) {
                Icon(
                    imageVector = Icons.AutoMirrored.Filled.Comment,
                    contentDescription = "Comments",
                    tint = MaterialTheme.colorScheme.onSurface,
                )
            }

            // Share (system share sheet)
            IconButton(
                onClick = onShare,
                modifier = Modifier.testTag("highlight_detail_share"),
            ) {
                Icon(
                    imageVector = Icons.Filled.IosShare,
                    contentDescription = "Share highlight",
                    tint = MaterialTheme.colorScheme.onSurface,
                )
            }

            // Bookmark — only for 30023 article-backed highlights (mirrors iOS)
            if (articleAddress != null) {
                IconButton(
                    onClick = { onBookmark(articleAddress) },
                    modifier = Modifier.testTag("highlight_detail_bookmark"),
                ) {
                    Icon(
                        imageVector = Icons.Filled.BookmarkBorder,
                        contentDescription = "Bookmark article",
                        tint = MaterialTheme.colorScheme.onSurface,
                    )
                }
            }

            Spacer(modifier = Modifier.weight(1f))
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

private fun sourceUrlHost(url: String): String? {
    val trimmed = url.trim()
    if (trimmed.isEmpty()) return null
    return try {
        val parsed = java.net.URI(trimmed)
        parsed.host?.takeIf { it.isNotEmpty() }
    } catch (_: Exception) {
        null
    }
}
