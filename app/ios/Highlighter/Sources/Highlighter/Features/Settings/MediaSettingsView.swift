import SwiftUI

struct MediaSettingsView: View {
    @Environment(HighlighterStore.self) private var store
    @State private var servers: [String] = []
    @State private var isLoading = true
    @State private var showAddSheet = false
    @State private var isSaving = false

    var body: some View {
        List {
            Section {
                if isLoading {
                    ProgressView()
                        .frame(maxWidth: .infinity, alignment: .center)
                        .padding(.vertical, 8)
                } else {
                    ForEach(servers, id: \.self) { server in
                        Text(server)
                            .lineLimit(1)
                            .truncationMode(.middle)
                    }
                    .onMove { indices, newOffset in
                        servers.move(fromOffsets: indices, toOffset: newOffset)
                        Task { await save() }
                    }
                    .onDelete { indices in
                        servers.remove(atOffsets: indices)
                        Task { await save() }
                    }
                }
            } header: {
                Text("Blossom Servers")
            } footer: {
                Text("Files are uploaded to the first reachable server. Drag to change priority.")
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle("Media")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    showAddSheet = true
                } label: {
                    Image(systemName: "plus")
                }
                .disabled(isSaving || isLoading)
            }
            ToolbarItem(placement: .topBarLeading) {
                if !isLoading {
                    EditButton()
                }
            }
        }
        .sheet(isPresented: $showAddSheet) {
            AddBlossomServerSheet(existingServers: servers) { url in
                servers.append(url)
                Task { await save() }
            }
        }
        .task { await load() }
    }

    private func load() async {
        let snapshot = await store.core.getBlossomServerSettingsSnapshot()
        servers = snapshot.servers
        isLoading = false
    }

    private func save() async {
        guard !servers.isEmpty else { return }
        isSaving = true
        let snapshot = await store.core.setBlossomServerSettings(servers: servers)
        servers = snapshot.servers
        isSaving = false
    }
}

private struct AddBlossomServerSheet: View {
    let existingServers: [String]
    let onAdd: (String) -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var urlText = ""

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("https://blossom.example.com", text: $urlText)
                        .keyboardType(.URL)
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                } header: {
                    Text("Server URL")
                } footer: {
                    Text("Enter the base URL of a Blossom-compatible media server.")
                }
            }
            .navigationTitle("Add Server")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Add") {
                        onAdd(submitUrl)
                        dismiss()
                    }
                    .disabled(!canAdd)
                }
            }
        }
        .presentationDetents([.medium])
    }

    private var submitUrl: String { urlText.trimmingCharacters(in: .whitespaces) }

    private var canAdd: Bool {
        let isValid = submitUrl.hasPrefix("https://") || submitUrl.hasPrefix("http://")
        let isDuplicate = existingServers.contains { $0.trimmingCharacters(in: .whitespaces) == submitUrl }
        return isValid && !isDuplicate
    }
}
