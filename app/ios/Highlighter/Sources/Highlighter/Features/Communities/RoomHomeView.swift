import SwiftUI

/// Per-community feed view. Owns its own `RoomStore` (view-scoped) so
/// Observation granularity stays tight — this view only re-renders when
/// its own room's data changes, not when other rooms update.
struct RoomHomeView: View {
    enum Tab: Hashable { case home, library, discussions }

    let groupId: String

    @Environment(HighlighterStore.self) private var app
    @State private var room = RoomStore()
    @State private var selectedTab: Tab = .home
    @State private var composerPresented: Bool = false
    @State private var shareTarget: ShareToCommunityTarget?

    var body: some View {
        VStack(spacing: 0) {
            Picker("", selection: $selectedTab) {
                Text("Home").tag(Tab.home)
                Text("Library").tag(Tab.library)
                Text("Discussions").tag(Tab.discussions)
            }
            .pickerStyle(.segmented)
            .padding(.horizontal)
            .padding(.top, 8)
            .padding(.bottom, 4)

            switch selectedTab {
            case .home:
                homeContent
            case .library:
                libraryContent
            case .discussions:
                DiscussionListView(groupId: groupId, composerPresented: $composerPresented)
            }
        }
        .navigationTitle(communityName)
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
        .sheet(item: $shareTarget) { target in
            ShareToCommunitySheet(target: target)
                .presentationDetents([.medium, .large])
        }
    }

    // MARK: - Home tab (format-aware lanes — the new surface)

    private var homeContent: some View {
        RoomLanesView(
            artifacts: room.artifacts,
            highlights: room.highlights,
            highlightsByAddress: room.highlightsByAddress,
            isLoading: room.isLoading,
            onShareToCommunity: { artifact in
                shareTarget = .artifact(artifact)
            }
        )
    }

    // MARK: - Library tab (unchanged — flat list of shares + highlights)

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
            ScrollView {
                LazyVStack(spacing: 0) {
                    if !room.artifacts.isEmpty {
                        ForEach(Array(room.artifacts.enumerated()), id: \.element.shareEventId) { index, a in
                            NavigationLink(value: a) {
                                artifactRow(a)
                            }
                            .buttonStyle(.plain)
                            .contextMenu {
                                Button {
                                    shareTarget = .artifact(a)
                                } label: {
                                    Label("Share to community", systemImage: "square.and.arrow.up")
                                }
                            }

                            if index < room.artifacts.count - 1 {
                                Rectangle()
                                    .fill(Color.highlighterRule)
                                    .frame(height: 1)
                            }
                        }
                    }

                    if !room.highlights.isEmpty {
                        highlightsSection
                    }
                }
                .padding(.horizontal, 20)
            }
            .background(Color.highlighterPaper.ignoresSafeArea())
        }
    }

    @ViewBuilder
    private func artifactRow(_ a: ArtifactRecord) -> some View {
        switch a.preview.source {
        case "article":
            RoomLibraryArticleCardView(artifact: a)
        case "book":
            RoomLibraryBookCardView(artifact: a)
        case "podcast":
            RoomLibraryPodcastCardView(artifact: a)
        default:
            HStack {
                Text(a.preview.title.isEmpty ? "Untitled" : a.preview.title)
                    .foregroundStyle(Color.highlighterInkStrong)
                Spacer()
                Image(systemName: "chevron.right")
                    .font(.footnote)
                    .foregroundStyle(Color.highlighterInkMuted)
            }
            .padding(.vertical, 14)
        }
    }

    @ViewBuilder
    private var highlightsSection: some View {
        if !room.artifacts.isEmpty {
            Rectangle()
                .fill(Color.highlighterRule)
                .frame(height: 1)
        }

        Text("Highlights")
            .font(.footnote.weight(.semibold))
            .foregroundStyle(Color.highlighterInkMuted)
            .textCase(.uppercase)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.top, 18)
            .padding(.bottom, 8)

        ForEach(Array(room.highlights.enumerated()), id: \.element.highlight.eventId) { index, h in
            Text(h.highlight.quote)
                .lineLimit(3)
                .foregroundStyle(Color.highlighterInkStrong)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.vertical, 14)

            if index < room.highlights.count - 1 {
                Rectangle()
                    .fill(Color.highlighterRule)
                    .frame(height: 1)
            }
        }
    }

    // MARK: - Header

    private var communityName: String {
        let match = app.joinedCommunities.first { $0.id == groupId }
        if let name = match?.name, !name.isEmpty { return name }
        return "Community"
    }
}
