import Foundation
import Observation

/// App-scope store for the Network Settings screen. Owns the user's relay
/// rows (config) + the live diagnostics snapshot.
///
/// Architecture contract: nostrdb is the source of truth. `load()` asks the
/// Rust core (which reads from nostrdb / cached kind:10002 + kind:30078);
/// writes go through `HighlighterCore` which publishes new events and
/// reconciles the live pool. Live status deltas arrive via `EventBridge`
/// on the app-scope bus (subscription_id == 0).
@MainActor
@Observable
final class NetworkSettingsStore {
    var relays: [RelayConfig] = []
    var diagnostics: [String: RelayDiagnostic] = [:]
    var autoConnectedConfigs: [String: RelayConfig] = [:]
    var autoConnectedUrls: [String] = []
    var totalVisibleRelays: Int = 0
    var connectedCount: Int = 0
    var aggregateStateLabel: String = "No relays"
    var hasOutbox: Bool = false
    var allConnectedForHeader: Bool = false
    var anyConnectedForHeader: Bool = false
    var nip11ByUrl: [String: Nip11Document] = [:]
    var cacheStats: CacheStats?
    var isLoading: Bool = true
    var lastError: String?
    private(set) var wifiOnlyEnabled: Bool = false

    @ObservationIgnored private let core: SafeHighlighterCore
    @ObservationIgnored private weak var appStore: HighlighterStore?
    @ObservationIgnored private var inFlightNip11: [String] = []
    /// Phase 7: the kernel is the SOLE WRITER for relay config. Read paths
    /// (load / diagnostics) still come from the bespoke core (reads coexist
    /// until Part C); writes dispatch kernel actions.
    @ObservationIgnored private let kernel: HighlighterAppKernel

    init(core: SafeHighlighterCore, appStore: HighlighterStore, kernel: HighlighterAppKernel) {
        self.core = core
        self.appStore = appStore
        self.kernel = kernel
    }

    // MARK: - Kernel write helpers (Phase 7)

    /// Route a relay's kind:10002 (NIP-65) membership using the KERNEL's
    /// `nip65RelayRole` decision (single source of truth, parity-tested against
    /// bespoke nip65_tags): read|write → set_role with the marker (both/read/write)
    /// → kind:10002; read=write=false → removeRelay (omit from kind:10002 — a
    /// rooms/indexer-only relay lives ONLY in the kind:30078 app-data). Swift no
    /// longer makes the marker decision locally, so it can't drift.
    private func dispatchNip65(url: String, read: Bool, write: Bool) {
        if let role = nip65RelayRole(read: read, write: write) {
            kernel.app.dispatch(.setRelayRole(url: url, role: role))
        } else {
            kernel.app.dispatch(.removeRelay(url: url))
        }
    }

    /// Build the FULL relay set's {url, rooms, indexer} app-data entries (the
    /// kind:30078 `com.highlighter.relays` event is a single replaceable record),
    /// optionally overriding one URL's flags for an in-flight edit.
    private func appDataEntries(
        override url: String? = nil,
        rooms: Bool = false,
        indexer: Bool = false
    ) -> [RelayAppDataEntry] {
        relays.map { cfg in
            if let url, cfg.url == url {
                return RelayAppDataEntry(url: cfg.url, rooms: rooms, indexer: indexer)
            }
            return RelayAppDataEntry(url: cfg.url, rooms: cfg.rooms, indexer: cfg.indexer)
        }
    }

    /// Index diagnostics by URL for O(1) lookup from row views.
    func diagnostic(for url: String) -> RelayDiagnostic? {
        diagnostics[url]
    }

    /// Cached NIP-11 document for a relay, or `nil` if not yet fetched / the
    /// relay doesn't serve one.
    func nip11(for url: String) -> Nip11Document? {
        nip11ByUrl[url]
    }

    func autoConnectedConfig(for url: String) -> RelayConfig? {
        autoConnectedConfigs[url]
    }

    func relayRowProjection(config: RelayConfig) -> RelayRowProjection {
        core.projectRelayRow(input: RelayRowProjectionInput(
            config: config,
            diagnostic: diagnostic(for: config.url),
            nip11: nip11(for: config.url)
        ))
    }

    func relayDetailProjection(url: String, orphanedRoomNames: [String]) -> RelayDetailProjection {
        core.projectRelayDetail(input: RelayDetailProjectionInput(
            url: url,
            diagnostic: diagnostic(for: url),
            nip11: nip11(for: url),
            orphanedRoomNames: orphanedRoomNames
        ))
    }

    func relayRemoveProjection(url: String, orphanedRoomNames: [String]) -> RelayRemoveProjection {
        core.projectRelayRemove(input: RelayRemoveProjectionInput(
            url: url,
            orphanedRoomNames: orphanedRoomNames,
            emptyMessageUsesUrl: true
        ))
    }

    /// Kick the pool to attempt a reconnect on every disconnected relay.
    func reconnectAll() async {
        let snapshot = await core.reconnectAll()
        let apply = core.projectNetworkSettingsMutationApply(
            input: NetworkSettingsMutationApplyInput(snapshot: snapshot)
        )
        if let errorMessage = apply.errorMessage {
            lastError = errorMessage
        }
    }

    /// Toggle Wi-Fi-only mode. The app store owns the `NWPathMonitor`
    /// capability; Rust owns the durable preference and relay connection
    /// policy.
    func setWifiOnly(_ on: Bool) async {
        wifiOnlyEnabled = on
        let snapshot = await core.setWifiOnlyEnabled(on)
        applyWifiOnlyPreferenceSnapshot(snapshot)
    }

    // MARK: - Lifecycle

    func load() async {
        let snapshot = await core.getNetworkSettingsSnapshot(previousRelays: relays)
        applyNetworkSettingsSnapshot(snapshot)
        isLoading = false
        // Fire-and-forget NIP-11 probes for every relay we don't already
        // have cached. Each probe updates `nip11ByUrl` as it resolves, so
        // the rows progressively fill in their icons and names. Fails are
        // silent — a row without a NIP-11 doc just keeps its URL fallback.
        let probePlan = core.planRelayNip11Probes(input: RelayNip11ProbePlanInput(
            relays: relays,
            cachedUrls: Array(nip11ByUrl.keys),
            inFlightUrls: inFlightNip11
        ))
        inFlightNip11 = probePlan.inFlightUrls
        for url in probePlan.urlsToProbe {
            let core = self.core
            Task { [weak self] in
                defer {
                    Task { @MainActor [weak self] in
                        guard let self else { return }
                        self.inFlightNip11 = self.core.finishRelayNip11Probe(
                            inFlightUrls: self.inFlightNip11,
                            url: url
                        )
                    }
                }
                let snapshot = await core.probeRelayNip11Snapshot(url)
                guard let doc = snapshot.document else { return }
                await MainActor.run { self?.nip11ByUrl[url] = doc }
            }
        }
    }

    func startLiveUpdates() {
        Task {
            let snapshot = await self.core.subscribeRelayStatus()
            let projection = self.core.projectAppSubscriptionStart(
                input: AppSubscriptionStartProjectionInput(start: snapshot)
            )
            if projection.hasError {
                self.lastError = projection.errorMessage
            }
            await self.refreshCacheStats()
        }
    }

    // MARK: - Cache

    func refreshCacheStats() async {
        let snapshot = await core.getNetworkCacheStatsSnapshot()
        if let stats = snapshot.stats {
            cacheStats = stats
        }
    }

    // MARK: - Writes

    func upsert(_ cfg: RelayConfig) async {
        // Optimistic local update so the next app-data publish + the UI reflect
        // the change immediately (kernel publish is fire-and-forget).
        if let idx = relays.firstIndex(where: { $0.url == cfg.url }) {
            relays[idx] = cfg
        } else {
            relays.append(cfg)
        }
        // kind:10002 (NIP-65): add/edit with the read/write marker, or remove from
        // 10002 if rooms/indexer-only (read=write=false). kind:30078 app-data carries
        // the full relay set's rooms/indexer flags. Kernel sole writer.
        dispatchNip65(url: cfg.url, read: cfg.read, write: cfg.write)
        kernel.app.dispatch(.setRoomsRelayList(entries: appDataEntries()))
        await load()
    }

    func remove(_ url: String) async {
        relays.removeAll { $0.url == url }
        // kind:10002 via removeRelay (nmp auto-publishes the updated list); kind:30078
        // app-data rebuilt without the removed relay.
        kernel.app.dispatch(.removeRelay(url: url))
        kernel.app.dispatch(.setRoomsRelayList(entries: appDataEntries()))
        await load()
    }

    func relayHostedRooms(hostedOnRelay url: String) async -> RelayHostedRoomsSnapshot {
        let snapshot = await core.getRelayHostedRoomsSnapshot(hostedOnRelay: url)
        let apply = core.projectRelayHostedRoomsApply(
            input: RelayHostedRoomsApplyInput(snapshot: snapshot)
        )
        if let errorMessage = apply.errorMessage {
            lastError = errorMessage
        }
        return snapshot
    }

    func setRoles(url: String, read: Bool, write: Bool, rooms: Bool, indexer: Bool) async {
        // Optimistic local update so the app-data publish carries the new flags.
        if let idx = relays.firstIndex(where: { $0.url == url }) {
            relays[idx] = RelayConfig(
                url: url, read: read, write: write, rooms: rooms, indexer: indexer
            )
        }
        // read/write → kind:10002 (NIP-65 marker); read=write=false → removed from
        // kind:10002 (rooms/indexer-only relays live only in app-data). rooms/indexer
        // → kind:30078 app-data (full set, this url's flags updated). Kernel sole writer.
        dispatchNip65(url: url, read: read, write: write)
        kernel.app.dispatch(
            .setRoomsRelayList(entries: appDataEntries(override: url, rooms: rooms, indexer: indexer))
        )
        await load()
    }

    // MARK: - Delta hook

    /// Called by `EventBridge` on `RelayStatusChanged`. Updates the local
    /// diagnostic for the single relay without reloading the whole list.
    func applyStatus(url: String, state: RelayStatus) {
        if var existing = diagnostics[url] {
            existing.state = state
            diagnostics[url] = existing
        } else {
            diagnostics[url] = RelayDiagnostic(
                url: url,
                state: state,
                rttMs: nil,
                bytesSent: 0,
                bytesReceived: 0,
                connectedSinceTs: nil
            )
        }
        let rows = Array(diagnostics.values)
        let snapshot = core.projectNetworkDiagnosticsSnapshot(
            configuredRelays: relays,
            diagnostics: rows
        )
        applyNetworkDiagnosticsSnapshot(snapshot)
    }

    /// Called by `EventBridge` on `RelayDiagnosticsUpdated`. Applies the
    /// Rust-owned bounded diagnostics projection without a native polling
    /// loop.
    func applyDiagnostics(_ rows: [RelayDiagnostic]) async {
        let snapshot = core.projectNetworkDiagnosticsSnapshot(
            configuredRelays: relays,
            diagnostics: rows
        )
        applyNetworkDiagnosticsSnapshot(snapshot)
    }

    // MARK: - Private

    private func applyNetworkSettingsSnapshot(_ snapshot: NetworkSettingsSnapshot) {
        let apply = core.projectNetworkSettingsSnapshotApply(
            input: NetworkSettingsSnapshotApplyInput(snapshot: snapshot)
        )
        relays = apply.relays
        wifiOnlyEnabled = apply.wifiOnlyEnabled
        appStore?.applyNetworkPathMonitorEnabled(apply.pathMonitorEnabled)
        applyRelaySettingsProjection(apply.settingsProjection, rows: apply.diagnostics)
        lastError = apply.errorMessage
    }

    private func applyWifiOnlyPreferenceSnapshot(_ snapshot: NetworkWifiOnlyPreferenceSnapshot) {
        let apply = core.projectNetworkWifiOnlyPreferenceApply(
            input: NetworkWifiOnlyPreferenceApplyInput(snapshot: snapshot)
        )
        wifiOnlyEnabled = apply.wifiOnlyEnabled
        appStore?.applyNetworkPathMonitorEnabled(apply.pathMonitorEnabled)
        if let errorMessage = apply.errorMessage {
            lastError = errorMessage
        }
    }

    private func applyNetworkDiagnosticsSnapshot(_ snapshot: NetworkDiagnosticsSnapshot) {
        let apply = core.projectNetworkDiagnosticsSnapshotApply(
            input: NetworkDiagnosticsSnapshotApplyInput(snapshot: snapshot)
        )
        applyRelaySettingsProjection(apply.settingsProjection, rows: apply.diagnostics)
        if let errorMessage = apply.errorMessage {
            lastError = errorMessage
        }
    }

    private func applyRelaySettingsProjection(
        _ projection: RelaySettingsProjection,
        rows: [RelayDiagnostic]
    ) {
        diagnostics = Dictionary(uniqueKeysWithValues: rows.map { ($0.url, $0) })
        autoConnectedUrls = projection.autoConnectedUrls
        autoConnectedConfigs = Dictionary(
            uniqueKeysWithValues: projection.autoConnectedConfigs.map { ($0.url, $0) }
        )
        totalVisibleRelays = Int(projection.totalVisibleRelays)
        connectedCount = Int(projection.connectedCount)
        aggregateStateLabel = projection.aggregateStateLabel
        hasOutbox = projection.hasOutbox
        allConnectedForHeader = projection.allConnectedForHeader
        anyConnectedForHeader = projection.anyConnectedForHeader
    }
}
