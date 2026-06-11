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
    var autoConnectedRelays: [RelayConfig] = []
    var diagnostics: [String: RelayDiagnostic] = [:]
    var nip11ByUrl: [String: Nip11Document] = [:]
    var cacheStats: CacheStats?
    var isLoading: Bool = true
    var lastError: String?

    @ObservationIgnored private let core: SafeHighlighterCore
    @ObservationIgnored private var diagnosticsRefreshTask: Task<Void, Never>?
    @ObservationIgnored private var diagnosticsRefreshPending = false
    @ObservationIgnored private var inFlightNip11: Set<String> = []

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

    /// URLs of relays in the live pool that the user didn't configure. Rust
    /// owns why each relay is present; Swift only renders the bounded display
    /// projection.
    var autoConnectedUrls: [String] {
        autoConnectedRelays.map(\.url)
    }

    /// Diagnostics rows for the auto-connected URLs, in the same order.
    var autoConnectedDiagnostics: [RelayDiagnostic] {
        autoConnectedUrls.compactMap { diagnostics[$0] }
    }

    /// Total relays the user can see in the screen — configured + auto.
    var totalVisibleRelays: Int {
        relays.count + autoConnectedUrls.count
    }

    /// Number of relays currently reporting `Connected`. Used for the header
    /// "Online — N of M" pill. Counts every pool relay (configured + auto)
    /// since both groups are visible in the UI.
    var connectedCount: Int {
        diagnostics.values.filter { $0.state == .connected }.count
    }

    /// Human-readable aggregate state for the header pill. The denominator
    /// matches what's actually rendered (configured + auto-connected) so
    /// the user never sees nonsense like "10 of 5".
    var aggregateStateLabel: String {
        let total = totalVisibleRelays
        let online = connectedCount
        if total == 0 { return "No relays" }
        if online == 0 { return "Offline" }
        if online == total { return "Online — \(online) of \(total)" }
        return "\(online) of \(total) online"
    }

    /// True when at least one relay has the `write` flag on. When false,
    /// the user's published events can't reach anyone — show the
    /// no-outbox banner.
    var hasOutbox: Bool { relays.contains { $0.write } }

    // MARK: - Lifecycle

    func load() async {
        do {
            let rows = try await core.getRelays()
            relays = rows
            await refreshDiagnostics()
            lastError = nil
        } catch {
            lastError = String(describing: error)
        }
        isLoading = false
        // Fire-and-forget NIP-11 probes for every relay we don't already
        // have cached. Each probe updates `nip11ByUrl` as it resolves, so
        // the rows progressively fill in their icons and names. Fails are
        // silent — a row without a NIP-11 doc just keeps its URL fallback.
        for row in relays where nip11ByUrl[row.url] == nil && !inFlightNip11.contains(row.url) {
            inFlightNip11.insert(row.url)
            let core = self.core
            let url = row.url
            Task { [weak self] in
                defer { Task { @MainActor [weak self] in self?.inFlightNip11.remove(url) } }
                guard let doc = try? await core.probeRelayNip11(url) else { return }
                await MainActor.run { self?.nip11ByUrl[url] = doc }
            }
        }
    }

    func startLiveUpdates() {
        Task { await self.refreshCacheStats() }
    }

    func stopLiveUpdates() {
        diagnosticsRefreshTask?.cancel()
        diagnosticsRefreshTask = nil
        diagnosticsRefreshPending = false
    }

    // MARK: - Cache

    func refreshCacheStats() async {
        if let stats = try? await core.getCacheStats() {
            cacheStats = stats
        }
    }

    // MARK: - Writes

    func upsert(_ cfg: RelayConfig) async {
        do {
            try await core.upsertRelay(cfg)
            await load()
        } catch {
            lastError = "Couldn't add relay — \(error)"
        }
    }

    func remove(_ url: String) async {
        do {
            try await core.removeRelay(url)
            await load()
        } catch {
            lastError = "Couldn't remove relay — \(error)"
        }
    }

    func setRoles(url: String, read: Bool, write: Bool, rooms: Bool, indexer: Bool) async {
        do {
            try await core.setRelayRoles(
                url: url, read: read, write: write, rooms: rooms, indexer: indexer
            )
            await load()
        } catch {
            lastError = "Couldn't update roles — \(error)"
        }
    }

    // MARK: - Delta hook

    /// Called by `EventBridge` on `RelayStatusChanged`. Updates the local
    /// diagnostic for the single relay immediately, then coalesces a Rust
    /// snapshot refresh so derived auto-connected rows and metrics catch up
    /// without a native polling loop.
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
        scheduleDiagnosticsRefresh()
    }

    // MARK: - Private

    private func scheduleDiagnosticsRefresh() {
        if diagnosticsRefreshTask != nil {
            diagnosticsRefreshPending = true
            return
        }
        diagnosticsRefreshTask = Task { @MainActor [weak self] in
            guard let self else { return }
            repeat {
                self.diagnosticsRefreshPending = false
                await self.refreshDiagnostics()
            } while self.diagnosticsRefreshPending && !Task.isCancelled
            self.diagnosticsRefreshTask = nil
        }
    }

    private func refreshDiagnostics() async {
        do {
            let rows = try await core.getRelayDiagnostics()
            diagnostics = Dictionary(uniqueKeysWithValues: rows.map { ($0.url, $0) })
        } catch {
            // Diagnostics failures are non-fatal — the config rows are still
            // accurate; we just can't show live state this tick.
        }
        do {
            autoConnectedRelays = try await core.getAutoConnectedRelays()
        } catch {
            // Auto-connected display roles are derived from the same Rust relay
            // state. Keep the last projection when the transient read fails.
        }
    }
}
