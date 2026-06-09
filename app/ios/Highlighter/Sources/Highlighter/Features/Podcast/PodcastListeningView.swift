import Kingfisher
import SwiftUI

typealias TimelineRowState = PodcastTimelineRowState

extension PodcastTimelineRow: Identifiable {}

private let waveformTickWindow: Double = 30

// MARK: - Main view

struct PodcastListeningView: View {
    enum Presentation { case sheet, pushed }

    /// How this view is being shown. `.sheet` (the MiniPlayer entry point)
    /// wraps in its own NavigationStack and shows a "Done" toolbar button.
    /// `.pushed` (e.g. tapping a podcast row in a room) renders inline so
    /// the host stack supplies the back chevron.
    var presentation: Presentation = .sheet

    /// When provided, the player loads this artifact on appear if it's not
    /// already the current episode. Used by pushed entry points so the user
    /// doesn't need a separate "load + dismiss" hop.
    var artifact: ArtifactRecord? = nil

    /// `matchedTransitionSource` ID from the MiniPlayer artwork. The hero
    /// artwork in this sheet adopts the same source so iOS 26's zoom transition
    /// morphs the MiniPlayer pill into this view.
    var heroSourceID: String? = nil
    var heroNamespace: Namespace.ID? = nil

    @Environment(HighlighterStore.self) private var app
    @Environment(\.dismiss) private var dismiss

    // Layer toggles
    @State private var showTranscript = true
    @State private var showChapters = true
    @State private var showClips = true

    // Clipping flow
    @State private var clipArmed = false
    @State private var clipRangeStart: Double? = nil
    @State private var clipRangeEnd: Double? = nil
    @State private var showComposer = false

    // Auto-scroll
    @State private var lastManualScroll = Date.distantPast
    @State private var memberClips: [HighlightRecord] = []

    private var player: PodcastPlayerStore { app.podcastPlayer }

    var body: some View {
        Group {
            switch presentation {
            case .sheet:
                NavigationStack { content }
            case .pushed:
                content
            }
        }
        .sheet(isPresented: $showComposer, onDismiss: {
            Task { await loadClips() }
        }) {
            if let artifact = player.currentArtifact,
               let start = clipRangeStart,
               let end = clipRangeEnd {
                ClipComposerSheet(
                    artifact: artifact,
                    startSeconds: Binding(
                        get: { clipRangeStart ?? start },
                        set: { clipRangeStart = $0 }
                    ),
                    endSeconds: Binding(
                        get: { clipRangeEnd ?? end },
                        set: { clipRangeEnd = $0 }
                    )
                )
                .environment(app)
            }
        }
        .task(id: artifact?.shareEventId) {
            if let artifact, artifact.shareEventId != player.currentArtifact?.shareEventId {
                player.load(artifact: artifact)
            }
        }
        .task(id: player.currentArtifact?.shareEventId) {
            await loadClips()
        }
    }

    @ViewBuilder
    private var content: some View {
        let projection = listeningProjection
        ZStack(alignment: .bottomTrailing) {
            VStack(spacing: 0) {
                episodeHeader(projection: projection)
                layerToggles(projection: projection)
                timeline(projection: projection)
            }

            clipFab
                .padding(.trailing, 20)
                .padding(.bottom, 80)
        }
        .navigationTitle("Now Playing")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .navigationBarLeading) {
                if presentation == .sheet {
                    Button("Done") { dismiss() }
                }
            }
        }
    }

    private var listeningProjection: PodcastListeningProjection {
        app.safeCore.getPodcastListeningProjection(
            input: PodcastListeningProjectionInput(
                artifact: player.currentArtifact,
                clips: memberClips,
                transcriptSegments: player.transcriptSegments,
                transcriptAvailable: player.transcriptAvailability == .available,
                showTranscript: showTranscript,
                showChapters: showChapters,
                showClips: showClips,
                playerDurationSeconds: player.duration,
                currentTimeSeconds: player.currentTime,
                waveformTickWindowSeconds: waveformTickWindow
            )
        )
    }

    // MARK: - Episode header

    private func episodeHeader(projection: PodcastListeningProjection) -> some View {
        HStack(alignment: .top, spacing: 14) {
            episodeArtwork(imageUrl: projection.imageUrl)
                .frame(width: 60, height: 60)

            VStack(alignment: .leading, spacing: 4) {
                if !projection.showTitle.isEmpty {
                    Text(projection.showTitle)
                        .font(.caption2.weight(.semibold))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }

                Text(projection.episodeTitle)
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundStyle(.primary)
                    .lineLimit(2)
                    .fixedSize(horizontal: false, vertical: true)

                Text(projection.episodeMeta)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer(minLength: 0)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    @ViewBuilder
    private func episodeArtwork(imageUrl: String) -> some View {
        let base = Group {
            if !imageUrl.isEmpty, let url = URL(string: imageUrl) {
                KFImage(url)
                    .placeholder { artworkPlaceholder }
                    .fade(duration: 0.15)
                    .resizable()
                    .scaledToFill()
            } else {
                artworkPlaceholder
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))

        if let sourceID = heroSourceID, let ns = heroNamespace {
            base.matchedTransitionSource(id: sourceID, in: ns)
        } else {
            base
        }
    }

    private var artworkPlaceholder: some View {
        ZStack {
            Color(.secondarySystemFill)
            Image(systemName: "waveform")
                .font(.footnote)
                .foregroundStyle(.secondary)
        }
    }

    // MARK: - Layer toggles

    private func layerToggles(projection: PodcastListeningProjection) -> some View {
        HStack(spacing: 10) {
            layerPill("Transcript", active: showTranscript, disabled: player.transcriptAvailability == .unavailable) {
                showTranscript.toggle()
            }
            layerPill("Chapters", active: showChapters, disabled: !projection.hasChapters) {
                showChapters.toggle()
            }
            layerPill("Clips", active: showClips, disabled: false) {
                showClips.toggle()
            }
            Spacer()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
    }

    private func layerPill(_ label: String, active: Bool, disabled: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(label)
                .font(.caption.weight(.semibold))
                .foregroundStyle(active && !disabled ? Color(.systemBackground) : Color.secondary)
                .padding(.horizontal, 12)
                .padding(.vertical, 6)
                .background(
                    Capsule()
                        .fill(active && !disabled ? Color.primary : Color.clear)
                )
                .overlay(
                    Capsule()
                        .strokeBorder(Color(.separator), lineWidth: 1)
                        .opacity(active && !disabled ? 0 : 1)
                )
        }
        .buttonStyle(.plain)
        .disabled(disabled)
        .opacity(disabled ? 0.35 : 1.0)
    }

    // MARK: - Timeline rail

    private func timeline(projection: PodcastListeningProjection) -> some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(projection.rows) { row in
                        rowView(for: row)
                            .id(row.id)
                            .background(
                                row.state == .active
                                    ? Color(.separator).opacity(0.2)
                                    : Color.clear
                            )
                    }
                    // Bottom padding so the audio pill doesn't cover the last row.
                    Color.clear.frame(height: 96)
                }
            }
            .simultaneousGesture(
                DragGesture(minimumDistance: 10)
                    .onChanged { _ in lastManualScroll = Date() }
            )
            .onChange(of: projection.activeRowId) { _, newId in
                guard let id = newId else { return }
                let gracePassed = Date().timeIntervalSince(lastManualScroll) > 1.5
                if player.isPlaying && gracePassed {
                    withAnimation(.easeInOut(duration: 0.4)) {
                        proxy.scrollTo(id, anchor: UnitPoint(x: 0.5, y: 0.2))
                    }
                }
            }
        }
        .overlay(alignment: .bottom) {
            audioPill(projection: projection)
                .padding(.horizontal, 12)
                .padding(.bottom, 8)
        }
    }

    @ViewBuilder
    private func rowView(for row: PodcastTimelineRow) -> some View {
        switch row.kind {
        case .chapter:
            ChapterRow(
                t: row.t,
                timestampLabel: row.timestampLabel,
                title: row.chapterTitle,
                state: row.state,
                onSeek: { player.seek(to: $0) }
            )
        case .clip:
            if let highlight = row.clip {
                MemberClipRow(
                    highlight: highlight,
                    rangeLabel: row.clipRangeLabel,
                    state: row.state,
                    onSeek: { player.seek(to: $0) }
                )
            }
        case .transcript:
            if let segment = row.transcriptSegment {
                TranscriptRow(
                    segment: segment,
                    timestampLabel: row.timestampLabel,
                    state: row.state,
                    onSeek: {
                        player.seek(to: $0)
                        if !player.isPlaying { player.play() }
                    }
                )
            }
        case .waveformTick:
            WaveformTickRow(
                t: row.t,
                timestampLabel: row.timestampLabel,
                state: row.state,
                windowSeconds: row.waveformWindowSeconds,
                peaks: player.waveformPeaks(from: row.t, to: row.t + row.waveformWindowSeconds),
                onSeek: { player.seek(to: $0) }
            )
        }
    }

    // MARK: - Audio pill

    private func audioPill(projection: PodcastListeningProjection) -> some View {
        HStack(spacing: 14) {
            Button {
                player.toggle()
            } label: {
                ZStack {
                    Circle()
                        .fill(Color.primary)
                        .frame(width: 40, height: 40)
                    if player.isBuffering {
                        ProgressView()
                            .controlSize(.small)
                            .tint(Color(.systemBackground))
                    } else {
                        Image(systemName: player.isPlaying ? "pause.fill" : "play.fill")
                            .font(.system(size: 16, weight: .semibold))
                            .foregroundStyle(Color(.systemBackground))
                    }
                }
            }
            .buttonStyle(.plain)

            VStack(alignment: .leading, spacing: 2) {
                Text("now playing")
                    .font(.caption2)
                    .foregroundStyle(.secondary)

                Text(projection.currentSpeakerOrTimestamp)
                    .font(.caption.weight(.semibold).monospacedDigit())
                    .foregroundStyle(.primary)
                    .lineLimit(1)
            }

            Spacer(minLength: 0)

            // Progress strip
            GeometryReader { geo in
                let fraction: Double = player.duration > 0
                    ? min(1, max(0, player.currentTime / player.duration))
                    : 0
                ZStack(alignment: .leading) {
                    RoundedRectangle(cornerRadius: 2)
                        .fill(Color(.separator))
                    RoundedRectangle(cornerRadius: 2)
                        .fill(Color.primary)
                        .frame(width: max(2, geo.size.width * fraction))
                }
                .frame(height: 4)
                .contentShape(Rectangle())
                .onTapGesture { location in
                    let seekFraction = location.x / max(1, geo.size.width)
                    player.seek(to: seekFraction * player.duration)
                }
            }
            .frame(width: 80, height: 4)
        }
        .padding(.horizontal, 16)
        .frame(height: 56)
        .glassEffect(.regular, in: .capsule)
    }

    // MARK: - Clipping FAB

    private var clipFab: some View {
        VStack(spacing: 4) {
            Button {
                handleFabTap()
            } label: {
                ZStack {
                    Circle()
                        .fill(clipArmed ? Color.primary : Color.highlighterAccent)
                        .frame(width: 56, height: 56)
                    Image(systemName: "pencil")
                        .font(.system(size: 18, weight: .semibold))
                        .foregroundStyle(clipArmed ? Color(.systemBackground) : .white)
                }
            }
            .buttonStyle(.plain)

            Text(fabLabel)
                .font(.system(size: 9, weight: .semibold))
                .foregroundStyle(.secondary)
        }
    }

    private var fabLabel: String {
        if !clipArmed { return "CLIP" }
        if clipRangeStart == nil { return "PICK START" }
        return "PICK END"
    }

    private func handleFabTap() {
        if !clipArmed {
            clipArmed = true
            clipRangeStart = nil
            clipRangeEnd = nil
            return
        }
        if clipRangeStart == nil {
            clipRangeStart = player.currentTime
            return
        }
        let end = player.currentTime
        let start = clipRangeStart ?? 0
        player.setClipStart(start)
        player.setClipEnd(end)
        clipRangeEnd = end
        clipArmed = false
        showComposer = true
    }

    // MARK: - Helpers

    private func loadClips() async {
        let snapshot = await app.safeCore.getPodcastListeningClipsSnapshot(
            artifact: player.currentArtifact
        )
        memberClips = snapshot.clips
    }
}
