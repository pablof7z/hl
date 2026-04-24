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
                    ForEach(Lane.build(artifacts: artifacts, highlights: highlights)) { lane in
                        laneView(for: lane)
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
    /// Highlights without a hydrated artifact are dropped (they can't be
    /// placed). Shared artifacts with no highlights appear as dormant
    /// lanes. Active lanes sort by most-recent activity desc; dormant
    /// lanes follow, sorted by share time desc.
    static func build(
        artifacts: [ArtifactRecord],
        highlights: [HydratedHighlight]
    ) -> [Lane] {
        var buckets: [String: (artifact: ArtifactRecord, items: [HydratedHighlight])] = [:]
        for h in highlights {
            guard let art = h.artifact else { continue }
            let key = laneKey(for: art)
            var bucket = buckets[key] ?? (artifact: art, items: [])
            bucket.items.append(h)
            buckets[key] = bucket
        }
        for key in buckets.keys {
            buckets[key]?.items.sort {
                ($0.highlight.createdAt ?? 0) > ($1.highlight.createdAt ?? 0)
            }
        }

        var lanes = buckets.map { key, bucket in
            Lane(id: key, artifact: bucket.artifact, highlights: bucket.items)
        }

        let activeKeys = Set(lanes.map(\.id))
        for art in artifacts where !activeKeys.contains(laneKey(for: art)) {
            lanes.append(Lane(id: laneKey(for: art), artifact: art, highlights: []))
        }

        return lanes.sorted { a, b in
            switch (a.isDormant, b.isDormant) {
            case (false, true): return true
            case (true, false): return false
            default: return (a.latestActivity ?? 0) > (b.latestActivity ?? 0)
            }
        }
    }

    /// Stable identity for bucketing. Share event id is the preferred key
    /// (one kind:11 per shared artifact); preview id / reference tags are
    /// fallbacks for artifacts only reached via a highlight's hydrated
    /// reference. Prefixes prevent collisions across fallback tiers.
    private static func laneKey(for artifact: ArtifactRecord) -> String {
        if !artifact.shareEventId.isEmpty { return artifact.shareEventId }
        if !artifact.preview.id.isEmpty { return "p:" + artifact.preview.id }
        if !artifact.preview.highlightTagValue.isEmpty {
            return "h:" + artifact.preview.highlightTagValue
        }
        if !artifact.preview.referenceTagValue.isEmpty {
            return "r:" + artifact.preview.referenceTagValue
        }
        return "t:" + artifact.preview.title
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
