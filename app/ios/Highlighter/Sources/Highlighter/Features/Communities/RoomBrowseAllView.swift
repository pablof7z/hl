import SwiftUI

/// Dense 2-column grid of every room cached locally. Search filters by name
/// and description. Reached from the Explorer home via the "Browse all
/// rooms" footer.
struct RoomBrowseAllView: View {
    @Environment(HighlighterStore.self) private var appStore

    @State private var rooms: [CommunitySummary] = []
    @State private var search: String = ""
    @State private var previewRoom: CommunitySummary?

    private let columns = [
        GridItem(.flexible(), spacing: 14),
        GridItem(.flexible(), spacing: 14),
    ]

    private var visible: [CommunitySummary] {
        appStore.safeCore.searchRooms(rooms: rooms, query: search)
    }

    var body: some View {
        ScrollView {
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
            .padding(18)
        }
        .background(Color.highlighterPaper.ignoresSafeArea())
        .navigationTitle("Browse rooms")
        .navigationBarTitleDisplayMode(.inline)
        .searchable(text: $search, placement: .navigationBarDrawer(displayMode: .always))
        .task {
            await appStore.safeCore.startRoomDiscovery()
            let outcome = await appStore.safeCore.getAllRooms(limit: 200)
            rooms = outcome.error.isEmpty ? outcome.values : []
        }
        .refreshable {
            let outcome = await appStore.safeCore.getAllRooms(limit: 200)
            rooms = outcome.error.isEmpty ? outcome.values : []
        }
        .sheet(item: $previewRoom) { room in
            NavigationStack {
                RoomPreviewSheet(
                    room: room,
                    onJoin: {
                        Task {
                            _ = await appStore.safeCore.requestJoinRoom(groupId: room.id, roomName: room.name)
                        }
                        previewRoom = nil
                    }
                )
            }
            .environment(appStore)
        }
    }
}
