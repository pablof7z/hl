import SwiftUI

/// Dense 2-column grid of every room cached locally. Search filters by name
/// and description. Reached from the Explorer home via the "Browse all
/// rooms" footer.
struct RoomBrowseAllView: View {
    @Environment(HighlighterStore.self) private var appStore

    @State private var search: String = ""
    @State private var previewRoom: CommunitySummary?

    private let columns = [
        GridItem(.flexible(), spacing: 14),
        GridItem(.flexible(), spacing: 14),
    ]

    private var visible: [CommunitySummary] {
        let q = search.trimmingCharacters(in: .whitespaces).lowercased()
        let rooms = appStore.roomExplorer.allRooms
        guard !q.isEmpty else { return rooms }
        return rooms.filter {
            $0.name.lowercased().contains(q) || $0.about.lowercased().contains(q)
        }
    }

    var body: some View {
        ScrollView {
            LazyVStack(spacing: 14) {
                if appStore.roomExplorer.isBrowseLoading && visible.isEmpty {
                    RoundedRectangle(cornerRadius: 16)
                        .fill(Color.highlighterRule.opacity(0.4))
                        .frame(height: 180)
                        .padding(.horizontal, 18)
                } else if let message = appStore.roomExplorer.errorMessage, visible.isEmpty {
                    Text(message)
                        .font(.subheadline)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal, 18)
                }

                LazyVGrid(columns: columns, spacing: 18) {
                    ForEach(visible, id: \.id) { room in
                        Button {
                            previewRoom = room
                        } label: {
                            RoomCoverCard(room: room)
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(.horizontal, 18)
            }
            .padding(.vertical, 18)
        }
        .background(Color.highlighterPaper.ignoresSafeArea())
        .navigationTitle("Browse rooms")
        .navigationBarTitleDisplayMode(.inline)
        .searchable(text: $search, placement: .navigationBarDrawer(displayMode: .always))
        .task {
            appStore.refreshRoomBrowseAll()
        }
        .refreshable {
            appStore.refreshRoomBrowseAll()
        }
        .sheet(item: $previewRoom) { room in
            NavigationStack {
                RoomPreviewSheet(
                    room: room,
                    onJoin: {
                        appStore.requestJoinRoom(groupId: room.id, roomName: room.name)
                        previewRoom = nil
                    }
                )
            }
            .environment(appStore)
        }
    }
}
