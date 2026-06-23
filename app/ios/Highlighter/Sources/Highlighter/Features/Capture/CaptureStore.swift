import Foundation
import Observation
import UIKit

/// Orchestrates capture → OCR + upload → review → publish.
///
/// Phase 7 (β cut): the KERNEL owns OCR reconstruction, the capture draft
/// (quote/context/note/target-group/artifact), the publish FSM, and is the SOLE
/// WRITER of every nostr event (kind:9802 highlight / kind:20 picture / kind:11
/// artifact / kind:16 share — full parity in reduce_action_publish + the blossom
/// upload). NATIVE owns only PIXEL work: EXIF strip, the two-page-spread
/// crop-to-page, and the highlight crop/annotate — all device-capability steps
/// (NOT a second nostr lane). Native writes a `data_dir` image handle and hands
/// the path to the kernel (hl.ocr.recognize / hl.blossom.upload); no raw bytes
/// cross the FFI. The view reads draft/markdown/words/canPublish from
/// `kernel.captureSnapshot`.
///
/// Photo-always invariant: every publish carries the photo (kernel enforces it;
/// the annotated image is uploaded via Blossom before publish becomes enabled).
@MainActor
@Observable
final class CaptureStore {
    enum Phase: Equatable {
        case idle
        case processing       // native EXIF/crop + kernel OCR in flight
        case reviewing
        case publishing
        case done(String?)
        case error(String)
    }

    private(set) var phase: Phase = .idle
    /// Locally-processed JPEG (post EXIF strip + crop-to-page). Native — the
    /// review canvas shows it; the annotate step crops/marks it for upload.
    private(set) var thumbnail: UIImage?
    /// Kernel-reconstructed markdown (captureSnapshot.markdown).
    private(set) var ocrMarkdown: String = ""
    /// Kernel raw OCR lines (captureSnapshot.rawLines) — the drag-select canvas.
    private(set) var ocrLines: [OCRLine] = []
    /// Kernel selectable words (captureSnapshot.selectableWords). Indices into
    /// THIS array are what `captureSelectWord` expects (kernel is the authority).
    private(set) var selectableWords: [OcrWord] = []
    /// The stashed quote (captureSnapshot.draftQuote); `nil` → publishing is a
    /// kind:20 picture.
    private(set) var stashedQuote: String?
    private(set) var stashedContext: String = ""

    /// Free-form note. User input → kernel (hl.capture.set_note); the kernel is
    /// authoritative, this is the editable mirror the TextField binds to.
    var note: String = "" {
        didSet {
            guard note != oldValue else { return }
            kernel.app.dispatch(.captureSetNote(note: note))
        }
    }
    /// Picked book. User input → kernel artifact-setter (bec363da).
    var selectedBook: BookSelection? {
        didSet { dispatchSelectedBook() }
    }
    /// Target room. User input → kernel (hl.capture.set_target_group).
    var selectedGroupId: String? {
        didSet {
            if let id = selectedGroupId, !id.isEmpty {
                kernel.app.dispatch(.captureSetTargetGroup(groupId: id))
            } else {
                kernel.app.dispatch(.captureClearTargetGroup)
            }
        }
    }

    /// `true` while a Blossom upload (of the annotated image) is in flight. The
    /// kernel owns the upload; this is the Swift-side spinner hint, cleared when
    /// the next snapshot reflects the result (canPublish flips).
    private(set) var isUploading: Bool = false
    private(set) var uploadError: String?
    /// Current crop box for the selected passage (Vision normalized). Native.
    private(set) var highlightCropBox: CGRect?

    @ObservationIgnored private let core: HighlighterCore
    @ObservationIgnored private let kernel: HighlighterAppKernel
    @ObservationIgnored private var processedJPEG: ImageProcessing.Result?
    @ObservationIgnored private var selectedHighlightBoxes: [CGRect] = []
    @ObservationIgnored private var highlightCropMarginFraction: Double = 0.08

    init(core: HighlighterCore, kernel: HighlighterAppKernel) {
        self.core = core
        self.kernel = kernel
    }

    /// `canPublish` is kernel-owned (draft FSM + has_upload gating).
    var canPublish: Bool {
        kernel.captureSnapshot?.canPublish ?? false
    }

    // MARK: - Kernel snapshot

    /// Apply the kernel capture snapshot. Wired from CapturePageView's
    /// `.onChange(of: kernel.captureSnapshot)`. The kernel owns OCR/draft/FSM;
    /// native owns thumbnail/crop/upload-flag.
    func applyKernelSnapshot() {
        guard let snap = kernel.captureSnapshot else { return }
        ocrMarkdown = snap.markdown
        ocrLines = snap.rawLines
        selectableWords = snap.selectableWords
        stashedQuote = snap.draftQuote.isEmpty ? nil : snap.draftQuote
        stashedContext = snap.draftContext
        // A snapshot with reconstructed words means OCR finished → reviewing.
        if phase == .processing, !snap.pending, !snap.selectableWords.isEmpty {
            phase = .reviewing
        }
        // The snapshot reflecting the latest blossom/publish state clears the
        // transient upload spinner.
        if snap.canPublish { isUploading = false }
        switch snap.publishPhase {
        case .publishing: phase = .publishing
        case .done: phase = .done(nil)
        case .error: phase = .error(snap.publishError)
        case .idle, .reviewing: break
        }
    }

    // MARK: - Capture entry (native pixel pipeline → kernel OCR)

    /// User snapped a photo. Native: strip EXIF, run OCR once to detect a
    /// two-page spread, crop to the dominant page. Then hand the cropped handle
    /// to the kernel for the authoritative OCR → snapshot.
    func handleCapturedImage(_ image: UIImage) {
        reset(keepingPickerSelection: false)
        phase = .processing
        thumbnail = image
        kernel.openCapture()
        prefillRecentBook()

        Task {
            guard let initial = ImageProcessing.stripMetadataAndEncode(image) else {
                self.uploadError = ImageProcessing.failureMessage
                self.phase = .reviewing
                return
            }
            // Native OCR ONLY to detect + crop a two-page spread (pixel work);
            // the kernel re-OCRs the cropped image for the authoritative draft.
            let initialLines = await self.recognize(processed: initial)
            let processed: ImageProcessing.Result
            if let detection = self.core.detectOcrActivePage(lines: initialLines),
               let cropped = ImageProcessing.cropToPage(initial, pageRect: detection.pageRect.cgRect) {
                processed = cropped
                if let croppedThumb = UIImage(data: cropped.data) { self.thumbnail = croppedThumb }
            } else {
                processed = initial
            }
            self.processedJPEG = processed
            // Hand the cropped page to the kernel: write a data_dir handle, then
            // hl.ocr.recognize → kernel OCR → captureSnapshot (markdown/words).
            guard let handle = CapturePresenter.writeHandle(processed.data, ext: "jpg") else {
                self.uploadError = ImageProcessing.failureMessage
                self.phase = .reviewing
                return
            }
            self.kernel.app.dispatch(.ocrRecognize(imageHandle: handle))
            // Upload the un-annotated page now so a picture-only (no-quote)
            // publish has its photo; a later stash re-uploads the annotated crop.
            self.startUpload(processed: processed)
        }
    }

    private func prefillRecentBook() {
        guard selectedBook == nil else { return }
        Task {
            let snapshot = await core.getBookPickerSnapshot(query: "", recentLimit: 1, searchLimit: 0)
            guard let book = snapshot.recents.first, self.selectedBook == nil else { return }
            self.selectedBook = .existing(book)
        }
    }

    // MARK: - Highlight stash (word-index selection + native annotate → blossom)

    /// Stash the current word selection: the kernel builds the draft quote from
    /// the selected word INDICES (selectWord); native crops/annotates the boxes
    /// and uploads the marked image via Blossom (kernel sole writer of the blob).
    func stashHighlight(wordIndices: [Int], selectedBoxes: [CGRect]) {
        kernel.app.dispatch(.captureClearSelection)
        for idx in wordIndices where idx >= 0 {
            kernel.app.dispatch(.captureSelectWord(wordIndex: UInt64(idx)))
        }
        selectedHighlightBoxes = selectedBoxes
        if let processedJPEG {
            highlightCropBox = defaultHighlightCropBox(processed: processedJPEG)
        }
        prepareHighlightedCrop()
    }

    func clearStash() {
        kernel.app.dispatch(.captureClearSelection)
        selectedHighlightBoxes = []
        highlightCropMarginFraction = 0.08
        highlightCropBox = nil
        if let processedJPEG { startUpload(processed: processedJPEG) }
    }

    func updateHighlightCropMargin(_ margin: Double, reupload: Bool) {
        highlightCropMarginFraction = margin
        guard !selectedHighlightBoxes.isEmpty else { return }
        prepareHighlightedCrop(reupload: reupload)
    }

    func updateHighlightCropBox(_ cropBox: CGRect, reupload: Bool) {
        let sanitized: CGRect = {
            let minX = max(0.0, min(cropBox.minX, 1.0))
            let minY = max(0.0, min(cropBox.minY, 1.0))
            let maxX = max(minX, min(cropBox.maxX, 1.0))
            let maxY = max(minY, min(cropBox.maxY, 1.0))
            return CGRect(x: minX, y: minY, width: maxX - minX, height: maxY - minY)
        }()
        highlightCropBox = sanitized
        if reupload { prepareHighlightedCrop(reupload: true) }
    }

    func retryUpload() {
        if !selectedHighlightBoxes.isEmpty {
            prepareHighlightedCrop(reupload: true)
        } else if let processedJPEG {
            startUpload(processed: processedJPEG)
        }
    }

    // MARK: - Publish (kernel sole writer)

    /// Publish the capture — kernel-sole-writer via the full-parity
    /// reduce_action_publish (highlight+imeta+artifact / kind:20 picture /
    /// pending-book multi-event / kind:16 share). The draft (quote/context/note/
    /// target-group/artifact) + the uploaded blob are already on the kernel.
    func publish() {
        phase = .publishing
        kernel.app.dispatch(.capturePublish)
    }

    func reset(keepingPickerSelection: Bool) {
        kernel.app.dispatch(.captureReset)
        phase = .idle
        thumbnail = nil
        ocrMarkdown = ""
        ocrLines = []
        selectableWords = []
        stashedQuote = nil
        stashedContext = ""
        note = ""
        isUploading = false
        uploadError = nil
        processedJPEG = nil
        selectedHighlightBoxes = []
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

    private func dispatchSelectedBook() {
        switch selectedBook {
        case .existing(let record):
            kernel.app.dispatch(.captureSetArtifactRecord(artifactJson: captureArtifactRecordJson(artifact: record)))
        case .pending(let preview):
            kernel.app.dispatch(.captureSetArtifactPreview(previewJson: captureArtifactPreviewJson(preview: preview)))
        case nil:
            kernel.app.dispatch(.captureClearArtifact)
        }
    }

    /// Crop + annotate the selected boxes into the upload image, write the
    /// handle, and hand it to the kernel's Blossom upload (sole blob writer).
    private func prepareHighlightedCrop(reupload: Bool = true) {
        guard !selectedHighlightBoxes.isEmpty, let processed = processedJPEG else { return }
        if highlightCropBox == nil {
            highlightCropBox = defaultHighlightCropBox(processed: processed)
        }
        guard let highlightCropBox,
              let highlighted = ImageProcessing.cropAndAnnotateHighlight(
                processed,
                highlightBoxes: selectedHighlightBoxes,
                cropBox: highlightCropBox
              ) else {
            uploadError = ImageProcessing.failureMessage
            return
        }
        uploadError = nil
        if reupload { startUpload(processed: highlighted) }
    }

    /// Write the image to a data_dir handle and dispatch hl.blossom.upload
    /// (kernel uploads + records the blob descriptor on the draft).
    private func startUpload(processed: ImageProcessing.Result) {
        uploadError = nil
        guard let handle = CapturePresenter.writeHandle(processed.data, ext: "jpg") else {
            uploadError = ImageProcessing.failureMessage
            return
        }
        isUploading = true
        kernel.app.dispatch(.blossomUpload(imageHandle: handle, servers: []))
    }

    private func defaultHighlightCropBox(processed: ImageProcessing.Result) -> CGRect? {
        core.defaultHighlightCropBox(
            highlightBoxes: selectedHighlightBoxes.map { OcrRect($0) },
            imageWidth: Double(processed.width),
            imageHeight: Double(processed.height),
            marginFraction: highlightCropMarginFraction
        )?.cgRect
    }
}
