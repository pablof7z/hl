import Kingfisher
import SwiftUI

// MARK: - Row state

enum TimelineRowState {
    case played, active, future
}

// MARK: - Timeline row model

enum TimelineRow: Identifiable {
    case chapter(t: Double, title: String)
    case clip(HighlightRecord)
    case transcript(TranscriptSegment)
    case waveformTick(t: Double)

    var id: String {
        switch self {
        case let .chapter(t, _): return "chapter-\(t)"
        case let .clip(h): return "clip-\(h.eventId)"
        case let .transcript(s): return "transcript-\(s.id)"
        case let .waveformTick(t): return "waveform-\(t)"
        }
    }

    var t: Double {
        switch self {
        case let .chapter(t, _): return t
        case let .clip(h): return h.clipStartSeconds ?? 0
        case let .transcript(s): return s.start
        case let .waveformTick(t): return t
        }
    }
}

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

    private var player: PodcastPlayerStore { app.podcastPlayer }

    private var memberClips: [HighlightRecord] {
        guard let target = currentClipReference else { return [] }
        return (app.referenceHighlights(tagName: target.tagName, tagValue: target.tagValue) ?? [])
            .sorted { ($0.clipStartSeconds ?? 0) < ($1.clipStartSeconds ?? 0) }
    }

    private var currentClipReference: (tagName: String, tagValue: String)? {
        guard let artifact = player.currentArtifact else { return nil }
        let guid = artifact.preview.podcastItemGuid
        let tagValue = guid.isEmpty
            ? artifact.shareEventId
            : "podcast:item:guid:\(guid)"
        return ("i", tagValue)
    }

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
               let end = clipRangeEnd
            {
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
        ZStack(alignment: .bottomTrailing) {
            VStack(spacing: 0) {
                episodeHeader
                layerToggles
                timeline
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

    // MARK: - Episode header

    private var episodeHeader: some View {
        HStack(alignment: .top, spacing: 14) {
            episodeArtwork
                .frame(width: 60, height: 60)

            VStack(alignment: .leading, spacing: 4) {
                let artifact = player.currentArtifact
                let showTitle = artifact.map {
                    $0.preview.podcastShowTitle.isEmpty ? $0.preview.author : $0.preview.podcastShowTitle
                } ?? ""

                if !showTitle.isEmpty {
                    Text(showTitle)
                        .font(.caption2.weight(.semibold))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }

                Text(artifact?.preview.title.isEmpty == false
                    ? (artifact?.preview.title ?? "Untitled episode")
                    : "Untitled episode")
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundStyle(.primary)
                    .lineLimit(2)
                    .fixedSize(horizontal: false, vertical: true)

                Text(episodeMeta)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer(minLength: 0)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    private var episodeMeta: String {
        var parts: [String] = []
        if let artifact = player.currentArtifact,
           let dur = artifact.preview.durationSeconds, dur > 0
        {
            let h = dur / 3600
            let m = (dur % 3600) / 60
            if h > 0 { parts.append("\(h)h \(m)m") }
            else { parts.append("\(m)m") }
        } else if player.duration > 0 {
            let total = Int(player.duration)
            let h = total / 3600
            let m = (total % 3600) / 60
            if h > 0 { parts.append("\(h)h \(m)m") }
            else { parts.append("\(m)m") }
        }
        let clipCount = memberClips.count
        if clipCount > 0 { parts.append("\(clipCount) clip\(clipCount == 1 ? "" : "s")") }
        return parts.joined(separator: " · ")
    }

    @ViewBuilder
    private var episodeArtwork: some View {
        let imageUrl = player.currentArtifact?.preview.image ?? ""
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

    private var layerToggles: some View {
        HStack(spacing: 10) {
            layerPill("Transcript", active: showTranscript, disabled: player.transcriptAvailability == .unavailable) {
                showTranscript.toggle()
            }
            layerPill("Chapters", active: showChapters, disabled: availableChapters.isEmpty) {
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

    private var timeline: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(timelineRows) { row in
                        rowView(for: row)
                            .id(row.id)
                            .background(
                                rowState(for: row) == .active
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
            .onChange(of: activeRowId) { _, newId in
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
            audioPill
                .padding(.horizontal, 12)
                .padding(.bottom, 8)
        }
    }

    @ViewBuilder
    private func rowView(for row: TimelineRow) -> some View {
        let state = rowState(for: row)
        switch row {
        case let .chapter(t, title):
            ChapterRow(t: t, title: title, state: state, onSeek: { player.seek(to: $0) })
        case let .clip(h):
            MemberClipRow(highlight: h, state: state, onSeek: { player.seek(to: $0) })
        case let .transcript(seg):
            TranscriptRow(segment: seg, state: state, onSeek: {
                player.seek(to: $0)
                if !player.isPlaying { player.play() }
            })
        case let .waveformTick(t):
            WaveformTickRow(
                t: t,
                state: state,
                windowSeconds: waveformTickWindow,
                peaks: player.waveformPeaks(from: t, to: t + waveformTickWindow),
                onSeek: { player.seek(to: $0) }
            )
        }
    }

    private func rowState(for row: TimelineRow) -> TimelineRowState {
        let t = row.t
        if t > player.currentTime { return .future }
        if row.id == activeRowId { return .active }
        return .played
    }

    private var activeRowId: String? {
        // Latest row whose t <= currentTime.
        timelineRows
            .filter { $0.t <= player.currentTime }
            .last
            .map { $0.id }
    }

    // MARK: - Row builder

    private var availableChapters: [Chapter] {
        player.currentArtifact?.preview.chapters ?? []
    }

    private var timelineRows: [TimelineRow] {
        var rows: [TimelineRow] = []

        if showClips {
            for h in memberClips {
                rows.append(.clip(h))
            }
        }

        if showChapters {
            for chapter in availableChapters {
                rows.append(.chapter(t: chapter.startSeconds, title: chapter.title))
            }
        }

        if showTranscript && player.transcriptAvailability == .available {
            for seg in player.transcriptSegments {
                rows.append(.transcript(seg))
            }
        } else {
            let occupiedTimes = rows.map { $0.t }
            let totalDuration = player.duration > 0 ? player.duration : 3600
            var t: Double = 0
            while t < totalDuration {
                let hasNeighbor = occupiedTimes.contains { abs($0 - t) < 8 }
                if !hasNeighbor {
                    rows.append(.waveformTick(t: t))
                }
                t += waveformTickWindow
            }
        }

        return rows.sorted { $0.t < $1.t }
    }

    // MARK: - Audio pill

    private var audioPill: some View {
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
            .accessibilityLabel(player.isPlaying ? "Pause" : "Play")

            VStack(alignment: .leading, spacing: 2) {
                Text("now playing")
                    .font(.caption2)
                    .foregroundStyle(.secondary)

                Text(currentSpeakerOrTimestamp)
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
                .accessibilityAddTraits(.isButton)
                .accessibilityLabel("Playback position")
                .accessibilityHint("Tap to seek")
            }
            .frame(width: 80, height: 4)
        }
        .padding(.horizontal, 16)
        .frame(height: 56)
        .glassEffect(.regular, in: .capsule)
    }

    private var currentSpeakerOrTimestamp: String {
        if player.transcriptAvailability == .available {
            let currentSeg = player.transcriptSegments
                .filter { $0.start <= player.currentTime }
                .last
            if let seg = currentSeg, !seg.speaker.isEmpty {
                return seg.speaker
            }
        }
        return formatTimestamp(player.currentTime)
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
            .accessibilityLabel(clipArmed ? "Mark clip point" : "Clip this moment")

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
        guard let target = currentClipReference else { return }
        app.requestReferenceHighlights(tagName: target.tagName, tagValue: target.tagValue, limit: 128)
    }
}

// MARK: - Formatting helpers

private func formatTimestamp(_ seconds: Double) -> String {
    guard seconds.isFinite, seconds >= 0 else { return "0:00" }
    let total = Int(seconds)
    let h = total / 3600
    let m = (total % 3600) / 60
    let s = total % 60
    if h > 0 { return String(format: "%d:%02d:%02d", h, m, s) }
    return String(format: "%d:%02d", m, s)
}
