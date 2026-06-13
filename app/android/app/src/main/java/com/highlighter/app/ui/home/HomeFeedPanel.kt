package com.highlighter.app.ui.home

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.components.AvatarImage
import com.highlighter.app.ui.components.CoverShape
import com.highlighter.app.ui.components.EmptyPanel
import com.highlighter.app.ui.components.RemoteImage
import com.highlighter.app.ui.components.SectionHeader
import com.highlighter.app.util.LocalIsbnPreviews
import com.highlighter.app.util.LocalProfiles
import com.highlighter.app.util.LocalWebMetadata
import com.highlighter.app.util.avatarUrl
import com.highlighter.app.util.displayNameOr
import com.highlighter.app.util.feedCountLabel
import com.highlighter.app.util.previewForIsbn
import com.highlighter.app.util.profileFor
import com.highlighter.app.util.webMetadataFor
import uniffi.highlighter_core.ArtifactPreview
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterHomeFeedItem
import uniffi.highlighter_core.HighlighterHomeFeedItemKind
import uniffi.highlighter_core.HighlighterHomeFeedSnapshot
import uniffi.highlighter_core.HighlighterHomeReadItem

/**
 * Populates a [LazyListScope] with the home feed content so that all items are
 * individually virtualized.  The caller owns the [LazyColumn] and passes its
 * scope here; this avoids the old pattern of a single `item { Column { forEach
 * } }` that composed every card at once (140+ cards → multi-hundred-ms frames).
 *
 * Layout emitted into the scope:
 *  1. Header row (single item, key="feed_header")
 *  2. Optional error message (single item, key="feed_error")
 *  3a. Loading spinner when isLoading && items.isEmpty (key="feed_loading")
 *  3b. Empty panel when !isLoading && items.isEmpty (key="feed_empty")
 *  3c. Lazy items(feed.items, key = { it.stableId }) — one item per card
 */
internal fun LazyListScope.homeFeedItems(
    feed: HighlighterHomeFeedSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
    onOpenHighlightDetail: ((uniffi.highlighter_core.HydratedHighlight) -> Unit)? = null,
) {
    // Debug-only: skip the joinToString allocation in release builds.
    if (android.util.Log.isLoggable("highlighter-feed", android.util.Log.INFO)) {
        android.util.Log.i(
            "highlighter-feed",
            "render items=${feed.items.size} count=${feed.itemCount} " +
                "loading=${feed.isLoading} err=${feed.errorMessage ?: "-"}",
        )
    }

    // ── 1. Header ──────────────────────────────────────────────────────────
    item(key = "feed_header") {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            SectionHeader("Highlights", feed.itemCount.toString())
        }
    }

    // ── 2. Optional error message ───────────────────────────────────────────
    feed.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
        item(key = "feed_error") {
            Text(
                text = message,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.tertiary,
            )
        }
    }

    // ── 3. Loading / empty / item list ─────────────────────────────────────
    when {
        // Distinct LOADING state: feed is syncing and no items have arrived yet.
        feed.isLoading && feed.items.isEmpty() -> {
            item(key = "feed_loading") {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .testTag("feed_loading"),
                    horizontalArrangement = Arrangement.Center,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    CircularProgressIndicator(
                        modifier = Modifier
                            .size(24.dp)
                            .padding(end = 8.dp),
                        strokeWidth = 2.dp,
                    )
                    Text(
                        text = "Syncing highlights…",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
        // EMPTY state: sync finished but the account has no feed items.
        feed.items.isEmpty() -> {
            item(key = "feed_empty") {
                EmptyPanel("No highlights yet")
            }
        }
        // Items available — render each as a separate lazy item so only visible
        // cards are composed.  The testTag on the LazyColumn itself is set by
        // the caller (HighlightsTab) via Modifier.testTag("feed_item_list").
        else -> {
            items(
                items = feed.items,
                key = { item -> item.stableId },
            ) { item ->
                HomeFeedRow(item, dispatch, onOpenHighlightDetail)
                Spacer(modifier = Modifier.height(10.dp))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Row dispatcher
// ---------------------------------------------------------------------------

@Composable
private fun HomeFeedRow(
    item: HighlighterHomeFeedItem,
    dispatch: (HighlighterAppAction) -> Unit,
    onOpenHighlightDetail: ((uniffi.highlighter_core.HydratedHighlight) -> Unit)? = null,
) {
    when (item.kind) {
        HighlighterHomeFeedItemKind.HIGHLIGHTS -> {
            val leadHydrated = item.highlights.firstOrNull()
            val lead = leadHydrated?.highlight
            if (lead != null) {
                HighlightFeedCard(
                    item = item,
                    dispatch = dispatch,
                    onOpenDetail = onOpenHighlightDetail,
                )
            }
        }
        HighlighterHomeFeedItemKind.READ -> {
            val read = item.read ?: return
            ReadingFeedCard(read = read, dispatch = dispatch)
        }
    }
}

// ---------------------------------------------------------------------------
// Highlight card (HIGHLIGHTS kind)
// ---------------------------------------------------------------------------

/**
 * Mirrors iOS `HighlightFeedCardView`:
 *  - Fires [HighlighterAppAction.RequestProfile] for the highlighter pubkey
 *    once per unique pubkey (LaunchedEffect keyed on pubkey).
 *  - Fires [HighlighterAppAction.RequestIsbnPreview] when the highlight
 *    references an ISBN artifact (externalReference or artifactAddress starts
 *    with "isbn:").
 *  - Fires [HighlighterAppAction.RequestWebMetadata] when the source is a
 *    plain web URL (sourceUrl non-blank and not isbn/article).
 *  - Reads resolved data back from LocalProfiles / LocalIsbnPreviews /
 *    LocalWebMetadata to render author avatar+name, cover image, and source
 *    title — falling back to raw quote/data when hydration hasn't landed yet.
 *  - Never gates the card Surface on hydration (iOS contract: render
 *    immediately, upgrade when data arrives).
 *
 * Because this composable is now inside a LazyColumn item, the LaunchedEffects
 * only run while the card is visible — naturally throttling the dispatch burst
 * that previously fired for all 140+ items simultaneously.
 */
@Composable
private fun HighlightFeedCard(
    item: HighlighterHomeFeedItem,
    dispatch: (HighlighterAppAction) -> Unit,
    onOpenDetail: ((uniffi.highlighter_core.HydratedHighlight) -> Unit)? = null,
) {
    val leadHydrated = item.highlights.firstOrNull() ?: return
    val lead = leadHydrated.highlight

    val profiles = LocalProfiles.current
    val isbnPreviews = LocalIsbnPreviews.current
    val webMetadataList = LocalWebMetadata.current

    // ── Hydration dispatches (once per stable key) ──────────────────────────

    // 1. Profile for the highlighter author
    LaunchedEffect(lead.pubkey) {
        if (lead.pubkey.isNotBlank() && profiles.profileFor(lead.pubkey) == null) {
            dispatch(HighlighterAppAction.RequestProfile(lead.pubkey))
        }
    }

    // 2. ISBN preview when the artifact is a book
    val isbn = run {
        val extRef = lead.externalReference.trim()
        if (extRef.startsWith("isbn:")) extRef.removePrefix("isbn:")
        else {
            val addr = lead.artifactAddress.trim()
            if (addr.startsWith("isbn:")) addr.removePrefix("isbn:") else null
        }
    }
    LaunchedEffect(isbn) {
        if (!isbn.isNullOrBlank() && isbnPreviews.previewForIsbn(isbn) == null) {
            dispatch(HighlighterAppAction.RequestIsbnPreview(isbn))
        }
    }

    // 3. Article author profile (pubkey extracted from "30023:<pubkey>:<dTag>")
    val articlePubkeyDTag = run {
        val addr = lead.artifactAddress.trim()
        if (addr.startsWith("30023:")) {
            val parts = addr.split(":", limit = 3)
            if (parts.size == 3 && parts[1].isNotBlank() && parts[2].isNotBlank())
                Pair(parts[1], parts[2])
            else null
        } else null
    }
    LaunchedEffect(articlePubkeyDTag?.first) {
        val pubkey = articlePubkeyDTag?.first
        if (!pubkey.isNullOrBlank() && profiles.profileFor(pubkey) == null) {
            dispatch(HighlighterAppAction.RequestProfile(pubkey))
        }
    }

    // 4. Web metadata when the source is a web URL (not article/isbn)
    val webUrl = run {
        if (isbn != null) null // book path handles its own cover
        else if (articlePubkeyDTag != null) null // article path
        else {
            val artifactUrl = leadHydrated.artifact?.preview?.url?.trim()
            if (!artifactUrl.isNullOrBlank()) artifactUrl
            else {
                val src = lead.sourceUrl.trim()
                if (src.isNotBlank()) src else null
            }
        }
    }
    LaunchedEffect(webUrl) {
        if (!webUrl.isNullOrBlank() && webMetadataList.webMetadataFor(webUrl) == null) {
            dispatch(HighlighterAppAction.RequestWebMetadata(webUrl))
        }
    }

    // ── Resolved enrichment ──────────────────────────────────────────────────

    val authorProfile = profiles.profileFor(lead.pubkey)
    val authorName = authorProfile.displayNameOr(lead.pubkey)
    val authorAvatarUrl = authorProfile.avatarUrl()

    // Cover image: artifact preview > isbn preview > web metadata image
    val artifactPreview: ArtifactPreview? = leadHydrated.artifact?.preview
    val isbnArtifactPreview = if (isbn != null) isbnPreviews.previewForIsbn(isbn) else null
    val webMeta = if (webUrl != null) webMetadataList.webMetadataFor(webUrl) else null

    val coverUrl: String? = when {
        !artifactPreview?.image.isNullOrBlank() -> artifactPreview!!.image
        isbnArtifactPreview != null && isbnArtifactPreview.image.isNotBlank() -> isbnArtifactPreview.image
        webMeta != null && webMeta.image.isNotBlank() -> webMeta.image
        webMeta != null && webMeta.favicon.isNotBlank() -> webMeta.favicon
        lead.imageUrl.isNotBlank() -> lead.imageUrl
        else -> null
    }

    // Source title: artifact > isbn > web > fallback
    val sourceTitle: String = when {
        !artifactPreview?.title.isNullOrBlank() -> artifactPreview!!.title
        isbnArtifactPreview != null && isbnArtifactPreview.title.isNotBlank() -> isbnArtifactPreview.title
        webMeta != null && webMeta.title.isNotBlank() -> webMeta.title
        else -> ""
    }

    // Source author/domain for subtitle row
    val sourceAuthor: String = when {
        !artifactPreview?.author.isNullOrBlank() -> artifactPreview!!.author
        isbnArtifactPreview != null && isbnArtifactPreview.author.isNotBlank() -> isbnArtifactPreview.author
        webMeta != null && webMeta.siteName.isNotBlank() -> webMeta.siteName
        webMeta != null && webMeta.author.isNotBlank() -> webMeta.author
        else -> ""
    }

    // ── Render ───────────────────────────────────────────────────────────────

    Surface(
        modifier = Modifier
            .fillMaxWidth()
            .clickable {
                if (onOpenDetail != null) {
                    onOpenDetail(leadHydrated)
                }
            }
            .testTag("feed_highlight_card"),
        color = MaterialTheme.colorScheme.surface,
        shape = RoundedCornerShape(8.dp),
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outline),
    ) {
        Column(modifier = Modifier.padding(12.dp)) {
            // Resource header row: cover + title/author
            if (coverUrl != null || sourceTitle.isNotBlank()) {
                Row(
                    verticalAlignment = Alignment.Top,
                    modifier = Modifier
                        .fillMaxWidth()
                        .testTag("card_cover"),
                ) {
                    if (coverUrl != null) {
                        RemoteImage(
                            url = coverUrl,
                            contentDescription = null,
                            modifier = Modifier.size(44.dp),
                            shape = RoundedCornerShape(6.dp),
                        )
                        Spacer(modifier = Modifier.width(10.dp))
                    }
                    Column(modifier = Modifier.weight(1f)) {
                        if (sourceTitle.isNotBlank()) {
                            Text(
                                text = sourceTitle,
                                style = MaterialTheme.typography.bodyMedium,
                                fontWeight = FontWeight.SemiBold,
                                color = MaterialTheme.colorScheme.onSurface,
                                maxLines = 2,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                        if (sourceAuthor.isNotBlank()) {
                            Text(
                                text = sourceAuthor.uppercase(),
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    }
                }
                Spacer(modifier = Modifier.height(10.dp))
            }

            // Author byline row: avatar + resolved display name
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.testTag("card_author"),
            ) {
                AvatarImage(
                    url = authorAvatarUrl,
                    name = authorName,
                    size = 22.dp,
                )
                Spacer(modifier = Modifier.width(8.dp))
                Text(
                    text = authorName,
                    style = MaterialTheme.typography.labelMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Spacer(modifier = Modifier.height(8.dp))

            // Pull-quote (accent rail + italic text, matching iOS text treatment)
            Row(
                verticalAlignment = Alignment.Top,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Surface(
                    modifier = Modifier
                        .width(3.dp)
                        .height(60.dp),
                    color = MaterialTheme.colorScheme.primary,
                    shape = RoundedCornerShape(1.5.dp),
                ) {}
                Text(
                    text = lead.quote.trim().ifBlank { "Untitled highlight" },
                    style = MaterialTheme.typography.bodyLarge.copy(fontStyle = FontStyle.Italic),
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 4,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Spacer(modifier = Modifier.height(6.dp))
            Text(
                text = item.highlightCount.feedCountLabel("highlight"),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Reading card (READ kind)
// ---------------------------------------------------------------------------

/**
 * Mirrors iOS `ReadingFeedCardView`:
 *  - Fires [HighlighterAppAction.RequestProfile] for the article author pubkey
 *    and for the primary interactor pubkey (LaunchedEffect keyed on each).
 *  - Reads resolved profiles back from LocalProfiles to render author avatar
 *    and display name, and shows a social badge ("From someone you follow" /
 *    "{name} liked this") matching the iOS reading card social signal.
 *  - Cover image and title rendered immediately; author/social upgraded when
 *    profiles land — never gates the Surface on hydration.
 */
@Composable
private fun ReadingFeedCard(
    read: HighlighterHomeReadItem,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    val profiles = LocalProfiles.current

    // ── Hydration dispatches (once per stable key) ──────────────────────────

    LaunchedEffect(read.pubkey) {
        if (read.pubkey.isNotBlank() && profiles.profileFor(read.pubkey) == null) {
            dispatch(HighlighterAppAction.RequestProfile(read.pubkey))
        }
    }

    val primaryInteractor = read.interactorPubkeys.firstOrNull()
    LaunchedEffect(primaryInteractor ?: "") {
        if (!primaryInteractor.isNullOrBlank() && profiles.profileFor(primaryInteractor) == null) {
            dispatch(HighlighterAppAction.RequestProfile(primaryInteractor))
        }
    }

    // ── Resolved enrichment ──────────────────────────────────────────────────

    val authorProfile = profiles.profileFor(read.pubkey)
    val authorName = authorProfile.displayNameOr(read.pubkey)
    val authorAvatarUrl = authorProfile.avatarUrl()

    val interactorProfile = if (!primaryInteractor.isNullOrBlank())
        profiles.profileFor(primaryInteractor) else null
    val interactorName = interactorProfile.displayNameOr(primaryInteractor ?: "")

    val socialText: String = when {
        read.authorFollowed && read.interactorPubkeys.isEmpty() -> "From someone you follow"
        read.interactorPubkeys.size == 1 ->
            if (read.authorFollowed) "$interactorName and the author liked this"
            else "$interactorName liked this"
        read.interactorPubkeys.size == 2 -> "$interactorName and 1 other"
        read.interactorPubkeys.size > 2 -> "$interactorName and ${read.interactorPubkeys.size - 1} others"
        else -> ""
    }

    // ── Render ───────────────────────────────────────────────────────────────

    Surface(
        modifier = Modifier
            .fillMaxWidth()
            .clickable {
                dispatch(
                    HighlighterAppAction.OpenArticleReader(
                        read.pubkey,
                        read.identifier,
                        null,
                    ),
                )
            }
            .testTag("feed_reading_card"),
        color = MaterialTheme.colorScheme.surface,
        shape = RoundedCornerShape(8.dp),
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outline),
    ) {
        Column(modifier = Modifier.padding(12.dp)) {
            Row(verticalAlignment = Alignment.Top) {
                // Cover image
                if (read.image.isNotBlank()) {
                    RemoteImage(
                        url = read.image,
                        contentDescription = null,
                        modifier = Modifier
                            .size(56.dp)
                            .testTag("card_cover"),
                        shape = CoverShape,
                    )
                    Spacer(modifier = Modifier.width(12.dp))
                }
                Column(modifier = Modifier.weight(1f)) {
                    // Article title
                    Text(
                        text = read.title.ifBlank { read.identifier },
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurface,
                        fontWeight = FontWeight.Medium,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                    if (read.summary.isNotBlank()) {
                        Spacer(modifier = Modifier.height(4.dp))
                        Text(
                            text = read.summary,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 2,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }

            Spacer(modifier = Modifier.height(8.dp))

            // Author byline row
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.testTag("card_author"),
            ) {
                AvatarImage(
                    url = authorAvatarUrl,
                    name = authorName,
                    size = 22.dp,
                )
                Spacer(modifier = Modifier.width(8.dp))
                Text(
                    text = authorName,
                    style = MaterialTheme.typography.labelMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }

            // Social signal badge
            if (socialText.isNotBlank()) {
                Spacer(modifier = Modifier.height(6.dp))
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    // Interactor avatar stack (up to 3)
                    read.interactorPubkeys.take(3).forEach { pk ->
                        val pf = profiles.profileFor(pk)
                        AvatarImage(
                            url = pf.avatarUrl(),
                            name = pf.displayNameOr(pk),
                            size = 18.dp,
                        )
                    }
                    Text(
                        text = socialText,
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
