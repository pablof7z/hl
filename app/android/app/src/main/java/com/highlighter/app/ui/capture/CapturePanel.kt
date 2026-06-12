package com.highlighter.app.ui.capture

import android.content.Context
import android.graphics.BitmapFactory
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.components.Chip
import com.highlighter.app.ui.components.CoverShape
import com.highlighter.app.ui.components.Panel
import com.highlighter.app.ui.components.RemoteImage
import com.highlighter.app.ui.components.SectionHeader
import com.highlighter.app.ui.components.ToggleButton
import com.highlighter.app.ui.search.SearchGroupHeader
import uniffi.highlighter_core.ArtifactRecord
import uniffi.highlighter_core.CommunitySummary
import uniffi.highlighter_core.HighlightDraft
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterBookPickerSnapshot
import uniffi.highlighter_core.HighlighterCaptureArtifact
import uniffi.highlighter_core.HighlighterCaptureSnapshot

@Composable
internal fun CapturePanel(
    capture: HighlighterCaptureSnapshot,
    bookPicker: HighlighterBookPickerSnapshot,
    communities: List<CommunitySummary>,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    val context = LocalContext.current
    var query by remember { mutableStateOf("") }
    var quote by remember { mutableStateOf("") }
    var note by remember { mutableStateOf("") }
    var selectedArtifact by remember { mutableStateOf<ArtifactRecord?>(null) }
    var selectedGroupId by remember { mutableStateOf<String?>(null) }
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        if (uri != null) {
            readPickedImage(context, uri)?.let { image ->
                dispatch(
                    HighlighterAppAction.UploadCapturePhoto(
                        image.bytes,
                        image.mime,
                        image.width,
                        image.height,
                        note.trim(),
                    ),
                )
            }
        }
    }
    Panel {
        SectionHeader("Capture", if (capture.isPublishing) "Publishing" else "Highlight")
        Spacer(modifier = Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(
                onClick = {
                    picker.launch(
                        PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly),
                    )
                },
                shape = RoundedCornerShape(8.dp),
                enabled = !capture.isUploading,
            ) {
                Text(if (capture.isUploading) "Uploading" else "Photo")
            }
            capture.upload?.let { upload ->
                Chip("${upload.width}x${upload.height}")
            }
        }
        Spacer(modifier = Modifier.height(8.dp))
        OutlinedTextField(
            value = query,
            onValueChange = {
                query = it
                if (it.trim().length >= 2) {
                    dispatch(HighlighterAppAction.SearchBookPickerArtifacts(it.trim(), 20u))
                }
            },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text("Find artifact") },
        )
        if (query.isNotBlank()) {
            TextButton(onClick = {
                query = ""
                dispatch(HighlighterAppAction.ClearBookPickerSearch)
            }) {
                Text("Clear search")
            }
        }
        val artifactRows = if (bookPicker.searchQuery.isNotBlank()) {
            bookPicker.searchResults
        } else {
            bookPicker.recentBooks
        }
        artifactRows.take(5).forEach { record ->
            ArtifactPickerRow(
                record = record,
                selected = selectedArtifact?.shareEventId == record.shareEventId,
                onSelect = { selectedArtifact = record },
            )
        }
        if (communities.isNotEmpty()) {
            SearchGroupHeader("Community", communities.size.toString())
            LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                items(communities, key = { it.id }) { community ->
                    ToggleButton(
                        label = community.name.ifBlank { community.id }.take(18),
                        selected = selectedGroupId == community.id,
                    ) {
                        selectedGroupId = if (selectedGroupId == community.id) null else community.id
                    }
                }
            }
        }
        Spacer(modifier = Modifier.height(8.dp))
        OutlinedTextField(
            value = quote,
            onValueChange = { quote = it },
            modifier = Modifier.fillMaxWidth(),
            minLines = 2,
            maxLines = 5,
            label = { Text("Quote") },
        )
        Spacer(modifier = Modifier.height(8.dp))
        OutlinedTextField(
            value = note,
            onValueChange = { note = it },
            modifier = Modifier.fillMaxWidth(),
            minLines = 1,
            maxLines = 4,
            label = { Text("Note / alt text") },
        )
        capture.uploadErrorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.tertiary)
        }
        capture.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.tertiary)
            TextButton(onClick = { dispatch(HighlighterAppAction.ClearCaptureError) }) {
                Text("Dismiss")
            }
        }
        capture.publishedEventId?.takeIf { it.isNotBlank() }?.let { id ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = "Published ${id.take(12)}", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            TextButton(onClick = { dispatch(HighlighterAppAction.ClearCaptureResult) }) {
                Text("Clear")
            }
        }
        Spacer(modifier = Modifier.height(10.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = {
                    val artifact = selectedArtifact ?: return@Button
                    dispatch(
                        HighlighterAppAction.PublishCaptureHighlight(
                            HighlighterCaptureArtifact.Existing(artifact),
                            selectedGroupId,
                            HighlightDraft(
                                quote.trim(),
                                "",
                                note.trim(),
                                null,
                                null,
                                "",
                                emptyList(),
                                capture.upload,
                            ),
                        ),
                    )
                },
                shape = RoundedCornerShape(8.dp),
                enabled = selectedArtifact != null && quote.isNotBlank() && !capture.isPublishing,
            ) {
                Text(if (capture.isPublishing) "Saving" else "Highlight")
            }
            OutlinedButton(
                onClick = {
                    val upload = capture.upload ?: return@OutlinedButton
                    dispatch(
                        HighlighterAppAction.PublishCapturePicture(
                            selectedArtifact?.let { HighlighterCaptureArtifact.Existing(it) },
                            selectedGroupId,
                            upload,
                            note.trim(),
                        ),
                    )
                },
                shape = RoundedCornerShape(8.dp),
                enabled = capture.upload != null && !capture.isPublishing,
            ) {
                Text("Picture")
            }
        }
    }
}

@Composable
private fun ArtifactPickerRow(
    record: ArtifactRecord,
    selected: Boolean,
    onSelect: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onSelect)
            .padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (record.preview.image.isNotBlank()) {
            RemoteImage(
                url = record.preview.image,
                contentDescription = null,
                modifier = Modifier.size(40.dp),
                shape = CoverShape,
            )
            Spacer(modifier = Modifier.width(12.dp))
        }
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = record.preview.title.ifBlank { record.preview.url.ifBlank { "Untitled" } },
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = record.preview.author.ifBlank { record.preview.domain },
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        TextButton(onClick = onSelect) {
            Text(if (selected) "Selected" else "Use")
        }
    }
}

private data class PickedImage(
    val bytes: ByteArray,
    val mime: String,
    val width: UInt,
    val height: UInt,
)

private fun readPickedImage(context: Context, uri: Uri): PickedImage? =
    runCatching {
        val bytes = context.contentResolver.openInputStream(uri)?.use { it.readBytes() }
            ?: return null
        val options = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options)
        PickedImage(
            bytes = bytes,
            mime = context.contentResolver.getType(uri) ?: "image/jpeg",
            width = options.outWidth.coerceAtLeast(0).toUInt(),
            height = options.outHeight.coerceAtLeast(0).toUInt(),
        )
    }.getOrNull()
