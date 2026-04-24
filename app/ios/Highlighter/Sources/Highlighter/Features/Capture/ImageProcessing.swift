import CoreGraphics
import ImageIO
import UIKit
import UniformTypeIdentifiers

enum ImageProcessing {
    struct Result {
        let data: Data
        let width: Int
        let height: Int
        let mime: String
    }

    enum Error: Swift.Error, LocalizedError {
        case noCGImage
        case encodingFailed

        var errorDescription: String? {
            switch self {
            case .noCGImage: return "Couldn't read the captured image."
            case .encodingFailed: return "Couldn't prepare the image for upload."
            }
        }
    }

    /// Re-encode `image` as JPEG, scaling its long edge to at most `maxEdge`
    /// and stripping all metadata (EXIF, GPS, TIFF, IPTC). The output is safe
    /// to upload publicly without leaking the user's location.
    static func stripMetadataAndEncode(
        _ image: UIImage,
        maxEdge: CGFloat = 2048,
        quality: CGFloat = 0.85
    ) throws -> Result {
        let scaled = image.resizedRespectingOrientation(maxEdge: maxEdge)
        guard let cgImage = scaled.cgImage else {
            throw Error.noCGImage
        }

        let buffer = NSMutableData()
        let type = UTType.jpeg.identifier as CFString
        guard let destination = CGImageDestinationCreateWithData(
            buffer as CFMutableData,
            type,
            1,
            nil
        ) else {
            throw Error.encodingFailed
        }

        // Pass an empty properties dictionary (plus quality) so the destination
        // does NOT copy the source's metadata. The kCGImageMetadata* keys are
        // the ones that would carry GPS/EXIF; omitting them is the strip.
        let properties: [CFString: Any] = [
            kCGImageDestinationLossyCompressionQuality: quality
        ]
        CGImageDestinationAddImage(destination, cgImage, properties as CFDictionary)
        guard CGImageDestinationFinalize(destination) else {
            throw Error.encodingFailed
        }

        return Result(
            data: buffer as Data,
            width: cgImage.width,
            height: cgImage.height,
            mime: "image/jpeg"
        )
    }
}

private extension UIImage {
    /// Resize to fit `maxEdge` on the long side, preserving aspect ratio.
    /// Bakes the orientation into the output pixels so downstream code that
    /// reads `cgImage.width/height` sees the correct dimensions.
    func resizedRespectingOrientation(maxEdge: CGFloat) -> UIImage {
        let pixelSize = CGSize(
            width: size.width * scale,
            height: size.height * scale
        )
        let longest = max(pixelSize.width, pixelSize.height)
        let scaleFactor = longest > maxEdge ? maxEdge / longest : 1.0
        let target = CGSize(
            width: pixelSize.width * scaleFactor,
            height: pixelSize.height * scaleFactor
        )

        let format = UIGraphicsImageRendererFormat.default()
        format.scale = 1.0 // we're already in pixel space
        format.opaque = true
        let renderer = UIGraphicsImageRenderer(size: target, format: format)
        return renderer.image { _ in
            draw(in: CGRect(origin: .zero, size: target))
        }
    }
}
