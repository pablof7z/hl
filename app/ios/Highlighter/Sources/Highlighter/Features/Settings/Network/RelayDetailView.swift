import SwiftUI

/// Detail screen for a single relay. Big state header + cumulative traffic +
/// role toggles + Remove action.
struct RelayDetailView: View {
    let url: String
    let store: NetworkSettingsStore

    @Environment(\.dismiss) private var dismiss
    @State private var showRemoveConfirm = false
    @State private var isSaving = false

    var body: some View {
        List {
            headerSection
            statsSection
            rolesSection
            removeSection
        }
        .listStyle(.insetGrouped)
        .navigationTitle("Relay")
        .navigationBarTitleDisplayMode(.inline)
        .confirmationDialog(
            "Remove this relay?",
            isPresented: $showRemoveConfirm,
            titleVisibility: .visible
        ) {
            Button("Remove", role: .destructive) {
                Task {
                    await store.remove(url)
                    dismiss()
                }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Highlighter will stop sending and receiving events through this relay.")
        }
    }

    // MARK: - Sections

    private var config: RelayConfig? {
        store.relays.first(where: { $0.url == url })
    }

    private var diagnostic: RelayDiagnostic? {
        store.diagnostic(for: url)
    }

    @ViewBuilder
    private var headerSection: some View {
        Section {
            VStack(alignment: .leading, spacing: 8) {
                Text(url)
                    .font(.subheadline.monospaced())
                    .lineLimit(2)
                    .truncationMode(.middle)
                HStack(spacing: 8) {
                    stateDot
                    Text(stateLabel)
                        .font(.headline)
                    Spacer()
                    if let rtt = diagnostic?.rttMs {
                        Text("\(rtt) ms")
                            .font(.subheadline.monospacedDigit())
                            .foregroundStyle(.secondary)
                    }
                }
            }
            .padding(.vertical, 4)
        }
    }

    @ViewBuilder
    private var statsSection: some View {
        if let d = diagnostic {
            Section("Traffic") {
                LabeledContent("Received", value: formatBytes(d.bytesReceived))
                LabeledContent("Sent", value: formatBytes(d.bytesSent))
                if let since = d.connectedSinceTs {
                    LabeledContent(
                        "Connected since",
                        value: formatUnixSeconds(since)
                    )
                }
            }
        }
    }

    @ViewBuilder
    private var rolesSection: some View {
        if let cfg = config {
            Section {
                ToggleRow(label: "Read", isOn: cfg.read) { on in
                    Task { await applyRoles(cfg, read: on) }
                }
                ToggleRow(label: "Write", isOn: cfg.write) { on in
                    Task { await applyRoles(cfg, write: on) }
                }
                ToggleRow(label: "Rooms", isOn: cfg.rooms) { on in
                    Task { await applyRoles(cfg, rooms: on) }
                }
                ToggleRow(label: "Indexer", isOn: cfg.indexer) { on in
                    Task { await applyRoles(cfg, indexer: on) }
                }
            } header: {
                Text("Roles")
            } footer: {
                Text("Changing Read or Write republishes your kind:10002 relay list.")
            }
        }
    }

    private var removeSection: some View {
        Section {
            Button(role: .destructive) {
                showRemoveConfirm = true
            } label: {
                HStack {
                    Spacer()
                    Text("Remove Relay").fontWeight(.semibold)
                    Spacer()
                }
            }
            .disabled(isSaving)
        }
    }

    // MARK: - State pieces

    @ViewBuilder
    private var stateDot: some View {
        let color: Color = {
            switch diagnostic?.state {
            case .connected: return .green
            case .connecting: return .yellow
            case .disconnected, .terminated, .banned: return .red
            case .none: return .gray
            }
        }()
        Circle().fill(color).frame(width: 12, height: 12)
    }

    private var stateLabel: String {
        switch diagnostic?.state {
        case .connected: return "Connected"
        case .connecting: return "Connecting…"
        case .disconnected: return "Disconnected"
        case .terminated: return "Terminated"
        case .banned: return "Banned"
        case .none: return "Unknown"
        }
    }

    // MARK: - Actions

    private func applyRoles(
        _ cfg: RelayConfig,
        read: Bool? = nil,
        write: Bool? = nil,
        rooms: Bool? = nil,
        indexer: Bool? = nil
    ) async {
        isSaving = true
        defer { isSaving = false }
        await store.setRoles(
            url: cfg.url,
            read: read ?? cfg.read,
            write: write ?? cfg.write,
            rooms: rooms ?? cfg.rooms,
            indexer: indexer ?? cfg.indexer
        )
    }

    // MARK: - Formatting

    private func formatBytes(_ bytes: UInt64) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(bytes), countStyle: .binary)
    }

    private func formatUnixSeconds(_ seconds: UInt64) -> String {
        let date = Date(timeIntervalSince1970: TimeInterval(seconds))
        let f = DateFormatter()
        f.dateStyle = .short
        f.timeStyle = .short
        return f.string(from: date)
    }
}

/// Thin Toggle wrapper that notifies on commit only — avoids firing the
/// async save for each interim state during a drag.
private struct ToggleRow: View {
    let label: String
    let isOn: Bool
    let onChange: (Bool) -> Void

    @State private var localValue: Bool = false
    @State private var didInit = false

    var body: some View {
        Toggle(label, isOn: Binding(
            get: { didInit ? localValue : isOn },
            set: { newValue in
                localValue = newValue
                didInit = true
                onChange(newValue)
            }
        ))
        .onChange(of: isOn) { _, newValue in
            // Source-of-truth sync after the parent reloads from Rust.
            localValue = newValue
            didInit = true
        }
    }
}
