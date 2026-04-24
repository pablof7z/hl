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

    private let safeCore: SafeHighlighterCore
    private var processedJPEG: ImageProcessing.Result?

    init(safeCore: SafeHighlighterCore) {
        self.safeCore = safeCore
    }

    var isUploading: Bool {
        if case .processing = phase { return upload == nil && uploadError == nil }
        return false
    }

    var canPublish: Bool {
        switch phase {
        case .reviewing, .processing:
            break
        default:
            return false
        }
        return upload != nil
    }

    /// Entry point: user just snapped a photo. Strip metadata, kick OCR +
    /// upload in parallel, reconstruct structure once OCR returns, then sit
    /// in reviewing until the user hits Publish.
    func handleCapturedImage(_ image: UIImage) {
        reset(keepingPickerSelection: false)
        phase = .processing
        thumbnail = image

        Task {
            do {
                let processed = try ImageProcessing.stripMetadataAndEncode(image)
                self.processedJPEG = processed

                async let ocrTask = recognize(processed: processed)
                async let uploadTask = upload(processed: processed, alt: "")

                let (lines, uploaded) = try await (ocrTask, uploadTask)
                let markdown = OCRStructureReconstructor.toMarkdown(lines)
                self.ocrMarkdown = markdown

                // The imeta alt is a one-line summary; flatten the markdown
                // for it (paragraph breaks → spaces).
                let altText = flattenForAlt(markdown)
                self.upload = BlossomUpload(
                    url: uploaded.url,
                    sha256Hex: uploaded.sha256Hex,
                    mime: uploaded.mime,
                    sizeBytes: uploaded.sizeBytes,
                    width: uploaded.width,
                    height: uploaded.height,
                    alt: altText
                )
                self.phase = .reviewing
            } catch {
                // OCR alone never fails here (it returns []); this catches
                // upload errors. If upload already succeeded via the task
                // group, leave it alone and slide into reviewing so the user
                // can still edit text; otherwise surface the error.
                if self.upload == nil {
                    self.uploadError = (error as? LocalizedError)?.errorDescription
                        ?? error.localizedDescription
                }
                self.phase = .reviewing
            }
        }
    }

    func retryUpload() {
        guard let processed = processedJPEG else { return }
        uploadError = nil
        Task {
            do {
                let altText = flattenForAlt(ocrMarkdown)
                let uploaded = try await upload(processed: processed, alt: altText)
                self.upload = BlossomUpload(
                    url: uploaded.url,
                    sha256Hex: uploaded.sha256Hex,
                    mime: uploaded.mime,
                    sizeBytes: uploaded.sizeBytes,
                    width: uploaded.width,
                    height: uploaded.height,
                    alt: altText
                )
            } catch {
                self.uploadError = error.localizedDescription
            }
        }
    }

    /// Stash the user's current text selection as a pending highlight. Does
    /// not publish — Publish is the terminal action.
    func stashHighlight(quote: String, context: String) {
        let trimmedQuote = quote.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedQuote.isEmpty else { return }
        stashedQuote = trimmedQuote
        stashedContext = context.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    func clearStash() {
        stashedQuote = nil
        stashedContext = ""
    }

    /// Publish the capture. If `stashedQuote` is set AND a book is picked,
    /// goes via the highlight (kind:9802) path; otherwise publishes a kind:20
    /// picture event. When `selectedGroupId` is set, the highlight is also
    /// shared into the room via a kind:16 repost.
    ///
    /// For a `.pending` book with a room, the kind:11 artifact share is
    /// auto-published first. Without a room, an `ArtifactRecord` is synthesised
    /// from the preview so the highlight still carries the reference tags.
    func publish() {
        guard let upload else { return }
        let trimmedNote = note.trimmingCharacters(in: .whitespacesAndNewlines)
        let selection = selectedBook
        let groupId = selectedGroupId
        let quote = stashedQuote?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""

        // Refresh the imeta alt to reflect the current (possibly edited) OCR.
        let imageWithAlt = BlossomUpload(
            url: upload.url,
            sha256Hex: upload.sha256Hex,
            mime: upload.mime,
            sizeBytes: upload.sizeBytes,
            width: upload.width,
            height: upload.height,
            alt: flattenForAlt(ocrMarkdown)
        )

        phase = .publishing
        Task {
            do {
                if !quote.isEmpty, let selection {
                    let artifact = try await resolveArtifact(selection, groupId: groupId)
                    let draft = HighlightDraft(
                        quote: quote,
                        context: stashedContext,
                        note: trimmedNote,
                        clipStartSeconds: nil,
                        clipEndSeconds: nil,
                        clipSpeaker: "",
                        clipTranscriptSegmentIds: [],
                        image: imageWithAlt
                    )
                    if let groupId {
                        let records = try await safeCore.publishHighlightsAndShare(
                            artifact: artifact,
                            drafts: [draft],
                            targetGroupId: groupId
                        )
                        self.phase = .done(records.first?.eventId)
                    } else {
                        let record = try await safeCore.publishHighlight(draft: draft, artifact: artifact)
                        self.phase = .done(record.eventId)
                    }
                } else {
                    let artifactForPicture: ArtifactRecord?
                    switch selection {
                    case .existing(let record):
                        artifactForPicture = record
                    case .pending(let preview):
                        if let groupId {
                            artifactForPicture = try await safeCore.publishArtifact(
                                preview: preview,
                                groupId: groupId,
                                note: nil
                            )
                        } else {
                            artifactForPicture = ArtifactRecord(
                                preview: preview,
                                groupId: "",
                                shareEventId: "",
                                pubkey: "",
                                createdAt: nil,
                                note: ""
                            )
                        }
                    case nil:
                        artifactForPicture = nil
                    }
                    let draft = PictureDraft(
                        image: imageWithAlt,
                        note: trimmedNote,
                        artifact: artifactForPicture,
                        targetGroupId: groupId
                    )
                    let record = try await safeCore.publishPicture(draft)
                    self.phase = .done(record.eventId)
                }
            } catch {
                self.phase = .error(error.localizedDescription)
            }
        }
    }

    /// Produce an `ArtifactRecord` for the given selection.
    /// For `.existing`, returns as-is. For `.pending` with a group, publishes
    /// the kind:11 artifact share first; without a group, synthesises a record
    /// from the preview so the highlight event can carry the reference tags.
    private func resolveArtifact(_ selection: BookSelection, groupId: String?) async throws -> ArtifactRecord {
        switch selection {
        case .existing(let record):
            return record
        case .pending(let preview):
            if let groupId {
                return try await safeCore.publishArtifact(
                    preview: preview,
                    groupId: groupId,
                    note: nil
                )
            } else {
                return ArtifactRecord(
                    preview: preview,
                    groupId: "",
                    shareEventId: "",
                    pubkey: "",
                    createdAt: nil,
                    note: ""
                )
            }
        }
    }

    func reset(keepingPickerSelection: Bool) {
        phase = .idle
        thumbnail = nil
        ocrMarkdown = ""
        stashedQuote = nil
        stashedContext = ""
        note = ""
        upload = nil
        uploadError = nil
        processedJPEG = nil
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
        return (try? await OCRService.recognizeLines(in: cgImage)) ?? []
    }

    private func upload(
        processed: ImageProcessing.Result,
        alt: String
    ) async throws -> BlossomUpload {
        try await safeCore.uploadPhoto(
            bytes: processed.data,
            mime: processed.mime,
            width: UInt32(processed.width),
            height: UInt32(processed.height),
            alt: alt
        )
    }

    private func flattenForAlt(_ markdown: String) -> String {
        markdown
            .replacingOccurrences(of: "\n\n", with: " ")
            .replacingOccurrences(of: "\n", with: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
