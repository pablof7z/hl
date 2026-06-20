import AVFoundation
import CoreGraphics
import Foundation
import ImageIO
import UIKit
import Vision

/// Native capability executor for the Rust kernel (Phase 7 — Part A).
///
/// The kernel emits `CapabilityRequest`s via `HighlighterObserver`; this bridge
/// runs the raw OS capability and feeds the result back through
/// `HighlighterApp.provideCapabilityResult`. No business logic lives here — the
/// kernel decides what each result means and what happens next (D7). Native only
/// executes Vision / AVFoundation / App-Group I/O.
///
/// Capabilities handled:
/// - `.ocr`      → Vision `VNRecognizeTextRequest` over the image at `imageHandle`.
/// - `.audio`    → `AVPlayer` transport (load/play/pause/seek/stop/waveform) with
///                 bounded-cadence (~1 s) progress reporting (D8).
/// - `.share`    → App-Group drain / communities-snapshot write (Phase 5K).
/// - `.camera`   → page scan / barcode scan; requires a presenting screen, so it
///                 routes through `cameraPresenter` (registered by the capture
///                 screen in Part B). When no presenter is registered the bridge
///                 reports `.cancelled` — the safe, typed default (D6).
///
/// Keychain is fulfilled directly in `HighlighterAppKernel` (Phase 1) and is not
/// routed here.
@MainActor
final class KernelCapabilityBridge {

    /// Weak back-reference to the kernel handle so results can be returned.
    /// Set by `HighlighterAppKernel` right after it constructs the bridge.
    weak var app: HighlighterApp?

    /// Camera capture is a UI-presentation capability: it must push a
    /// `VNDocumentCameraViewController` / barcode scanner onto a live screen.
    /// The capture screen registers this presenter; the bridge invokes it and
    /// forwards the screen's result to the kernel. `nil` → `.cancelled`.
    var cameraPresenter: ((CameraOp) async -> CameraResult)?

    private let audio = AudioCapabilityPlayer()

    init() {}

    /// Entry point invoked from `HighlighterAppKernel.fulfill(request:)` for the
    /// non-keychain capabilities. Each branch returns its result asynchronously
    /// and calls `provideCapabilityResult` on completion.
    func fulfill(_ request: CapabilityRequest) {
        switch request {
        case .keychain:
            // Handled in HighlighterAppKernel (Phase 1). Never routed here.
            break
        case .ocr(let op):
            fulfillOcr(op)
        case .audio(let op):
            audio.execute(op) { [weak self] result in
                self?.app?.provideCapabilityResult(result: .audio(result))
            }
        case .share(let op):
            fulfillShare(op)
        case .camera(let op):
            fulfillCamera(op)
        }
    }

    // MARK: - OCR (Vision)

    private func fulfillOcr(_ op: OcrOp) {
        switch op {
        case .recognizeText(let imageHandle):
            Task { [weak self] in
                let result = await Self.recognize(imageHandle: imageHandle)
                self?.app?.provideCapabilityResult(result: .ocr(result))
            }
        }
    }

    /// Load the image at `imageHandle` (a `data_dir` temp path) and run Vision
    /// text recognition off the main thread. Returns raw line observations; the
    /// kernel reconstructs markdown and projects selectable words from them.
    private static func recognize(imageHandle: String) async -> OcrResult {
        guard let cgImage = loadCGImage(path: imageHandle) else {
            return .error("ocr: could not load image at handle")
        }
        let lines = await OCRService.recognizeLines(in: cgImage)
        return .lines(lines)
    }

    private static func loadCGImage(path: String) -> CGImage? {
        let url = URL(fileURLWithPath: path)
        guard let source = CGImageSourceCreateWithURL(url as CFURL, nil) else {
            return nil
        }
        return CGImageSourceCreateImageAtIndex(source, 0, nil)
    }

    // MARK: - Share (App Group)

    private func fulfillShare(_ op: ShareOp) {
        let result: ShareResult
        switch op {
        case .drainQueue:
            // Mirror the live-lane drain: read + delete the handoff file, map
            // each PendingShare into the kernel's raw payload (D1: raw strings).
            let pending = ShareQueue.drain()
            let payloads = pending.map { share in
                RawSharePayload(
                    id: share.id.uuidString,
                    groupId: share.groupId,
                    url: share.url,
                    note: share.note,
                    createdAtUnixSeconds: share.createdAt.timeIntervalSince1970
                )
            }
            result = .pending(payloads)
        case .writeCommunitiesSnapshot(let jsonBytes):
            // The kernel built the JSON; native only writes it atomically into
            // the App Group container the share extension reads at launch.
            SharedCommunitiesSnapshot.save(Data(jsonBytes))
            result = .communitiesWritten
        }
        app?.provideCapabilityResult(result: .share(result))
    }

    // MARK: - Camera (presentation capability)

    private func fulfillCamera(_ op: CameraOp) {
        guard let presenter = cameraPresenter else {
            // No live capture screen registered a presenter — treat as a user
            // cancellation (typed state, D6: errors/cancellations are data).
            app?.provideCapabilityResult(result: .camera(.cancelled))
            return
        }
        Task { [weak self] in
            let result = await presenter(op)
            self?.app?.provideCapabilityResult(result: .camera(result))
        }
    }
}

// MARK: - Audio capability player (AVPlayer)

/// Wraps a single `AVPlayer` to execute the kernel's audio transport ops and
/// report raw progress back at a **bounded cadence** (~1 s, D8). The kernel owns
/// all playback STATE, resume policy, seek bounds, and clip semantics; this
/// player is a pure capability executor.
@MainActor
private final class AudioCapabilityPlayer {
    private var player: AVPlayer?
    private var timeObserver: Any?
    private var endObserver: NSObjectProtocol?
    /// Callback used to deliver `Progress` / `Ended` updates that originate
    /// outside a direct op response (periodic ticks, natural end).
    private var report: ((AudioResult) -> Void)?

    func execute(_ op: AudioOp, completion: @escaping (AudioResult) -> Void) {
        // Retain the reporter so the periodic observer can push progress and the
        // end observer can push `.ended` between explicit ops.
        report = completion

        switch op {
        case .load(let url, let resumeAtSeconds):
            load(url: url, resumeAt: resumeAtSeconds, completion: completion)
        case .play:
            configureSession()
            player?.play()
            completion(progressResult())
        case .pause:
            player?.pause()
            completion(progressResult())
        case .seek(let seconds):
            seek(to: seconds, completion: completion)
        case .stop:
            teardown()
            completion(.progress(currentSeconds: 0, isPlaying: false))
        case .extractWaveform(let url, let bucketCount):
            Self.extractWaveform(url: url, bucketCount: bucketCount, completion: completion)
        }
    }

    private func load(url: String, resumeAt: Double?, completion: @escaping (AudioResult) -> Void) {
        guard let assetURL = URL(string: url) else {
            completion(.error("audio: invalid url"))
            return
        }
        teardown()
        configureSession()

        let item = AVPlayerItem(url: assetURL)
        let newPlayer = AVPlayer(playerItem: item)
        player = newPlayer

        // Bounded-cadence progress: one tick per ~1 s (D8 — not raw 0.25 s).
        let interval = CMTime(seconds: 1.0, preferredTimescale: 600)
        timeObserver = newPlayer.addPeriodicTimeObserver(
            forInterval: interval,
            queue: .main
        ) { [weak self] _ in
            guard let self else { return }
            self.report?(self.progressResult())
        }

        endObserver = NotificationCenter.default.addObserver(
            forName: AVPlayerItem.didPlayToEndTimeNotification,
            object: item,
            queue: .main
        ) { [weak self] _ in
            self?.report?(.ended)
        }

        // Report duration once the item is ready; seek to the resume point first.
        Task { [weak self] in
            let duration = (try? await item.asset.load(.duration)) ?? .zero
            let seconds = CMTimeGetSeconds(duration)
            let durationSeconds = seconds.isFinite && seconds > 0 ? seconds : 0
            if let resumeAt, resumeAt > 0 {
                await newPlayer.seek(
                    to: CMTime(seconds: resumeAt, preferredTimescale: 600),
                    toleranceBefore: .zero,
                    toleranceAfter: .zero
                )
            }
            self?.report?(.loaded(durationSeconds: durationSeconds))
            completion(.loaded(durationSeconds: durationSeconds))
        }
    }

    private func seek(to seconds: Double, completion: @escaping (AudioResult) -> Void) {
        guard let player else {
            completion(.error("audio: seek with no loaded item"))
            return
        }
        let target = CMTime(seconds: max(0, seconds), preferredTimescale: 600)
        player.seek(to: target, toleranceBefore: .zero, toleranceAfter: .zero) { [weak self] _ in
            guard let self else { return }
            completion(self.progressResult())
        }
    }

    private func progressResult() -> AudioResult {
        guard let player else {
            return .progress(currentSeconds: 0, isPlaying: false)
        }
        let current = CMTimeGetSeconds(player.currentTime())
        let seconds = current.isFinite && current >= 0 ? current : 0
        let isPlaying = player.timeControlStatus == .playing || player.rate > 0
        return .progress(currentSeconds: seconds, isPlaying: isPlaying)
    }

    private func configureSession() {
        let session = AVAudioSession.sharedInstance()
        try? session.setCategory(.playback, mode: .spokenAudio)
        try? session.setActive(true)
    }

    private func teardown() {
        if let timeObserver {
            player?.removeTimeObserver(timeObserver)
            self.timeObserver = nil
        }
        if let endObserver {
            NotificationCenter.default.removeObserver(endObserver)
            self.endObserver = nil
        }
        player?.pause()
        player = nil
    }

    /// Decode the audio at `url` and return `bucketCount` normalized amplitude
    /// buckets in `[0, 1]`. Runs off the main thread. Empty on failure (D6).
    private static func extractWaveform(
        url: String,
        bucketCount: UInt32,
        completion: @escaping (AudioResult) -> Void
    ) {
        guard let assetURL = URL(string: url) else {
            completion(.waveformPeaks(url: url, buckets: []))
            return
        }
        Task {
            let buckets = await WaveformExtractor.rawPeaks(
                forAudioURL: assetURL,
                bucketCount: Int(bucketCount)
            )
            completion(.waveformPeaks(url: url, buckets: buckets))
        }
    }
}
