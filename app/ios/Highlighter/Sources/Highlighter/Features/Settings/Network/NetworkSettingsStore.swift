import Foundation
import Network
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
    @ObservationIgnored private var pathMonitor: NWPathMonitor?
    @ObservationIgnored private var inFlightNip11: [String] = []

    init(core: SafeHighlighterCore) {
        self.core = core
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
        if !snapshot.errorMessage.isEmpty {
            lastError = snapshot.errorMessage
        }
    }

    /// Toggle Wi-Fi-only mode. When on, an `NWPathMonitor` suspends the
    /// relay pool on cellular and resumes it when Wi-Fi comes back. The
    /// Rust core owns the durable preference; Swift owns path monitoring.
    func setWifiOnly(_ on: Bool) async {
        let previous = wifiOnlyEnabled
        wifiOnlyEnabled = on
        applyWifiOnlyEnforcement(on)
        let snapshot = await core.setWifiOnlyEnabled(on)
        if !snapshot.applied {
            wifiOnlyEnabled = previous
            applyWifiOnlyEnforcement(previous)
            lastError = snapshot.errorMessage
        }
    }

    // MARK: - Lifecycle

    func load() async {
        wifiOnlyEnabled = await core.isWifiOnlyEnabled()
        let outcome = await core.getRelays()
        if outcome.error.isEmpty {
            relays = outcome.values
            applyRelaySettingsProjection(rows: Array(diagnostics.values))
            await refreshDiagnostics()
            lastError = nil
        } else {
            lastError = outcome.error
        }
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
                let outcome = await core.probeRelayNip11(url)
                guard outcome.error.isEmpty, let doc = outcome.value else { return }
                await MainActor.run { self?.nip11ByUrl[url] = doc }
            }
        }
    }

    func startLiveUpdates() {
        if wifiOnlyEnabled && pathMonitor == nil {
            startPathMonitor()
        }
        Task { await self.refreshCacheStats() }
    }

    func stopLiveUpdates() {
        // Leave the path monitor running — Wi-Fi-only enforcement should
        // keep working after the user leaves the Network screen.
    }

    // MARK: - Cache

    func refreshCacheStats() async {
        let outcome = await core.getCacheStats()
        if outcome.error.isEmpty, let stats = outcome.value {
            cacheStats = stats
        }
    }

    // MARK: - NWPathMonitor

    private func applyWifiOnlyEnforcement(_ enabled: Bool) {
        if enabled {
            startPathMonitor()
        } else {
            pathMonitor?.cancel()
            pathMonitor = nil
            Task { await reconnectAll() }
        }
    }

    private func startPathMonitor() {
        guard pathMonitor == nil else { return }
        let monitor = NWPathMonitor()
        monitor.pathUpdateHandler = { [weak self] path in
            guard let self else { return }
            let isWifi = path.usesInterfaceType(.wifi)
            Task { @MainActor in
                guard self.wifiOnlyEnabled else { return }
                if isWifi {
                    _ = await self.core.reconnectAll()
                } else {
                    _ = await self.core.disconnectAll()
                }
            }
        }
        monitor.start(queue: .global(qos: .utility))
        pathMonitor = monitor
    }

    // MARK: - Writes

    func upsert(_ cfg: RelayConfig) async {
        let snapshot = await core.upsertRelay(cfg)
        if snapshot.shouldReload {
            await load()
        } else if !snapshot.errorMessage.isEmpty {
            lastError = snapshot.errorMessage
        }
    }

    func remove(_ url: String) async {
        let snapshot = await core.removeRelay(url)
        if snapshot.shouldReload {
            await load()
        } else if !snapshot.errorMessage.isEmpty {
            lastError = snapshot.errorMessage
        }
    }

    func relayHostedRooms(hostedOnRelay url: String) async -> RelayHostedRoomsSnapshot {
        let snapshot = await core.getRelayHostedRoomsSnapshot(hostedOnRelay: url)
        if !snapshot.errorMessage.isEmpty {
            lastError = snapshot.errorMessage
        }
        return snapshot
    }

    func setRoles(url: String, read: Bool, write: Bool, rooms: Bool, indexer: Bool) async {
        let snapshot = await core.setRelayRoles(
            url: url, read: read, write: write, rooms: rooms, indexer: indexer
        )
        if snapshot.shouldReload {
            await load()
        } else if !snapshot.errorMessage.isEmpty {
            lastError = snapshot.errorMessage
        }
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
        applyRelaySettingsProjection(rows: Array(diagnostics.values))
    }

    /// Called by `EventBridge` on `RelayDiagnosticsUpdated`. Applies the
    /// Rust-owned bounded diagnostics projection without a native polling
    /// loop.
    func applyDiagnostics(_ rows: [RelayDiagnostic]) async {
        applyRelaySettingsProjection(rows: rows)
    }

    // MARK: - Private

    private func refreshDiagnostics() async {
        let outcome = await core.getRelayDiagnostics()
        if outcome.error.isEmpty {
            await applyDiagnostics(outcome.values)
        }
    }

    private func applyRelaySettingsProjection(rows: [RelayDiagnostic]) {
        let projection = core.projectRelaySettings(
            configuredRelays: relays,
            diagnostics: rows
        )
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
