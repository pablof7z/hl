import SwiftUI

/// Native-feeling Settings surface. Mirrors the layout of iOS System
/// Settings: inset-grouped list, account card on top, small grouped
/// sections below, destructive Log Out at the bottom.
struct SettingsView: View {
    @Environment(HighlighterStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    @State private var showLogoutConfirm = false
    @State private var copiedNpub = false

    var body: some View {
        NavigationStack {
            List {
                accountSection
                mediaSection
                aboutSection
                logOutSection
            }
            .listStyle(.insetGrouped)
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") { dismiss() }
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
                HStack(spacing: 14) {
                    ZStack {
                        Circle()
                            .fill(.tertiary)
                            .frame(width: 60, height: 60)
                        Image(systemName: "person.fill")
                            .font(.title)
                            .foregroundStyle(.secondary)
                    }
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Nostr Account")
                            .font(.headline)
                        Text(shortenedNpub(user.npub))
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                            .monospaced()
                    }
                    Spacer()
                }
                .padding(.vertical, 6)

                Button {
                    UIPasteboard.general.string = user.npub
                    copiedNpub = true
                    Task {
                        try? await Task.sleep(for: .seconds(2))
                        copiedNpub = false
                    }
                } label: {
                    HStack {
                        Label(copiedNpub ? "Copied" : "Copy npub",
                              systemImage: copiedNpub ? "checkmark" : "doc.on.doc")
                        Spacer()
                    }
                }
            }
        }
    }

    private var mediaSection: some View {
        Section {
            NavigationLink("Media") {
                MediaSettingsView()
            }
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

    private func shortenedNpub(_ npub: String) -> String {
        guard npub.count > 20 else { return npub }
        return "\(npub.prefix(12))…\(npub.suffix(6))"
    }

    private var appVersionString: String {
        let info = Bundle.main.infoDictionary
        let version = info?["CFBundleShortVersionString"] as? String ?? "—"
        let build = info?["CFBundleVersion"] as? String ?? "—"
        return "\(version) (\(build))"
    }
}
