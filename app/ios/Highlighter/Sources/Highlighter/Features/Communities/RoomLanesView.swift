import SwiftUI

/// The community home's Home tab — a stream of artifact-grouped highlight
/// modules, identical in shape to the Highlights tab. Each lane pairs one
/// artifact with the room's recent highlights on it; dormant lanes (no
/// highlights and no comments) are filtered out — the Library tab is the
/// place to browse every artifact regardless of activity.
///
/// Highlight data flows in two streams because the Rust core's
/// `get_highlights(groupId:)` filters on `#h` tags that kind:9802 events
/// don't carry (community association lives on the kind:16 repost, not
/// on the highlight itself). So for articles we fetch per-address via
/// `get_highlights_for_article`. Books and podcasts don't yet have an
/// equivalent per-artifact query; their lanes appear without pull-quotes
/// until that lands.
struct RoomLanesView: View {
    @Environment(HighlighterStore.self) private var appStore

    let artifacts: [ArtifactRecord]
    let highlights: [HydratedHighlight]
    let highlightsByReference: [String: [HighlightRecord]]
    let commentsByReference: [String: [CommentRecord]]
    let isLoading: Bool
    let onShareToCommunity: (ArtifactRecord) -> Void

    var body: some View {
        if isLoading && artifacts.isEmpty {
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if visibleLanes.isEmpty {
            ContentUnavailableView(
                "Nothing here yet",
                systemImage: "square.stack.3d.up",
                description: Text("Highlights from the room's library will appear here.")
            )
        } else {
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(Array(visibleLanes.enumerated()), id: \.element.id) { index, lane in
                        laneView(for: lane)
                        if index < visibleLanes.count - 1 {
                            Rectangle()
                                .fill(Color.highlighterRule)
                                .frame(height: 1)
                        }
                    }
                }
                .padding(.horizontal, 12)
            }
            .background(Color.highlighterPaper.ignoresSafeArea())
        }
    }

    private var visibleLanes: [RoomLane] {
        let highlightBuckets = highlightsByReference.map { key, values in
            HighlightReferenceBucket(lookupKey: key, highlights: values)
        }
        let commentBuckets = commentsByReference.map { key, values in
            CommentReferenceBucket(commentKey: key, comments: values)
        }
        return appStore.safeCore.buildVisibleRoomLanes(
            artifacts: artifacts,
            highlights: highlights,
            highlightsByReference: highlightBuckets,
            commentsByReference: commentBuckets
        )
    }

    @ViewBuilder
    private func laneView(for lane: RoomLane) -> some View {
        if !lane.highlights.isEmpty {
            NavigationLink(value: lane.artifact) {
                HighlightFeedCardView(items: lane.highlights)
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
    }
}
