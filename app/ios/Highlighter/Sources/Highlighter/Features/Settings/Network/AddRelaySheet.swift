import SwiftUI

/// Sheet for adding a new relay. URL field + role chips. Sane defaults:
/// Read + Write on, Rooms and Indexer off. A user can tap chips after the
/// relay is in the list if they want to change the roles.
struct AddRelaySheet: View {
    @Environment(\.dismiss) private var dismiss

    let onAdd: (RelayConfig) -> Void

    @State private var urlText = ""
    @State private var read = true
    @State private var write = true
    @State private var rooms = false
    @State private var indexer = false

    /// Whether the URL looks like a wss:// or ws:// URL.
    private var isValid: Bool {
        let trimmed = urlText.trimmingCharacters(in: .whitespaces)
        return trimmed.hasPrefix("wss://") || trimmed.hasPrefix("ws://")
    }

    private var isUnencrypted: Bool {
        urlText.trimmingCharacters(in: .whitespaces).hasPrefix("ws://")
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("wss://relay.example.com", text: $urlText)
                        .keyboardType(.URL)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    if isUnencrypted {
                        Label("Unencrypted connection — use wss:// when possible.", systemImage: "exclamationmark.triangle")
                            .font(.caption)
                            .foregroundStyle(.orange)
                    }
                    if let paste = clipboardURL, paste != urlText {
                        Button {
                            urlText = paste
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
                } header: {
                    Text("Relay URL")
                } footer: {
                    Text("Use wss:// for a secure connection. The relay's NIP-11 doc will be fetched after adding.")
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
                        let trimmed = urlText.trimmingCharacters(in: .whitespaces)
                        onAdd(
                            RelayConfig(
                                url: trimmed,
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
}
