import SwiftUI

/// Sheet for adding a new relay. URL field + role chips. Sane defaults:
/// Read + Write on, Rooms and Indexer off. A user can tap chips after the
/// relay is in the list if they want to change the roles.
struct AddRelaySheet: View {
    @Environment(\.dismiss) private var dismiss

    let onAdd: (RelayConfig) -> Void

    @State private var urlText: String
    @State private var read: Bool
    @State private var write: Bool
    @State private var rooms: Bool
    @State private var indexer: Bool

    /// NIP-11 probe status. Populated after the URL field loses focus (or
    /// after a 600ms debounce) so the user sees what relay they're about
    /// to add without the probe firing on every keystroke.
    @State private var probeResult: Nip11Document?
    @State private var probeFailed = false
    @State private var probeInFlight = false
    @State private var probeTask: Task<Void, Never>?

    init(initialDraft: RelayConfig, onAdd: @escaping (RelayConfig) -> Void) {
        self.onAdd = onAdd
        _urlText = State(initialValue: initialDraft.url)
        _read = State(initialValue: initialDraft.read)
        _write = State(initialValue: initialDraft.write)
        _rooms = State(initialValue: initialDraft.rooms)
        _indexer = State(initialValue: initialDraft.indexer)
    }

    private var projection: AddRelaySheetProjection {
        let normalizedUrl = urlText.trimmingCharacters(in: .whitespaces)
        let clipboardUrl: String? = UIPasteboard.general.string.flatMap { text in
            let trimmed = text.trimmingCharacters(in: .whitespaces)
            guard (trimmed.hasPrefix("wss://") || trimmed.hasPrefix("ws://")) && trimmed != normalizedUrl else { return nil }
            return trimmed
        }
        let isValid = normalizedUrl.hasPrefix("wss://") || normalizedUrl.hasPrefix("ws://")
        let (probeStatus, probeText): (AddRelayProbeStatus, String) = {
            if probeInFlight { return (.checking, "Checking relay…") }
            if let doc = probeResult {
                let softwareLabel: String? = doc.software.map { name in
                    doc.version.map { "\(name) \($0)" } ?? name
                }
                let nipCount = doc.supportedNips.isEmpty ? nil : "\(doc.supportedNips.count) NIPs"
                let parts = [doc.name, softwareLabel, nipCount].compactMap { $0 }
                return (.reachable, parts.isEmpty ? "Reachable (no NIP-11 metadata)" : parts.joined(separator: " • "))
            }
            if probeFailed { return (.unreachable, "Couldn't reach the relay — you can still add it.") }
            return (.idle, "")
        }()
        return AddRelaySheetProjection(
            normalizedUrl: normalizedUrl,
            clipboardUrl: clipboardUrl,
            isValid: isValid,
            isUnencrypted: normalizedUrl.hasPrefix("ws://"),
            canAdd: isValid,
            addConfig: RelayConfig(url: normalizedUrl, read: read, write: write, rooms: rooms, indexer: indexer),
            probeStatus: probeStatus,
            probeText: probeText
        )
    }

    var body: some View {
        let currentProjection = projection

        NavigationStack {
            Form {
                Section {
                    TextField("wss://relay.example.com", text: $urlText)
                        .keyboardType(.URL)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .onChange(of: urlText) { _, _ in scheduleProbe() }
                    if currentProjection.isUnencrypted {
                        Label("Unencrypted connection — use wss:// when possible.", systemImage: "exclamationmark.triangle")
                            .font(.caption)
                            .foregroundStyle(.orange)
                    }
                    if let paste = currentProjection.clipboardUrl {
                        Button {
                            urlText = paste
                            scheduleProbe()
                        } label: {
                            HStack {
                                Image(systemName: "doc.on.clipboard")
                                Text("Paste \(paste)")
                                    .lineLimit(1)
                                    .truncationMode(.middle)
                            }
                            .font(.caption)
                        }
                    }
                    probeStatus
                } header: {
                    Text("Relay URL")
                } footer: {
                    Text("Use wss:// for a secure connection.")
                }

                Section {
                    Toggle("Read", isOn: $read)
                    Toggle("Write", isOn: $write)
                    Toggle("Rooms", isOn: $rooms)
                    Toggle("Indexer", isOn: $indexer)
                } header: {
                    Text("Roles")
                } footer: {
                    Text("Read/Write affect the kind:10002 event your app publishes. Rooms routes NIP-29 group traffic. Indexer is the outbox-model bootstrap pool.")
                }
            }
            .navigationTitle("Add Relay")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Add") {
                        onAdd(currentProjection.addConfig)
                        dismiss()
                    }
                    .disabled(!currentProjection.canAdd)
                }
            }
        }
    }

    // MARK: - NIP-11 probe

    /// Inline status line below the URL field. Shows the fetched relay
    /// software / name after a successful probe, a muted note while the
    /// probe is in flight, or a gentle warning if the probe failed.
    /// Probe failure never blocks Add — relays go up and down all the
    /// time.
    @ViewBuilder
    private var probeStatus: some View {
        let currentProjection = projection

        switch currentProjection.probeStatus {
        case .checking:
            HStack(spacing: 6) {
                ProgressView().scaleEffect(0.7)
                Text(currentProjection.probeText)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        case .reachable:
            HStack(spacing: 6) {
                Image(systemName: "checkmark.seal.fill")
                    .foregroundStyle(.green)
                Text(currentProjection.probeText)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }
        case .unreachable:
            HStack(spacing: 6) {
                Image(systemName: "questionmark.circle")
                    .foregroundStyle(.secondary)
                Text(currentProjection.probeText)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        case .idle:
            EmptyView()
        }
    }

    /// Cancels an in-flight probe if the URL changes before it resolves.
    private func scheduleProbe() {
        probeTask?.cancel()
        probeResult = nil
        probeFailed = false
        let currentProjection = projection
        guard currentProjection.isValid else { return }
        let url = currentProjection.normalizedUrl
        probeTask = Task { [url] in
            guard !Task.isCancelled else { return }
            probeInFlight = true
            defer { probeInFlight = false }
            let result = await Self.fetchNip11(relayUrl: url)
            guard !Task.isCancelled else { return }
            if let doc = result {
                probeResult = doc
                probeFailed = false
            } else {
                probeResult = nil
                probeFailed = true
            }
        }
    }

    /// Fetch NIP-11 relay metadata via HTTP GET with `Accept: application/nostr+json`.
    /// Converts wss:// → https:// (ws:// → http://) per NIP-11 spec.
    private static func fetchNip11(relayUrl: String) async -> Nip11Document? {
        let httpUrl = relayUrl
            .replacingOccurrences(of: "wss://", with: "https://")
            .replacingOccurrences(of: "ws://", with: "http://")
        guard let url = URL(string: httpUrl) else { return nil }
        var request = URLRequest(url: url, timeoutInterval: 8)
        request.setValue("application/nostr+json", forHTTPHeaderField: "Accept")
        guard let (data, response) = try? await URLSession.shared.data(for: request),
              let httpResponse = response as? HTTPURLResponse,
              httpResponse.statusCode == 200,
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return nil }
        return Nip11Document(
            url: relayUrl,
            name: json["name"] as? String,
            description: json["description"] as? String,
            pubkey: json["pubkey"] as? String,
            contact: json["contact"] as? String,
            software: json["software"] as? String,
            version: json["version"] as? String,
            supportedNips: (json["supported_nips"] as? [Int])?.map { UInt32($0) } ?? [],
            icon: json["icon"] as? String
        )
    }
}
