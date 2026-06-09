import Foundation
import Observation
import UIKit

/// Orchestrates capture → OCR + upload → review → publish.
///
/// The moment a photo is captured the store kicks off OCR (Vision, on-device)
/// and Blossom upload in parallel. OCR output is structurally reconstructed
/// into markdown so the review screen can typeset it like a book page; the
/// user selects a span from the rendered page to "stash" as a pending
/// highlight, then taps Publish.
///
/// Photo-always invariant: every successful publish carries the photo. If the
/// upload fails, the user can retry; we never silently fall back to a
/// no-photo publish.
@MainActor
@Observable
final class CaptureStore {
    enum Phase: Equatable {
        case idle
        case processing       // OCR + upload in flight
        case reviewing
        case publishing
        case done(String?)    // event id, if meaningful for navigation
        case error(String)
    }

    var phase: Phase = .idle
    /// Locally-processed JPEG (post EXIF strip + resize). Kept so the review
    /// screen can show a thumbnail + zoom view before upload completes.
    var thumbnail: UIImage?
    /// Structurally reconstructed markdown derived from OCR. Editable via the
    /// review screen's pencil escape hatch; re-rendered on change.
    var ocrMarkdown: String = ""
    /// Raw OCR lines with normalized bounding boxes — used by the photo-canvas
    /// review screen so the user can drag to select text directly on the image.
    var ocrLines: [OCRLine] = []
    /// The quote the user stashed by selecting text + tapping Highlight.
    /// `nil` means no stash — publishing becomes a kind:20 picture.
    var stashedQuote: String?
    /// Paragraph surrounding the stashed quote (for `context` on the highlight
    /// event). Empty when the selection is already a whole paragraph.
    var stashedContext: String = ""
    /// Free-form note attached to the publish.
    var note: String = ""
    /// Picked book. Optional — picture-only posts without an artifact are
    /// valid. `.pending` selections (from ISBN scan/lookup) get their kind:11
    /// share auto-published at the moment the user hits Publish.
    var selectedBook: BookSelection?
    /// Target room. Required to enable Publish.
    var selectedGroupId: String?
    /// Blossom upload result. Publish is disabled until this exists.
    var upload: BlossomUpload?
    /// Last upload error — surfaces a retry control.
    var uploadError: String?
    /// Margin used when cropping around a selected passage. Larger values keep
    /// more surrounding page context.
    var highlightCropMarginFraction: Double = 0.08
    /// Current crop box for the selected passage, in Vision normalized
    /// coordinates. `nil` means the full scanned page is the active image.
    var highlightCropBox: CGRect?

    private let safeCore: SafeHighlighterCore
    private var processedJPEG: ImageProcessing.Result?
    private var preparedUploadJPEG: ImageProcessing.Result?
    private var selectedHighlightBoxes: [CGRect] = []
    private var uploadGeneration = 0

    init(safeCore: SafeHighlighterCore) {
        self.safeCore = safeCore
    }

    var isUploading: Bool {
        switch phase {
        case .processing, .reviewing:
            return upload == nil && uploadError == nil
        default:
            break
        }
        return false
    }

    var canPublish: Bool {
        safeCore.projectCapturePublish(
            input: CapturePublishProjectionInput(
                phase: capturePublishPhase,
                hasUpload: upload != nil
            )
        ).canPublish
    }

    /// Entry point: user just snapped a photo. Strip metadata, kick OCR +
    /// upload in parallel, reconstruct structure once OCR returns, then sit
    /// in reviewing until the user hits Publish.
    func handleCapturedImage(_ image: UIImage) {
        reset(keepingPickerSelection: false)
        phase = .processing
        thumbnail = image
        prefillRecentBook()

        Task {
            guard let initial = ImageProcessing.stripMetadataAndEncode(image) else {
                self.uploadError = ImageProcessing.failureMessage
                self.phase = .reviewing
                return
            }

            // Run OCR first so we can decide whether the capture is a
            // two-page book spread that should be auto-cropped down to
            // the dominant page before we upload. The sequential cost
            // (~1-2s) buys us a single canonical image: the user sees
            // just the page they meant to capture, OCR doesn't carry
            // text from the other side, and we don't waste an upload.
            let initialLines = await recognize(processed: initial)

            let processed: ImageProcessing.Result
            let lines: [OCRLine]
            if let detection = safeCore.detectOcrActivePage(initialLines),
               let cropped = ImageProcessing.cropToPage(initial, pageRect: detection.pageRect.cgRect) {
                processed = cropped
                lines = safeCore.cropOcrLines(initialLines, to: detection.pageRect)
                if let croppedThumb = UIImage(data: cropped.data) {
                    self.thumbnail = croppedThumb
                }
            } else {
                processed = initial
                lines = initialLines
            }

            self.processedJPEG = processed
            self.preparedUploadJPEG = processed
            self.ocrLines = lines
            let markdown = safeCore.reconstructOcrMarkdown(lines)
            self.ocrMarkdown = markdown

            // The imeta alt is a one-line summary; flatten the markdown
            // for it (paragraph breaks → spaces).
            let altText = safeCore.ocrAltText(from: markdown)
            let uploadSnapshot = await upload(processed: processed, alt: altText)
            if uploadSnapshot.error.isEmpty, let uploaded = uploadSnapshot.upload {
                self.upload = BlossomUpload(
                    url: uploaded.url,
                    sha256Hex: uploaded.sha256Hex,
                    mime: uploaded.mime,
                    sizeBytes: uploaded.sizeBytes,
                    width: uploaded.width,
                    height: uploaded.height,
                    alt: altText
                )
            } else {
                self.uploadError = uploadSnapshot.error
            }
            self.phase = .reviewing
        }
    }

    /// Default the picker to the user's most recent book — typically the one
    /// they're actively reading. Skipped if a selection already exists, and
    /// we re-check before assigning so we never overwrite a deliberate pick.
    private func prefillRecentBook() {
        guard selectedBook == nil else { return }
        Task {
            let snapshot = await safeCore.getBookPickerSnapshot(
                query: "",
                recentLimit: 1,
                searchLimit: 0
            )
            guard let book = snapshot.recents.first else { return }
            if self.selectedBook == nil {
                self.selectedBook = .existing(book)
            }
        }
    }

    func retryUpload() {
        guard let processed = preparedUploadJPEG ?? processedJPEG else { return }
        startUpload(processed: processed)
    }

    /// Stash the user's current text selection as a pending highlight. Does
    /// not publish — Publish is the terminal action.
    func stashHighlight(quote: String, context: String, selectedBoxes: [CGRect] = []) {
        let projection = safeCore.projectCaptureStash(
            input: CaptureStashProjectionInput(
                quote: quote,
                context: context
            )
        )
        guard projection.shouldStash else { return }
        stashedQuote = projection.quote
        stashedContext = projection.context
        selectedHighlightBoxes = selectedBoxes
        if let processedJPEG {
            highlightCropBox = defaultHighlightCropBox(processed: processedJPEG)
        }
        prepareHighlightedCrop(reupload: true)
    }

    func clearStash() {
        stashedQuote = nil
        stashedContext = ""
        selectedHighlightBoxes = []
        highlightCropMarginFraction = 0.08
        highlightCropBox = nil
        preparedUploadJPEG = processedJPEG
        if let processedJPEG, let image = UIImage(data: processedJPEG.data) {
            thumbnail = image
        }
        upload = nil
        uploadError = nil
        if let processedJPEG {
            startUpload(processed: processedJPEG)
        }
    }

    func updateHighlightCropMargin(_ margin: Double, reupload: Bool) {
        highlightCropMarginFraction = margin
        guard !selectedHighlightBoxes.isEmpty else { return }
        prepareHighlightedCrop(reupload: reupload)
    }

    func updateHighlightCropBox(_ cropBox: CGRect, reupload: Bool) {
        let fallback = highlightCropBox.map { OcrRect($0) }
        highlightCropBox = safeCore.sanitizeHighlightCropBox(
            OcrRect(cropBox),
            fallback: fallback
        ).cgRect
        if reupload {
            prepareHighlightedCrop(reupload: true)
        }
    }

    /// Publish the capture. Rust owns the highlight-vs-picture decision,
    /// artifact-share creation, and final event id projection.
    func publish() {
        guard let upload else { return }
        let selection = selectedBook
        let groupId = selectedGroupId
        let existingArtifact: ArtifactRecord?
        let pendingPreview: ArtifactPreview?
        switch selection {
        case .existing(let record):
            existingArtifact = record
            pendingPreview = nil
        case .pending(let preview):
            existingArtifact = nil
            pendingPreview = preview
        case nil:
            existingArtifact = nil
            pendingPreview = nil
        }

        // Refresh the imeta alt to reflect the current (possibly edited) OCR.
        let imageWithAlt = BlossomUpload(
            url: upload.url,
            sha256Hex: upload.sha256Hex,
            mime: upload.mime,
            sizeBytes: upload.sizeBytes,
            width: upload.width,
            height: upload.height,
            alt: safeCore.ocrAltText(from: ocrMarkdown)
        )

        phase = .publishing
        Task {
            let outcome = await safeCore.publishCapture(
                input: CapturePublishInput(
                    image: imageWithAlt,
                    quote: stashedQuote ?? "",
                    context: stashedContext,
                    note: note,
                    existingArtifact: existingArtifact,
                    pendingPreview: pendingPreview,
                    targetGroupId: groupId
                )
            )
            if outcome.error.isEmpty {
                self.phase = .done(outcome.eventId)
            } else {
                self.phase = .error(outcome.error)
            }
        }
    }

    func reset(keepingPickerSelection: Bool) {
        phase = .idle
        thumbnail = nil
        ocrMarkdown = ""
        ocrLines = []
        stashedQuote = nil
        stashedContext = ""
        note = ""
        upload = nil
        uploadError = nil
        processedJPEG = nil
        preparedUploadJPEG = nil
        selectedHighlightBoxes = []
        uploadGeneration = 0
        highlightCropMarginFraction = 0.08
        highlightCropBox = nil
        if !keepingPickerSelection {
            selectedBook = nil
            selectedGroupId = nil
        }
    }

    // MARK: - Internals

    private func recognize(processed: ImageProcessing.Result) async -> [OCRLine] {
        guard let provider = CGDataProvider(data: processed.data as CFData),
              let cgImage = CGImage(
                jpegDataProviderSource: provider,
                decode: nil,
                shouldInterpolate: true,
                intent: .defaultIntent
              ) else {
            return []
        }
        return await OCRService.recognizeLines(in: cgImage)
    }

    private func upload(
        processed: ImageProcessing.Result,
        alt: String
    ) async -> BlossomUploadSnapshot {
        await safeCore.uploadPhoto(
            bytes: processed.data,
            mime: processed.mime,
            width: UInt32(processed.width),
            height: UInt32(processed.height),
            alt: alt
        )
    }

    private func prepareHighlightedCrop(reupload: Bool) {
        guard !selectedHighlightBoxes.isEmpty, let processed = processedJPEG else { return }

        if highlightCropBox == nil {
            highlightCropBox = defaultHighlightCropBox(processed: processed)
        }
        guard let highlightCropBox else {
            preparedUploadJPEG = processed
            if reupload {
                startUpload(processed: processed)
            }
            return
        }

        guard let highlighted = ImageProcessing.cropAndAnnotateHighlight(
            processed,
            highlightBoxes: selectedHighlightBoxes,
            cropBox: highlightCropBox
        ) else {
            upload = nil
            uploadError = ImageProcessing.failureMessage
            return
        }
        preparedUploadJPEG = highlighted
        upload = nil
        uploadError = nil
        if reupload {
            startUpload(processed: highlighted)
        }
    }

    private func startUpload(processed: ImageProcessing.Result) {
        uploadGeneration += 1
        let generation = uploadGeneration
        upload = nil
        uploadError = nil

        Task {
            let altText = safeCore.ocrAltText(from: ocrMarkdown)
            let outcome = await upload(processed: processed, alt: altText)
            guard generation == self.uploadGeneration else { return }
            guard outcome.error.isEmpty, let uploaded = outcome.upload else {
                self.uploadError = outcome.error
                return
            }
            self.upload = BlossomUpload(
                url: uploaded.url,
                sha256Hex: uploaded.sha256Hex,
                mime: uploaded.mime,
                sizeBytes: uploaded.sizeBytes,
                width: uploaded.width,
                height: uploaded.height,
                alt: altText
            )
        }
    }

    private func defaultHighlightCropBox(processed: ImageProcessing.Result) -> CGRect? {
        safeCore.defaultHighlightCropBox(
            highlightBoxes: selectedHighlightBoxes.map { OcrRect($0) },
            imageWidth: Double(processed.width),
            imageHeight: Double(processed.height),
            marginFraction: highlightCropMarginFraction
        )?.cgRect
    }

    private var capturePublishPhase: CapturePublishPhase {
        switch phase {
        case .idle:
            return .idle
        case .processing:
            return .processing
        case .reviewing:
            return .reviewing
        case .publishing:
            return .publishing
        case .done:
            return .done
        case .error:
            return .error
        }
    }
}
