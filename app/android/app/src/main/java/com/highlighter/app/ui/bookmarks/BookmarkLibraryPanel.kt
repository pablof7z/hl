package com.highlighter.app.ui.bookmarks

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.BookmarkAdd
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.components.CoverShape
import com.highlighter.app.ui.components.EmptyPanel
import com.highlighter.app.ui.components.RemoteImage
import com.highlighter.app.ui.components.SectionHeader
import com.highlighter.app.ui.search.SearchGroupHeader
import com.highlighter.app.ui.search.SearchResultRow
import uniffi.highlighter_core.ArticleRecord
import uniffi.highlighter_core.BookmarkSetRecord
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterBookmarksSnapshot
import uniffi.highlighter_core.WebBookmarkRecord

@Composable
internal fun BookmarkLibraryPanel(
    bookmarks: HighlighterBookmarksSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        SectionHeader("Bookmarks", bookmarks.articleCount.toString())
        bookmarks.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Text(
                text = message,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.tertiary,
            )
        }
        when {
            bookmarks.isLoading && bookmarks.articles.isEmpty() -> EmptyPanel("Loading bookmarks")
            bookmarks.articles.isEmpty() -> EmptyPanel("No bookmarked articles")
            else -> {
                SearchGroupHeader("Articles", bookmarks.articleCount.toString())
                bookmarks.articles.take(5).forEach { article ->
                    ArticleBookmarkRow(article, dispatch)
                }
            }
        }
        val myCollections = bookmarks.myBookmarkSets + bookmarks.myCurationSets
        if (myCollections.isNotEmpty()) {
            SearchGroupHeader(
                "Collections",
                (bookmarks.myBookmarkSetCount + bookmarks.myCurationSetCount).toString(),
            )
            myCollections.take(5).forEach { collection ->
                BookmarkCollectionRow(collection)
            }
        }
        if (bookmarks.webBookmarks.isNotEmpty()) {
            SearchGroupHeader("Web", bookmarks.webBookmarkCount.toString())
            bookmarks.webBookmarks.take(5).forEach { bookmark ->
                WebBookmarkRow(bookmark)
            }
        }
        if (bookmarks.followingCurationSets.isNotEmpty()) {
            SearchGroupHeader("Explore", bookmarks.followingCurationSetCount.toString())
            bookmarks.followingCurationSets.take(5).forEach { collection ->
                BookmarkCollectionRow(collection)
            }
        }
    }
}

@Composable
private fun ArticleBookmarkRow(
    article: ArticleRecord,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    // NIP-33 a-tag value for a long-form article (kind:30023).
    val articleAddress = "30023:${article.pubkey}:${article.identifier}"
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(modifier = Modifier.weight(1f)) {
            SearchResultRow(
                title = article.title.ifBlank { article.identifier },
                subtitle = article.summary.ifBlank { article.pubkey },
                onClick = {
                    dispatch(
                        HighlighterAppAction.OpenArticleReader(
                            article.pubkey,
                            article.identifier,
                            article,
                        ),
                    )
                },
                leading = article.image.takeIf { it.isNotBlank() }?.let { image ->
                    {
                        RemoteImage(
                            url = image,
                            contentDescription = null,
                            modifier = Modifier.size(40.dp),
                            shape = CoverShape,
                            targetSize = 40.dp,
                        )
                    }
                },
            )
        }
        IconButton(onClick = { dispatch(HighlighterAppAction.OpenCurationMenu(articleAddress)) }) {
            Icon(
                imageVector = Icons.Outlined.BookmarkAdd,
                contentDescription = "Add to collection",
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun BookmarkCollectionRow(record: BookmarkSetRecord) {
    val title = record.title.ifBlank { record.id.ifBlank { "Untitled" } }
    val itemCount = record.articleAddresses.size + record.noteIds.size
    SearchResultRow(
        title = title,
        subtitle = when {
            record.description.isNotBlank() -> record.description
            itemCount == 1 -> "1 item"
            else -> "$itemCount items"
        },
    )
}

@Composable
private fun WebBookmarkRow(bookmark: WebBookmarkRecord) {
    SearchResultRow(
        title = bookmark.title.ifBlank { bookmark.url },
        subtitle = bookmark.description.ifBlank { bookmark.url },
    )
}
