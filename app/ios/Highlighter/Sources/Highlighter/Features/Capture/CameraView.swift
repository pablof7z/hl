import AVFoundation
import SwiftUI
import UIKit
import Vision

struct CameraView: UIViewControllerRepresentable {
    enum Result {
        case captured(UIImage)
        case cancelled
    }

    let onResult: @MainActor (Result) -> Void

    func makeUIViewController(context: Context) -> DocumentCameraVC {
        DocumentCameraVC(onResult: onResult)
    }

    func updateUIViewController(_ uiViewController: DocumentCameraVC, context: Context) {}
    func makeCoordinator() -> Coordinator { Coordinator() }
    final class Coordinator: NSObject {}
}

// MARK: - DocumentCameraVC

@MainActor
final class DocumentCameraVC: UIViewController {

    private let onResult: @MainActor (CameraView.Result) -> Void

    private let session = AVCaptureSession()
    private let photoOutput = AVCapturePhotoOutput()
    private let sessionQueue = DispatchQueue(label: "com.hl.cam.session", qos: .userInitiated)

    // Delegates (retain them as properties to prevent dealloc)
    private var videoDelegate: VideoAnalysisDelegate?
    private var captureDelegate: PhotoCaptureDelegate?

    // UI
    private let shutterBar = UIView()
    private var shutterHeight: NSLayoutConstraint?

    init(onResult: @escaping @MainActor (CameraView.Result) -> Void) {
        self.onResult = onResult
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { fatalError() }

    // MARK: - Lifecycle

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        configureSession()
        buildUI()
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        sessionQueue.async { if !self.session.isRunning { self.session.startRunning() } }
    }

    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        sessionQueue.async { if self.session.isRunning { self.session.stopRunning() } }
    }

    // MARK: - Session

    private func configureSession() {
        guard
            let device = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .back),
            let input = try? AVCaptureDeviceInput(device: device)
        else {
            onResult(.cancelled)
            return
        }

        let vid = VideoAnalysisDelegate()
        vid.vc = self
        videoDelegate = vid

        let videoOutput = AVCaptureVideoDataOutput()
        let analysisQueue = DispatchQueue(label: "com.hl.cam.analysis", qos: .default)
        videoOutput.setSampleBufferDelegate(vid, queue: analysisQueue)
        videoOutput.alwaysDiscardsLateVideoFrames = true

        sessionQueue.async { [session = self.session, photoOutput = self.photoOutput] in
            session.beginConfiguration()
            session.sessionPreset = .photo
            if session.canAddInput(input) { session.addInput(input) }
            if session.canAddOutput(videoOutput) { session.addOutput(videoOutput) }
            if session.canAddOutput(photoOutput) { session.addOutput(photoOutput) }
            session.commitConfiguration()
        }

        let preview = AVCaptureVideoPreviewLayer(session: session)
        preview.videoGravity = .resizeAspectFill
        preview.frame = view.bounds
        view.layer.addSublayer(preview)
    }

    // MARK: - UI

    private func buildUI() {
        let cancelBtn = UIButton(type: .system)
        cancelBtn.setImage(
            UIImage(systemName: "xmark",
                    withConfiguration: UIImage.SymbolConfiguration(pointSize: 16, weight: .medium)),
            for: .normal
        )
        cancelBtn.tintColor = .white
        cancelBtn.backgroundColor = UIColor(white: 1, alpha: 0.2)
        cancelBtn.layer.cornerRadius = 20
        cancelBtn.addTarget(self, action: #selector(cancelTapped), for: .touchUpInside)
        cancelBtn.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(cancelBtn)

        shutterBar.backgroundColor = UIColor(white: 1, alpha: 0.45)
        shutterBar.layer.cornerRadius = 4
        shutterBar.translatesAutoresizingMaskIntoConstraints = false
        shutterBar.isUserInteractionEnabled = true
        shutterBar.addGestureRecognizer(
            UITapGestureRecognizer(target: self, action: #selector(shutterTapped))
        )
        view.addSubview(shutterBar)

        let h = shutterBar.heightAnchor.constraint(equalToConstant: 8)
        shutterHeight = h

        let hint = UILabel()
        hint.text = "Point at a page"
        hint.font = .systemFont(ofSize: 13, weight: .medium)
        hint.textColor = UIColor(white: 1, alpha: 0.55)
        hint.textAlignment = .center
        hint.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(hint)

        NSLayoutConstraint.activate([
            cancelBtn.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 16),
            cancelBtn.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 20),
            cancelBtn.widthAnchor.constraint(equalToConstant: 40),
            cancelBtn.heightAnchor.constraint(equalToConstant: 40),

            shutterBar.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor, constant: -32),
            shutterBar.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            shutterBar.widthAnchor.constraint(equalToConstant: 140),
            h,

            hint.bottomAnchor.constraint(equalTo: shutterBar.topAnchor, constant: -14),
            hint.centerXAnchor.constraint(equalTo: view.centerXAnchor)
        ])
    }

    func applyShutterStyle(stable: Bool) {
        let targetH: CGFloat = stable ? 14 : 8
        let targetAlpha: CGFloat = stable ? 1.0 : 0.45
        UIView.animate(withDuration: 0.25, delay: 0,
                       usingSpringWithDamping: 0.7, initialSpringVelocity: 0) {
            self.shutterHeight?.constant = targetH
            self.shutterBar.layer.cornerRadius = targetH / 2
            self.shutterBar.alpha = targetAlpha
            self.view.layoutIfNeeded()
        }
        if stable { UIImpactFeedbackGenerator(style: .light).impactOccurred() }
    }

    // MARK: - Shutter action

    @objc private func shutterTapped() {
        UIView.animate(withDuration: 0.08, animations: {
            self.shutterBar.transform = CGAffineTransform(scaleX: 1.3, y: 2)
            self.shutterBar.alpha = 1
        }) { _ in
            UIView.animate(withDuration: 0.2) { self.shutterBar.transform = .identity }
        }

        let capture = PhotoCaptureDelegate()
        capture.vc = self
        captureDelegate = capture

        let settings = AVCapturePhotoSettings()
        settings.flashMode = .off
        photoOutput.capturePhoto(with: settings, delegate: capture)
    }

    @objc private func cancelTapped() { onResult(.cancelled) }

    func handlePhotoResult(_ result: Swift.Result<UIImage, Error>) {
        switch result {
        case .success(let image): onResult(.captured(image))
        case .failure: onResult(.cancelled)
        }
        captureDelegate = nil
    }
}

// MARK: - VideoAnalysisDelegate

private final class VideoAnalysisDelegate: NSObject, @unchecked Sendable,
                                            AVCaptureVideoDataOutputSampleBufferDelegate {
    weak var vc: DocumentCameraVC?

    private var frameCount = 0
    private var lastRect: VNRectangleObservation?
    private var stableCount = 0
    private var isStable = false

    func captureOutput(
        _ output: AVCaptureOutput,
        didOutput sampleBuffer: CMSampleBuffer,
        from connection: AVCaptureConnection
    ) {
        frameCount += 1
        guard frameCount % 6 == 0,
              let pixelBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }

        let req = VNDetectRectanglesRequest()
        req.maximumObservations = 1
        req.minimumConfidence = 0.55
        req.minimumAspectRatio = 0.25
        req.maximumAspectRatio = 1.0
        try? VNImageRequestHandler(cvPixelBuffer: pixelBuffer, options: [:]).perform([req])

        let best = req.results?.first as? VNRectangleObservation
        updateStability(detection: best)
    }

    private func updateStability(detection: VNRectangleObservation?) {
        if let d = detection, let prev = lastRect, rectsSimilar(d, prev) {
            stableCount = min(stableCount + 1, 20)
        } else {
            stableCount = max(0, stableCount - 2)
        }
        lastRect = detection
        let nowStable = detection != nil && stableCount >= 10
        guard nowStable != isStable else { return }
        isStable = nowStable
        let stable = nowStable
        Task { @MainActor [weak vc] in vc?.applyShutterStyle(stable: stable) }
    }

    private func rectsSimilar(
        _ a: VNRectangleObservation, _ b: VNRectangleObservation, tol: CGFloat = 0.03
    ) -> Bool {
        abs(a.topLeft.x - b.topLeft.x) < tol && abs(a.topLeft.y - b.topLeft.y) < tol &&
        abs(a.bottomRight.x - b.bottomRight.x) < tol && abs(a.bottomRight.y - b.bottomRight.y) < tol
    }
}

// MARK: - PhotoCaptureDelegate

private final class PhotoCaptureDelegate: NSObject, @unchecked Sendable, AVCapturePhotoCaptureDelegate {
    weak var vc: DocumentCameraVC?

    func photoOutput(
        _ output: AVCapturePhotoOutput,
        didFinishProcessingPhoto photo: AVCapturePhoto,
        error: Error?
    ) {
        if let error {
            Task { @MainActor [weak vc] in vc?.handlePhotoResult(.failure(error)) }
            return
        }
        guard let data = photo.fileDataRepresentation(),
              let raw = UIImage(data: data) else {
            Task { @MainActor [weak vc] in vc?.handlePhotoResult(.failure(CaptureError.noImageData)) }
            return
        }
        let corrected = perspectiveCorrected(raw) ?? raw
        Task { @MainActor [weak vc] in vc?.handlePhotoResult(.success(corrected)) }
    }
}

private enum CaptureError: Error { case noImageData }

// MARK: - Perspective correction

private func perspectiveCorrected(_ image: UIImage) -> UIImage? {
    guard let cgImage = image.cgImage else { return nil }
    let orientation = CGImagePropertyOrientation(image.imageOrientation)

    let req = VNDetectRectanglesRequest()
    req.maximumObservations = 1
    req.minimumConfidence = 0.5
    req.minimumAspectRatio = 0.25
    let handler = VNImageRequestHandler(cgImage: cgImage, orientation: orientation, options: [:])
    try? handler.perform([req])
    guard let obs = req.results?.first as? VNRectangleObservation else { return nil }

    guard let ci = CIImage(image: image) else { return nil }
    let uprighted = ci.oriented(forExifOrientation: Int32(orientation.rawValue))
    let size = uprighted.extent.size

    func v(_ pt: CGPoint) -> CIVector { CIVector(x: pt.x * size.width, y: pt.y * size.height) }

    guard let filter = CIFilter(name: "CIPerspectiveCorrection") else { return nil }
    filter.setValue(uprighted,       forKey: kCIInputImageKey)
    filter.setValue(v(obs.topLeft),  forKey: "inputTopLeft")
    filter.setValue(v(obs.topRight), forKey: "inputTopRight")
    filter.setValue(v(obs.bottomLeft),  forKey: "inputBottomLeft")
    filter.setValue(v(obs.bottomRight), forKey: "inputBottomRight")

    guard let out = filter.outputImage else { return nil }
    let ctx = CIContext()
    guard let cg = ctx.createCGImage(out, from: out.extent) else { return nil }
    return UIImage(cgImage: cg, scale: image.scale, orientation: .up)
}

// MARK: - UIImage.imageOrientation → CGImagePropertyOrientation

private extension CGImagePropertyOrientation {
    init(_ ui: UIImage.Orientation) {
        switch ui {
        case .up:            self = .up
        case .upMirrored:    self = .upMirrored
        case .down:          self = .down
        case .downMirrored:  self = .downMirrored
        case .left:          self = .left
        case .leftMirrored:  self = .leftMirrored
        case .right:         self = .right
        case .rightMirrored: self = .rightMirrored
        @unknown default:    self = .up
        }
    }
}
