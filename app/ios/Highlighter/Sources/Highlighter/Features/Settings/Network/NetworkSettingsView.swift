import SwiftUI

/// Network Settings main screen. Unified list of the user's relays, each row
/// with live status dot + role chips. Taps open `RelayDetailView`; toolbar
/// `+` opens `AddRelaySheet`.
struct NetworkSettingsView: View {
    @Environment(HighlighterStore.self) private var appStore
    @State private var store: NetworkSettingsStore?
    @State private var showAddSheet = false

    var body: some View {
        List {
            if let store {
                headerSection(store)
                relaysSection(store)
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
                Task {
                    for idx in indexSet {
                        guard idx < store.relays.count else { continue }
                        await store.remove(store.relays[idx].url)
                    }
                }
            }
        } header: {
            Text("Relays")
        } footer: {
            Text("Your Read and Write relays are published as a kind:10002 event. Other nostr users can see where you read and publish.")
        }
    }

    private var footerSection: some View {
        Section {
            EmptyView()
        } footer: {
            Text("Tap a relay to see diagnostics, change its roles, or remove it.")
        }
    }

    @ViewBuilder
    private func stateDot(allConnected: Bool, anyConnected: Bool) -> some View {
        Circle()
            .fill(allConnected ? .green : (anyConnected ? .yellow : .red))
            .frame(width: 10, height: 10)
    }
}
