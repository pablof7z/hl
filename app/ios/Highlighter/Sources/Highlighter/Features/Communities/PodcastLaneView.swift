import Kingfisher
import SwiftUI

/// Podcast lane. Dark audio interior — warm near-black surface, lamp-lit
/// off-white text, mono timestamps. A thin episode-wide timeline shows
/// where each highlight clip sits along the episode; the hero clip is
/// lit with the accent. Speaker labels are rendered as small-caps
/// kickers above each pull.
struct PodcastLaneView: View {
    @Environment(HighlighterStore.self) private var app

    let lane: Lane
    let onShareToCommunity: (ArtifactRecord) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            NavigationLink(value: lane.artifact) {
                identity
            }
            .buttonStyle(.plain)
            .contextMenu {
                Button {
                    onShareToCommunity(lane.artifact)
                } label: {
                    Label("Share to community", systemImage: "square.and.arrow.up")
                }
            }

            if let hero = lane.highlights.first {
                episodeTimeline(hero)
                heroClip(hero)
            }

            if lane.highlights.count > 1 {
                supportingStrip
            }
        }
        .padding(.vertical, 28)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.laneAudioSurface)
        .environment(\.colorScheme, .dark)
    }

    // MARK: - Episode identity

    private var identity: some View {
        HStack(alignment: .top, spacing: 14) {
            artwork
                .frame(width: 56, height: 56)

            VStack(alignment: .leading, spacing: 4) {
                Text(lane.artifact.preview.title.isEmpty ? "Untitled" : lane.artifact.preview.title)
                    .font(.headline.weight(.semibold))
                    .foregroundStyle(Color.laneAudioInk)
                    .lineLimit(2)
                    .fixedSize(horizontal: false, vertical: true)

                if !showTitle.isEmpty {
                    Text(showTitle)
                        .font(.subheadline)
                        .foregroundStyle(Color.laneAudioInkMuted)
                        .lineLimit(1)
                }

                if let duration = formattedDuration {
                    Text(duration)
                        .font(.system(.caption, design: .monospaced))
                        .foregroundStyle(Color.laneAudioInkMuted)
                }
            }

            Spacer(minLength: 0)
        }
        .padding(.horizontal, 24)
    }

    @ViewBuilder
    private var artwork: some View {
        let image = lane.artifact.preview.image
        Group {
            if !image.isEmpty, let url = URL(string: image) {
                KFImage(url)
                    .placeholder { artworkPlaceholder }
                    .fade(duration: 0.15)
                    .resizable()
                    .scaledToFill()
            } else {
                artworkPlaceholder
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
    }

    private var artworkPlaceholder: some View {
        RoundedRectangle(cornerRadius: 6, style: .continuous)
            .fill(Color.laneAudioRule)
            .overlay(
                Image(systemName: "waveform")
                    .font(.footnote)
                    .foregroundStyle(Color.laneAudioInkMuted)
            )
    }

    // MARK: - Episode timeline + clip-range marker

    private func episodeTimeline(_ h: HydratedHighlight) -> some View {
        GeometryReader { geo in
            let w = geo.size.width
            let (startFrac, endFrac) = clipFractions(for: h)
            let markerX = w * startFrac
            let clipW = max(w * (endFrac - startFrac), 2)

            ZStack(alignment: .leading) {
                Rectangle()
                    .fill(Color.laneAudioRule)
                    .frame(height: 2)

                Rectangle()
                    .fill(Color.highlighterAccent)
                    .frame(width: clipW, height: 2)
                    .offset(x: markerX)

                Circle()
                    .fill(Color.highlighterAccent)
                    .frame(width: 8, height: 8)
                    .offset(x: markerX - 4)
            }
            .frame(height: 10)
        }
        .frame(height: 10)
        .padding(.horizontal, 24)
    }

    // MARK: - Hero clip

    private func heroClip(_ h: HydratedHighlight) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 10) {
                Text(timestampRange(for: h))
                    .font(.system(.footnote, design: .monospaced).weight(.medium))
                    .foregroundStyle(Color.laneAudioInkMuted)

                if !h.highlight.clipSpeaker.isEmpty {
                    Text("·").foregroundStyle(Color.laneAudioInkMuted)
                    Text(h.highlight.clipSpeaker.uppercased())
                        .font(.caption.weight(.semibold))
                        .tracking(1.2)
                        .foregroundStyle(Color.laneAudioInkMuted)
                        .lineLimit(1)
                }

                Spacer(minLength: 0)
            }

            Text("\u{201C}\(h.highlight.quote)\u{201D}")
                .font(.system(.title3).weight(.regular))
                .foregroundStyle(Color.laneAudioInk)
                .lineSpacing(3)
                .multilineTextAlignment(.leading)
                .fixedSize(horizontal: false, vertical: true)

            if !h.highlight.note.isEmpty {
                Text(h.highlight.note)
                    .font(.subheadline.italic())
                    .foregroundStyle(Color.laneAudioInkMuted)
                    .lineSpacing(2)
                    .fixedSize(horizontal: false, vertical: true)
            }

            HStack(spacing: 8) {
                AuthorAvatar(
                    pubkey: h.highlight.pubkey,
                    pictureURL: app.profileCache[h.highlight.pubkey]?.picture ?? "",
                    displayInitial: initial(for: h.highlight.pubkey),
                    size: 20
                )
                Text(name(for: h.highlight.pubkey))
                    .font(.footnote.weight(.semibold))
                    .foregroundStyle(Color.laneAudioInk)
                    .lineLimit(1)
                if let t = relative(h.highlight.createdAt) {
                    Text("·").foregroundStyle(Color.laneAudioInkMuted)
                    Text(t)
                        .font(.footnote)
                        .foregroundStyle(Color.laneAudioInkMuted)
                        .lineLimit(1)
                }
                Spacer(minLength: 0)
            }
        }
        .padding(.horizontal, 24)
        .task(id: h.highlight.pubkey) {
            await app.requestProfile(pubkeyHex: h.highlight.pubkey)
        }
    }

    // MARK: - Supporting clips

    private var supportingStrip: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(alignment: .top, spacing: 14) {
                ForEach(Array(lane.highlights.dropFirst()), id: \.highlight.eventId) { h in
                    PodcastLaneClipCard(highlight: h)
                }
            }
            .padding(.horizontal, 24)
        }
    }

    // MARK: - Helpers

    private var showTitle: String {
        lane.artifact.preview.podcastShowTitle.isEmpty
            ? lane.artifact.preview.author
            : lane.artifact.preview.podcastShowTitle
    }

    private var formattedDuration: String? {
        guard let secs = lane.artifact.preview.durationSeconds, secs > 0 else { return nil }
        let h = secs / 3600
        let m = (secs % 3600) / 60
        if h > 0 { return "\(h)h \(m)m" }
        return "\(m)m"
    }

    private func clipFractions(for h: HydratedHighlight) -> (Double, Double) {
        guard let total = lane.artifact.preview.durationSeconds, total > 0 else {
            return (0, 0.02)
        }
        let s = h.highlight.clipStartSeconds ?? 0
        let e = h.highlight.clipEndSeconds ?? (s + 30)
        let startFrac = max(0, min(1, s / Double(total)))
        let endFrac = max(startFrac + 0.005, min(1, e / Double(total)))
        return (startFrac, endFrac)
    }

    private func timestampRange(for h: HydratedHighlight) -> String {
        let s = format(h.highlight.clipStartSeconds)
        let e = format(h.highlight.clipEndSeconds)
        if let s, let e { return "\(s) — \(e)" }
        if let s { return s }
        return "—"
    }

    private func format(_ seconds: Double?) -> String? {
        guard let s = seconds, s >= 0 else { return nil }
        let total = Int(s.rounded())
        let h = total / 3600
        let m = (total % 3600) / 60
        let sec = total % 60
        if h > 0 {
            return String(format: "%d:%02d:%02d", h, m, sec)
        }
        return String(format: "%d:%02d", m, sec)
    }

    private func name(for pubkey: String) -> String {
        let profile = app.profileCache[pubkey]
        if let dn = profile?.displayName, !dn.isEmpty { return dn }
        if let n = profile?.name, !n.isEmpty { return n }
        return String(pubkey.prefix(10))
    }

    private func initial(for pubkey: String) -> String {
        name(for: pubkey).first.map { String($0).uppercased() } ?? ""
    }

    private func relative(_ seconds: UInt64?) -> String? {
        guard let s = seconds, s > 0 else { return nil }
        let date = Date(timeIntervalSince1970: TimeInterval(s))
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        formatter.dateTimeStyle = .numeric
        return formatter.localizedString(for: date, relativeTo: Date())
    }
}

/// Supporting clip card in the podcast lane's horizontal strip. Timestamp
/// range + speaker kicker + quote in the same dark idiom as the hero.
struct PodcastLaneClipCard: View {
    @Environment(HighlighterStore.self) private var app
    let highlight: HydratedHighlight

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 6) {
                Text(timestampRange)
                    .font(.system(.caption2, design: .monospaced).weight(.medium))
                    .foregroundStyle(Color.laneAudioInkMuted)

                if !highlight.highlight.clipSpeaker.isEmpty {
                    Text("·").foregroundStyle(Color.laneAudioInkMuted)
                    Text(highlight.highlight.clipSpeaker.uppercased())
                        .font(.system(size: 10).weight(.semibold))
                        .tracking(1.0)
                        .foregroundStyle(Color.laneAudioInkMuted)
                        .lineLimit(1)
                }

                Spacer(minLength: 0)
            }

            Text(highlight.highlight.quote)
                .font(.footnote)
                .foregroundStyle(Color.laneAudioInk)
                .lineLimit(5)
                .multilineTextAlignment(.leading)

            Spacer(minLength: 0)

            HStack(spacing: 6) {
                AuthorAvatar(
                    pubkey: highlight.highlight.pubkey,
                    pictureURL: app.profileCache[highlight.highlight.pubkey]?.picture ?? "",
                    displayInitial: initial,
                    size: 16
                )
                Text(name)
                    .font(.caption2)
                    .foregroundStyle(Color.laneAudioInkMuted)
                    .lineLimit(1)
            }
        }
        .padding(12)
        .frame(width: 220, height: 160, alignment: .topLeading)
        .background(
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .stroke(Color.laneAudioRule, lineWidth: 1)
        )
        .task(id: highlight.highlight.pubkey) {
            await app.requestProfile(pubkeyHex: highlight.highlight.pubkey)
        }
    }

    private var timestampRange: String {
        let s = format(highlight.highlight.clipStartSeconds)
        let e = format(highlight.highlight.clipEndSeconds)
        if let s, let e { return "\(s) — \(e)" }
        if let s { return s }
        return "—"
    }

    private func format(_ seconds: Double?) -> String? {
        guard let s = seconds, s >= 0 else { return nil }
        let total = Int(s.rounded())
        let h = total / 3600
        let m = (total % 3600) / 60
        let sec = total % 60
        if h > 0 {
            return String(format: "%d:%02d:%02d", h, m, sec)
        }
        return String(format: "%d:%02d", m, sec)
    }

    private var name: String {
        let profile = app.profileCache[highlight.highlight.pubkey]
        if let dn = profile?.displayName, !dn.isEmpty { return dn }
        if let n = profile?.name, !n.isEmpty { return n }
        return String(highlight.highlight.pubkey.prefix(10))
    }

    private var initial: String {
        name.first.map { String($0).uppercased() } ?? ""
    }
}
