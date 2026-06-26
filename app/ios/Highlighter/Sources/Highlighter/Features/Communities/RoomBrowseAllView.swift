import SwiftUI

/// Dense 2-column grid of every room cached locally. Search filters by name
/// and description. Reached from the Explorer home via the "Browse all
/// rooms" footer.
///
/// Phase 7 cutover: discovery data comes exclusively from the
/// `HighlighterAppKernel` typed snapshot `KernelRoomExplorerSnapshot`. The view
/// opens the RoomExplorer kernel view on `.task` (the actor's lifecycle hook
/// auto-starts room discovery) and reads `newNoteworthy` directly, filtering
/// client-side by the search query. Joining dispatches the `JoinRoom` kernel
/// action. The bespoke room discovery/read/join calls were removed.
struct RoomBrowseAllView: View {
    @Environment(HighlighterStore.self) private var appStore
    @Environment(HighlighterAppKernel.self) private var kernel

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
            // Open the RoomExplorer kernel view. The actor's lifecycle hook
            // fires StartRoomDiscovery automatically (RoomPolicy.discoveryRelay).
            // Discovery results arrive as KernelEvent::DiscoveredGroupsUpdated →
            // kernel.roomExplorer, which re-filters via onChange below.
            kernel.openRoomExplorer()
            await loadRooms()
        }
        .onChange(of: search) { _, _ in
            Task { await loadRooms() }
        }
        .onChange(of: kernel.roomExplorer) { _, _ in
            // Re-filter when the kernel pushes a fresh discovery snapshot.
            Task { await loadRooms() }
        }
        .refreshable {
            kernel.refreshRoomExplorer()
            await loadRooms()
        }
        // NOTE: no .onDisappear { kernel.closeRoomExplorer() } — this view is
        // pushed on top of RoomExplorerView, which owns the explorer lifecycle.
        // closeRoomExplorer() nils the shared snapshot, so closing here would
        // blank the parent on pop-back. The parent closes it when the tab leaves.
        .sheet(item: $previewRoom) { room in
            NavigationStack {
                RoomPreviewSheet(
                    room: room,
                    onJoin: {
                        // Dispatch JoinRoom kernel action — fire-and-forget (D6).
                        // Core resolves host_relay_url from discovered_groups / communities (D3).
                        kernel.app.dispatch(.joinRoom(
                            groupId: room.id,
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

    /// Filter the kernel's discovered rooms by the current search query.
    /// Reads `kernel.roomExplorer.newNoteworthy` synchronously — no I/O.
    private func loadRooms() async {
        let query = search.lowercased()
        let all = kernel.roomExplorer?.newNoteworthy.map { $0.asCommunitySummary() } ?? []
        if query.isEmpty {
            rooms = all
        } else {
            rooms = all.filter {
                $0.name.lowercased().contains(query) ||
                $0.about.lowercased().contains(query)
            }
        }
    }
}
