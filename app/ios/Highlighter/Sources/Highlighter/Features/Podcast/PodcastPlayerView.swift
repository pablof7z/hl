import SwiftUI

/// Full podcast player UI: artwork header, transport, scrub timeline,
/// clip-range selector, speaker + note fields, transcript with tap-to-seek,
/// and a save-clip button that publishes via the Rust core.
struct PodcastPlayerView: View {
    let artifact: ArtifactRecord

    @Environment(HighlighterStore.self) private var app
    @State private var player = PodcastPlayerStore()
    @State private var segments: [TranscriptSegment] = []
    @State private var note: String = ""
    @State private var isTranscriptLoading = false
    @State private var transcriptError: String?
    @State private var publishedOk = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                artworkHeader
                transportRow
                timeline
                clipControls
                speakerField
                noteField
                transcriptSection
                saveButton
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
        }
        .background(Color(uiColor: .systemBackground))
        .task { await startPlayback() }
        .onDisappear { player.pause() }
    }

    // MARK: - Sections

    @ViewBuilder
    private var artworkHeader: some View {
        let image = artifact.preview.image
        HStack(alignment: .top, spacing: 14) {
            if !image.isEmpty, let url = URL(string: image) {
                AsyncImage(url: url) { phase in
                    switch phase {
                    case .success(let img):
                        img.resizable().scaledToFill()
                    case .failure:
                        placeholderArt
                    case .empty:
                        placeholderArt
                    @unknown default:
                        placeholderArt
                    }
                }
                .frame(width: 96, height: 96)
                .clipShape(RoundedRectangle(cornerRadius: 14))
            } else {
                placeholderArt.frame(width: 96, height: 96).clipShape(RoundedRectangle(cornerRadius: 14))
            }

            VStack(alignment: .leading, spacing: 4) {
                Text(artifact.preview.title.isEmpty ? "Untitled episode" : artifact.preview.title)
                    .font(.title3.weight(.semibold))
                    .lineLimit(3)
                if !artifact.preview.podcastShowTitle.isEmpty {
                    Text(artifact.preview.podcastShowTitle)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
                if !artifact.preview.author.isEmpty {
                    Text(artifact.preview.author)
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)
                }
            }
            Spacer(minLength: 0)
        }
    }

    private var placeholderArt: some View {
        ZStack {
            Color.secondary.opacity(0.15)
            Image(systemName: "waveform").font(.title).foregroundStyle(.secondary)
        }
    }

    private var transportRow: some View {
        HStack(spacing: 28) {
            Button { player.skip(by: -15) } label: {
                Image(systemName: "gobackward.15").font(.title2)
            }
            Button { player.toggle() } label: {
                ZStack {
                    if player.isBuffering {
                        ProgressView()
                            .controlSize(.large)
                            .tint(Color.highlighterAccent)
                            .frame(width: 52, height: 52)
                    } else {
                        Image(systemName: player.isPlaying ? "pause.circle.fill" : "play.circle.fill")
                            .font(.system(size: 52))
                            .foregroundStyle(Color.highlighterAccent)
                    }
                }
            }
            Button { player.skip(by: 30) } label: {
                Image(systemName: "goforward.30").font(.title2)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 14)
        .padding(.horizontal, 20)
        .glassEffectCompat()
        .foregroundStyle(.primary)
    }

    private var timeline: some View {
        VStack(spacing: 6) {
            if let error = player.lastError {
                HStack(spacing: 6) {
                    Image(systemName: "exclamationmark.circle.fill")
                        .foregroundStyle(.red)
                    Text(error)
                        .font(.caption)
                        .foregroundStyle(.red)
                        .lineLimit(2)
                    Spacer()
                }
                .padding(10)
                .background(Color.red.opacity(0.08), in: RoundedRectangle(cornerRadius: 10))
            }
            ClipTimelineView(
                clipStart: Binding(get: { player.clipStart }, set: { newValue in
                    if let v = newValue { player.setClipStart(v) }
                }),
                clipEnd: Binding(get: { player.clipEnd }, set: { newValue in
                    if let v = newValue { player.setClipEnd(v) }
                }),
                currentTime: Binding(get: { player.currentTime }, set: { _ in }),
                duration: effectiveDuration,
                loadedTimeRanges: player.loadedTimeRanges,
                onSeek: { player.seek(to: $0) }
            )
        }
    }

    private var effectiveDuration: TimeInterval {
        if player.duration > 0 { return player.duration }
        if let d = artifact.preview.durationSeconds, d > 0 { return TimeInterval(d) }
        return 0
    }

    private var clipControls: some View {
        HStack(spacing: 10) {
            Button {
                player.markIn()
            } label: {
                Label("Mark in", systemImage: "arrow.down.to.line.compact")
                    .labelStyle(.titleAndIcon)
            }
            .buttonStyle(.bordered)
            .tint(Color.highlighterAccent)

            Button {
                player.markOut()
            } label: {
                Label("Mark out", systemImage: "arrow.up.to.line.compact")
            }
            .buttonStyle(.bordered)
            .tint(Color.highlighterAccent)

            if player.clipStart != nil || player.clipEnd != nil {
                Button(role: .destructive) {
                    player.clearClip()
                } label: {
                    Label("Clear", systemImage: "xmark.circle")
                        .labelStyle(.iconOnly)
                }
                .buttonStyle(.bordered)
            }

            Spacer()

            if let start = player.clipStart, let end = player.clipEnd, end > start {
                Text(clipRangeLabel(start: start, end: end))
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var speakerField: some View {
        TextField(
            "Speaker (optional)",
            text: Binding(
                get: { player.speaker },
                set: { player.speaker = $0 }
            )
        )
        .textFieldStyle(.roundedBorder)
        .textInputAutocapitalization(.words)
        .autocorrectionDisabled()
    }

    private var noteField: some View {
        TextField("Note (optional)", text: $note, axis: .vertical)
            .lineLimit(1...3)
            .textFieldStyle(.roundedBorder)
    }

    @ViewBuilder
    private var transcriptSection: some View {
        if isTranscriptLoading {
            HStack {
                ProgressView().controlSize(.small)
                Text("Loading transcript…").font(.caption).foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.vertical, 8)
        } else if !segments.isEmpty {
            VStack(alignment: .leading, spacing: 6) {
                Text("Transcript")
                    .font(.headline)
                TranscriptView(
                    segments: segments,
                    currentTime: player.currentTime,
                    selectedSegmentIds: player.selectedSegmentIds,
                    onTapSegment: { segment in
                        player.seek(to: segment.start)
                        player.extendClipToSegment(segment)
                    }
                )
                .frame(maxHeight: 360)
            }
        } else if artifact.preview.transcriptUrl.isEmpty {
            Text("No transcript available")
                .font(.caption)
                .foregroundStyle(.tertiary)
        } else if let err = transcriptError {
            Text("Transcript unavailable (\(err))")
                .font(.caption)
                .foregroundStyle(.tertiary)
        }
    }

    private var saveButton: some View {
        VStack(spacing: 8) {
            if let err = player.publishError {
                Text(err)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            Button {
                Task { await savePressed() }
            } label: {
                HStack {
                    if player.isPublishing {
                        ProgressView().controlSize(.small).tint(.white)
                    }
                    Text(publishedOk ? "Saved" : "Save clip")
                        .font(.body.weight(.semibold))
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 12)
            }
            .buttonStyle(.borderedProminent)
            .tint(Color.highlighterAccent)
            .disabled(!canSave || player.isPublishing)
        }
        .padding(.top, 8)
    }

    // MARK: - Actions

    private var canSave: Bool {
        guard let start = player.clipStart, let end = player.clipEnd else { return false }
        return end > start
    }

    private func startPlayback() async {
        let audio = artifact.preview.audioUrl
        if !audio.isEmpty, let url = URL(string: audio) {
            player.load(url: url)
        }
        let t = artifact.preview.transcriptUrl
        if !t.isEmpty, let url = URL(string: t) {
            await loadTranscript(from: url)
        }
    }

    private func loadTranscript(from url: URL) async {
        isTranscriptLoading = true
        transcriptError = nil
        defer { isTranscriptLoading = false }

        do {
            let (data, response) = try await URLSession.shared.data(from: url)
            let contentType = (response as? HTTPURLResponse)?.value(forHTTPHeaderField: "Content-Type")
            let ext = url.pathExtension.isEmpty ? nil : url.pathExtension
            let parsed = TranscriptParser.parse(
                data: data,
                contentType: contentType,
                fileExtension: ext
            )
            self.segments = parsed
        } catch {
            transcriptError = "\(error.localizedDescription)"
        }
    }

    private func savePressed() async {
        let core = app.safeCore
        do {
            _ = try await player.publish(
                artifact: artifact,
                targetGroupId: artifact.groupId,
                note: note,
                segments: segments,
                core: core
            )
            publishedOk = true
        } catch {
            // Error is already surfaced via `player.publishError`.
        }
    }

    // MARK: - Formatting

    private func clipRangeLabel(start: TimeInterval, end: TimeInterval) -> String {
        "\(formatTime(start))–\(formatTime(end))"
    }

    private func formatTime(_ seconds: TimeInterval) -> String {
        guard seconds.isFinite, seconds >= 0 else { return "00:00" }
        let total = Int(seconds)
        let hours = total / 3600
        let minutes = (total % 3600) / 60
        let secs = total % 60
        if hours > 0 {
            return String(format: "%d:%02d:%02d", hours, minutes, secs)
        }
        return String(format: "%02d:%02d", minutes, secs)
    }
}

// MARK: - Liquid Glass compatibility

private extension View {
    /// Wraps the modern `.glassEffect(.regular)` where available and falls
    /// back to an ultra-thin material on older runtimes during local dev.
    @ViewBuilder
    func glassEffectCompat() -> some View {
        if #available(iOS 26.0, *) {
            self.glassEffect(.regular, in: .rect(cornerRadius: 20))
        } else {
            self.background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 20))
        }
    }
}
