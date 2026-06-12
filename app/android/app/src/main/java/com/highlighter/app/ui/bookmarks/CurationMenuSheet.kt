package com.highlighter.app.ui.bookmarks

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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uniffi.highlighter_core.BookmarkSetRecord
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterCurationMenuSnapshot

/**
 * Collections (curation set) picker, driven entirely by [HighlighterCurationMenuSnapshot].
 * Open when [HighlighterCurationMenuSnapshot.articleAddress] is non-blank — the
 * Rust core loads the current user's kind:30004 sets and marks membership via
 * each set's `articleAddresses`.
 *
 * Rendered state-driven from `RootScene` (like the What's New dialog). Toggling a
 * row dispatches `SetAddressInCurationSet(dTag, address, member)`; the inline
 * "New collection…" field dispatches `CreateCurationSetAndAdd(title, address)`;
 * dismissing dispatches `CloseCurationMenu`. Mirrors the iOS `BookmarkMenuButton`.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun CurationMenuSheet(
    menu: HighlighterCurationMenuSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    if (menu.articleAddress.isBlank()) return

    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val close = { dispatch(HighlighterAppAction.CloseCurationMenu) }

    ModalBottomSheet(
        onDismissRequest = close,
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surface,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 20.dp)
                .padding(bottom = 28.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                text = "Add to collection",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurface,
            )

            when {
                menu.isLoading && menu.curationSets.isEmpty() -> {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        CircularProgressIndicator(
                            modifier = Modifier.size(18.dp),
                            strokeWidth = 2.dp,
                        )
                        Text(
                            text = "Loading collections",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }

                menu.curationSets.isEmpty() -> {
                    Text(
                        text = "No collections yet",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(vertical = 8.dp),
                    )
                }

                else -> {
                    menu.curationSets.forEach { set ->
                        CurationSetRow(
                            set = set,
                            isMember = set.articleAddresses.contains(menu.articleAddress),
                        ) { nowMember ->
                            dispatch(
                                HighlighterAppAction.SetAddressInCurationSet(
                                    set.id,
                                    menu.articleAddress,
                                    nowMember,
                                ),
                            )
                        }
                    }
                }
            }

            menu.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
                Text(
                    text = message,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                )
            }

            Spacer(modifier = Modifier.height(4.dp))

            NewCollectionField { title ->
                dispatch(HighlighterAppAction.CreateCurationSetAndAdd(title, menu.articleAddress))
            }
        }
    }
}

@Composable
private fun CurationSetRow(
    set: BookmarkSetRecord,
    isMember: Boolean,
    onToggle: (nowMember: Boolean) -> Unit,
) {
    val title = set.title.ifBlank { set.id.ifBlank { "Untitled" } }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onToggle(!isMember) }
            .padding(vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = title,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (set.description.isNotBlank()) {
                Text(
                    text = set.description,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        if (isMember) {
            Icon(
                imageVector = Icons.Filled.Check,
                contentDescription = "In $title",
                tint = MaterialTheme.colorScheme.primary,
            )
        }
    }
}

@Composable
private fun NewCollectionField(onCreate: (title: String) -> Unit) {
    var expanded by remember { mutableStateOf(false) }
    var title by remember { mutableStateOf("") }

    if (!expanded) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable { expanded = true }
                .padding(vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                imageVector = Icons.Filled.Add,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.primary,
            )
            Spacer(modifier = Modifier.width(10.dp))
            Text(
                text = "New collection…",
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.primary,
            )
        }
    } else {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(
                value = title,
                onValueChange = { title = it },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                label = { Text("Collection name") },
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TextButton(onClick = {
                    expanded = false
                    title = ""
                }) {
                    Text("Cancel")
                }
                TextButton(
                    onClick = {
                        val trimmed = title.trim()
                        if (trimmed.isNotEmpty()) {
                            onCreate(trimmed)
                            expanded = false
                            title = ""
                        }
                    },
                    enabled = title.isNotBlank(),
                    shape = RoundedCornerShape(8.dp),
                ) {
                    Text("Create & add")
                }
            }
        }
    }
}
