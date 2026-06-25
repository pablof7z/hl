package com.highlighter.app.ui.capture

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import com.highlighter.app.ui.components.Chip
import com.highlighter.app.ui.components.CoverShape
import com.highlighter.app.ui.components.Panel
import com.highlighter.app.ui.components.RemoteImage
import com.highlighter.app.ui.components.SectionHeader
import com.highlighter.app.ui.components.ToggleButton
import com.highlighter.app.ui.search.SearchGroupHeader
import com.highlighter.app.util.LocalIsbnPreviews
import com.highlighter.app.util.previewForIsbn
import com.highlighter.app.util.readPickedImage
import uniffi.highlighter_core.ArtifactPreview
import uniffi.highlighter_core.ArtifactRecord
import uniffi.highlighter_core.BlossomUpload
import uniffi.highlighter_core.CommunitySummary
import uniffi.highlighter_core.HighlightDraft
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterBookPickerSnapshot
import uniffi.highlighter_core.HighlighterCaptureArtifact
import uniffi.highlighter_core.HighlighterCaptureSnapshot
import uniffi.highlighter_core.normalizeIsbn

// How many recent books to prime on first appearance — matches iOS limit of 24.
private const val RECENTS_LIMIT: UInt = 24u

/** Internal phase for the OCR capture sub-flow. */
private enum class CapturePhase {
    /** Composite panel: gallery + book picker + manual ISBN. */
    Idle,
    /** Full-screen camera viewfinder. */
    Camera,
    /** Page image review + OCR text quote selection. */
    Review,
}

@Composable
internal fun CapturePanel(
    capture: HighlighterCaptureSnapshot,
    bookPicker: HighlighterBookPickerSnapshot,
    communities: List<CommunitySummary>,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    val context = LocalContext.current
    var phase by rememberSaveable { mutableStateOf(CapturePhase.Idle) }
    var captureResult by remember { mutableStateOf<CaptureResult?>(null) }

    // ── Camera permission handling ────────────────────────────────────────────
    var hasCameraPermission by remember {
        mutableStateOf(
            ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) ==
                PackageManager.PERMISSION_GRANTED,
        )
    }
    val cameraPermissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted ->
        hasCameraPermission = granted
        if (granted) phase = CapturePhase.Camera
    }

    // Route to full-screen sub-flows
    when (phase) {
        CapturePhase.Camera -> {
            CameraCaptureScreen(
                onCapture = { result ->
                    captureResult = result
                    // Kick off upload immediately (parallel to review).
                    dispatch(
                        HighlighterAppAction.UploadCapturePhoto(
                            result.jpegBytes,
                            "image/jpeg",
                            result.width,
                            result.height,
                            "",
                        ),
                    )
                    phase = CapturePhase.Review
                },
                onDismiss = { phase = CapturePhase.Idle },
            )
            return
        }
        CapturePhase.Review -> {
            val result = captureResult
            if (result != null) {
                CapturePanelReviewWrapper(
                    captureResult = result,
                    capture = capture,
                    bookPicker = bookPicker,
                    communities = communities,
                    dispatch = dispatch,
                    onRetake = {
                        dispatch(HighlighterAppAction.ClearCaptureUpload)
                        captureResult = null
                        phase = CapturePhase.Camera
                    },
                    onDone = {
                        captureResult = null
                        phase = CapturePhase.Idle
                    },
                )
                return
            } else {
                phase = CapturePhase.Idle
            }
        }
        CapturePhase.Idle -> { /* fall through to main panel */ }
    }

    // ── Main Idle panel ───────────────────────────────────────────────────────
    CapturePanelIdle(
        capture = capture,
        bookPicker = bookPicker,
        communities = communities,
        dispatch = dispatch,
        onCameraClick = {
            if (hasCameraPermission) {
                phase = CapturePhase.Camera
            } else {
                cameraPermissionLauncher.launch(Manifest.permission.CAMERA)
            }
        },
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Idle panel (gallery + book picker + ISBN)
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun CapturePanelIdle(
    capture: HighlighterCaptureSnapshot,
    bookPicker: HighlighterBookPickerSnapshot,
    communities: List<CommunitySummary>,
    dispatch: (HighlighterAppAction) -> Unit,
    onCameraClick: () -> Unit,
) {
    val context = LocalContext.current
    var query by remember { mutableStateOf("") }
    var quote by remember { mutableStateOf("") }
    var note by remember { mutableStateOf("") }
    var selectedArtifact by remember { mutableStateOf<HighlighterCaptureArtifact?>(null) }
    var selectedGroupId by remember { mutableStateOf<String?>(null) }

    // ISBN manual-entry state
    var isbnRaw by remember { mutableStateOf("") }
    var isbnError by remember { mutableStateOf<String?>(null) }
    var resolvingIsbn by remember { mutableStateOf<String?>(null) }
    var showBarcodeScanner by remember { mutableStateOf(false) }

    val isbnPreviews = LocalIsbnPreviews.current
    val resolvedPreview: ArtifactPreview? = resolvingIsbn?.let { isbnPreviews.previewForIsbn(it) }

    LaunchedEffect(Unit) {
        if (bookPicker.recentBooks.isEmpty() && !bookPicker.isLoadingRecents) {
            dispatch(HighlighterAppAction.RequestBookPickerRecents(RECENTS_LIMIT))
        }
    }

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

    // Show barcode scanner as a full-screen overlay
    if (showBarcodeScanner) {
        IsbnBarcodeScannerScreen(
            onResult = { isbn ->
                showBarcodeScanner = false
                if (isbn != null) {
                    isbnRaw = isbn
                    isbnError = null
                    val catalogId = "isbn:$isbn"
                    val existing = bookPicker.recentBooks.firstOrNull {
                        it.preview.catalogId == catalogId
                    }
                    if (existing != null) {
                        selectedArtifact = HighlighterCaptureArtifact.Existing(existing)
                        resolvingIsbn = null
                    } else {
                        resolvingIsbn = isbn
                        dispatch(HighlighterAppAction.RequestIsbnPreview(isbn))
                    }
                }
            },
        )
        return
    }

    Panel {
        SectionHeader("Capture", if (capture.isPublishing) "Publishing" else "Highlight")
        Spacer(modifier = Modifier.height(8.dp))

        // ── Photo buttons ─────────────────────────────────────────────────────
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            // Camera button — opens camera capture flow
            OutlinedButton(
                onClick = onCameraClick,
                shape = RoundedCornerShape(8.dp),
                enabled = !capture.isUploading,
                modifier = Modifier.testTag("capture_camera_button"),
            ) {
                Text("Camera")
            }
            // Gallery button — existing photo-picker path
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

        Spacer(modifier = Modifier.height(12.dp))
        HorizontalDivider()
        Spacer(modifier = Modifier.height(12.dp))

        // ── Book picker section ───────────────────────────────────────────────
        Text("Book", style = MaterialTheme.typography.titleSmall)
        Spacer(modifier = Modifier.height(8.dp))

        OutlinedTextField(
            value = query,
            onValueChange = {
                query = it
                if (it.trim().length >= 2) {
                    dispatch(HighlighterAppAction.SearchBookPickerArtifacts(it.trim(), 20u))
                } else if (it.isBlank()) {
                    dispatch(HighlighterAppAction.ClearBookPickerSearch)
                }
            },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text("Search your books") },
        )
        if (query.isNotBlank()) {
            TextButton(onClick = {
                query = ""
                dispatch(HighlighterAppAction.ClearBookPickerSearch)
            }) {
                Text("Clear search")
            }
        }
        Spacer(modifier = Modifier.height(8.dp))

        if (query.isBlank()) {
            when {
                bookPicker.isLoadingRecents -> {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        modifier = Modifier.padding(vertical = 8.dp),
                    ) {
                        CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
                        Text("Loading your books…", style = MaterialTheme.typography.bodySmall)
                    }
                }
                bookPicker.recentBooks.isEmpty() -> {
                    Text(
                        text = "No books yet — paste an ISBN below to start your library.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(vertical = 8.dp),
                    )
                }
                else -> {
                    Text(
                        text = "Recent",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Spacer(modifier = Modifier.height(6.dp))
                    LazyRow(
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                        modifier = Modifier.testTag("capture_book_recents"),
                    ) {
                        items(bookPicker.recentBooks, key = { it.shareEventId }) { book ->
                            val isSelected = selectedArtifact?.let {
                                it is HighlighterCaptureArtifact.Existing &&
                                    it.record.shareEventId == book.shareEventId
                            } ?: false
                            RecentBookCard(
                                book = book,
                                selected = isSelected,
                                onSelect = {
                                    selectedArtifact = HighlighterCaptureArtifact.Existing(book)
                                },
                            )
                        }
                    }
                }
            }
        } else {
            if (bookPicker.isSearching) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.padding(vertical = 8.dp),
                ) {
                    CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
                    Text("Searching…", style = MaterialTheme.typography.bodySmall)
                }
            } else if (bookPicker.searchResults.isEmpty()) {
                Text(
                    text = "No matches — try an ISBN below.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(vertical = 8.dp),
                )
            } else {
                bookPicker.searchResults.take(8).forEach { record ->
                    ArtifactPickerRow(
                        record = record,
                        selected = selectedArtifact?.let {
                            it is HighlighterCaptureArtifact.Existing &&
                                it.record.shareEventId == record.shareEventId
                        } ?: false,
                        onSelect = { selectedArtifact = HighlighterCaptureArtifact.Existing(record) },
                    )
                }
            }
        }

        Spacer(modifier = Modifier.height(12.dp))

        // ── Manual ISBN entry + barcode scan ──────────────────────────────────
        Text(
            text = "Manual ISBN",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(modifier = Modifier.height(4.dp))
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            OutlinedTextField(
                value = isbnRaw,
                onValueChange = {
                    isbnRaw = it
                    isbnError = null
                    if (resolvingIsbn != null && it.trim() != resolvingIsbn) {
                        resolvingIsbn = null
                    }
                },
                modifier = Modifier
                    .weight(1f)
                    .testTag("capture_isbn_field"),
                singleLine = true,
                label = { Text("978-…") },
                isError = isbnError != null,
                supportingText = isbnError?.let { { Text(it) } },
            )
            OutlinedButton(
                onClick = {
                    val normalized = normalizeIsbn(isbnRaw.trim())
                    if (normalized == null) {
                        isbnError = "Enter a valid 10- or 13-digit ISBN"
                    } else {
                        isbnError = null
                        val catalogId = "isbn:$normalized"
                        val existing = bookPicker.recentBooks.firstOrNull {
                            it.preview.catalogId == catalogId
                        }
                        if (existing != null) {
                            selectedArtifact = HighlighterCaptureArtifact.Existing(existing)
                            resolvingIsbn = null
                        } else {
                            resolvingIsbn = normalized
                            dispatch(HighlighterAppAction.RequestIsbnPreview(normalized))
                        }
                    }
                },
                shape = RoundedCornerShape(8.dp),
            ) {
                Text("Find")
            }
            // Barcode scan button
            OutlinedButton(
                onClick = { showBarcodeScanner = true },
                shape = RoundedCornerShape(8.dp),
                modifier = Modifier.testTag("capture_scan_barcode"),
            ) {
                Text("Scan")
            }
        }

        if (resolvingIsbn != null) {
            Spacer(modifier = Modifier.height(10.dp))
            IsbnPreviewCard(
                isbn = resolvingIsbn!!,
                preview = resolvedPreview,
                onUse = { preview ->
                    selectedArtifact = HighlighterCaptureArtifact.Pending(preview)
                    resolvingIsbn = null
                    isbnRaw = ""
                },
                onDismiss = { resolvingIsbn = null },
            )
        }

        // ── Communities ───────────────────────────────────────────────────────
        if (communities.isNotEmpty()) {
            Spacer(modifier = Modifier.height(8.dp))
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

        // ── Quote & note fields ───────────────────────────────────────────────
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

        // ── Error / status messages ───────────────────────────────────────────
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

        // ── Publish buttons ───────────────────────────────────────────────────
        Spacer(modifier = Modifier.height(10.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = {
                    val artifact = selectedArtifact ?: return@Button
                    dispatch(
                        HighlighterAppAction.PublishCaptureHighlight(
                            artifact,
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
                modifier = Modifier.testTag("capture_publish"),
            ) {
                Text(if (capture.isPublishing) "Saving" else "Highlight")
            }
            OutlinedButton(
                onClick = {
                    val upload = capture.upload ?: return@OutlinedButton
                    dispatch(
                        HighlighterAppAction.PublishCapturePicture(
                            selectedArtifact,
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

// ─────────────────────────────────────────────────────────────────────────────
// Review wrapper — OCR page review + book/community picker + publish
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Wraps [CapturePageReview] with the full publish flow: book picker, community
 * picker, and the final publish dispatch — mirrors what CaptureStore.publish()
 * does on iOS.
 */
@Composable
private fun CapturePanelReviewWrapper(
    captureResult: CaptureResult,
    capture: HighlighterCaptureSnapshot,
    bookPicker: HighlighterBookPickerSnapshot,
    communities: List<CommunitySummary>,
    dispatch: (HighlighterAppAction) -> Unit,
    onRetake: () -> Unit,
    onDone: () -> Unit,
) {
    var quote by rememberSaveable { mutableStateOf("") }
    var context by rememberSaveable { mutableStateOf("") }
    var note by rememberSaveable { mutableStateOf("") }
    var selectedArtifact by remember { mutableStateOf<HighlighterCaptureArtifact?>(null) }
    var selectedGroupId by remember { mutableStateOf<String?>(null) }
    var reviewDone by remember { mutableStateOf(false) }

    // Pre-fill with the most recent book (mirrors iOS prefillRecentBook).
    LaunchedEffect(bookPicker.recentBooks) {
        if (selectedArtifact == null) {
            bookPicker.recentBooks.firstOrNull()?.let {
                selectedArtifact = HighlighterCaptureArtifact.Existing(it)
            }
        }
    }

    // Observe publish success → reset.
    LaunchedEffect(capture.publishedEventId) {
        if (!capture.publishedEventId.isNullOrBlank()) {
            dispatch(HighlighterAppAction.ClearCaptureResult)
            onDone()
        }
    }

    if (!reviewDone) {
        // Phase 1: page image + OCR review + quote selection
        CapturePageReview(
            captureResult = captureResult,
            ocrMarkdown = capture.ocrMarkdown,
            isOcrPending = capture.isOcrPending,
            isUploading = capture.isUploading,
            onQuoteSelected = { q, ctx ->
                quote = q
                context = ctx
                reviewDone = true
            },
            onDismiss = onRetake,
        )
    } else {
        // Phase 2: metadata + publish
        Panel {
            SectionHeader("Publish", if (capture.isPublishing) "Saving" else "Highlight")
            Spacer(modifier = Modifier.height(8.dp))

            // Selected quote preview
            Text("Quote", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(modifier = Modifier.height(4.dp))
            OutlinedTextField(
                value = quote,
                onValueChange = { quote = it },
                modifier = Modifier
                    .fillMaxWidth()
                    .testTag("capture_select_quote"),
                minLines = 2,
                maxLines = 6,
                label = { Text("Quote") },
            )

            Spacer(modifier = Modifier.height(8.dp))
            OutlinedTextField(
                value = note,
                onValueChange = { note = it },
                modifier = Modifier.fillMaxWidth(),
                minLines = 1,
                maxLines = 4,
                label = { Text("Note") },
            )

            Spacer(modifier = Modifier.height(12.dp))

            // Upload status
            if (capture.isUploading) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    CircularProgressIndicator(modifier = Modifier.size(14.dp), strokeWidth = 2.dp)
                    Text("Uploading image…", style = MaterialTheme.typography.bodySmall)
                }
                Spacer(modifier = Modifier.height(8.dp))
            }

            // ── Book picker (compact) ─────────────────────────────────────────
            BookPickerCompact(
                bookPicker = bookPicker,
                selectedArtifact = selectedArtifact,
                onSelect = { selectedArtifact = it },
                dispatch = dispatch,
            )

            // ── Communities ───────────────────────────────────────────────────
            if (communities.isNotEmpty()) {
                Spacer(modifier = Modifier.height(8.dp))
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

            // ── Errors ────────────────────────────────────────────────────────
            capture.uploadErrorMessage?.takeIf { it.isNotBlank() }?.let { msg ->
                Spacer(modifier = Modifier.height(6.dp))
                Text(text = msg, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.tertiary)
            }
            capture.errorMessage?.takeIf { it.isNotBlank() }?.let { msg ->
                Spacer(modifier = Modifier.height(6.dp))
                Text(text = msg, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.tertiary)
                TextButton(onClick = { dispatch(HighlighterAppAction.ClearCaptureError) }) { Text("Dismiss") }
            }

            Spacer(modifier = Modifier.height(10.dp))

            // ── Publish row ───────────────────────────────────────────────────
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(
                    onClick = {
                        val artifact = selectedArtifact ?: return@Button
                        val upload = capture.upload ?: return@Button

                        // Alt text is presentation metadata on the image edge;
                        // OCR markdown itself is produced by the Rust kernel.
                        val altText = capture.ocrMarkdown.toImageAltText()
                        val imageWithAlt = BlossomUpload(
                            url = upload.url,
                            sha256Hex = upload.sha256Hex,
                            mime = upload.mime,
                            sizeBytes = upload.sizeBytes,
                            width = upload.width,
                            height = upload.height,
                            alt = altText,
                        )

                        dispatch(
                            HighlighterAppAction.PublishCaptureHighlight(
                                artifact,
                                selectedGroupId,
                                HighlightDraft(
                                    quote = quote.trim(),
                                    context = context.trim(),
                                    note = note.trim(),
                                    clipStartSeconds = null,
                                    clipEndSeconds = null,
                                    clipSpeaker = "",
                                    clipTranscriptSegmentIds = emptyList(),
                                    image = imageWithAlt,
                                ),
                            ),
                        )
                    },
                    shape = RoundedCornerShape(8.dp),
                    enabled = selectedArtifact != null &&
                        quote.isNotBlank() &&
                        capture.upload != null &&
                        !capture.isPublishing,
                    modifier = Modifier.testTag("capture_publish"),
                ) {
                    Text(if (capture.isPublishing) "Saving" else "Publish highlight")
                }

                OutlinedButton(
                    onClick = { reviewDone = false },
                    shape = RoundedCornerShape(8.dp),
                ) {
                    Text("Back")
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Compact book picker used in the publish step
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun BookPickerCompact(
    bookPicker: HighlighterBookPickerSnapshot,
    selectedArtifact: HighlighterCaptureArtifact?,
    onSelect: (HighlighterCaptureArtifact) -> Unit,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    Text("Book", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    Spacer(modifier = Modifier.height(4.dp))

    if (bookPicker.recentBooks.isNotEmpty()) {
        LazyRow(
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            modifier = Modifier.testTag("capture_book_recents"),
        ) {
            items(bookPicker.recentBooks.take(12), key = { it.shareEventId }) { book ->
                val isSelected = selectedArtifact?.let {
                    it is HighlighterCaptureArtifact.Existing &&
                        it.record.shareEventId == book.shareEventId
                } ?: false
                RecentBookCard(
                    book = book,
                    selected = isSelected,
                    onSelect = { onSelect(HighlighterCaptureArtifact.Existing(book)) },
                )
            }
        }
    } else {
        Text(
            text = "No books — go back and enter an ISBN.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

private fun String.toImageAltText(): String =
    replace("\n\n", " ")
        .replace("\n", " ")
        .trim()

// ─────────────────────────────────────────────────────────────────────────────
// Recent book card (cover + title, horizontal row)
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun RecentBookCard(
    book: ArtifactRecord,
    selected: Boolean,
    onSelect: () -> Unit,
) {
    Column(
        modifier = Modifier
            .width(80.dp)
            .clickable(onClick = onSelect)
            .testTag("capture_recent_book"),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(
            modifier = Modifier
                .width(80.dp)
                .aspectRatio(2f / 3f),
        ) {
            RemoteImage(
                url = book.preview.image.takeIf { it.isNotBlank() },
                contentDescription = book.preview.title,
                modifier = Modifier.matchParentSize(),
                shape = CoverShape,
            )
            if (selected) {
                Box(
                    modifier = Modifier
                        .matchParentSize()
                        .padding(2.dp),
                ) {
                    Card(
                        modifier = Modifier.matchParentSize(),
                        shape = CoverShape,
                        colors = CardDefaults.cardColors(
                            containerColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.25f),
                        ),
                    ) {}
                }
            }
        }
        if (book.preview.title.isNotBlank()) {
            Spacer(modifier = Modifier.height(4.dp))
            Text(
                text = book.preview.title,
                style = MaterialTheme.typography.labelSmall,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                color = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
            )
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Artifact picker row (search results)
// ─────────────────────────────────────────────────────────────────────────────

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
                targetSize = 40.dp,
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

// ─────────────────────────────────────────────────────────────────────────────
// ISBN preview card — "Is this right?"
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun IsbnPreviewCard(
    isbn: String,
    preview: ArtifactPreview?,
    onUse: (ArtifactPreview) -> Unit,
    onDismiss: () -> Unit,
) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .testTag("capture_isbn_preview"),
        shape = RoundedCornerShape(10.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant),
    ) {
        Column(modifier = Modifier.padding(12.dp)) {
            Text(
                text = "Is this right?",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(modifier = Modifier.height(10.dp))

            if (preview == null) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    CircularProgressIndicator(modifier = Modifier.size(18.dp), strokeWidth = 2.dp)
                    Text(
                        text = "Looking up ISBN $isbn…",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            } else {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    RemoteImage(
                        url = preview.image.takeIf { it.isNotBlank() },
                        contentDescription = preview.title,
                        modifier = Modifier
                            .width(56.dp)
                            .aspectRatio(2f / 3f),
                        shape = CoverShape,
                    )
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = preview.title.ifBlank { isbn },
                            style = MaterialTheme.typography.bodyMedium,
                            maxLines = 2,
                            overflow = TextOverflow.Ellipsis,
                        )
                        if (preview.author.isNotBlank()) {
                            Text(
                                text = preview.author,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                        Text(
                            text = isbn,
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }

            Spacer(modifier = Modifier.height(10.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (preview != null) {
                    Button(
                        onClick = {
                            val catalogId = "isbn:$isbn"
                            val committed = preview.copy(
                                source = "book",
                                catalogId = catalogId,
                                catalogKind = "isbn",
                                referenceTagName = "i",
                                referenceTagValue = catalogId,
                                referenceKind = "isbn",
                                highlightTagName = "i",
                                highlightTagValue = catalogId,
                                highlightReferenceKey = "i:$catalogId",
                            )
                            onUse(committed)
                        },
                        shape = RoundedCornerShape(8.dp),
                        modifier = Modifier.testTag("capture_isbn_use"),
                    ) {
                        Text("Use")
                    }
                }
                TextButton(onClick = onDismiss) {
                    Text("Cancel")
                }
            }
        }
    }
}
