import SwiftUI

/// Detail screen for a single relay. Big state header + cumulative traffic +
/// role toggles + Remove action.
struct RelayDetailView: View {
    let url: String
    let store: NetworkSettingsStore

    @Environment(\.dismiss) private var dismiss
    @State private var orphanedRoomNames: [String] = []
    @State private var showRemoveConfirm = false
    @State private var isSaving = false

    var body: some View {
        let currentProjection = projection

        List {
            headerSection(currentProjection)
            orphanRoomsSection(currentProjection)
            statsSection
            rolesSection
            removeSection
        }
        .listStyle(.insetGrouped)
        .navigationTitle("Relay")
        .navigationBarTitleDisplayMode(.inline)
        .task(id: url) {
            orphanedRoomNames = await store.joinedRoomNames(hostedOnRelay: url)
        }
        .confirmationDialog(
            currentProjection.remove.title,
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
            Text(currentProjection.remove.message)
        }
    }

    // MARK: - Sections

    private var config: RelayConfig? {
        store.relays.first(where: { $0.url == url })
    }

    private var diagnostic: RelayDiagnostic? {
        store.diagnostic(for: url)
    }

    private var projection: RelayDetailProjection {
        store.relayDetailProjection(url: url, orphanedRoomNames: orphanedRoomNames)
    }

    @ViewBuilder
    private func headerSection(_ projection: RelayDetailProjection) -> some View {
        Section {
            VStack(alignment: .leading, spacing: 10) {
                HStack(alignment: .top, spacing: 12) {
                    RelayAvatar(projection: projection.avatar, size: 52)
                    VStack(alignment: .leading, spacing: 2) {
                        if let name = projection.name {
                            Text(name).font(.title3.weight(.semibold))
                        }
                        Text(url)
                            .font(.caption.monospaced())
                            .foregroundStyle(.secondary)
                            .lineLimit(2)
                            .truncationMode(.middle)
                        if let desc = projection.description {
                            Text(desc)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .lineLimit(3)
                                .padding(.top, 2)
                        }
                    }
                }
                HStack(spacing: 8) {
                    stateDot(projection.statusTone)
                    Text(projection.stateLabel).font(.subheadline.weight(.medium))
                    Spacer()
                    if let rtt = projection.rttLabel {
                        Text(rtt)
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

    @ViewBuilder
    private func orphanRoomsSection(_ projection: RelayDetailProjection) -> some View {
        if let orphanSummary = projection.remove.orphanSummary {
            Section {
                VStack(alignment: .leading, spacing: 4) {
                    Label("Hosts your rooms", systemImage: "person.3.fill")
                        .font(.subheadline.weight(.semibold))
                        .foregroundStyle(.orange)
                    Text(orphanSummary)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .padding(.vertical, 2)
            } footer: {
                Text("Removing this relay will cut you off from these rooms until you re-add it or leave them.")
            }
        }
    }

    // MARK: - State pieces

    private func stateDot(_ tone: RelayStatusTone) -> some View {
        Circle().fill(statusColor(tone)).frame(width: 12, height: 12)
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
