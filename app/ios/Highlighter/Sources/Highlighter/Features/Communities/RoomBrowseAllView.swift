import SwiftUI

/// Dense 2-column grid of every room cached locally. Search filters by name
/// and description. Reached from the Explorer home via the "Browse all
/// rooms" footer.
struct RoomBrowseAllView: View {
    @Environment(HighlighterStore.self) private var appStore
    @Environment(HighlighterAppKernel.self) private var kernel

    /// Called when the user taps "Open room" (or the already-joined primary
    /// button) inside the preview sheet. The parent NavigationStack owner
    /// should push the room's id onto its path.
    var onOpenRoom: ((String) -> Void)? = nil

    @State private var search: String = ""
    @State private var previewRoom: CommunitySummary?

    private let columns = [
        GridItem(.flexible(), spacing: 14),
        GridItem(.flexible(), spacing: 14),
    ]

    /// Rooms derived from the kernel's roomExplorer snapshot, filtered by
    /// the current search query. Updates reactively as the snapshot changes.
    private var rooms: [CommunitySummary] {
        let all = kernel.roomExplorer?.newNoteworthy.map { $0.asCommunitySummary() } ?? []
        guard !search.isEmpty else { return all }
        let q = search.lowercased()
        return all.filter {
            $0.name.lowercased().contains(q) || $0.about.lowercased().contains(q)
        }
    }

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
        .refreshable {
            kernel.refreshRoomExplorer()
        }
        .sheet(item: $previewRoom) { room in
            NavigationStack {
                RoomPreviewSheet(
                    room: room,
                    onJoin: {
                        kernel.app.dispatch(.joinRoom(
                            groupId: room.id,
                            hostRelayUrl: room.relayUrl,
                            inviteCode: nil
                        ))
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
}
