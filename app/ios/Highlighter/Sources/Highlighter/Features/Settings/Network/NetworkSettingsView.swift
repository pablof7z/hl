import SwiftUI

/// Network Settings main screen. Unified list of the user's relays, each row
/// with live status dot + role chips. Taps open `RelayDetailView`; toolbar
/// `+` opens `AddRelaySheet`.
struct NetworkSettingsView: View {
    @Environment(HighlighterStore.self) private var appStore
    @State private var store: NetworkSettingsStore?
    @State private var showAddSheet = false
    @State private var pendingRemove: PendingRemove?

    private struct PendingRemove: Identifiable {
        let id = UUID()
        let url: String
        let orphanedRoomNames: [String]
    }

    var body: some View {
        List {
            if let store {
                headerSection(store)
                safetySection(store)
                relaysSection(store)
                actionsSection(store)
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
                .disabled(store == nil)
            }
        }
        .sheet(isPresented: $showAddSheet) {
            if let store {
                AddRelaySheet { cfg in
                    Task { await store.upsert(cfg) }
                }
            }
        }
        .confirmationDialog(
            pendingRemove?.orphanedRoomNames.isEmpty == false
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
                Task { await store?.remove(remove.url) }
            }
            Button("Cancel", role: .cancel) {}
        } message: { remove in
            if remove.orphanedRoomNames.isEmpty {
                Text("Highlighter will stop sending and receiving events through \(remove.url).")
            } else {
                Text("This relay hosts \(remove.orphanedRoomNames.count) of your rooms (\(remove.orphanedRoomNames.prefix(3).joined(separator: ", "))\(remove.orphanedRoomNames.count > 3 ? ", …" : "")). Removing it will cut you off from them until you re-add it.")
            }
        }
        .task {
            if store == nil {
                store = NetworkSettingsStore(core: appStore.safeCore)
                appStore.eventBridge?.registerNetworkStore(store!)
            }
            await store?.load()
            store?.startLiveUpdates()
        }
        .onDisappear {
            store?.stopLiveUpdates()
        }
    }

    // MARK: - Sections

    @ViewBuilder
    private func headerSection(_ store: NetworkSettingsStore) -> some View {
        Section {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    stateDot(
                        allConnected: store.connectedCount == store.relays.count && !store.relays.isEmpty,
                        anyConnected: store.connectedCount > 0
                    )
                    Text(store.aggregateStateLabel)
                        .font(.headline)
                }
                if let err = store.lastError {
                    Text(err)
                        .font(.caption)
                        .foregroundStyle(.red)
                }
            }
            .padding(.vertical, 4)
        }
    }

    private func relaysSection(_ store: NetworkSettingsStore) -> some View {
        Section {
            ForEach(store.relays, id: \.url) { row in
                NavigationLink {
                    RelayDetailView(url: row.url, store: store)
                } label: {
                    RelayRowView(
                        config: row,
                        diagnostic: store.diagnostic(for: row.url)
                    )
                }
            }
            .onDelete { indexSet in
                // Route every delete through the confirmation dialog so the
                // orphan-rooms check applies whether the user swiped or
                // tapped into the detail view.
                for idx in indexSet where idx < store.relays.count {
                    let url = store.relays[idx].url
                    let orphans = orphanedRooms(for: url)
                    pendingRemove = PendingRemove(
                        url: url,
                        orphanedRoomNames: orphans
                    )
                    break // confirm one at a time
                }
            }
        } header: {
            Text("Relays")
        } footer: {
            Text("Your Read and Write relays are published as a kind:10002 event. Other nostr users can see where you read and publish.")
        }
    }

    @ViewBuilder
    private func safetySection(_ store: NetworkSettingsStore) -> some View {
        if !store.hasOutbox || !store.hasIndexer {
            Section {
                if !store.hasOutbox {
                    banner(
                        icon: "exclamationmark.triangle.fill",
                        tint: .orange,
                        title: "No outbox relays",
                        detail: "Turn on Write for at least one relay — otherwise your posts won't reach anyone."
                    )
                }
                if !store.hasIndexer {
                    banner(
                        icon: "magnifyingglass",
                        tint: .yellow,
                        title: "No indexer relays",
                        detail: "Profile and follow-list lookups for other users may fail until you turn on Indexer for at least one relay."
                    )
                }
            }
        }
    }

    private func actionsSection(_ store: NetworkSettingsStore) -> some View {
        Section {
            Button {
                Task { await store.reconnectAll() }
            } label: {
                Label("Reconnect All", systemImage: "arrow.clockwise")
            }
        }
    }

    private var footerSection: some View {
        Section {
            EmptyView()
        } footer: {
            Text("Tap a relay to see diagnostics, change its roles, or remove it.")
        }
    }

    /// Joined-room names that live on the given relay URL, compared by
    /// trimmed string equality. Used by the remove-confirmation flow.
    private func orphanedRooms(for url: String) -> [String] {
        let target = url.trimmingCharacters(in: .whitespaces)
        return appStore.joinedCommunities
            .filter { $0.relayUrl.trimmingCharacters(in: .whitespaces) == target }
            .map { $0.name.isEmpty ? $0.id : $0.name }
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
