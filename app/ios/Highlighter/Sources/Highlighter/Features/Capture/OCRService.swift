import CoreGraphics
import Foundation
import Vision

enum OCRService {
    /// Run on-device text recognition over `cgImage` and return the observed
    /// lines with their normalized bounding boxes, ready for structural
    /// reconstruction. Returns an empty array if nothing was detected.
    ///
    /// Uses `.accurate` recognition with language correction — handles most
    /// photographed paperback pages well. Cloud fallback is out of scope.
    static func recognizeLines(in cgImage: CGImage) async throws -> [OCRLine] {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<[OCRLine], Swift.Error>) in
            let request = VNRecognizeTextRequest { request, error in
                if let error {
                    continuation.resume(throwing: error)
                    return
                }
                guard let observations = request.results as? [VNRecognizedTextObservation] else {
                    continuation.resume(returning: [])
                    return
                }
                let lines: [OCRLine] = observations.compactMap { obs in
                    guard let candidate = obs.topCandidates(1).first else { return nil }
                    return OCRLine(
                        text: candidate.string,
                        bbox: obs.boundingBox,
                        confidence: candidate.confidence
                    )
                }
                continuation.resume(returning: lines)
            }
            request.recognitionLevel = .accurate
            request.usesLanguageCorrection = true

            let handler = VNImageRequestHandler(cgImage: cgImage, orientation: .up, options: [:])
            do {
                try handler.perform([request])
            } catch {
                continuation.resume(throwing: error)
            }
        }
    }
}
