import AVFoundation
import Foundation
import Observation

/// View-scoped audio-player state for a single podcast artifact. Wraps
/// `AVPlayer`, drives scrub / play-pause / clip-range selection, and
/// publishes via the shared `SafeHighlighterCore` when the user saves.
@MainActor
@Observable
final class PodcastPlayerStore {
    // MARK: - Observable state

    private(set) var audioUrl: URL?
    private(set) var currentTime: TimeInterval = 0
    private(set) var duration: TimeInterval = 0
    private(set) var isPlaying: Bool = false
    private(set) var clipStart: TimeInterval?
    private(set) var clipEnd: TimeInterval?
    var speaker: String = ""
    private(set) var selectedSegmentIds: Set<String> = []
    private(set) var isPublishing: Bool = false
    private(set) var publishError: String?

    // MARK: - Private plumbing

    @ObservationIgnored private var player: AVPlayer?
    // `nonisolated(unsafe)` is required for the three observer tokens so that
    // `deinit` (which Swift 6 treats as nonisolated) can tear them down. The
    // stored values only move between MainActor-isolated setters and the
    // one-shot nonisolated `deinit` read — no concurrent access.
    @ObservationIgnored private nonisolated(unsafe) var timeObserver: Any?
    @ObservationIgnored private nonisolated(unsafe) var statusObserver: NSKeyValueObservation?
    @ObservationIgnored private nonisolated(unsafe) var playbackEndObserver: NSObjectProtocol?

    // MARK: - Lifecycle

    deinit {
        // We can't hop to MainActor from deinit — just detach the observer
        // tokens and pause the player. All of these APIs are safe off-main.
        if let player, let timeObserver {
            player.removeTimeObserver(timeObserver)
        }
        statusObserver?.invalidate()
        if let playbackEndObserver {
            NotificationCenter.default.removeObserver(playbackEndObserver)
        }
        player?.pause()
    }

    // MARK: - Loading

    func load(url: URL) {
        guard audioUrl != url else { return }
        audioUrl = url

        // Activate the playback session so we play even with the ring switch off.
        try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .spokenAudio)
        try? AVAudioSession.sharedInstance().setActive(true)

        let item = AVPlayerItem(url: url)
        let newPlayer = AVPlayer(playerItem: item)
        self.player = newPlayer

        installTimeObserver(on: newPlayer)
        observeItem(item)
        observePlaybackEnd(item: item)
    }

    private func installTimeObserver(on player: AVPlayer) {
        let interval = CMTime(seconds: 0.25, preferredTimescale: 600)
        timeObserver = player.addPeriodicTimeObserver(forInterval: interval, queue: .main) { [weak self] time in
            // `addPeriodicTimeObserver` fires on the main queue when we pass
            // `.main` above; hop to the main actor to mutate state safely.
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
                guard item.status == .readyToPlay else { return }
                do {
                    let loaded = try await item.asset.load(.duration)
                    let seconds = loaded.seconds
                    if seconds.isFinite, seconds > 0 {
                        self.duration = seconds
                    }
                } catch {
                    // Duration unavailable — leave at 0; UI renders without a scrub range.
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
        player?.play()
        isPlaying = true
    }

    func pause() {
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

    /// Direct updates from drag gestures on the timeline; also called by
    /// `ClipTimelineView` when the user drags a clip thumb.
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

        // Concatenate the selected segments in chronological order for the
        // highlight body. If no transcript segments are selected, the body
        // is empty — clip bounds alone convey the highlight.
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
            clipTranscriptSegmentIds: Array(selectedSegmentIds)
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
