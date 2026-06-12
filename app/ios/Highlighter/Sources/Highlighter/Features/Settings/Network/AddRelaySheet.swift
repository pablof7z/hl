import SwiftUI

/// Sheet for adding a new relay. URL field + role chips. Sane defaults:
/// Read + Write on, Rooms and Indexer off. A user can tap chips after the
/// relay is in the list if they want to change the roles.
struct AddRelaySheet: View {
    @Environment(HighlighterStore.self) private var appStore
    @Environment(\.dismiss) private var dismiss

    @State private var urlText = ""
    @State private var read = true
    @State private var write = true
    @State private var rooms = false
    @State private var indexer = false
    @FocusState private var urlFieldFocused: Bool

    /// Whether the URL looks like a wss:// or ws:// URL.
    private var isValid: Bool {
        let trimmed = urlText.trimmingCharacters(in: .whitespaces)
        return trimmed.hasPrefix("wss://") || trimmed.hasPrefix("ws://")
    }

    private var isUnencrypted: Bool {
        urlText.trimmingCharacters(in: .whitespaces).hasPrefix("ws://")
    }

    private var trimmedUrl: String {
        urlText.trimmingCharacters(in: .whitespaces)
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("wss://relay.example.com", text: $urlText)
                        .keyboardType(.URL)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .focused($urlFieldFocused)
                        .onSubmit { startProbeForCurrentURL() }
                        .onChange(of: urlFieldFocused) { _, focused in
                            if !focused {
                                startProbeForCurrentURL()
                            }
                        }
                    if isUnencrypted {
                        Label("Unencrypted connection — use wss:// when possible.", systemImage: "exclamationmark.triangle")
                            .font(.caption)
                            .foregroundStyle(.orange)
                    }
                    if let paste = clipboardURL, paste != urlText {
                        Button {
                            urlText = paste
                            startProbeForCurrentURL()
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
                        appStore.upsertNetworkRelay(
                            RelayConfig(
                                url: trimmedUrl,
                                read: read,
                                write: write,
                                rooms: rooms,
                                indexer: indexer
                            )
                        )
                        dismiss()
                    }
                    .disabled(!isValid)
                }
            }
        }
    }

    /// Returns the clipboard string if and only if it looks like a wss URL.
    /// Avoids noisy paste prompts for arbitrary text.
    private var clipboardURL: String? {
        guard let s = UIPasteboard.general.string?.trimmingCharacters(in: .whitespaces) else {
            return nil
        }
        guard s.hasPrefix("wss://") || s.hasPrefix("ws://") else { return nil }
        return s
    }

    // MARK: - NIP-11 probe

    /// Inline status line below the URL field. Shows the fetched relay
    /// software / name after a successful probe, a muted note while the
    /// probe is in flight, or a gentle warning if the probe failed.
    /// Probe failure never blocks Add — relays go up and down all the
    /// time.
    @ViewBuilder
    private var probeStatus: some View {
        let probe = appStore.networkNip11(url: trimmedUrl)
        if probe?.isLoading == true {
            HStack(spacing: 6) {
                ProgressView().scaleEffect(0.7)
                Text("Checking relay…")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        } else if let doc = probe?.document {
            HStack(spacing: 6) {
                Image(systemName: "checkmark.seal.fill")
                    .foregroundStyle(.green)
                Text(nip11Summary(doc))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }
        } else if let err = probe?.errorMessage {
            HStack(spacing: 6) {
                Image(systemName: "questionmark.circle")
                    .foregroundStyle(.secondary)
                Text(err)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private func nip11Summary(_ doc: Nip11Document) -> String {
        let softwareLabel: String? = doc.software.map { name in
            if let version = doc.version {
                return "\(name) \(version)"
            }
            return name
        }
        let parts: [String?] = [
            doc.name,
            softwareLabel,
            doc.supportedNips.isEmpty ? nil : "\(doc.supportedNips.count) NIPs",
        ]
        let joined = parts.compactMap { $0 }.joined(separator: " • ")
        return joined.isEmpty ? "Reachable (no NIP-11 metadata)" : joined
    }

    private func startProbeForCurrentURL() {
        guard isValid else { return }
        appStore.probeNetworkRelayNip11(url: trimmedUrl)
    }
}
