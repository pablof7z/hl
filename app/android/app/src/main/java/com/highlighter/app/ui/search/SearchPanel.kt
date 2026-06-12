package com.highlighter.app.ui.search

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.components.AvatarImage
import com.highlighter.app.ui.components.CoverShape
import com.highlighter.app.ui.components.Panel
import com.highlighter.app.ui.components.RemoteImage
import com.highlighter.app.ui.components.SectionHeader
import uniffi.highlighter_core.ArticleRecord
import uniffi.highlighter_core.CommunitySummary
import uniffi.highlighter_core.HighlightRecord
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterSearchSnapshot
import uniffi.highlighter_core.ProfileMetadata

@Composable
internal fun SearchPanel(
    search: HighlighterSearchSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    val hasResults = search.highlights.isNotEmpty() ||
        search.articles.isNotEmpty() ||
        search.communities.isNotEmpty() ||
        search.profiles.isNotEmpty()
    Panel {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = "Search",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
            )
            if (search.query.isNotBlank()) {
                TextButton(onClick = { dispatch(HighlighterAppAction.ClearSearch) }) {
                    Text("Clear")
                }
            }
        }
        Spacer(modifier = Modifier.height(10.dp))
        OutlinedTextField(
            value = search.query,
            onValueChange = { dispatch(HighlighterAppAction.SetSearchQuery(it)) },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text("Quotes, essays, people, rooms") },
            keyboardOptions = KeyboardOptions(
                capitalization = KeyboardCapitalization.None,
                keyboardType = KeyboardType.Text,
                imeAction = ImeAction.Search,
            ),
            keyboardActions = KeyboardActions(
                onSearch = { dispatch(HighlighterAppAction.SubmitSearch(search.query)) },
            ),
        )
        Spacer(modifier = Modifier.height(12.dp))
        when {
            search.query.isBlank() -> SearchHint(
                search = search,
                dispatch = dispatch,
            )
            search.isLocalLoading && !hasResults -> Text(
                text = "Searching...",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            !hasResults && !search.isRelayLoading -> Text(
                text = "No results yet",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            else -> SearchResults(search = search, dispatch = dispatch)
        }
    }
}

@Composable
private fun SearchHint(
    search: HighlighterSearchSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    if (search.recentQueries.isNotEmpty()) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = "Recent",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = search.recentQueryCount.toString(),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            TextButton(onClick = { dispatch(HighlighterAppAction.ClearRecentSearches) }) {
                Text("Clear")
            }
        }
        search.recentQueries.forEach { query ->
            TextButton(onClick = { dispatch(HighlighterAppAction.SubmitSearch(query)) }) {
                Text(
                    text = query,
                    modifier = Modifier.fillMaxWidth(),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        Spacer(modifier = Modifier.height(10.dp))
    }
    Text(
        text = if (search.searchRelays.isEmpty()) {
            "Search your local Highlighter library."
        } else {
            "Search your local library and configured NIP-50 relays."
        },
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
private fun SearchResults(
    search: HighlighterSearchSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    if (search.highlights.isNotEmpty()) {
        SearchGroupHeader("Highlights", search.highlightCount.toString())
        search.highlights.take(3).forEach { highlight ->
            HighlightSearchRow(highlight)
        }
    }
    if (search.articles.isNotEmpty()) {
        SearchGroupHeader("Articles", search.articleCount.toString())
        search.articles.take(3).forEach { article ->
            ArticleSearchRow(article, dispatch)
        }
    }
    if (search.communities.isNotEmpty()) {
        SearchGroupHeader("Communities", search.communityCount.toString())
        search.communities.take(3).forEach { community ->
            CommunitySearchRow(community)
        }
    }
    if (search.profiles.isNotEmpty()) {
        SearchGroupHeader("People", search.profileCount.toString())
        search.profiles.take(3).forEach { profile ->
            ProfileSearchRow(profile)
        }
    }
    if (search.isRelayLoading) {
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "Checking relays...",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
internal fun SearchGroupHeader(title: String, count: String) {
    Spacer(modifier = Modifier.height(12.dp))
    SectionHeader(title, count)
}

@Composable
internal fun HighlightSearchRow(highlight: HighlightRecord) {
    SearchResultRow(
        title = highlight.quote,
        subtitle = highlight.note.ifBlank { highlight.sourceUrl },
    )
}

@Composable
internal fun ArticleSearchRow(
    article: ArticleRecord,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    SearchResultRow(
        title = article.title.ifBlank { article.identifier },
        subtitle = article.summary,
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
                )
            }
        },
    )
}

@Composable
internal fun CommunitySearchRow(community: CommunitySummary) {
    SearchResultRow(
        title = community.name.ifBlank { community.id },
        subtitle = community.about,
        leading = {
            AvatarImage(
                url = community.picture,
                name = community.name.ifBlank { "#" },
                size = 40.dp,
            )
        },
    )
}

@Composable
private fun ProfileSearchRow(profile: ProfileMetadata) {
    SearchResultRow(
        title = profile.displayName.ifBlank { profile.name.ifBlank { profile.pubkey } },
        subtitle = profile.about.ifBlank { profile.nip05 },
        leading = {
            AvatarImage(
                url = profile.picture,
                name = profile.displayName.ifBlank { profile.name.ifBlank { profile.pubkey } },
                size = 40.dp,
            )
        },
    )
}

@Composable
internal fun SearchResultRow(
    title: String,
    subtitle: String,
    onClick: (() -> Unit)? = null,
    leading: (@Composable () -> Unit)? = null,
) {
    val modifier = if (onClick == null) {
        Modifier
            .fillMaxWidth()
            .padding(vertical = 7.dp)
    } else {
        Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(vertical = 7.dp)
    }
    Row(modifier = modifier, verticalAlignment = Alignment.CenterVertically) {
        if (leading != null) {
            leading()
            Spacer(modifier = Modifier.size(12.dp))
        }
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = title.ifBlank { "Untitled" },
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
                fontWeight = FontWeight.Medium,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            if (subtitle.isNotBlank()) {
                Spacer(modifier = Modifier.height(3.dp))
                Text(
                    text = subtitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}
