import SwiftUI

/// Lets the user import another nostr account's relay list. Takes an npub
/// (or hex pubkey), previews that user's NMP-cached kind:10002 mailbox,
/// and shows the discovered relays with checkboxes. Merging is opt-in —
/// only rows the user ticks get upserted.
struct ImportRelaysSheet: View {
    let store: NetworkSettingsStore

    @Environment(\.dismiss) private var dismiss

    @State private var npubText: String = ""
    @State private var fetched: [RelayConfig] = []
    @State private var selectedUrls: [String] = []
    @State private var isFetching = false
    @State private var errorText: String?
    @State private var isApplying = false

    private var projection: ImportRelaysProjection {
        store.importRelaysProjection(fetched: fetched, selectedUrls: selectedUrls)
    }

    private var sourceProjection: ImportRelaysSourceProjection {
        store.importRelaysSourceProjection(npub: npubText, isFetching: isFetching)
    }

    var body: some View {
        let currentProjection = projection

        NavigationStack {
            Form {
                npubSection
                if !currentProjection.rows.isEmpty {
                    foundSection(currentProjection)
                }
                if let err = errorText {
                    errorSection(err)
                }
            }
            .navigationTitle("Import from npub")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Add \(currentProjection.selectedCount)") {
                        Task { await applySelected() }
                    }
                    .disabled(!currentProjection.canApply || isApplying)
                }
            }
        }
    }

    // MARK: - Sections

    private var npubSection: some View {
        Section {
            TextField("npub1… or hex pubkey", text: $npubText)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .monospaced()
            Button {
                Task { await fetch() }
            } label: {
                if isFetching {
                    HStack {
                        ProgressView().scaleEffect(0.7)
                        Text("Fetching…")
                    }
                } else {
                    Label("Fetch relays", systemImage: "arrow.down.circle")
                }
            }
            .disabled(!sourceProjection.canFetch)
        } header: {
            Text("Source")
        }
    }

    private func foundSection(_ projection: ImportRelaysProjection) -> some View {
        Section {
            ForEach(projection.rows, id: \.config.url) { row in
                Button {
                    toggle(row.config.url)
                } label: {
                    HStack {
                        Image(systemName: row.isSelected ? "checkmark.circle.fill" : "circle")
                            .foregroundStyle(row.isSelected ? Color.accentColor : .secondary)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(row.displayUrl)
                                .font(.subheadline)
                                .lineLimit(1)
                                .truncationMode(.middle)
                            Text(row.roleLabel)
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                    }
                }
                .buttonStyle(.plain)
            }
        } header: {
            Text(projection.foundTitle)
        } footer: {
            Text("Selected relays will be added or updated in your list with their original Read/Write roles. Rooms and Indexer stay off — tap a relay later to turn them on.")
        }
    }

    private func errorSection(_ err: String) -> some View {
        Section {
            Label(err, systemImage: "exclamationmark.triangle")
                .font(.caption)
                .foregroundStyle(.orange)
        }
    }

    // MARK: - Actions

    private func fetch() async {
        let source = sourceProjection
        guard source.canFetch else { return }
        errorText = nil
        fetched = []
        selectedUrls = []
        isFetching = true
        defer { isFetching = false }
        let snapshot = store.importRelaysForPubkey(source.submitNpub)
        if !snapshot.errorMessage.isEmpty {
            errorText = snapshot.errorMessage
        } else if snapshot.fetched.isEmpty {
            errorText = "No cached relay list found for that pubkey."
        } else {
            fetched = snapshot.fetched
            selectedUrls = snapshot.selectedUrls
        }
    }

    private func applySelected() async {
        isApplying = true
        defer { isApplying = false }
        let selectedConfigs = projection.selectedConfigs
        for row in selectedConfigs {
            await store.upsert(row)
        }
        dismiss()
    }

    private func toggle(_ url: String) {
        selectedUrls = store.toggleImportRelaySelection(
            fetched: fetched,
            selectedUrls: selectedUrls,
            url: url
        )
    }
}
