import SwiftUI

struct SettingsView: View {
    @Environment(HighlighterStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    @State private var showLogoutConfirm = false

    var body: some View {
        NavigationStack {
            List {
                accountSection
                connectionsSection
                keysSection
                aboutSection
                logOutSection
            }
            .listStyle(.insetGrouped)
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") { dismiss() }
                        .fontWeight(.semibold)
                }
            }
            .confirmationDialog(
                "Log out of Highlighter?",
                isPresented: $showLogoutConfirm,
                titleVisibility: .visible
            ) {
                Button("Log Out", role: .destructive) {
                    store.logout()
                    dismiss()
                }
                Button("Cancel", role: .cancel) {}
            } message: {
                Text("You'll need your signer to sign back in.")
            }
        }
    }

    // MARK: - Sections

    @ViewBuilder
    private var accountSection: some View {
        if let user = store.currentUser {
            Section {
                HStack(spacing: 16) {
                    let projection = profileDisplay(for: user)
                    AuthorAvatar(
                        pubkey: user.pubkey,
                        pictureURL: projection.pictureUrl,
                        displayInitial: projection.displayInitial,
                        size: 68
                    )
                    VStack(alignment: .leading, spacing: 4) {
                        Text(projection.displayName)
                            .font(.title3.weight(.semibold))
                        Text(publicKeyDisplay(for: user).compactLabel)
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                            .monospaced()
                            .lineLimit(1)
                    }
                    Spacer()
                }
                .padding(.vertical, 6)
            }
        }
    }

    private var connectionsSection: some View {
        Section {
            NavigationLink {
                NetworkSettingsView()
            } label: {
                Label("Network", systemImage: "network")
            }
            NavigationLink {
                MediaSettingsView()
            } label: {
                Label("Media", systemImage: "photo.on.rectangle.angled")
            }
        }
    }

    private var keysSection: some View {
        Section {
            NavigationLink {
                KeysView()
            } label: {
                Label("Secret Key", systemImage: "key.fill")
            }
        } header: {
            Text("Keys")
        } footer: {
            Text("Your nsec is the master key to your Nostr identity. Never share it.")
        }
    }

    private var aboutSection: some View {
        Section("About") {
            LabeledContent("Version", value: appVersionString)
        }
    }

    private var logOutSection: some View {
        Section {
            Button(role: .destructive) {
                showLogoutConfirm = true
            } label: {
                HStack {
                    Spacer()
                    Text("Log Out")
                        .fontWeight(.semibold)
                    Spacer()
                }
            }
        }
    }

    // MARK: - Helpers

    private func profileDisplay(for user: CurrentUser) -> ProfileDisplayProjection {
        let profile = store.currentUserProfile
        let name = (profile?.displayName ?? "").isEmpty
            ? ((profile?.name ?? "").isEmpty ? "Nostr Account" : profile!.name)
            : profile!.displayName
        let initial = (profile?.displayName ?? "").isEmpty
            ? ((profile?.name ?? "").isEmpty ? "" : String(profile!.name.prefix(1)))
            : String(profile!.displayName.prefix(1))
        return ProfileDisplayProjection(
            displayName: name,
            displayInitial: initial,
            pictureUrl: profile?.picture ?? ""
        )
    }

    private func publicKeyDisplay(for user: CurrentUser) -> PublicKeyDisplayProjection {
        let npub = user.npub
        let label: String
        if npub.count <= 20 {
            label = npub
        } else {
            let prefix = String(npub.prefix(10))
            let suffix = String(npub.suffix(8))
            label = "\(prefix)…\(suffix)"
        }
        return PublicKeyDisplayProjection(compactLabel: label)
    }

    private var appVersionString: String {
        let info = Bundle.main.infoDictionary
        let version = info?["CFBundleShortVersionString"] as? String ?? "—"
        let build = info?["CFBundleVersion"] as? String ?? "—"
        return "\(version) (\(build))"
    }
}
