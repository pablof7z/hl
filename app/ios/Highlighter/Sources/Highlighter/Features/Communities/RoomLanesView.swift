import Kingfisher
import SwiftUI

/// The community home's library surface rendered as stacked lanes. Each
/// lane pairs one artifact with the community's recent highlights on it,
/// sorted so the most-alive artifact sits at the top. Artifacts shared to
/// the room but not yet highlighted sit below as dormant lanes.
///
/// v1 renders every lane with a generic treatment; the per-format
/// atmospheres (book / podcast / article) replace individual branches of
/// `LaneView.laneHead` in subsequent steps.
struct RoomLanesView: View {
    let artifacts: [ArtifactRecord]
    let highlights: [HydratedHighlight]
    let isLoading: Bool
    let onShareToCommunity: (ArtifactRecord) -> Void

    var body: some View {
        if isLoading && artifacts.isEmpty && highlights.isEmpty {
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if artifacts.isEmpty && highlights.isEmpty {
            ContentUnavailableView(
                "Nothing here yet",
                systemImage: "square.stack.3d.up",
                description: Text("Shares and highlights will appear as activity flows in.")
            )
        } else {
            ScrollView {
                LazyVStack(spacing: 0) {
                    let lanes = Lane.build(artifacts: artifacts, highlights: highlights)
                    ForEach(Array(lanes.enumerated()), id: \.element.id) { index, lane in
                        laneView(for: lane)
                        if index < lanes.count - 1 {
                            let from = LaneSurface(for: lane)
                            let to = LaneSurface(for: lanes[index + 1])
                            if from != to {
                                LaneTransitionView(from: from, to: to)
                            }
                        }
                    }
                }
            }
            .background(Color.highlighterPaper.ignoresSafeArea())
        }
    }

    @ViewBuilder
    private func laneView(for lane: Lane) -> some View {
        if lane.isDormant {
            DormantLaneRow(lane: lane, onShareToCommunity: onShareToCommunity)
        } else {
            switch lane.artifact.preview.source {
            case "book":
                BookLaneView(lane: lane, onShareToCommunity: onShareToCommunity)
            case "podcast":
                PodcastLaneView(lane: lane, onShareToCommunity: onShareToCommunity)
            case "article":
                ArticleLaneView(lane: lane, onShareToCommunity: onShareToCommunity)
            default:
                LaneView(lane: lane, onShareToCommunity: onShareToCommunity)
            }
        }
    }
}

/// Format-surface category for a lane. Used by transitions to pick the
/// right gradient and height between adjacent lanes. Dormant lanes always
/// read as `.neutral` because they render directly on the page paper.
enum LaneSurface: Equatable {
    case paper      // book
    case dark       // podcast
    case white      // article
    case neutral    // generic / dormant / unknown format

    init(for lane: Lane) {
        if lane.isDormant { self = .neutral; return }
        switch lane.artifact.preview.source {
        case "book":    self = .paper
        case "podcast": self = .dark
        case "article": self = .white
        default:        self = .neutral
        }
    }

    var color: Color {
        switch self {
        case .paper:   return .laneBookPaper
        case .dark:    return .laneAudioSurface
        case .white:   return .laneArticlePage
        case .neutral: return .highlighterPaper
        }
    }
}

/// Designed transition between two adjacent lanes. A dark surface on
/// either side makes the transition dramatic (40pt dusk / dawn). All
/// other pairs breathe in a shorter neutral fold. Same-to-same gets a
/// minimal hairline — shouldn't happen for consecutive artifacts of the
/// same format, but it's defensive.
struct LaneTransitionView: View {
    let from: LaneSurface
    let to: LaneSurface

    var body: some View {
        LinearGradient(
            colors: [from.color, to.color],
            startPoint: .top,
            endPoint: .bottom
        )
        .frame(height: height)
    }

    private var height: CGFloat {
        switch (from, to) {
        case (.dark, _), (_, .dark): return 40    // dusk / dawn
        case (.paper, .white), (.white, .paper): return 22  // fold
        default: return 14
        }
    }
}

// MARK: - Lane model

/// A single lane on the community home: an artifact together with the
/// community's recent highlights on it. Pure Swift — the grouping logic
/// below runs client-side against `RoomStore`'s two reactive streams.
struct Lane: Identifiable {
    let id: String
    let artifact: ArtifactRecord
    /// Newest-first.
    let highlights: [HydratedHighlight]

    /// Most recent created_at across this lane's highlights, falling back
    /// to the artifact's share time for lanes without highlights.
    var latestActivity: UInt64? {
        if let newest = highlights.compactMap({ $0.highlight.createdAt }).max() {
            return newest
        }
        return artifact.createdAt
    }

    var isDormant: Bool { highlights.isEmpty }

    /// Build lanes from the room's flat artifact + highlight lists.
    ///
    /// Grouping uses **semantic identity** — the canonical reference that
    /// both sides of the bridge agree on (NIP-23 `a`-tag value, or the
    /// core's `highlightReferenceKey` / `sourceReferenceKey` pairing).
    /// Share event id is share-specific and cannot be used to match a
    /// highlight's hydrated artifact against the room's share of the
    /// same thing — they're different kind:11 events.
    ///
    /// Shared artifacts with no matching highlights become dormant
    /// lanes. Active lanes sort by most-recent activity desc; dormant
    /// lanes follow, sorted by share time desc.
    static func build(
        artifacts: [ArtifactRecord],
        highlights: [HydratedHighlight]
    ) -> [Lane] {
        var buckets: [String: (artifact: ArtifactRecord?, items: [HydratedHighlight])] = [:]

        for h in highlights {
            let key = laneKey(for: h)
            var bucket = buckets[key] ?? (artifact: h.artifact, items: [])
            if bucket.artifact == nil { bucket.artifact = h.artifact }
            bucket.items.append(h)
            buckets[key] = bucket
        }

        for art in artifacts {
            let key = laneKey(for: art)
            if var bucket = buckets[key] {
                // Prefer the room's share record — it's the anchor the
                // user actually put into this community.
                bucket.artifact = art
                buckets[key] = bucket
            } else {
                buckets[key] = (artifact: art, items: [])
            }
        }

        let lanes: [Lane] = buckets.compactMap { key, bucket in
            guard let artifact = bucket.artifact else { return nil }
            let sorted = bucket.items.sorted {
                ($0.highlight.createdAt ?? 0) > ($1.highlight.createdAt ?? 0)
            }
            return Lane(id: key, artifact: artifact, highlights: sorted)
        }

        return lanes.sorted { a, b in
            switch (a.isDormant, b.isDormant) {
            case (false, true): return true
            case (true, false): return false
            default: return (a.latestActivity ?? 0) > (b.latestActivity ?? 0)
            }
        }
    }

    /// Semantic identity for grouping. Designed so a highlight and the
    /// artifact share of the same thing produce the **same** key.
    /// `highlightReferenceKey` / `sourceReferenceKey` is the core's
    /// canonical pairing when present; the NIP-23 address fills in for
    /// anything addressable by `a`-tag; the tagged event id fills in
    /// for plain event references.
    private static func laneKey(for artifact: ArtifactRecord) -> String {
        if !artifact.preview.highlightReferenceKey.isEmpty {
            return "srk:" + artifact.preview.highlightReferenceKey
        }
        if artifact.preview.highlightTagName == "a",
           !artifact.preview.highlightTagValue.isEmpty {
            return "addr:" + artifact.preview.highlightTagValue
        }
        if artifact.preview.referenceTagName == "a",
           !artifact.preview.referenceTagValue.isEmpty {
            return "addr:" + artifact.preview.referenceTagValue
        }
        if !artifact.preview.referenceTagValue.isEmpty {
            return "evt:" + artifact.preview.referenceTagValue
        }
        if !artifact.preview.highlightTagValue.isEmpty {
            return "evt:" + artifact.preview.highlightTagValue
        }
        if !artifact.preview.id.isEmpty { return "pid:" + artifact.preview.id }
        if !artifact.shareEventId.isEmpty { return "share:" + artifact.shareEventId }
        return "t:" + artifact.preview.title
    }

    private static func laneKey(for h: HydratedHighlight) -> String {
        if !h.highlight.sourceReferenceKey.isEmpty {
            return "srk:" + h.highlight.sourceReferenceKey
        }
        if !h.highlight.artifactAddress.isEmpty {
            return "addr:" + h.highlight.artifactAddress
        }
        if !h.highlight.eventReference.isEmpty {
            return "evt:" + h.highlight.eventReference
        }
        if let art = h.artifact {
            return laneKey(for: art)
        }
        // Orphan — unique key so it sits alone (gets dropped later since
        // it has no artifact record).
        return "orphan:" + h.highlight.eventId
    }
}

// MARK: - Dormant lane row

/// Compact row for an artifact the community has shared but not yet
/// highlighted. Renders on the page paper — no format-specific surface —
/// with just enough identity to be recognizable. Quietly signals
/// "nothing to read here yet" through density alone.
struct DormantLaneRow: View {
    let lane: Lane
    let onShareToCommunity: (ArtifactRecord) -> Void

    var body: some View {
        NavigationLink(value: lane.artifact) {
            HStack(alignment: .center, spacing: 12) {
                cover

                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                        .font(titleFont)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(1)
                    if !subtitle.isEmpty {
                        Text(subtitle)
                            .font(.caption)
                            .foregroundStyle(Color.highlighterInkMuted.opacity(0.75))
                            .lineLimit(1)
                    }
                }

                Spacer(minLength: 0)

                Image(systemName: "chevron.right")
                    .font(.caption2)
                    .foregroundStyle(Color.highlighterInkMuted.opacity(0.6))
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 10)
        }
        .buttonStyle(.plain)
        .contextMenu {
            Button {
                onShareToCommunity(lane.artifact)
            } label: {
                Label("Share to community", systemImage: "square.and.arrow.up")
            }
        }
    }

    @ViewBuilder
    private var cover: some View {
        let image = lane.artifact.preview.image
        Group {
            if !image.isEmpty, let url = URL(string: image) {
                KFImage(url)
                    .placeholder { placeholder }
                    .fade(duration: 0.15)
                    .resizable()
                    .scaledToFill()
            } else {
                placeholder
            }
        }
        .frame(width: coverWidth, height: 36)
        .clipShape(RoundedRectangle(cornerRadius: coverRadius, style: .continuous))
    }

    private var placeholder: some View {
        RoundedRectangle(cornerRadius: coverRadius, style: .continuous)
            .fill(Color.highlighterRule.opacity(0.6))
            .overlay(
                Image(systemName: placeholderIcon)
                    .font(.caption2)
                    .foregroundStyle(Color.highlighterInkMuted.opacity(0.6))
            )
    }

    private var coverWidth: CGFloat {
        switch lane.artifact.preview.source {
        case "book":    return 24
        case "podcast": return 36
        default:        return 36
        }
    }

    private var coverRadius: CGFloat {
        switch lane.artifact.preview.source {
        case "book":    return 2
        case "podcast": return 4
        default:        return 3
        }
    }

    private var placeholderIcon: String {
        switch lane.artifact.preview.source {
        case "book":    return "book.closed"
        case "podcast": return "waveform"
        case "article": return "doc.text"
        default:        return "circle"
        }
    }

    private var title: String {
        lane.artifact.preview.title.isEmpty ? "Untitled" : lane.artifact.preview.title
    }

    private var titleFont: Font {
        switch lane.artifact.preview.source {
        case "book", "article":
            return .system(.subheadline, design: .serif)
        default:
            return .subheadline
        }
    }

    private var subtitle: String {
        if !lane.artifact.preview.author.isEmpty { return lane.artifact.preview.author }
        if !lane.artifact.preview.domain.isEmpty { return lane.artifact.preview.domain }
        return ""
    }
}

// MARK: - Lane view (v1 generic treatment)

struct LaneView: View {
    let lane: Lane
    let onShareToCommunity: (ArtifactRecord) -> Void

    var body: some View {
        VStack(spacing: 0) {
            NavigationLink(value: lane.artifact) {
                laneHead
                    .padding(.horizontal, 20)
            }
            .buttonStyle(.plain)
            .contextMenu {
                Button {
                    onShareToCommunity(lane.artifact)
                } label: {
                    Label("Share to community", systemImage: "square.and.arrow.up")
                }
            }

            if !lane.highlights.isEmpty {
                highlightsStrip
                    .padding(.top, 2)
            }

            Rectangle()
                .fill(Color.highlighterRule)
                .frame(height: 1)
                .padding(.top, 14)
        }
    }

    @ViewBuilder
    private var laneHead: some View {
        switch lane.artifact.preview.source {
        case "book":    RoomLibraryBookCardView(artifact: lane.artifact)
        case "podcast": RoomLibraryPodcastCardView(artifact: lane.artifact)
        case "article": RoomLibraryArticleCardView(artifact: lane.artifact)
        default:        genericHead
        }
    }

    private var genericHead: some View {
        HStack {
            Text(lane.artifact.preview.title.isEmpty ? "Untitled" : lane.artifact.preview.title)
                .font(.headline)
                .foregroundStyle(Color.highlighterInkStrong)
            Spacer()
            Image(systemName: "chevron.right")
                .font(.footnote)
                .foregroundStyle(Color.highlighterInkMuted)
        }
        .padding(.vertical, 14)
    }

    private var highlightsStrip: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(alignment: .top, spacing: 12) {
                ForEach(lane.highlights, id: \.highlight.eventId) { h in
                    LaneHighlightCardView(highlight: h)
                }
            }
            .padding(.horizontal, 20)
        }
    }
}

// MARK: - Highlight card (generic for v1; atmospheres specialize later)

struct LaneHighlightCardView: View {
    @Environment(HighlighterStore.self) private var app
    let highlight: HydratedHighlight

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(highlight.highlight.quote)
                .font(.callout)
                .foregroundStyle(Color.highlighterInkStrong)
                .lineLimit(5)
                .multilineTextAlignment(.leading)

            Spacer(minLength: 0)

            HStack(spacing: 6) {
                AuthorAvatar(
                    pubkey: highlight.highlight.pubkey,
                    pictureURL: app.profileCache[highlight.highlight.pubkey]?.picture ?? "",
                    displayInitial: highlighterInitial,
                    size: 18
                )
                Text(highlighterName)
                    .font(.caption2.weight(.medium))
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(1)
            }
        }
        .padding(14)
        .frame(width: 260, height: 170, alignment: .topLeading)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(Color.white.opacity(0.45))
                .overlay(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .stroke(Color.highlighterRule, lineWidth: 1)
                )
        )
        .task(id: highlight.highlight.pubkey) {
            await app.requestProfile(pubkeyHex: highlight.highlight.pubkey)
        }
    }

    private var highlighterName: String {
        let profile = app.profileCache[highlight.highlight.pubkey]
        if let dn = profile?.displayName, !dn.isEmpty { return dn }
        if let n = profile?.name, !n.isEmpty { return n }
        return String(highlight.highlight.pubkey.prefix(10))
    }

    private var highlighterInitial: String {
        highlighterName.first.map { String($0).uppercased() } ?? ""
    }
}
