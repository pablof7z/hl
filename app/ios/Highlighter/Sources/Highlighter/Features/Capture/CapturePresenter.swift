import SwiftUI
import UIKit

/// Bridges the kernel's `CapabilityRequest::Camera` to native SwiftUI camera
/// presentation (Phase 7 Capture cutover).
///
/// The kernel drives capture: `hl.camera.capture_page` / `hl.camera.scan_barcode`
/// emit a `CameraOp`; this presenter (registered on the `KernelCapabilityBridge`)
/// presents `CameraView` (document scan) or `BookScannerView` (barcode) over the
/// key window, then returns a `CameraResult` the kernel routes onward (page image
/// → OCR; barcode → ISBN lookup).
///
/// Per Q1 (image-ownership): NATIVE owns all pixel work. For `CapturePage` the
/// presenter strips EXIF + encodes the captured `UIImage` and writes the JPEG to
/// a `data_dir` temp handle — NO raw bytes cross the FFI boundary, only the path.
@MainActor
enum CapturePresenter {
    /// The kernel's `data_dir` (application support). Capability handles are
    /// written here so the OCR executor (which loads by path) can read them.
    static var dataDir: URL {
        FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSTemporaryDirectory())
    }

    /// Present the camera for `op` and return the raw `CameraResult`. Invoked by
    /// `KernelCapabilityBridge.cameraPresenter`.
    static func present(_ op: CameraOp) async -> CameraResult {
        switch op {
        case .capturePage:
            return await presentDocumentScanner()
        case .scanBarcode:
            return await presentBarcodeScanner()
        }
    }

    // MARK: - Page (document) scan → OCR pipeline

    private static func presentDocumentScanner() async -> CameraResult {
        let image: UIImage? = await withCheckedContinuation { continuation in
            let view = CameraView { result in
                switch result {
                case .captured(let image):
                    continuation.resume(returning: image)
                case .cancelled:
                    continuation.resume(returning: nil)
                }
                dismissTop()
            }
            present(view)
        }

        guard let image else { return .cancelled }

        // Native pixel work (Q1): strip EXIF + encode, then write the handle.
        guard let processed = ImageProcessing.stripMetadataAndEncode(image) else {
            return .error("capture: could not encode image")
        }
        guard let handle = writeHandle(processed.data, ext: "jpg") else {
            return .error("capture: could not write image handle")
        }
        return .pageImage(
            imageHandle: handle,
            width: UInt32(processed.width),
            height: UInt32(processed.height)
        )
    }

    // MARK: - Barcode scan → ISBN

    private static func presentBarcodeScanner() async -> CameraResult {
        let isbn: String? = await withCheckedContinuation { continuation in
            let view = BookScannerView { result in
                continuation.resume(returning: result)
                dismissTop()
            }
            present(view)
        }
        guard let isbn else { return .cancelled }
        return .barcode(rawString: isbn)
    }

    // MARK: - Handle writing (native owns the bytes; FFI carries the path)

    /// Write `data` to a unique `data_dir` temp file and return the path handle.
    static func writeHandle(_ data: Data, ext: String) -> String? {
        let url = dataDir.appendingPathComponent("capture-\(UUID().uuidString).\(ext)")
        do {
            try FileManager.default.createDirectory(
                at: dataDir, withIntermediateDirectories: true
            )
            try data.write(to: url, options: [.atomic])
            return url.path
        } catch {
            return nil
        }
    }

    // MARK: - UIKit presentation plumbing

    private static func present<V: View>(_ view: V) {
        guard let top = topViewController() else { return }
        let host = UIHostingController(rootView: view.ignoresSafeArea())
        host.modalPresentationStyle = .fullScreen
        top.present(host, animated: true)
    }

    private static func dismissTop() {
        topViewController()?.dismiss(animated: true)
    }

    private static func topViewController() -> UIViewController? {
        let scene = UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .first { $0.activationState == .foregroundActive }
        guard let root = scene?.keyWindow?.rootViewController else { return nil }
        var top = root
        while let presented = top.presentedViewController {
            top = presented
        }
        return top
    }
}
