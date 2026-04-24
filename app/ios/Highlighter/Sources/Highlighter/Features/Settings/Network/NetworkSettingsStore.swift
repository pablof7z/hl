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
    var isLoading: Bool = true
    var lastError: String?

    @ObservationIgnored private let core: SafeHighlighterCore
    @ObservationIgnored private var pollTask: Task<Void, Never>?

    init(core: SafeHighlighterCore) {
        self.core = core
    }

    /// Index diagnostics by URL for O(1) lookup from row views.
    func diagnostic(for url: String) -> RelayDiagnostic? {
        diagnostics[url]
    }

    /// Number of relays currently reporting `Connected`. Used for the header
    /// "Online — N of M" pill.
    var connectedCount: Int {
        diagnostics.values.filter { $0.state == .connected }.count
    }

    /// Human-readable aggregate state for the header pill.
    var aggregateStateLabel: String {
        let total = relays.count
        let online = connectedCount
        if total == 0 { return "No relays" }
        if online == 0 { return "Offline" }
        if online == total { return "Online — \(online) of \(total)" }
        return "\(online) of \(total) online"
    }

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
    }

    func startLiveUpdates() {
        // Already running
        guard pollTask == nil else { return }
        pollTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(2))
                guard let self else { return }
                await self.refreshDiagnostics()
            }
        }
    }

    func stopLiveUpdates() {
        pollTask?.cancel()
        pollTask = nil
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
    }

    // MARK: - Private

    private func refreshDiagnostics() async {
        do {
            let rows = try await core.getRelayDiagnostics()
            diagnostics = Dictionary(uniqueKeysWithValues: rows.map { ($0.url, $0) })
        } catch {
            // Diagnostics failures are non-fatal — the config rows are still
            // accurate; we just can't show live state this tick.
        }
    }
}
