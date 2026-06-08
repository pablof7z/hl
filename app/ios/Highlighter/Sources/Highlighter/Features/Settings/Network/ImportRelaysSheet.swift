import SwiftUI

/// Lets the user import another nostr account's relay list. Takes an npub
/// (or hex pubkey), fetches that user's kind:10002 via the Indexer pool,
/// and shows the discovered relays with checkboxes. Merging is opt-in —
/// only rows the user ticks get upserted.
struct ImportRelaysSheet: View {
    let store: NetworkSettingsStore

    @Environment(HighlighterStore.self) private var appStore
    @Environment(\.dismiss) private var dismiss

    @State private var npubText: String = ""
    @State private var fetched: [RelayConfig] = []
    @State private var selected: Set<String> = []
    @State private var isFetching = false
    @State private var errorText: String?
    @State private var isApplying = false

    private var projection: ImportRelaysProjection {
        appStore.safeCore.projectImportRelays(input: ImportRelaysProjectionInput(
            fetched: fetched,
            selectedUrls: Array(selected)
        ))
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
            .disabled(npubText.trimmingCharacters(in: .whitespaces).isEmpty || isFetching)
        } header: {
            Text("Source")
        } footer: {
            Text("Highlighter will fetch the user's kind:10002 event through your Indexer relays. Turn on Indexer for at least one relay first.")
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
        errorText = nil
        fetched = []
        selected = []
        isFetching = true
        defer { isFetching = false }
        let outcome = await appStore.safeCore
            .importRelaysFromNpub(npubText.trimmingCharacters(in: .whitespaces))
        if outcome.error.isEmpty {
            fetched = outcome.values
            selected = Set(appStore.safeCore.defaultImportRelaySelection(relays: outcome.values))
        } else {
            errorText = outcome.error
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
        if selected.contains(url) {
            selected.remove(url)
        } else {
            selected.insert(url)
        }
    }
}
