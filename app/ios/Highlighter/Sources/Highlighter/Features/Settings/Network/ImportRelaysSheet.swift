import SwiftUI

/// Lets the user import another nostr account's relay list. Rust owns the
/// fetch, candidate projection, selection set, and apply operation.
struct ImportRelaysSheet: View {
    @Environment(HighlighterStore.self) private var appStore
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Form {
                npubSection
                if !importState.candidates.isEmpty {
                    foundSection
                }
                if let err = importState.errorMessage {
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
                    Button("Add \(importState.selectedUrls.count)") {
                        appStore.applyNetworkImportRelays()
                    }
                    .disabled(importState.selectedUrls.isEmpty || importState.isApplying)
                }
            }
            .onChange(of: importState.isApplying) { wasApplying, isApplying in
                if wasApplying, !isApplying, importState.errorMessage == nil {
                    dismiss()
                }
            }
        }
    }

    private var importState: HighlighterNetworkImportSnapshot {
        appStore.network.importRelays
    }

    private var npubBinding: Binding<String> {
        Binding(
            get: { importState.npub },
            set: { appStore.setNetworkImportNpub($0) }
        )
    }

    private var selectedUrls: Set<String> {
        Set(importState.selectedUrls)
    }

    private var npubSection: some View {
        Section {
            TextField("npub1… or hex pubkey", text: npubBinding)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .monospaced()
            Button {
                appStore.fetchNetworkImportRelays()
            } label: {
                if importState.isFetching {
                    HStack {
                        ProgressView().scaleEffect(0.7)
                        Text("Fetching…")
                    }
                } else {
                    Label("Fetch relays", systemImage: "arrow.down.circle")
                }
            }
            .disabled(importState.npub.trimmingCharacters(in: .whitespaces).isEmpty || importState.isFetching)
        } header: {
            Text("Source")
        } footer: {
            Text("Highlighter will fetch the user's kind:10002 event through your Indexer relays. Turn on Indexer for at least one relay first.")
        }
    }

    private var foundSection: some View {
        Section {
            ForEach(importState.candidates, id: \.url) { row in
                Button {
                    appStore.toggleNetworkImportRelay(url: row.url)
                } label: {
                    HStack {
                        Image(systemName: selectedUrls.contains(row.url) ? "checkmark.circle.fill" : "circle")
                            .foregroundStyle(selectedUrls.contains(row.url) ? Color.accentColor : .secondary)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(displayURL(row.url))
                                .font(.subheadline)
                                .lineLimit(1)
                                .truncationMode(.middle)
                            Text(roleLabel(row))
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                    }
                }
                .buttonStyle(.plain)
            }
        } header: {
            Text("Found \(importState.candidates.count) relay\(importState.candidates.count == 1 ? "" : "s")")
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

    private func displayURL(_ raw: String) -> String {
        if raw.hasPrefix("wss://") { return String(raw.dropFirst(6)) }
        return raw
    }

    private func roleLabel(_ row: RelayConfig) -> String {
        switch (row.read, row.write) {
        case (true, true): return "Read + Write"
        case (true, false): return "Read"
        case (false, true): return "Write"
        default: return "No roles"
        }
    }
}
