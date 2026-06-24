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
    @State private var selectedUrls: [String] = []
    @State private var isFetching = false
    @State private var errorText: String?
    @State private var isApplying = false

    private var projection: ImportRelaysProjection {
        // D1: inline import_relays_projection — map fetched relays to rows, compute counts.
        let selectedSet = Set(selectedUrls)
        var selectedConfigs: [RelayConfig] = []
        let rows: [ImportRelayRow] = fetched.map { config in
            let isSelected = selectedSet.contains(config.url)
            if isSelected { selectedConfigs.append(config) }
            let displayUrl = config.url.hasPrefix("wss://")
                ? String(config.url.dropFirst(6))
                : config.url
            let roleLabel: String
            switch (config.read, config.write) {
            case (true, true):  roleLabel = "Read + Write"
            case (true, false): roleLabel = "Read"
            case (false, true): roleLabel = "Write"
            default:            roleLabel = "No roles"
            }
            return ImportRelayRow(
                config: config,
                displayUrl: displayUrl,
                roleLabel: roleLabel,
                isSelected: isSelected
            )
        }
        let foundCount = rows.count
        return ImportRelaysProjection(
            rows: rows,
            selectedCount: UInt64(selectedConfigs.count),
            foundTitle: "Found \(foundCount) relay\(foundCount == 1 ? "" : "s")",
            canApply: !selectedConfigs.isEmpty,
            selectedConfigs: selectedConfigs
        )
    }

    private var sourceProjection: ImportRelaysSourceProjection {
        let submitNpub = npubText.trimmingCharacters(in: .whitespaces)
        return ImportRelaysSourceProjection(submitNpub: submitNpub, canFetch: !submitNpub.isEmpty && !isFetching)
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
        let source = sourceProjection
        guard source.canFetch else { return }
        errorText = nil
        fetched = []
        selectedUrls = []
        isFetching = true
        defer { isFetching = false }
        // NOTE: importRelaysFromNpubSnapshot removed (relay_polish.rs deleted in
        // Phase 7 teardown). Relay import is unavailable until the kernel exposes it.
        _ = source.submitNpub
        errorText = "Relay import is temporarily unavailable."
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
        // D1: toggle url in selectedUrls while preserving fetched order.
        if selectedUrls.contains(url) {
            selectedUrls = selectedUrls.filter { $0 != url }
        } else {
            let fetchedOrder = fetched.map { $0.url }
            selectedUrls = (selectedUrls + [url]).sorted { a, b in
                (fetchedOrder.firstIndex(of: a) ?? Int.max) < (fetchedOrder.firstIndex(of: b) ?? Int.max)
            }
        }
    }
}
