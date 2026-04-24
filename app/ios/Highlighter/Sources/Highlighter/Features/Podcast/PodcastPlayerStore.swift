import AVFoundation
import Foundation
import Observation
import os

@MainActor
@Observable
final class PodcastPlayerStore {
    // MARK: - Observable state

    private(set) var audioUrl: URL?
    private(set) var currentTime: TimeInterval = 0
    private(set) var duration: TimeInterval = 0
    private(set) var isPlaying: Bool = false
    private(set) var isBuffering: Bool = false
    private(set) var loadedTimeRanges: [ClosedRange<TimeInterval>] = []
    private(set) var lastError: String?
    private(set) var clipStart: TimeInterval?
    private(set) var clipEnd: TimeInterval?
    var speaker: String = ""
    private(set) var selectedSegmentIds: Set<String> = []
    private(set) var isPublishing: Bool = false
    private(set) var publishError: String?

    // MARK: - Private plumbing

    @ObservationIgnored private var player: AVPlayer?
    @ObservationIgnored private let logger = Logger(subsystem: "com.highlighter.app", category: "PodcastPlayer")
    // `nonisolated(unsafe)` lets deinit tear down observer tokens without a MainActor hop.
    @ObservationIgnored private nonisolated(unsafe) var timeObserver: Any?
    @ObservationIgnored private nonisolated(unsafe) var statusObserver: NSKeyValueObservation?
    @ObservationIgnored private nonisolated(unsafe) var bufferingObserver: NSKeyValueObservation?
    @ObservationIgnored private nonisolated(unsafe) var rangesObserver: NSKeyValueObservation?
    @ObservationIgnored private nonisolated(unsafe) var errorObserver: NSKeyValueObservation?
    @ObservationIgnored private nonisolated(unsafe) var playbackEndObserver: NSObjectProtocol?

    // MARK: - Lifecycle

    deinit {
        if let player, let timeObserver {
            player.removeTimeObserver(timeObserver)
        }
        statusObserver?.invalidate()
        bufferingObserver?.invalidate()
        rangesObserver?.invalidate()
        errorObserver?.invalidate()
        if let playbackEndObserver {
            NotificationCenter.default.removeObserver(playbackEndObserver)
        }
        player?.pause()
    }

    // MARK: - Loading

    func load(url: URL) {
        guard audioUrl != url else { return }
        audioUrl = url
        lastError = nil
        isBuffering = false
        loadedTimeRanges = []
        logger.info("load url=\(url.absoluteString, privacy: .public)")

        try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .spokenAudio)
        try? AVAudioSession.sharedInstance().setActive(true)

        let item = AVPlayerItem(url: url)
        // Prefer smaller forward buffer to start playback sooner on slow connections.
        item.preferredForwardBufferDuration = 10

        let newPlayer = AVPlayer(playerItem: item)
        newPlayer.automaticallyWaitsToMinimizeStalling = true
        self.player = newPlayer

        installTimeObserver(on: newPlayer)
        observeItem(item)
        observeBuffering(item)
        observeLoadedRanges(item)
        observeError(item)
        observePlaybackEnd(item: item)
    }

    private func installTimeObserver(on player: AVPlayer) {
        let interval = CMTime(seconds: 0.25, preferredTimescale: 600)
        timeObserver = player.addPeriodicTimeObserver(forInterval: interval, queue: .main) { [weak self] time in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.currentTime = time.seconds.isFinite ? time.seconds : 0
            }
        }
    }

    private func observeItem(_ item: AVPlayerItem) {
        statusObserver = item.observe(\.status, options: [.initial, .new]) { [weak self, weak item] _, _ in
            guard let self, let item else { return }
            Task { @MainActor in
                let status = item.status
                self.logger.info("item status=\(status.rawValue)")
                guard status == .readyToPlay else { return }
                do {
                    let loaded = try await item.asset.load(.duration)
                    let seconds = loaded.seconds
                    if seconds.isFinite, seconds > 0 {
                        self.duration = seconds
                        self.logger.info("duration=\(seconds, format: .fixed(precision: 1))s")
                    }
                } catch {
                    self.logger.error("duration load failed: \(error.localizedDescription, privacy: .public)")
                }
            }
        }
    }

    private func observeBuffering(_ item: AVPlayerItem) {
        bufferingObserver = item.observe(
            \.isPlaybackLikelyToKeepUp,
            options: [.initial, .new]
        ) { [weak self, weak item] _, _ in
            guard let self, let item else { return }
            Task { @MainActor in
                let likelyToKeepUp = item.isPlaybackLikelyToKeepUp
                let bufferEmpty = item.isPlaybackBufferEmpty
                let newBuffering = !likelyToKeepUp && !bufferEmpty
                if self.isBuffering != newBuffering {
                    self.logger.info("buffering=\(newBuffering) likelyToKeepUp=\(likelyToKeepUp) bufferEmpty=\(bufferEmpty)")
                    self.isBuffering = newBuffering
                }
            }
        }
    }

    private func observeLoadedRanges(_ item: AVPlayerItem) {
        rangesObserver = item.observe(
            \.loadedTimeRanges,
            options: [.initial, .new]
        ) { [weak self, weak item] _, _ in
            guard let self, let item else { return }
            let ranges = item.loadedTimeRanges.compactMap { value -> ClosedRange<TimeInterval>? in
                let range = value.timeRangeValue
                let start = range.start.seconds
                let end = CMTimeRangeGetEnd(range).seconds
                guard start.isFinite, end.isFinite, end > start else { return nil }
                return start...end
            }
            Task { @MainActor in
                self.loadedTimeRanges = ranges
            }
        }
    }

    private func observeError(_ item: AVPlayerItem) {
        errorObserver = item.observe(\.error, options: [.new]) { [weak self, weak item] _, _ in
            guard let self, let item else { return }
            Task { @MainActor in
                if let error = item.error {
                    let msg = error.localizedDescription
                    self.logger.error("playback error: \(msg, privacy: .public)")
                    self.lastError = msg
                    self.isPlaying = false
                }
            }
        }
    }

    private func observePlaybackEnd(item: AVPlayerItem) {
        playbackEndObserver = NotificationCenter.default.addObserver(
            forName: .AVPlayerItemDidPlayToEndTime,
            object: item,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.isPlaying = false
            }
        }
    }

    // MARK: - Transport

    func play() {
        logger.info("play")
        player?.play()
        isPlaying = true
    }

    func pause() {
        logger.info("pause")
        player?.pause()
        isPlaying = false
    }

    func toggle() {
        if isPlaying { pause() } else { play() }
    }

    func seek(to seconds: TimeInterval) {
        let clamped = max(0, duration > 0 ? min(seconds, duration) : seconds)
        let time = CMTime(seconds: clamped, preferredTimescale: 600)
        player?.seek(to: time, toleranceBefore: .zero, toleranceAfter: .zero)
        currentTime = clamped
    }

    func skip(by delta: TimeInterval) {
        seek(to: currentTime + delta)
    }

    // MARK: - Clip selection

    func markIn() {
        clipStart = currentTime
        if let end = clipEnd, end < currentTime { clipEnd = nil }
    }

    func markOut() {
        clipEnd = currentTime
        if let start = clipStart, start > currentTime { clipStart = nil }
    }

    func clearClip() {
        clipStart = nil
        clipEnd = nil
        selectedSegmentIds.removeAll()
        speaker = ""
    }

    func extendClipToSegment(_ segment: TranscriptSegment) {
        let start = clipStart.map { min($0, segment.start) } ?? segment.start
        let end = clipEnd.map { max($0, segment.end) } ?? segment.end
        clipStart = start
        clipEnd = end
        selectedSegmentIds.insert(segment.id)
        if speaker.isEmpty, !segment.speaker.isEmpty {
            speaker = segment.speaker
        }
    }

    func setClipStart(_ value: TimeInterval) {
        var next = max(0, value)
        if let end = clipEnd { next = min(next, max(0, end - 0.05)) }
        clipStart = next
    }

    func setClipEnd(_ value: TimeInterval) {
        var next = duration > 0 ? min(value, duration) : value
        if let start = clipStart { next = max(next, start + 0.05) }
        clipEnd = next
    }

    // MARK: - Publish

    func publish(
        artifact: ArtifactRecord,
        targetGroupId: String,
        note: String,
        segments: [TranscriptSegment],
        core: SafeHighlighterCore
    ) async throws -> HighlightRecord {
        isPublishing = true
        publishError = nil
        defer { isPublishing = false }

        let selected = segments
            .filter { selectedSegmentIds.contains($0.id) }
            .sorted { $0.start < $1.start }
        let quote = selected.map(\.text).joined(separator: " ")

        let draft = HighlightDraft(
            quote: quote,
            context: "",
            note: note,
            clipStartSeconds: clipStart,
            clipEndSeconds: clipEnd,
            clipSpeaker: speaker,
            clipTranscriptSegmentIds: Array(selectedSegmentIds),
            image: nil
        )

        do {
            let results = try await core.publishHighlightsAndShare(
                artifact: artifact,
                drafts: [draft],
                targetGroupId: targetGroupId
            )
            guard let first = results.first else {
                throw PodcastPlayerError.emptyResult
            }
            return first
        } catch {
            publishError = "\(error)"
            throw error
        }
    }
}

enum PodcastPlayerError: Error, LocalizedError {
    case emptyResult

    var errorDescription: String? {
        switch self {
        case .emptyResult: return "No highlight returned from publish."
        }
    }
}
