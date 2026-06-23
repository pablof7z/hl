import SwiftUI

/// Dense 2-column grid of every room cached locally. Search filters by name
/// and description. Reached from the Explorer home via the "Browse all
/// rooms" footer.
struct RoomBrowseAllView: View {
    @Environment(HighlighterStore.self) private var appStore

    /// Called when the user taps "Open room" (or the already-joined primary
    /// button) inside the preview sheet. The parent NavigationStack owner
    /// should push the room's id onto its path.
    var onOpenRoom: ((String) -> Void)? = nil

    @State private var rooms: [CommunitySummary] = []
    @State private var search: String = ""
    @State private var previewRoom: CommunitySummary?

    private let columns = [
        GridItem(.flexible(), spacing: 14),
        GridItem(.flexible(), spacing: 14),
    ]

    var body: some View {
        ScrollView {
            LazyVGrid(columns: columns, spacing: 18) {
                ForEach(rooms, id: \.id) { room in
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
            await loadRooms()
        }
        .onChange(of: search) { _, _ in
            Task {
                await loadRooms()
            }
        }
        .refreshable {
            await loadRooms()
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
                    },
                    onOpenRoom: onOpenRoom.map { open in
                        {
                            previewRoom = nil
                            open(room.id)
                        }
                    }
                )
            }
            .environment(appStore)
        }
    }

    private func loadRooms() async {
        let query = search
        let snapshot = await appStore.safeCore.getRoomBrowseSnapshot(query: query, limit: 200)
        if query == search {
            rooms = snapshot.error.trimmingCharacters(in: .whitespaces).isEmpty ? snapshot.rooms : []
        }
    }
}
