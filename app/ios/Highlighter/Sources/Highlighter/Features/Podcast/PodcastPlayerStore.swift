import Foundation
import MediaPlayer
import Observation
import os
import UIKit

@MainActor
@Observable
final class PodcastPlayerStore {
    // MARK: - Observable state

    private(set) var currentArtifact: ArtifactRecord?
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
    private(set) var selectedSegmentIds: [String] = []
    private(set) var isPublishing: Bool = false
    private(set) var publishError: String?

    // Global transcript state
    private(set) var transcriptSegments: [TranscriptSegment] = []
    private(set) var transcriptAvailability: PodcastTranscriptAvailability = .unavailable

    // Clip comment cache keyed by clip event id
    var comments: [String: [CommentRecord]] = [:]

    // Apple Music–style: only one clip expanded at a time
    var expandedClipId: String? = nil

    /// One-peak-per-second amplitude envelope (0...1) for the loaded episode.
    /// Empty until extraction completes; nil after extraction was attempted
    /// but skipped (cellular, format unsupported, etc.). Used by the
    /// listening view's tick rows to show a real waveform instead of a
    /// placeholder.
    private(set) var waveformPeaks: [Float] = []

    // MARK: - Private plumbing

    @ObservationIgnored private let core: HighlighterCore
    /// Weak reference to the kernel; set by `AppEntry` after both objects are
    /// initialised. Used to dispatch audio actions (play/pause/seek/resume).
    @ObservationIgnored weak var kernel: HighlighterAppKernel?
    @ObservationIgnored private let logger = Logger(subsystem: "com.highlighter.app", category: "PodcastPlayer")
    @ObservationIgnored private var transcriptTask: Task<Void, Never>?
    @ObservationIgnored private var waveformTask: Task<Void, Never>?

    // MARK: - Lifecycle

    init(core: HighlighterCore) {
        self.core = core
    }

    deinit {
        transcriptTask?.cancel()
        waveformTask?.cancel()
    }

    // MARK: - Kernel snapshot ingestion

    /// Called by `HighlighterAppKernel` whenever the podcast-listening snapshot
    /// changes. Updates observable state so SwiftUI views react automatically.
    func receiveListeningSnapshot(_ snapshot: PodcastListeningSnapshot) {
        currentTime = snapshot.positionSeconds
        duration = snapshot.durationSeconds
        isPlaying = snapshot.isPlaying
        updateNowPlayingInfo()
    }

    // MARK: - Global load / clear

    func load(artifact: ArtifactRecord) {
        let url = artifact.preview.audioUrl
        guard !url.isEmpty else {
            logger.warning("load: no audio URL for artifact \(artifact.shareEventId, privacy: .public)")
            return
        }

        // Reset local state for the new episode.
        currentArtifact = artifact
        audioUrl = URL(string: url)
        lastError = nil
        isBuffering = false
        loadedTimeRanges = []
        transcriptSegments = []
        transcriptAvailability = .unavailable
        clearClip()
        publishError = nil
        currentTime = 0
        duration = 0

        logger.info("load artifact=\(artifact.shareEventId, privacy: .public) url=\(url, privacy: .public)")

        // Dispatch to the kernel — the capability bridge (AudioCapabilityPlayer)
        // owns the AVPlayer from here on. The kernel will seek to the saved
        // resume position and begin playback automatically.
        let artifactJson = captureArtifactRecordJson(artifact: artifact)
        kernel?.app.dispatch(.audioPlay(
            url: url,
            guid: artifact.preview.podcastItemGuid,
            artifactJson: artifactJson,
            resumePositionSeconds: nil
        ))

        configureRemoteCommandCenter()
        updateNowPlayingInfo()
        fetchAndApplyArtwork(from: artifact.preview.image)

        let transcriptUrl = artifact.preview.transcriptUrl
        if !transcriptUrl.isEmpty {
            transcriptAvailability = .loading
            transcriptTask?.cancel()
            transcriptTask = Task { await loadTranscript(from: transcriptUrl) }
        }

        // Background: extract or load-from-cache the audio waveform. The
        // listening view falls back to plain time pegs when peaks aren't
        // present, so playback isn't blocked by this work.
        waveformPeaks = []
        waveformTask?.cancel()
        let durationHint = Double(artifact.preview.durationSeconds ?? 0)
        guard let audioURL = URL(string: url) else { return }
        waveformTask = Task(priority: .background) { [weak self, audioURL, core] in
            let peaks = await WaveformExtractor.peaks(
                forAudioURL: audioURL,
                durationSeconds: durationHint,
                core: core
            )
            guard let self, !Task.isCancelled, let peaks else { return }
            await MainActor.run { self.waveformPeaks = peaks }
        }
    }

    /// Returns the 0...1 amplitude peak nearest the given timestamp, or nil
    /// when no waveform is loaded.
    func waveformPeak(at seconds: Double) -> Float? {
        guard !waveformPeaks.isEmpty else { return nil }
        let idx = max(0, min(waveformPeaks.count - 1, Int(seconds.rounded())))
        return waveformPeaks[idx]
    }

    /// Returns the slice of peaks covering [start, end) seconds. Used by the
    /// 30-second tick rows to render a mini-histogram of the actual audio.
    func waveformPeaks(from start: Double, to end: Double) -> [Float] {
        guard !waveformPeaks.isEmpty, end > start else { return [] }
        let lo = max(0, Int(start.rounded()))
        let hi = min(waveformPeaks.count, Int(end.rounded()))
        guard lo < hi else { return [] }
        return Array(waveformPeaks[lo..<hi])
    }

    func clear() {
        kernel?.app.dispatch(.audioSetResume(seconds: currentTime))
        kernel?.app.dispatch(.audioPause)
        transcriptTask?.cancel(); transcriptTask = nil
        waveformTask?.cancel(); waveformTask = nil
        currentArtifact = nil
        audioUrl = nil
        currentTime = 0
        duration = 0
        isPlaying = false
        isBuffering = false
        loadedTimeRanges = []
        lastError = nil
        clearClip()
        publishError = nil
        transcriptSegments = []
        transcriptAvailability = .unavailable
        waveformPeaks = []
        tearDownRemoteCommandCenter()
        MPNowPlayingInfoCenter.default().nowPlayingInfo = nil
    }

    // MARK: - Transport

    func play() {
        // Cold-launch case: MiniPlayer was rehydrated but the kernel has not yet
        // loaded the player. Route through `load` to dispatch audioPlay; the
        // kernel will seek to the saved position and begin playback.
        if kernel?.podcastListeningSnapshot == nil, let artifact = currentArtifact {
            logger.info("play (cold-launch rehydrate)")
            load(artifact: artifact)
            return
        }
        logger.info("play")
        kernel?.app.dispatch(.audioResume)
        updateNowPlayingInfo()
    }

    func pause() {
        logger.info("pause")
        kernel?.app.dispatch(.audioSetResume(seconds: currentTime))
        kernel?.app.dispatch(.audioPause)
        updateNowPlayingInfo()
    }

    func toggle() {
        if isPlaying { pause() } else { play() }
    }

    func seek(to seconds: TimeInterval) {
        let clamped = max(0, min(seconds, duration))
        kernel?.app.dispatch(.audioSeek(seconds: clamped))
        currentTime = clamped
    }

    func skip(by delta: TimeInterval) {
        seek(to: currentTime + delta)
    }

    private var clipSelection: PodcastClipSelection {
        PodcastClipSelection(
            clipStartSeconds: clipStart,
            clipEndSeconds: clipEnd,
            speaker: speaker,
            selectedSegmentIds: selectedSegmentIds
        )
    }

    private func applyClipSelection(_ selection: PodcastClipSelection) {
        clipStart = selection.clipStartSeconds
        clipEnd = selection.clipEndSeconds
        speaker = selection.speaker
        selectedSegmentIds = selection.selectedSegmentIds
    }

    // MARK: - Clip selection

    func markIn() {
        let t = currentTime
        clipStart = t
        if let end = clipEnd, end <= t { clipEnd = nil }
    }

    func markOut() {
        let t = currentTime
        guard t > (clipStart ?? 0) else { return }
        clipEnd = min(t, duration)
    }

    func clearClip() {
        clipStart = nil
        clipEnd = nil
        speaker = ""
        selectedSegmentIds = []
    }

    func extendClipToSegment(_ segment: TranscriptSegment) {
        if !selectedSegmentIds.contains(segment.id) {
            selectedSegmentIds.append(segment.id)
        }
        if !segment.speaker.isEmpty { speaker = segment.speaker }
        if let s = clipStart {
            clipStart = min(s, segment.start)
        } else {
            clipStart = segment.start
        }
        if let e = clipEnd {
            clipEnd = max(e, segment.end)
        } else {
            clipEnd = segment.end
        }
    }

    func setClipStart(_ value: TimeInterval) {
        let clamped = max(0, min(value, clipEnd ?? duration))
        clipStart = clamped
    }

    func setClipEnd(_ value: TimeInterval) {
        let clamped = max(clipStart ?? 0, min(value, duration))
        clipEnd = clamped
    }

    // MARK: - Publish

    func publish(
        artifact: ArtifactRecord,
        targetGroupId: String,
        note: String,
        segments: [TranscriptSegment],
        core: HighlighterCore
    ) async -> PodcastClipPublishSnapshot {
        isPublishing = true
        publishError = nil
        defer { isPublishing = false }

        let outcome = await core.publishPodcastClipHighlight(input: PodcastClipPublishInput(
            artifact: artifact,
            targetGroupId: targetGroupId,
            note: note,
            segments: segments,
            selectedSegmentIds: selectedSegmentIds,
            clipStartSeconds: clipStart,
            clipEndSeconds: clipEnd,
            clipSpeaker: speaker
        ))
        let result = core.projectPodcastClipPublishResult(
            input: PodcastClipPublishResultInput(snapshot: outcome)
        )
        guard result.didPublish else {
            publishError = result.errorMessage
            return outcome
        }
        return outcome
    }

    // MARK: - Transcript

    func loadTranscript(from url: String) async {
        transcriptAvailability = .loading
        let snapshot = await core.loadPodcastTranscript(url: url)
        let projection = core.projectPodcastTranscriptLoadApply(
            input: PodcastTranscriptLoadApplyInput(snapshot: snapshot)
        )
        if projection.shouldLogError {
            logger.error("\(projection.logMessage, privacy: .public)")
        }
        transcriptSegments = projection.segments
        transcriptAvailability = projection.availability
    }

    /// Cold-launch rehydration. Surfaces the MiniPlayer in a paused state with
    /// the last episode the user listened to (within the last 7 days). The
    /// kernel player is NOT started — that happens when the user taps play and
    /// we route through `load(artifact:)` which dispatches `audioPlay`.
    func rehydrateFromSavedRecord() async {
        let snapshot = await core.getPodcastPlaybackRehydrationSnapshot(
            hasCurrentArtifact: currentArtifact != nil
        )
        guard snapshot.shouldApply, let artifact = snapshot.artifact else { return }
        currentArtifact = artifact
        currentTime = snapshot.currentTimeSeconds
        duration = snapshot.durationSeconds
        isPlaying = snapshot.isPlaying
    }

    // MARK: - Remote Command Center

    /// Call once per loaded episode. Registers play/pause/skip/seek handlers
    /// on MPRemoteCommandCenter so the lock screen and Control Center controls
    /// actually work.
    private func configureRemoteCommandCenter() {
        let center = MPRemoteCommandCenter.shared()

        center.playCommand.isEnabled = true
        center.playCommand.addTarget { [weak self] _ in
            self?.play()
            return .success
        }

        center.pauseCommand.isEnabled = true
        center.pauseCommand.addTarget { [weak self] _ in
            self?.pause()
            return .success
        }

        center.togglePlayPauseCommand.isEnabled = true
        center.togglePlayPauseCommand.addTarget { [weak self] _ in
            self?.toggle()
            return .success
        }

        center.skipForwardCommand.isEnabled = true
        center.skipForwardCommand.preferredIntervals = [30]
        center.skipForwardCommand.addTarget { [weak self] event in
            guard let self, let e = event as? MPSkipIntervalCommandEvent else { return .commandFailed }
            skip(by: e.interval)
            return .success
        }

        center.skipBackwardCommand.isEnabled = true
        center.skipBackwardCommand.preferredIntervals = [15]
        center.skipBackwardCommand.addTarget { [weak self] event in
            guard let self, let e = event as? MPSkipIntervalCommandEvent else { return .commandFailed }
            skip(by: -e.interval)
            return .success
        }

        center.changePlaybackPositionCommand.isEnabled = true
        center.changePlaybackPositionCommand.addTarget { [weak self] event in
            guard let self, let e = event as? MPChangePlaybackPositionCommandEvent else { return .commandFailed }
            seek(to: e.positionTime)
            return .success
        }

        // Lock Screen custom actions note:
        // iOS does not expose a public API for adding arbitrary buttons (e.g.
        // "Clip") to the Now Playing lock-screen widget or Control Center.
        // MPRemoteCommandCenter only exposes a fixed set of well-known
        // commands. Lock Screen Widgets (WidgetKit) cannot interact with an
        // in-process media player. A Now Playing ActivityExtension / Live
        // Activity could show metadata but still cannot inject custom
        // commands. Therefore a "Clip" lock screen button is not viable with
        // current public APIs.
    }

    private func tearDownRemoteCommandCenter() {
        let center = MPRemoteCommandCenter.shared()
        center.playCommand.removeTarget(nil)
        center.pauseCommand.removeTarget(nil)
        center.togglePlayPauseCommand.removeTarget(nil)
        center.skipForwardCommand.removeTarget(nil)
        center.skipBackwardCommand.removeTarget(nil)
        center.changePlaybackPositionCommand.removeTarget(nil)
    }

    // MARK: - Now Playing Info Center

    /// Pushes current episode metadata + playback state to the system's
    /// Now Playing Info Center. Call whenever playback state or position
    /// changes. This drives the lock screen and Control Center artwork, title,
    /// progress bar, and elapsed/remaining counters.
    private func updateNowPlayingInfo(artwork: MPMediaItemArtwork? = nil) {
        guard let artifact = currentArtifact else {
            MPNowPlayingInfoCenter.default().nowPlayingInfo = nil
            return
        }

        let showTitle = artifact.preview.podcastShowTitle.isEmpty ? artifact.preview.author : artifact.preview.podcastShowTitle
        let episodeTitle = artifact.preview.title.isEmpty ? "Untitled episode" : artifact.preview.title
        var info: [String: Any] = [:]
        info[MPMediaItemPropertyTitle] = episodeTitle
        info[MPMediaItemPropertyArtist] = showTitle
        info[MPMediaItemPropertyMediaType] = MPMediaType.podcast.rawValue

        if duration > 0 {
            info[MPMediaItemPropertyPlaybackDuration] = duration
        }
        info[MPNowPlayingInfoPropertyElapsedPlaybackTime] = currentTime
        info[MPNowPlayingInfoPropertyPlaybackRate] = isPlaying ? 1.0 : 0.0
        info[MPNowPlayingInfoPropertyDefaultPlaybackRate] = 1.0

        if let artwork {
            info[MPMediaItemPropertyArtwork] = artwork
        } else if let existing = MPNowPlayingInfoCenter.default().nowPlayingInfo?[MPMediaItemPropertyArtwork] {
            // Preserve previously loaded artwork while async fetch runs.
            info[MPMediaItemPropertyArtwork] = existing
        }

        MPNowPlayingInfoCenter.default().nowPlayingInfo = info
    }

    /// Fetches episode artwork from the network and updates Now Playing Info.
    /// Runs entirely off the main thread; hops back to update state.
    private func fetchAndApplyArtwork(from urlString: String) {
        guard !urlString.isEmpty, let url = URL(string: urlString) else { return }
        Task(priority: .userInitiated) { [weak self] in
            guard let (data, _) = try? await URLSession.shared.data(from: url),
                  let uiImage = UIImage(data: data) else { return }
            let artwork = MPMediaItemArtwork(boundsSize: uiImage.size) { _ in uiImage }
            await MainActor.run { [weak self] in
                self?.updateNowPlayingInfo(artwork: artwork)
            }
        }
    }

}
