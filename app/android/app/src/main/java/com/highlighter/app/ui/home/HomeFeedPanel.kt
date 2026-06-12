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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.components.CoverShape
import com.highlighter.app.ui.components.EmptyPanel
import com.highlighter.app.ui.components.RemoteImage
import com.highlighter.app.ui.components.SectionHeader
import com.highlighter.app.util.feedCountLabel
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterHomeFeedItem
import uniffi.highlighter_core.HighlighterHomeFeedItemKind
import uniffi.highlighter_core.HighlighterHomeFeedSnapshot

@Composable
internal fun HomeFeedPanel(
    feed: HighlighterHomeFeedSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            SectionHeader("Highlights", feed.itemCount.toString())
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(
                onClick = { dispatch(HighlighterAppAction.RefreshHomeFeed) },
                shape = RoundedCornerShape(8.dp),
                border = BorderStroke(1.dp, MaterialTheme.colorScheme.outline),
                enabled = !feed.isLoading,
            ) {
                Text(if (feed.isLoading) "Refreshing" else "Refresh")
            }
        }
        feed.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Text(
                text = message,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.tertiary,
            )
        }
        when {
            feed.isLoading && feed.items.isEmpty() -> EmptyPanel("Loading highlights")
            feed.items.isEmpty() -> EmptyPanel("No highlights yet")
            else -> feed.items.take(8).forEach { item ->
                HomeFeedRow(item, dispatch)
            }
        }
    }
}

@Composable
private fun HomeFeedRow(
    item: HighlighterHomeFeedItem,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    when (item.kind) {
        HighlighterHomeFeedItemKind.HIGHLIGHTS -> {
            val leadHydrated = item.highlights.firstOrNull()
            val lead = leadHydrated?.highlight
            if (lead != null) {
                val artwork = lead.imageUrl.takeIf { it.isNotBlank() }
                    ?: leadHydrated.artifact?.preview?.image?.takeIf { it.isNotBlank() }
                Surface(
                    modifier = Modifier.fillMaxWidth(),
                    color = MaterialTheme.colorScheme.surface,
                    shape = RoundedCornerShape(8.dp),
                    border = BorderStroke(1.dp, MaterialTheme.colorScheme.outline),
                ) {
                    Row(modifier = Modifier.padding(12.dp)) {
                        if (artwork != null) {
                            RemoteImage(
                                url = artwork,
                                contentDescription = null,
                                modifier = Modifier.size(56.dp),
                                shape = CoverShape,
                            )
                            Spacer(modifier = Modifier.width(12.dp))
                        }
                        Column {
                            Text(
                                text = lead.quote.ifBlank { "Untitled highlight" },
                                style = MaterialTheme.typography.bodyLarge,
                                color = MaterialTheme.colorScheme.onSurface,
                                fontWeight = FontWeight.Medium,
                                maxLines = 3,
                                overflow = TextOverflow.Ellipsis,
                            )
                            Spacer(modifier = Modifier.height(6.dp))
                            Text(
                                text = item.highlightCount.feedCountLabel("highlight"),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }
            }
        }
        HighlighterHomeFeedItemKind.READ -> {
            val read = item.read ?: return
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
                    },
                color = MaterialTheme.colorScheme.surface,
                shape = RoundedCornerShape(8.dp),
                border = BorderStroke(1.dp, MaterialTheme.colorScheme.outline),
            ) {
                Row(modifier = Modifier.padding(12.dp)) {
                    if (read.image.isNotBlank()) {
                        RemoteImage(
                            url = read.image,
                            contentDescription = null,
                            modifier = Modifier.size(56.dp),
                            shape = CoverShape,
                        )
                        Spacer(modifier = Modifier.width(12.dp))
                    }
                    Column {
                        Text(
                            text = read.title.ifBlank { read.identifier },
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.onSurface,
                            fontWeight = FontWeight.Medium,
                            maxLines = 2,
                            overflow = TextOverflow.Ellipsis,
                        )
                        if (read.summary.isNotBlank()) {
                            Spacer(modifier = Modifier.height(5.dp))
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
            }
        }
    }
}
