import SwiftUI

/// Network Settings main screen. Renders the Rust-owned relay projection and
/// dispatches typed actions for all relay policy changes.
struct NetworkSettingsView: View {
    @Environment(HighlighterStore.self) private var appStore
    @State private var showAddSheet = false
    @State private var showImportSheet = false
    @State private var pendingRemove: PendingRemove?

    private struct PendingRemove: Identifiable {
        let id = UUID()
        let url: String
        let roomNames: [String]
        let roomCount: UInt64
    }

    var body: some View {
        List {
            if !appStore.network.isLoading || !appStore.network.relays.isEmpty {
                headerSection
                safetySection
                relaysSection
                autoConnectedSection
                actionsSection
                cacheSection
                connectivitySection
                footerSection
            } else {
                ProgressView()
                    .frame(maxWidth: .infinity, alignment: .center)
                    .listRowBackground(Color.clear)
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle("Network")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    showAddSheet = true
                } label: {
                    Image(systemName: "plus")
                }
                .disabled(appStore.network.isLoading || appStore.network.isSaving)
            }
        }
        .sheet(isPresented: $showAddSheet) {
            AddRelaySheet()
        }
        .sheet(isPresented: $showImportSheet) {
            ImportRelaysSheet()
        }
        .confirmationDialog(
            (pendingRemove?.roomCount ?? 0) > 0
                ? "Remove — you're a member of rooms here"
                : "Remove this relay?",
            isPresented: Binding(
                get: { pendingRemove != nil },
                set: { if !$0 { pendingRemove = nil } }
            ),
            titleVisibility: .visible,
            presenting: pendingRemove
        ) { remove in
            Button("Remove", role: .destructive) {
                appStore.removeNetworkRelay(url: remove.url)
            }
            Button("Cancel", role: .cancel) {}
        } message: { remove in
            if remove.roomCount == 0 {
                Text("Highlighter will stop sending and receiving events through \(remove.url).")
            } else {
                Text("This relay hosts \(remove.roomCount) of your rooms (\(remove.roomNames.prefix(3).joined(separator: ", "))\(remove.roomCount > 3 ? ", …" : "")). Removing it will cut you off from them until you re-add it.")
            }
        }
        .task {
            appStore.openNetworkSettings()
        }
        .onDisappear {
            appStore.closeNetworkSettings()
        }
    }

    @ViewBuilder
    private var headerSection: some View {
        Section {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    stateDot(
                        allConnected: appStore.network.connectedCount == appStore.network.visibleRelayCount && appStore.network.visibleRelayCount > 0,
                        anyConnected: appStore.network.connectedCount > 0
                    )
                    Text(aggregateStateLabel)
                        .font(.headline)
                }
                if let err = appStore.network.errorMessage ?? appStore.network.actionErrorMessage {
                    Text(err)
                        .font(.caption)
                        .foregroundStyle(.red)
                }
            }
            .padding(.vertical, 4)
        }
    }

    private var relaysSection: some View {
        Section {
            ForEach(appStore.network.relays, id: \.url) { row in
                NavigationLink {
                    RelayDetailView(url: row.url)
                } label: {
                    RelayRowView(
                        config: row,
                        diagnostic: appStore.networkDiagnostic(url: row.url),
                        nip11: appStore.networkNip11(url: row.url)?.document
                    )
                }
                .task(id: row.url) {
                    appStore.probeNetworkRelayNip11(url: row.url)
                }
            }
            .onDelete { indexSet in
                for idx in indexSet where idx < appStore.network.relays.count {
                    let url = appStore.network.relays[idx].url
                    let impact = appStore.networkRemovalImpact(url: url)
                    pendingRemove = PendingRemove(
                        url: url,
                        roomNames: impact?.roomNames ?? [],
                        roomCount: impact?.roomCount ?? 0
                    )
                    break
                }
            }
        } header: {
            Text("Relays")
        } footer: {
            Text("Your Read and Write relays are published as a kind:10002 event. Other nostr users can see where you read and publish.")
        }
    }

    @ViewBuilder
    private var autoConnectedSection: some View {
        if !appStore.network.autoConnectedRelays.isEmpty {
            Section {
                ForEach(appStore.network.autoConnectedRelays, id: \.url) { config in
                    RelayRowView(
                        config: config,
                        diagnostic: appStore.networkDiagnostic(url: config.url),
                        nip11: appStore.networkNip11(url: config.url)?.document
                    )
                    .task(id: config.url) {
                        appStore.probeNetworkRelayNip11(url: config.url)
                    }
                }
            } header: {
                Text("Auto-connected")
            } footer: {
                Text("Connected automatically for outbox routing and app indexer coverage. Not part of your published NIP-65.")
            }
        }
    }

    @ViewBuilder
    private var safetySection: some View {
        if !appStore.network.hasOutbox {
            Section {
                banner(
                    icon: "exclamationmark.triangle.fill",
                    tint: .orange,
                    title: "No outbox relays",
                    detail: "Turn on Write for at least one relay — otherwise your posts won't reach anyone."
                )
            }
        }
    }

    private var actionsSection: some View {
        Section {
            Button {
                appStore.reconnectNetwork()
            } label: {
                Label("Reconnect All", systemImage: "arrow.clockwise")
            }
            Button {
                showImportSheet = true
            } label: {
                Label("Import from another user…", systemImage: "person.crop.circle.badge.plus")
            }
        }
    }

    @ViewBuilder
    private var cacheSection: some View {
        Section {
            if let stats = appStore.network.cacheStats {
                LabeledContent("Events", value: "\(stats.eventCountEstimate)")
                LabeledContent("On disk", value: formatBytes(stats.diskBytes))
            } else {
                HStack {
                    ProgressView().scaleEffect(0.7)
                    Text("Measuring…").foregroundStyle(.secondary).font(.caption)
                }
            }
        } header: {
            Text("Local cache")
        } footer: {
            Text("Everything Highlighter has seen on relays lives here. Uninstall the app to clear it.")
        }
    }

    @ViewBuilder
    private var connectivitySection: some View {
        Section {
            Toggle(isOn: Binding(
                get: { appStore.network.wifiOnlyEnabled },
                set: { appStore.setNetworkWifiOnly($0) }
            )) {
                Label("Wi-Fi only", systemImage: "wifi")
            }
        } header: {
            Text("Connectivity")
        } footer: {
            Text("When on, Highlighter pauses relay connections on cellular to save mobile data. Resumes automatically on Wi-Fi.")
        }
    }

    private var footerSection: some View {
        Section {
            EmptyView()
        } footer: {
            Text("Tap a relay to see diagnostics, change its roles, or remove it.")
        }
    }

    private var aggregateStateLabel: String {
        let total = appStore.network.visibleRelayCount
        let online = appStore.network.connectedCount
        if total == 0 { return "No relays" }
        if online == 0 { return "Offline" }
        if online == total { return "Online — \(online) of \(total)" }
        return "\(online) of \(total) online"
    }

    private func formatBytes(_ bytes: UInt64) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(bytes), countStyle: .binary)
    }

    private func banner(icon: String, tint: Color, title: String, detail: String) -> some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: icon)
                .foregroundStyle(tint)
                .frame(width: 24, alignment: .center)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.subheadline.weight(.semibold))
                Text(detail).font(.caption).foregroundStyle(.secondary)
            }
        }
        .padding(.vertical, 4)
    }

    @ViewBuilder
    private func stateDot(allConnected: Bool, anyConnected: Bool) -> some View {
        Circle()
            .fill(allConnected ? .green : (anyConnected ? .yellow : .red))
            .frame(width: 10, height: 10)
    }
}
