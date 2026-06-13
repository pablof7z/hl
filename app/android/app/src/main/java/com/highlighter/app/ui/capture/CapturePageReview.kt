package com.highlighter.app.ui.capture

import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.components.SectionHeader

/**
 * Page review screen: shows the captured page image + recognized OCR text.
 *
 * The user can:
 *  1. Select text from the OCR output (via [SelectionContainer]) — Android's
 *     built-in text selection handles the tap-to-select / drag-selection UX.
 *  2. Paste or type a quote into the "Selected quote" field below.
 *  3. Hit "Use as quote" to stash it and proceed to publish.
 *
 * This is the Android parity equivalent of iOS's CapturePageView (drag-on-
 * image selection is a nice-to-have; block/text selection is the v1 target).
 *
 * @param captureResult The raw output of camera capture + OCR.
 * @param isUploading True while the Blossom upload is in flight.
 * @param onQuoteSelected Called with (quote, context) when the user confirms a selection.
 * @param onDismiss Called when the user cancels and wants to retake the photo.
 */
@Composable
internal fun CapturePageReview(
    captureResult: CaptureResult,
    isUploading: Boolean,
    onQuoteSelected: (quote: String, context: String) -> Unit,
    onDismiss: () -> Unit,
) {
    var manualQuote by rememberSaveable { mutableStateOf("") }

    val bitmap = remember(captureResult.jpegBytes) {
        BitmapFactory.decodeByteArray(captureResult.jpegBytes, 0, captureResult.jpegBytes.size)
    }

    Column(
        modifier = Modifier
            .padding(16.dp),
    ) {
        SectionHeader("Review", "Page")
        Spacer(modifier = Modifier.height(12.dp))

        // ── Page image thumbnail ──────────────────────────────────────────────
        if (bitmap != null) {
            Card(
                modifier = Modifier
                    .fillMaxWidth()
                    .aspectRatio(bitmap.width.toFloat() / bitmap.height.toFloat().coerceAtLeast(1f)),
                shape = RoundedCornerShape(8.dp),
                elevation = CardDefaults.cardElevation(defaultElevation = 2.dp),
            ) {
                Image(
                    bitmap = bitmap.asImageBitmap(),
                    contentDescription = "Captured page",
                    contentScale = ContentScale.Fit,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        }

        Spacer(modifier = Modifier.height(12.dp))

        // ── Upload status indicator ───────────────────────────────────────────
        if (isUploading) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                modifier = Modifier.padding(vertical = 4.dp),
            ) {
                CircularProgressIndicator(modifier = Modifier.size(14.dp), strokeWidth = 2.dp)
                Text(
                    "Uploading image…",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Spacer(modifier = Modifier.height(8.dp))
        }

        // ── Recognized text — selectable so user can copy/select ─────────────
        Text(
            text = "Recognized text",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(modifier = Modifier.height(4.dp))

        if (captureResult.ocrMarkdown.isNotBlank()) {
            Card(
                modifier = Modifier
                    .fillMaxWidth()
                    .testTag("capture_ocr_text"),
                shape = RoundedCornerShape(8.dp),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceVariant,
                ),
            ) {
                SelectionContainer(
                    modifier = Modifier.padding(12.dp),
                ) {
                    Text(
                        text = captureResult.ocrMarkdown,
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
            }
        } else {
            Text(
                text = "No text recognized — try retaking the photo with better lighting.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        Spacer(modifier = Modifier.height(16.dp))

        // ── Quote entry ───────────────────────────────────────────────────────
        Text(
            text = "Selected quote",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(modifier = Modifier.height(4.dp))
        OutlinedTextField(
            value = manualQuote,
            onValueChange = { manualQuote = it },
            modifier = Modifier
                .fillMaxWidth()
                .testTag("capture_select_quote"),
            minLines = 3,
            maxLines = 8,
            label = { Text("Paste or type the quote from the page above") },
        )

        Spacer(modifier = Modifier.height(12.dp))

        // ── Action row ────────────────────────────────────────────────────────
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = {
                    val quote = manualQuote.trim()
                    if (quote.isNotEmpty()) {
                        // Build a context paragraph: the markdown text that
                        // contains the quote (or the nearest surrounding text),
                        // mirroring iOS's stashedContext logic.
                        val context = findContext(captureResult.ocrMarkdown, quote)
                        onQuoteSelected(quote, context)
                    }
                },
                shape = RoundedCornerShape(8.dp),
                enabled = manualQuote.isNotBlank(),
            ) {
                Text("Use as quote")
            }
            OutlinedButton(
                onClick = onDismiss,
                shape = RoundedCornerShape(8.dp),
            ) {
                Text("Retake")
            }
        }
    }
}

/**
 * Finds the paragraph in [markdown] that contains [quote] and returns it as
 * the context string. Falls back to an empty string when no match is found
 * (e.g., the user manually typed a quote not present in the OCR text).
 */
private fun findContext(markdown: String, quote: String): String {
    if (quote.isBlank() || markdown.isBlank()) return ""
    val paragraphs = markdown.split(Regex("\n\n+"))
    val match = paragraphs.firstOrNull { it.contains(quote, ignoreCase = false) }
        ?: paragraphs.firstOrNull { it.contains(quote.take(20), ignoreCase = true) }
    return match?.trim() ?: ""
}
