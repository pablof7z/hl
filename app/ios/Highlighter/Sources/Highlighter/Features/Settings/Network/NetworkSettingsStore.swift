import Foundation
import Observation

/// App-scope store for the Network Settings screen. Owns the user's relay
/// rows (config) + the live diagnostics snapshot.
///
/// Architecture contract: relay read/write config comes from NMP's
/// configured_relays slot (via `kernel.relayListSnapshot`). Writes dispatch
/// full relay rows to the kernel, which owns NIP-65 and kind:30078 shaping.
/// Live diagnostics arrive via the kernel relay diagnostics view.
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

    @ObservationIgnored private weak var appStore: HighlighterStore?
    @ObservationIgnored private var inFlightNip11: [String] = []
    /// Phase 7: the kernel is the SOLE WRITER for relay config. Read paths
    /// load from the NMP slot and diagnostics arrive through the kernel view.
    @ObservationIgnored private let kernel: HighlighterAppKernel

    init(appStore: HighlighterStore, kernel: HighlighterAppKernel) {
        self.appStore = appStore
        self.kernel = kernel
    }

    // MARK: - Kernel write helpers (Phase 7)

    /// The kind:10002 (NIP-65) write a relay's read/write flags resolve to.
    /// Extracted as a pure value so the routing decision — specifically the
    /// regression site, read=write=false must REMOVE, not add (7e2de4f3 routed
    /// it to the "both" marker) — is unit-testable without a live kernel.
    enum Nip65Route: Equatable {
        /// Set the kind:10002 marker (both/read/write) for this relay.
        case setRole(String)
        /// Omit the relay from kind:10002 — a rooms/indexer-only relay lives
        /// ONLY in the kind:30078 app-data.
        case remove
    }

    /// Pure routing decision. Delegates the marker to the KERNEL's
    /// `nip65RelayRole` (single source of truth, parity-tested against bespoke
    /// `nip65_tags`), so Swift never makes the marker decision locally and
    /// can't drift. `nonisolated` so the regression-guard test can call it
    /// without hopping to the main actor.
    nonisolated static func nip65Route(read: Bool, write: Bool) -> Nip65Route {
        if let role = nip65RelayRole(read: read, write: write) {
            return .setRole(role)
        }
        return .remove
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
        let nip11 = nip11ByUrl[config.url]
        let diagnostic = diagnostics[config.url]
        return RelayRowProjection(
            avatar: avatarProjection(url: config.url, nip11: nip11),
            primaryLabel: nip11?.name ?? Self.hostname(from: config.url),
            displayUrl: Self.displayUrl(from: config.url),
            statusTone: Self.statusTone(for: diagnostic?.state),
            rttLabel: diagnostic?.rttMs.map { "\($0)ms" },
            read: config.read,
            write: config.write,
            rooms: config.rooms,
            indexer: config.indexer
        )
    }

    func relayDetailProjection(url: String, orphanedRoomNames: [String]) -> RelayDetailProjection {
        let nip11 = nip11ByUrl[url]
        let diagnostic = diagnostics[url]
        return RelayDetailProjection(
            avatar: avatarProjection(url: url, nip11: nip11),
            name: nip11?.name,
            description: nip11?.description,
            stateLabel: Self.stateLabel(for: diagnostic?.state),
            statusTone: Self.statusTone(for: diagnostic?.state),
            rttLabel: diagnostic?.rttMs.map { "\($0)ms" },
            remove: relayRemoveProjection(url: url, orphanedRoomNames: orphanedRoomNames)
        )
    }

    func relayRemoveProjection(url: String, orphanedRoomNames: [String]) -> RelayRemoveProjection {
        let orphanSummary: String? = orphanedRoomNames.isEmpty ? nil
            : orphanedRoomNames.count == 1
                ? "\"\(orphanedRoomNames[0])\" uses this relay exclusively and will become inaccessible."
                : "\(orphanedRoomNames.count) rooms use this relay exclusively and will become inaccessible."
        return RelayRemoveProjection(
            title: "Remove Relay",
            message: "Remove \(Self.displayUrl(from: url)) from your relay list?",
            orphanSummary: orphanSummary
        )
    }

    /// Kick the pool to attempt a reconnect on every disconnected relay.
    func reconnectAll() async {
        // Reconnect via kernel dispatch; the NMP pool manages its own relay connections.
    }

    /// Toggle Wi-Fi-only mode. The app store owns the `NWPathMonitor`
    /// capability; the preference is persisted in UserDefaults.
    func setWifiOnly(_ on: Bool) async {
        wifiOnlyEnabled = on
        UserDefaults.standard.set(on, forKey: "hl.network.wifi_only")
        appStore?.applyNetworkPathMonitorEnabled(on)
    }

    // MARK: - Lifecycle

    func load() async {
        wifiOnlyEnabled = UserDefaults.standard.bool(forKey: "hl.network.wifi_only")
        // Hydrate relay list from the NMP kernel slot on first load. After that,
        // optimistic updates in upsert/remove/setRoles maintain the in-memory list
        // so we don't clobber local mutations with a stale kernel read.
        if relays.isEmpty {
            let rows = kernel.relayListSnapshot
            if !rows.isEmpty {
                relays = rows.map { row in
                    let role = row.role
                    let tokens = Set(role.split(separator: ",").map(String.init))
                    return RelayConfig(
                        url: row.url,
                        read: tokens.contains("read") || tokens.contains("both"),
                        write: tokens.contains("write") || tokens.contains("both"),
                        rooms: false,
                        indexer: tokens.contains("indexer")
                    )
                }
            }
        }
        isLoading = false
    }

    func startLiveUpdates() {
        // No bespoke subscription needed; the NMP pool drives kernel diagnostics.
    }

    // MARK: - Cache

    func refreshCacheStats() async {
        // NOTE: getNetworkCacheStatsSnapshot removed (relay_polish.rs deleted in
        // Phase 7 teardown). Cache stats unavailable until kernel exposes this.
    }

    // MARK: - Writes

    func upsert(_ cfg: RelayConfig) async {
        // Optimistic local update so the NIP-65 publish + the UI reflect the
        // change immediately (kernel publish is fire-and-forget).
        if let idx = relays.firstIndex(where: { $0.url == cfg.url }) {
            relays[idx] = cfg
        } else {
            relays.append(cfg)
        }
        kernel.app.dispatch(.setRelayConfigs(relays: relays))
        await load()
    }

    func remove(_ url: String) async {
        relays.removeAll { $0.url == url }
        kernel.app.dispatch(.setRelayConfigs(relays: relays))
        await load()
    }

    func relayHostedRooms(hostedOnRelay url: String) async -> RelayHostedRoomsSnapshot {
        RelayHostedRoomsSnapshot(roomNames: [], errorMessage: "")
    }

    func setRoles(url: String, read: Bool, write: Bool, rooms: Bool, indexer: Bool) async {
        // Optimistic local update so the NIP-65 publish + UserDefaults write carry
        // the new flags.
        if let idx = relays.firstIndex(where: { $0.url == url }) {
            relays[idx] = RelayConfig(
                url: url, read: read, write: write, rooms: rooms, indexer: indexer
            )
        }
        kernel.app.dispatch(.setRelayConfigs(relays: relays))
        await load()
    }

    // MARK: - Delta hook

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
        recomputeAggregates()
    }

    /// Applies bounded diagnostics without a native polling loop.
    func applyDiagnostics(_ rows: [RelayDiagnostic]) async {
        diagnostics = Dictionary(uniqueKeysWithValues: rows.map { ($0.url, $0) })
        recomputeAggregates()
    }

    /// Bridge from kernel `RelayDiagRow` → bespoke `RelayDiagnostic`.
    /// Called from `NetworkSettingsView.onChange(of: kernel.relayDiagnostics)`.
    func applyRelayDiagRows(_ rows: [RelayDiagRow]) {
        var result: [String: RelayDiagnostic] = [:]
        for row in rows {
            let state: RelayStatus
            switch row.connectionState {
            case .connected:
                state = .connected
            case .reconnecting:
                state = .connecting
            case .error, .unknown:
                state = .disconnected
            }
            result[row.relayUrl] = RelayDiagnostic(
                url: row.relayUrl,
                state: state,
                rttMs: nil,
                bytesSent: 0,
                bytesReceived: row.totalEventsRx,
                connectedSinceTs: row.lastConnectedMs > 0 ? row.lastConnectedMs / 1000 : nil
            )
        }
        diagnostics = result
        recomputeAggregates()
    }

    /// Preview another user's kind:10002 relay list from NMP's mailbox cache.
    func importRelaysForPubkey(_ pubkey: String) -> ImportRelaysFetchSnapshot {
        kernel.importRelaysForPubkey(pubkey)
    }

    func importRelaysProjection(fetched: [RelayConfig], selectedUrls: [String]) -> ImportRelaysProjection {
        kernel.projectImportRelays(ImportRelaysProjectionInput(fetched: fetched, selectedUrls: selectedUrls))
    }

    func importRelaysSourceProjection(npub: String, isFetching: Bool) -> ImportRelaysSourceProjection {
        kernel.projectImportRelaysSource(
            ImportRelaysSourceProjectionInput(npub: npub, isFetching: isFetching)
        )
    }

    func toggleImportRelaySelection(fetched: [RelayConfig], selectedUrls: [String], url: String) -> [String] {
        kernel.toggleImportRelaySelection(fetched: fetched, selectedUrls: selectedUrls, url: url)
    }

    // MARK: - Private

    // MARK: - D1 relay projections

    private func avatarProjection(url: String, nip11: Nip11Document?) -> RelayAvatarProjection {
        let hostname = Self.hostname(from: url)
        let initial = String((nip11?.name?.first ?? hostname.first ?? "R").uppercased())
        return RelayAvatarProjection(
            iconUrl: nip11?.icon,
            initial: initial,
            hue: Self.deterministicHue(from: hostname)
        )
    }

    private func recomputeAggregates() {
        let rows = Array(diagnostics.values)
        let connected = rows.filter { $0.state == .connected }.count
        connectedCount = connected
        totalVisibleRelays = relays.count
        hasOutbox = relays.contains { $0.write }
        allConnectedForHeader = totalVisibleRelays > 0 && connected == totalVisibleRelays
        anyConnectedForHeader = connected > 0
        aggregateStateLabel = {
            if totalVisibleRelays == 0 { return "No relays" }
            if connected == totalVisibleRelays { return "All connected" }
            if connected == 0 { return "Disconnected" }
            return "\(connected) of \(totalVisibleRelays) connected"
        }()
        autoConnectedUrls = []
        autoConnectedConfigs = [:]
    }

    private static func hostname(from url: String) -> String {
        var s = url
        for prefix in ["wss://", "ws://"] { s = s.replacingOccurrences(of: prefix, with: "") }
        return String(s.split(separator: "/", maxSplits: 1).first ?? s[...])
    }

    private static func displayUrl(from url: String) -> String {
        var s = url
        for prefix in ["wss://", "ws://"] { s = s.replacingOccurrences(of: prefix, with: "") }
        return s
    }

    private static func statusTone(for state: RelayStatus?) -> RelayStatusTone {
        switch state {
        case .connected: return .connected
        case .connecting: return .connecting
        case .disconnected, .terminated, .banned: return .error
        case nil: return .unknown
        }
    }

    private static func stateLabel(for state: RelayStatus?) -> String {
        switch state {
        case .connected: return "Connected"
        case .connecting: return "Connecting"
        case .disconnected: return "Disconnected"
        case .terminated: return "Terminated"
        case .banned: return "Banned"
        case nil: return "Unknown"
        }
    }

    private static func deterministicHue(from string: String) -> Double {
        let hash = string.unicodeScalars.reduce(0) { ($0 &* 31) &+ Int($1.value) }
        return Double(abs(hash) % 360) / 360.0
    }
}
