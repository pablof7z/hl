import Kingfisher
import SwiftUI

/// Gorgeous profile screen. Reusable for any pubkey — the logged-in user's
/// own profile is just `ProfileView(pubkey: currentUser.pubkey)`. Navigation
/// is driven from the Communities tab's `NavigationStack` via
/// `ProfileDestination`.
struct ProfileView: View {
    @Environment(HighlighterStore.self) private var appStore
    @State private var editPresented = false
    @State private var activeTab: ProfileTab = .articles

    let pubkey: String

    var body: some View {
        Group {
            if isActiveProfile {
                content(profile: appStore.profileView)
            } else {
                ProgressView()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .background(Color.highlighterPaper.ignoresSafeArea())
        .navigationBarTitleDisplayMode(.inline)
        .task(id: pubkey) {
            appStore.openProfile(pubkeyHex: pubkey)
        }
        .onDisappear {
            appStore.closeProfile()
        }
    }

    private var isActiveProfile: Bool {
        appStore.profileView.pubkeyHex.caseInsensitiveCompare(
            pubkey.trimmingCharacters(in: .whitespacesAndNewlines)
        ) == .orderedSame
    }

    private func content(profile: HighlighterProfileViewSnapshot) -> some View {
        ScrollView {
            VStack(spacing: 0) {
                HeroBanner(bannerURL: profile.profile?.banner ?? "")
                VStack(spacing: 20) {
                    IdentityBlock(profile: profile, pubkey: pubkey)
                        .padding(.horizontal, 24)
                    ActionRow(profile: profile, onEdit: { editPresented = true })
                        .padding(.horizontal, 24)
                    StatsStrip(profile: profile)
                        .padding(.horizontal, 24)
                    TabBar(activeTab: Binding(
                        get: { activeTab },
                        set: { activeTab = $0 }
                    ))
                    .padding(.horizontal, 24)
                    .padding(.top, 8)
                    TabContent(profile: profile, activeTab: activeTab)
                        .padding(.horizontal, 24)
                        .padding(.bottom, 32)
                }
                .padding(.top, -44) // avatar overlaps the banner
            }
            .frame(maxWidth: 560)
            .frame(maxWidth: .infinity)
        }
        .ignoresSafeArea(edges: .top)
        .sheet(isPresented: $editPresented) {
            EditProfileSheet(initial: profile.profile) { _ in
                appStore.refreshProfile()
                appStore.requestProfile(pubkeyHex: pubkey)
            }
            .environment(appStore)
            .presentationDetents([.large])
        }
        .safeAreaInset(edge: .bottom) {
            if let message = profile.errorMessage {
                Text(message)
                    .font(.footnote)
                    .foregroundStyle(.white)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 10)
                    .background(Color.red.opacity(0.9), in: Capsule())
                    .padding(.horizontal, 24)
                    .padding(.bottom, 12)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
    }
}

private enum ProfileTab: Hashable {
    case articles, highlights, communities
}

// MARK: - Hero banner

private struct HeroBanner: View {
    let bannerURL: String

    @State private var showFullScreen = false

    var body: some View {
        GeometryReader { geo in
            Group {
                if let url = URL(string: bannerURL), !bannerURL.isEmpty {
                    KFImage(url)
                        .placeholder { fallback }
                        .fade(duration: 0.2)
                        .resizable()
                        .scaledToFill()
                        .onTapGesture { showFullScreen = true }
                        .accessibilityAddTraits(.isButton)
                        .accessibilityLabel("Profile banner")
                        .accessibilityHint("View full screen")
                } else {
                    fallback
                }
            }
            .frame(width: geo.size.width, height: bannerHeight)
            .clipped()
        }
        .frame(height: bannerHeight)
        .fullScreenCover(isPresented: $showFullScreen) {
            ImageZoomView(url: URL(string: bannerURL), onDismiss: { showFullScreen = false })
        }
    }

    private var bannerHeight: CGFloat { 160 }

    private var fallback: some View {
        LinearGradient(
            colors: [
                Color.highlighterAccent.opacity(0.35),
                Color.highlighterRule,
            ],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )
    }
}

// MARK: - Identity block

private struct IdentityBlock: View {
    let profile: HighlighterProfileViewSnapshot
    let pubkey: String

    var body: some View {
        VStack(spacing: 12) {
            AuthorAvatar(
                pubkey: pubkey,
                pictureURL: profile.profile?.picture ?? "",
                displayInitial: displayName.first.map { String($0) } ?? "",
                size: 88,
                ringWidth: 4
            )

            Text(displayName)
                .font(.system(.largeTitle, design: .default).weight(.semibold))
                .foregroundStyle(Color.highlighterInkStrong)
                .multilineTextAlignment(.center)
                .lineLimit(2)

            if let nip05 = verifiedNip05 {
                HStack(spacing: 4) {
                    Image(systemName: "checkmark.seal.fill")
                        .foregroundStyle(Color.highlighterAccent)
                        .font(.caption2)
                    Text(nip05)
                        .font(.caption)
                        .foregroundStyle(Color.highlighterInkMuted)
                }
            }

            if !bio.isEmpty {
                NostrRichText(content: bio, font: .body, ink: .highlighterInkMuted)
                    .frame(maxWidth: 480)
                    .multilineTextAlignment(.center)
            }
        }
    }

    private var displayName: String {
        let dn = profile.profile?.displayName ?? ""
        if !dn.isEmpty { return dn }
        let n = profile.profile?.name ?? ""
        if !n.isEmpty { return n }
        return String(pubkey.prefix(12))
    }

    private var bio: String {
        profile.profile?.about ?? ""
    }

    private var verifiedNip05: String? {
        let raw = profile.profile?.nip05 ?? ""
        guard !raw.isEmpty else { return nil }
        // Strip leading `_@` the spec permits for root-level identifiers.
        return raw.hasPrefix("_@") ? String(raw.dropFirst(2)) : raw
    }
}

// MARK: - Action row

private struct ActionRow: View {
    @Environment(HighlighterStore.self) private var appStore
    let profile: HighlighterProfileViewSnapshot
    let onEdit: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            if profile.isOwnProfile {
                editButton
            } else if profile.viewerPubkeyHex != nil {
                followButton
            }
            if let website = profile.profile?.website, !website.isEmpty,
               let url = URL(string: website)
            {
                Link(destination: url) {
                    Label(url.host ?? website, systemImage: "link")
                        .font(.subheadline.weight(.medium))
                }
                .buttonStyle(.glass)
                .tint(Color.highlighterAccent)
            }
        }
    }

    private var editButton: some View {
        Button(action: onEdit) {
            Text("Edit profile")
                .font(.subheadline.weight(.semibold))
                .frame(minWidth: 96)
        }
        .buttonStyle(.glass)
        .tint(Color.highlighterAccent)
    }

    @ViewBuilder
    private var followButton: some View {
        let action: () -> Void = {
            appStore.toggleProfileFollow()
        }
        if profile.isFollowing {
            Button(action: action) { followButtonLabel }
                .buttonStyle(.glass)
                .tint(Color.highlighterAccent)
                .disabled(profile.isMutatingFollow)
        } else {
            Button(action: action) { followButtonLabel }
                .buttonStyle(.glassProminent)
                .tint(Color.highlighterAccent)
                .disabled(profile.isMutatingFollow)
        }
    }

    private var followButtonLabel: some View {
        HStack(spacing: 6) {
            if profile.isMutatingFollow {
                ProgressView()
                    .controlSize(.small)
            }
            Text(profile.isFollowing ? "Following" : "Follow")
                .font(.subheadline.weight(.semibold))
        }
        .frame(minWidth: 96)
    }
}

// MARK: - Stats strip

private struct StatsStrip: View {
    let profile: HighlighterProfileViewSnapshot

    var body: some View {
        HStack(spacing: 18) {
            stat("\(profile.articleCount)", label: "articles")
            divider
            stat("\(profile.highlightCount)", label: "highlights")
            divider
            stat("\(profile.communityCount)", label: "communities")
        }
        .frame(maxWidth: .infinity)
    }

    private func stat(_ value: String, label: String) -> some View {
        VStack(spacing: 2) {
            Text(value)
                .font(.headline)
                .foregroundStyle(Color.highlighterInkStrong)
            Text(label)
                .font(.caption2)
                .foregroundStyle(Color.highlighterInkMuted)
                .textCase(.uppercase)
        }
    }

    private var divider: some View {
        Rectangle()
            .fill(Color.highlighterRule)
            .frame(width: 1, height: 24)
    }
}

// MARK: - Tab bar

private struct TabBar: View {
    @Binding var activeTab: ProfileTab

    var body: some View {
        HStack(spacing: 28) {
            tab(.articles, label: "Writing")
            tab(.highlights, label: "Highlights")
            tab(.communities, label: "Communities")
        }
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(Color.highlighterRule)
                .frame(height: 1)
        }
    }

    private func tab(_ value: ProfileTab, label: String) -> some View {
        let isActive = activeTab == value
        return Button {
            withAnimation(.easeOut(duration: 0.15)) {
                activeTab = value
            }
        } label: {
            VStack(spacing: 8) {
                Text(label)
                    .font(.subheadline.weight(.medium))
                    .foregroundStyle(
                        isActive ? Color.highlighterInkStrong : Color.highlighterInkMuted
                    )
                Rectangle()
                    .fill(isActive ? Color.highlighterAccent : Color.clear)
                    .frame(height: 2)
            }
        }
        .buttonStyle(.plain)
    }
}

// MARK: - Tab content

private struct TabContent: View {
    let profile: HighlighterProfileViewSnapshot
    let activeTab: ProfileTab
    @Environment(HighlighterStore.self) private var appStore
    @State private var previewRoom: CommunitySummary?
    @State private var pendingOpenRoomId: String?
    @State private var openRoomGroupId: String?

    var body: some View {
        switch activeTab {
        case .articles:
            articlesList
        case .highlights:
            highlightsList
        case .communities:
            communitiesList
        }
    }

    @ViewBuilder
    private var articlesList: some View {
        if profile.articles.isEmpty {
            emptyState(
                systemImage: "text.alignleft",
                title: "No articles yet",
                message: profile.isLoading ? "Loading..." : "This author hasn't published any long-form writing."
            )
        } else {
            LazyVStack(spacing: 0) {
                ForEach(Array(profile.articles.enumerated()), id: \.element.eventId) { index, article in
                    NavigationLink(value: ArticleReaderTarget(
                        pubkey: article.pubkey,
                        dTag: article.identifier,
                        seed: article
                    )) {
                        ArticleCardView(article: article)
                    }
                    .buttonStyle(.plain)
                    .articleRowActions(article: article)
                    if index < profile.articles.count - 1 {
                        Divider().foregroundStyle(Color.highlighterRule)
                    }
                }
            }
        }
    }

    @ViewBuilder
    private var highlightsList: some View {
        if profile.highlights.isEmpty {
            emptyState(
                systemImage: "quote.bubble",
                title: "No highlights yet",
                message: profile.isLoading ? "Loading..." : "Passages this person saves will appear here."
            )
        } else {
            LazyVStack(spacing: 16) {
                ForEach(profile.highlights, id: \.eventId) { highlight in
                    HighlightFeedCardView(items: [
                        HydratedHighlight(
                            highlight: highlight,
                            artifact: nil,
                            sharedByEventId: nil,
                            sharedByPubkey: nil
                        ),
                    ])
                }
            }
            .padding(.top, 8)
        }
    }

    @ViewBuilder
    private var communitiesList: some View {
        if profile.communities.isEmpty {
            emptyState(
                systemImage: "square.grid.2x2",
                title: "No communities",
                message: profile.isLoading ? "Loading..." : "Not a member of any Highlighter communities yet."
            )
        } else {
            LazyVStack(spacing: 0) {
                ForEach(Array(profile.communities.enumerated()), id: \.element.id) { index, community in
                    Button {
                        previewRoom = community
                    } label: {
                        CommunityRowView(community: community)
                    }
                    .buttonStyle(.plain)
                    if index < profile.communities.count - 1 {
                        Divider().foregroundStyle(Color.highlighterRule)
                    }
                }
            }
            .sheet(item: $previewRoom, onDismiss: {
                if let id = pendingOpenRoomId {
                    openRoomGroupId = id
                    pendingOpenRoomId = nil
                }
            }) { room in
                NavigationStack {
                    RoomPreviewSheet(
                        room: room,
                        onJoin: {
                            appStore.requestJoinRoom(groupId: room.id, roomName: room.name)
                            previewRoom = nil
                        },
                        onOpenRoom: {
                            pendingOpenRoomId = room.id
                            previewRoom = nil
                        }
                    )
                }
                .environment(appStore)
            }
            .navigationDestination(item: $openRoomGroupId) { id in
                RoomHomeView(groupId: id)
            }
        }
    }

    private func emptyState(systemImage: String, title: String, message: String) -> some View {
        VStack(spacing: 10) {
            Image(systemName: systemImage)
                .font(.title)
                .foregroundStyle(Color.highlighterInkMuted)
            Text(title)
                .font(.headline)
                .foregroundStyle(Color.highlighterInkStrong)
            Text(message)
                .font(.subheadline)
                .foregroundStyle(Color.highlighterInkMuted)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 48)
    }
}
