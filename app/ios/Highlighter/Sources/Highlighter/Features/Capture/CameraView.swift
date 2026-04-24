import SwiftUI
import UIKit

/// Thin SwiftUI wrapper around `UIImagePickerController` for the camera.
/// We use the system camera for v1: shutter, retake, and use-photo are
/// already polished, and there's nothing OCR-specific that benefits from
/// a custom AVCapture overlay yet.
struct CameraView: UIViewControllerRepresentable {
    enum Result {
        case captured(UIImage)
        case cancelled
    }

    let onResult: (Result) -> Void

    func makeUIViewController(context: Context) -> UIImagePickerController {
        let picker = UIImagePickerController()
        picker.sourceType = UIImagePickerController.isSourceTypeAvailable(.camera)
            ? .camera : .photoLibrary
        picker.allowsEditing = false
        picker.cameraCaptureMode = .photo
        picker.delegate = context.coordinator
        return picker
    }

    func updateUIViewController(_ uiViewController: UIImagePickerController, context: Context) {}

    func makeCoordinator() -> Coordinator {
        Coordinator(onResult: onResult)
    }

    final class Coordinator: NSObject, UIImagePickerControllerDelegate, UINavigationControllerDelegate {
        let onResult: (Result) -> Void

        init(onResult: @escaping (Result) -> Void) {
            self.onResult = onResult
        }

        func imagePickerController(
            _ picker: UIImagePickerController,
            didFinishPickingMediaWithInfo info: [UIImagePickerController.InfoKey: Any]
        ) {
            if let image = info[.originalImage] as? UIImage {
                onResult(.captured(image))
            } else {
                onResult(.cancelled)
            }
        }

        func imagePickerControllerDidCancel(_ picker: UIImagePickerController) {
            onResult(.cancelled)
        }
    }
}
