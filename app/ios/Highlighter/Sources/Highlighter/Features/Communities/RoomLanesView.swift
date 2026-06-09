import SwiftUI

/// The community home's Home tab — a stream of artifact-grouped highlight
/// modules, identical in shape to the Highlights tab. Rust supplies the
/// bounded visible lanes; this view only renders them.
struct RoomLanesView: View {
    let lanes: [RoomLane]
    let artifactCount: Int
    let isLoading: Bool
    let onShareToCommunity: (ArtifactRecord) -> Void

    var body: some View {
        if isLoading && artifactCount == 0 {
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if lanes.isEmpty {
            ContentUnavailableView(
                "Nothing here yet",
                systemImage: "square.stack.3d.up",
                description: Text("Highlights from the room's library will appear here.")
            )
        } else {
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(Array(lanes.enumerated()), id: \.element.id) { index, lane in
                        laneView(for: lane)
                        if index < lanes.count - 1 {
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
