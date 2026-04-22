import SwiftUI

struct CommunitiesTabView: View {
    @Environment(HighlighterStore.self) private var store

    @State private var showSettings = false

    var body: some View {
        NavigationStack {
            Group {
                if store.joinedCommunities.isEmpty {
                    emptyState
                } else {
                    List(store.joinedCommunities, id: \.id) { community in
                        NavigationLink(value: community.id) {
                            CommunityRow(community: community)
                        }
                    }
                    .listStyle(.plain)
                }
            }
            .navigationTitle("Communities")
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
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        showSettings = true
                    } label: {
                        Image(systemName: "gearshape")
                    }
                    .accessibilityLabel("Settings")
                }
                ToolbarItem(placement: .topBarTrailing) {
                    if let me = store.currentUser {
                        NavigationLink(value: ProfileDestination.pubkey(me.pubkey)) {
                            AuthorAvatar(
                                pubkey: me.pubkey,
                                pictureURL: store.currentUserProfile?.picture ?? "",
                                displayInitial: preferredInitial(for: me),
                                size: 30
                            )
                        }
                        .accessibilityLabel("Your profile")
                    }
                }
            }
            .sheet(isPresented: $showSettings) {
                SettingsView()
                    .environment(store)
            }
        }
    }

    private var emptyState: some View {
        VStack(spacing: 12) {
            Text("No communities yet")
                .font(.headline)
            Text("Join one from the web or ask for an invite.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    /// Best-effort single letter for the avatar fallback: first letter of
    /// the profile's display name, then name, then pubkey prefix.
    private func preferredInitial(for user: CurrentUser) -> String {
        if let profile = store.currentUserProfile {
            if let ch = profile.displayName.first { return String(ch) }
            if let ch = profile.name.first { return String(ch) }
        }
        return String(user.pubkey.prefix(1))
    }
}

struct CommunityRow: View {
    let community: CommunitySummary

    var body: some View {
        HStack(spacing: 12) {
            if let url = URL(string: community.picture), !community.picture.isEmpty {
                AsyncImage(url: url) { img in
                    img.resizable().scaledToFill()
                } placeholder: {
                    Color.secondary.opacity(0.1)
                }
                .frame(width: 48, height: 48)
                .clipShape(.rect(cornerRadius: 12))
            } else {
                RoundedRectangle(cornerRadius: 12)
                    .fill(.tertiary)
                    .frame(width: 48, height: 48)
            }
            VStack(alignment: .leading, spacing: 2) {
                Text(community.name)
                    .font(.body.weight(.medium))
                if !community.about.isEmpty {
                    Text(community.about)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                }
            }
            Spacer()
        }
        .padding(.vertical, 4)
    }
}
