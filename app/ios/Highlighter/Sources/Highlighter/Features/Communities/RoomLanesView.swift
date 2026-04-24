import SwiftUI

/// The community home's Home tab rendered as stacked, format-aware lanes.
/// Each lane pairs one artifact with the community's recent highlights on
/// it, sorted so the most-alive artifact sits at the top. Lanes without
/// highlights still render in their format atmosphere — the absence of
/// a hero pull-quote is the signal, not a separate "dormant" row.
///
/// Highlight data flows in two streams because the Rust core's
/// `get_highlights(groupId:)` filters on `#h` tags that kind:9802 events
/// don't carry (community association lives on the kind:16 repost, not
/// on the highlight itself). So for articles we fetch per-address via
/// `get_highlights_for_article`. Books and podcasts don't yet have an
/// equivalent per-artifact query; their lanes appear without pull-quotes
/// until that lands.
struct RoomLanesView: View {
    let artifacts: [ArtifactRecord]
    let highlights: [HydratedHighlight]
    let highlightsByAddress: [String: [HighlightRecord]]
    let isLoading: Bool
    let onShareToCommunity: (ArtifactRecord) -> Void

    var body: some View {
        if isLoading && artifacts.isEmpty {
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if artifacts.isEmpty {
            ContentUnavailableView(
                "Nothing here yet",
                systemImage: "square.stack.3d.up",
                description: Text("Shares and highlights will appear as activity flows in.")
            )
        } else {
            ScrollView {
                LazyVStack(spacing: 0) {
                    let lanes = Lane.build(
                        artifacts: artifacts,
                        highlights: highlights,
                        highlightsByAddress: highlightsByAddress
                    )
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

/// Format-surface category for a lane. Used by transitions to pick the
/// right gradient and height between adjacent lanes.
enum LaneSurface: Equatable {
    case paper      // book
    case dark       // podcast
    case white      // article
    case neutral    // generic / unknown format

    init(for lane: Lane) {
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
/// either side makes the transition dramatic (40pt dusk / dawn). Paper
/// to magazine-white folds in 22pt. Everything else breathes in 14pt.
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
        case (.dark, _), (_, .dark): return 40
        case (.paper, .white), (.white, .paper): return 22
        default: return 14
        }
    }
}

// MARK: - Lane model

/// A single lane on the community home: an artifact together with the
/// community's recent highlights on it.
struct Lane: Identifiable {
    let id: String
    let artifact: ArtifactRecord
    /// Newest-first.
    let highlights: [HydratedHighlight]

    var latestActivity: UInt64? {
        if let newest = highlights.compactMap({ $0.highlight.createdAt }).max() {
            return newest
        }
        return artifact.createdAt
    }

    var isDormant: Bool { highlights.isEmpty }

    /// Build lanes from `artifacts` + whatever highlight sources are available.
    /// Primary path for articles: `highlightsByAddress[<nip23-addr>]` — fetched
    /// via `get_highlights_for_article`. Secondary: the `highlights` stream
    /// from `get_highlights(groupId:)`, matched against each artifact with a
    /// permissive predicate. Books and podcasts rely on the secondary path
    /// (which may yield nothing until the core exposes per-artifact queries
    /// for those formats).
    static func build(
        artifacts: [ArtifactRecord],
        highlights: [HydratedHighlight],
        highlightsByAddress: [String: [HighlightRecord]]
    ) -> [Lane] {
        var lanes: [Lane] = artifacts.map { art in
            var bucket: [HydratedHighlight] = []

            if let addr = nip23Address(for: art),
               let records = highlightsByAddress[addr] {
                bucket.append(contentsOf: records.map { rec in
                    HydratedHighlight(
                        highlight: rec,
                        artifact: art,
                        sharedByEventId: nil,
                        sharedByPubkey: nil
                    )
                })
            }

            for h in highlights where matches(h, art) {
                if bucket.contains(where: { $0.highlight.eventId == h.highlight.eventId }) {
                    continue
                }
                bucket.append(h)
            }

            bucket.sort { ($0.highlight.createdAt ?? 0) > ($1.highlight.createdAt ?? 0) }

            return Lane(
                id: art.shareEventId.isEmpty ? art.preview.id : art.shareEventId,
                artifact: art,
                highlights: bucket
            )
        }

        lanes.sort { a, b in
            switch (a.isDormant, b.isDormant) {
            case (false, true): return true
            case (true, false): return false
            default: return (a.latestActivity ?? 0) > (b.latestActivity ?? 0)
            }
        }
        return lanes
    }

    /// Permissive predicate: does this highlight belong to this artifact?
    /// Tries the canonical `<refName>:<refValue>` pair first, then falls
    /// back to every other field that might match. Cheap to evaluate
    /// (string comparisons) so running it O(N·M) is fine for reasonable
    /// artifact counts.
    private static func matches(_ h: HydratedHighlight, _ art: ArtifactRecord) -> Bool {
        let hl = h.highlight
        let pv = art.preview

        if !pv.referenceTagName.isEmpty, !pv.referenceTagValue.isEmpty {
            let artKey = "\(pv.referenceTagName):\(pv.referenceTagValue)"
            if !hl.sourceReferenceKey.isEmpty, hl.sourceReferenceKey == artKey {
                return true
            }
        }

        if !hl.artifactAddress.isEmpty {
            if hl.artifactAddress == pv.referenceTagValue { return true }
            if hl.artifactAddress == pv.highlightTagValue { return true }
        }

        if !hl.eventReference.isEmpty {
            if hl.eventReference == pv.referenceTagValue { return true }
            if hl.eventReference == art.shareEventId { return true }
        }

        if !hl.sourceUrl.isEmpty {
            if hl.sourceUrl == pv.url { return true }
            if !pv.audioUrl.isEmpty, hl.sourceUrl == pv.audioUrl { return true }
        }

        return false
    }

    private static func nip23Address(for art: ArtifactRecord) -> String? {
        if art.preview.referenceTagName == "a", !art.preview.referenceTagValue.isEmpty {
            return art.preview.referenceTagValue
        }
        if art.preview.highlightTagName == "a", !art.preview.highlightTagValue.isEmpty {
            return art.preview.highlightTagValue
        }
        return nil
    }
}

// MARK: - Generic lane view (for unknown formats)

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

    private var laneHead: some View {
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
