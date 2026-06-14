package com.highlighter.app.ui.reader

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.components.EmptyPanel
import com.highlighter.app.ui.components.Panel
import com.highlighter.app.ui.components.SectionHeader
import com.highlighter.app.ui.search.HighlightSearchRow
import com.highlighter.app.ui.search.SearchGroupHeader
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterArticleReaderSnapshot

@Composable
internal fun ArticleReaderPanel(
    snapshot: HighlighterArticleReaderSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    val article = snapshot.article
    var quote by remember(snapshot.address) { mutableStateOf("") }
    var note by remember(snapshot.address) { mutableStateOf("") }

    Panel {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            SectionHeader("Reader", snapshot.highlightCount.toString())
        }
        TextButton(onClick = { dispatch(HighlighterAppAction.CloseArticleReader) }) {
            Text("Close")
        }
        snapshot.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = message,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.tertiary,
            )
        }
        when {
            snapshot.isLoading && article == null -> {
                Spacer(modifier = Modifier.height(10.dp))
                EmptyPanel("Loading article")
            }
            article == null -> {
                Spacer(modifier = Modifier.height(10.dp))
                EmptyPanel("Article unavailable")
            }
            else -> {
                Spacer(modifier = Modifier.height(12.dp))
                Text(
                    text = article.title.ifBlank { article.identifier },
                    style = MaterialTheme.typography.headlineSmall,
                    color = MaterialTheme.colorScheme.onSurface,
                    fontWeight = FontWeight.SemiBold,
                )
                val authorName = snapshot.authorProfile?.displayName?.takeIf { it.isNotBlank() }
                    ?: snapshot.authorProfile?.name?.takeIf { it.isNotBlank() }
                    ?: article.pubkey.take(12)
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = authorName,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (article.summary.isNotBlank()) {
                    Spacer(modifier = Modifier.height(10.dp))
                    Text(
                        text = article.summary,
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                Spacer(modifier = Modifier.height(14.dp))
                SelectionContainer {
                    Text(
                        text = article.content,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                }
                if (snapshot.highlights.isNotEmpty()) {
                    Spacer(modifier = Modifier.height(14.dp))
                    SearchGroupHeader("Highlights", snapshot.highlightCount.toString())
                    snapshot.highlights.take(8).forEach { highlight ->
                        HighlightSearchRow(highlight)
                    }
                }
                Spacer(modifier = Modifier.height(14.dp))
                OutlinedTextField(
                    value = quote,
                    onValueChange = { quote = it },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text("Quote") },
                    minLines = 2,
                    maxLines = 5,
                    keyboardOptions = KeyboardOptions(
                        capitalization = KeyboardCapitalization.Sentences,
                        keyboardType = KeyboardType.Text,
                    ),
                )
                Spacer(modifier = Modifier.height(8.dp))
                OutlinedTextField(
                    value = note,
                    onValueChange = { note = it },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text("Note") },
                    minLines = 1,
                    maxLines = 4,
                    keyboardOptions = KeyboardOptions(
                        capitalization = KeyboardCapitalization.Sentences,
                        keyboardType = KeyboardType.Text,
                    ),
                )
                Spacer(modifier = Modifier.height(8.dp))
                Button(
                    onClick = {
                        val cleanQuote = quote.trim()
                        val cleanNote = note.trim()
                        if (cleanQuote.isNotEmpty()) {
                            dispatch(
                                HighlighterAppAction.PublishArticleHighlight(
                                    cleanQuote,
                                    "",
                                    cleanNote,
                                ),
                            )
                            quote = ""
                            note = ""
                        }
                    },
                    shape = RoundedCornerShape(8.dp),
                    enabled = quote.isNotBlank() && !snapshot.isPublishingHighlight,
                ) {
                    Text(if (snapshot.isPublishingHighlight) "Saving" else "Highlight")
                }
            }
        }
    }
}
