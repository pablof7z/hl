import SwiftUI

/// Per-community feed view. Owns its own `RoomStore` (view-scoped) so
/// Observation granularity stays tight — this view only re-renders when
/// its own room's data changes, not when other rooms update.
struct RoomHomeView: View {
    enum Tab: Hashable { case library, discussions }

    let groupId: String

    @Environment(HighlighterStore.self) private var app
    @State private var room = RoomStore()
    @State private var selectedTab: Tab = .library
    @State private var composerPresented: Bool = false

    var body: some View {
        VStack(spacing: 0) {
            Picker("", selection: $selectedTab) {
                Text("Library").tag(Tab.library)
                Text("Discussions").tag(Tab.discussions)
            }
            .pickerStyle(.segmented)
            .padding(.horizontal)
            .padding(.top, 8)
            .padding(.bottom, 4)

            switch selectedTab {
            case .library:
                libraryContent
            case .discussions:
                DiscussionListView(groupId: groupId, composerPresented: $composerPresented)
            }
        }
        .navigationTitle("Community")
        .navigationBarTitleDisplayMode(.inline)
        .navigationDestination(for: ArtifactRecord.self) { artifact in
            ArtifactDetailView(artifact: artifact)
        }
        .toolbar {
            if selectedTab == .discussions {
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        composerPresented = true
                    } label: {
                        Image(systemName: "square.and.pencil")
                    }
                    .accessibilityLabel("New discussion")
                }
            }
        }
        .task {
            await room.start(groupId: groupId, core: app.safeCore, bridge: app.eventBridge)
        }
        .onDisappear {
            room.stop()
        }
    }

    @ViewBuilder
    private var libraryContent: some View {
        if room.isLoading && room.artifacts.isEmpty && room.highlights.isEmpty {
            ProgressView().controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if room.artifacts.isEmpty && room.highlights.isEmpty {
            ContentUnavailableView(
                "Nothing here yet",
                systemImage: "square.stack.3d.up",
                description: Text("Shares and highlights will appear as activity flows in.")
            )
        } else {
            List {
                if !room.artifacts.isEmpty {
                    Section("Library") {
                        ForEach(room.artifacts, id: \.shareEventId) { a in
                            NavigationLink(value: a) {
                                Text(a.preview.title.isEmpty ? "Untitled" : a.preview.title)
                            }
                        }
                    }
                }
                if !room.highlights.isEmpty {
                    Section("Highlights") {
                        ForEach(room.highlights, id: \.highlight.eventId) { h in
                            Text(h.highlight.quote).lineLimit(3)
                        }
                    }
                }
            }
            .listStyle(.plain)
        }
    }
}
