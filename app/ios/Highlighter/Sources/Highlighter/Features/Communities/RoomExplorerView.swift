import SwiftUI

/// Rooms tab root. One continuous scrolling surface in the Apple TV "Home"
/// style: featured hero at the top, followed by editorial and social
/// shelves, then a Browse-all entry point at the bottom. No segmented
/// toggles — "Your rooms" is just the first shelf among many.
///
/// Phase 3G cutover: reads joined-groups and discovery data from the
/// `HighlighterAppKernel` typed snapshots (`CommunitiesSnapshot` and
/// `KernelRoomExplorerSnapshot`) rather than from the live lane's
/// `HighlighterStore`/`RoomExplorerStore`. The kernel view is opened at
/// app startup in `HighlighterAppKernel.init()` (always resident).
/// `StartRoomDiscovery` is dispatched on appear to kick relay discovery.
struct RoomExplorerView: View {
    @Environment(HighlighterStore.self) private var appStore
    @Environment(HighlighterAppKernel.self) private var kernel
    @State private var previewRoom: CommunitySummary?
    @State private var createSheetPresented = false
    @State private var navigationPath = NavigationPath()
    @State private var hasStartedDiscovery = false

    // MARK: - Derived shelf data from kernel snapshots

    private var joinedRooms: [CommunitySummary] {
        kernel.communities?.groups.map { $0.asCommunitySummary() } ?? []
    }

    private var featured: [CommunitySummary] {
        kernel.roomExplorer?.featured.map { $0.asCommunitySummary() } ?? []
    }

    private var newNoteworthy: [CommunitySummary] {
        kernel.roomExplorer?.newNoteworthy.map { $0.asCommunitySummary() } ?? []
    }

    private var friendsShelf: [RoomRecommendation] {
        kernel.roomExplorer?.friendsShelf.map { $0.asRoomRecommendation(reason: .friends) } ?? []
    }

    private var authorsShelf: [RoomRecommendation] {
        kernel.roomExplorer?.authorsShelf.map { $0.asRoomRecommendation(reason: .authors) } ?? []
    }

    /// True while no discovery data has arrived yet (show shimmer placeholder).
    private var isFirstLoad: Bool {
        kernel.roomExplorer == nil
    }

    var body: some View {
        NavigationStack(path: $navigationPath) {
            ScrollView {
                LazyVStack(spacing: 0, pinnedViews: []) {
                    heroSection
                        .padding(.bottom, 32)

                    errorBanner
                    yourRoomsShelf
                    friendsShelfSection
                    featuredShelf
                    authorsShelfSection
                    newShelf

                    browseAllFooter
                        .padding(.horizontal, 18)
                        .padding(.top, 28)
                        .padding(.bottom, 40)
                }
                .padding(.top, 4)
            }
            .background(Color.highlighterPaper.ignoresSafeArea())
            .navigationTitle("Rooms")
            .navigationBarTitleDisplayMode(.large)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        createSheetPresented = true
                    } label: {
                        Image(systemName: "plus.circle")
                            .font(.title3)
                    }
                    .accessibilityLabel("New room")
                }
            }
            .navigationDestination(for: String.self) { groupId in
                RoomHomeView(groupId: groupId)
            }
            .navigationDestination(for: ProfileDestination.self) { destination in
                switch destination {
                case .pubkey(let pk):
                    ProfileView(pubkey: pk)
                }
            }
            .navigationDestination(for: ArticleReaderTarget.self) { target in
                ArticleReaderView(target: target)
            }
            .globalUserToolbar()
            .sheet(item: $previewRoom) { room in
                NavigationStack {
                    RoomPreviewSheet(
                        room: room,
                        onJoin: {
                            // Dispatch JoinRoom kernel action — fire-and-forget (D6).
                            if let discoveredRow = kernel.roomExplorer?.newNoteworthy.first(where: { $0.groupId == room.id })
                                ?? kernel.roomExplorer?.featured.first(where: { $0.groupId == room.id }) {
                                kernel.app.dispatch(action: .joinRoom(
                                    groupId: discoveredRow.groupId,
                                    hostRelayUrl: discoveredRow.hostRelayUrl,
                                    inviteCode: nil
                                ))
                            }
                            previewRoom = nil
                        },
                        onOpenRoom: {
                            previewRoom = nil
                            navigationPath.append(room.id)
                        }
                    )
                }
                .environment(appStore)
            }
            .sheet(isPresented: $createSheetPresented) {
                CreateRoomSheet()
                    .environment(appStore)
                    .presentationDetents([.large])
            }
        }
        .task {
            // Dispatch StartRoomDiscovery once to kick relay discovery;
            // the kernel wires the DiscoveredGroupsProjection which feeds
            // KernelRoomExplorerSnapshot updates back via the observer.
            guard !hasStartedDiscovery else { return }
            hasStartedDiscovery = true
            // Use the discovery relay from AppConfig / RoomPolicy (injected at
            // construction time in the kernel). For Phase 3G the action takes
            // the relay URL; the kernel has the policy value.
            // We dispatch via the legacy safeCore path for the relay URL resolution
            // until Phase 4 wires the policy injection into AppConfig.
            Task { await appStore.safeCore.startRoomDiscovery() }
        }
        .refreshable {
            // Pull-to-refresh re-dispatches discovery; kernel projection
            // update arrives via the observer callback.
            Task { await appStore.safeCore.startRoomDiscovery() }
        }
    }

    // MARK: - Sections

    @ViewBuilder
    private var heroSection: some View {
        if !featured.isEmpty {
            ExplorerHeroView(rooms: featured) { room in
                previewRoom = room
            }
            .padding(.top, 4)
        } else if isFirstLoad {
            ExplorerHeroPlaceholder()
                .padding(.top, 4)
        }
    }

    @ViewBuilder
    private var errorBanner: some View {
        EmptyView()
    }

    @ViewBuilder
    private var yourRoomsShelf: some View {
        if !joinedRooms.isEmpty {
            VStack(alignment: .leading, spacing: 12) {
                shelfTitle("Your rooms", rationale: nil)

                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(alignment: .top, spacing: 14) {
                        ForEach(joinedRooms, id: \.id) { room in
                            NavigationLink(value: room.id) {
                                RoomCoverCard(room: room, width: 140)
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .padding(.horizontal, 18)
                }
            }
            .padding(.bottom, 28)
        }
    }

    @ViewBuilder
    private var friendsShelfSection: some View {
        if !friendsShelf.isEmpty {
            shelf(
                title: "Friends are here",
                rationale: "People you follow are members",
                content: {
                    ForEach(friendsShelf, id: \.summary.id) { rec in
                        Button {
                            previewRoom = rec.summary
                        } label: {
                            FriendsOnRoomCard(recommendation: rec)
                        }
                        .buttonStyle(.plain)
                    }
                }
            )
        }
    }

    @ViewBuilder
    private var featuredShelf: some View {
        if featured.count > 1 {
            // After the hero, show the rest of the featured list as a
            // regular-sized shelf so the curator's full picks remain
            // accessible below the hero.
            shelf(
                title: "Featured",
                rationale: "Curated by Highlighter",
                content: {
                    ForEach(Array(featured.dropFirst()), id: \.id) { room in
                        Button {
                            previewRoom = room
                        } label: {
                            RoomSquareTile(room: room)
                        }
                        .buttonStyle(.plain)
                    }
                }
            )
        }
    }

    @ViewBuilder
    private var authorsShelfSection: some View {
        if !authorsShelf.isEmpty {
            shelf(
                title: "Writers you read",
                rationale: "Authors you've highlighted post here",
                content: {
                    ForEach(authorsShelf, id: \.summary.id) { rec in
                        Button {
                            previewRoom = rec.summary
                        } label: {
                            RoomSquareTile(room: rec.summary)
                        }
                        .buttonStyle(.plain)
                    }
                }
            )
        }
    }

    @ViewBuilder
    private var newShelf: some View {
        if !newNoteworthy.isEmpty {
            shelf(
                title: "New & noteworthy",
                rationale: "Recently added rooms",
                content: {
                    ForEach(newNoteworthy, id: \.id) { room in
                        Button {
                            previewRoom = room
                        } label: {
                            RoomSquareTile(room: room)
                        }
                        .buttonStyle(.plain)
                    }
                }
            )
        }
    }

    private var browseAllFooter: some View {
        NavigationLink {
            RoomBrowseAllView(onOpenRoom: { id in
                navigationPath.append(id)
            })
        } label: {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Browse all rooms")
                        .font(.body.weight(.medium))
                        .foregroundStyle(Color.highlighterInkStrong)
                    Text("The full catalog, searchable")
                        .font(.footnote)
                        .foregroundStyle(Color.highlighterInkMuted)
                }
                Spacer()
                Image(systemName: "chevron.right")
                    .font(.footnote.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkMuted)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 18)
            .background(
                RoundedRectangle(cornerRadius: 14)
                    .stroke(Color.highlighterRule, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
    }

    // MARK: - Shelf shell

    private func shelfTitle(_ title: String, rationale: String?) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title.uppercased())
                .font(.footnote.weight(.semibold))
                .tracking(1.2)
                .foregroundStyle(Color.highlighterInkMuted)
            if let rationale {
                Text(rationale)
                    .font(.subheadline)
                    .foregroundStyle(Color.highlighterInkStrong)
            }
        }
        .padding(.horizontal, 18)
    }

    @ViewBuilder
    private func shelf<Content: View>(
        title: String,
        rationale: String,
        @ViewBuilder content: () -> Content
    ) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            shelfTitle(title, rationale: rationale)

            ScrollView(.horizontal, showsIndicators: false) {
                HStack(alignment: .top, spacing: 14) {
                    content()
                }
                .padding(.horizontal, 18)
            }
        }
        .padding(.bottom, 28)
    }
}

// MARK: - CommunitySummary + Identifiable

extension CommunitySummary: Identifiable {}

// MARK: - Placeholder

private struct ExplorerHeroPlaceholder: View {
    var body: some View {
        RoundedRectangle(cornerRadius: 20)
            .fill(Color.highlighterRule.opacity(0.4))
            .frame(height: 260)
            .padding(.horizontal, 18)
            .shimmer()
    }
}

// MARK: - Simple shimmer

private struct ShimmerModifier: ViewModifier {
    @State private var phase: CGFloat = -1

    func body(content: Content) -> some View {
        content
            .overlay(
                LinearGradient(
                    colors: [.clear, Color.white.opacity(0.25), .clear],
                    startPoint: .leading,
                    endPoint: .trailing
                )
                .rotationEffect(.degrees(20))
                .offset(x: phase * 400)
                .blendMode(.plusLighter)
                .mask(content)
            )
            .onAppear {
                withAnimation(.linear(duration: 1.4).repeatForever(autoreverses: false)) {
                    phase = 1.5
                }
            }
    }
}

private extension View {
    func shimmer() -> some View { modifier(ShimmerModifier()) }
}
