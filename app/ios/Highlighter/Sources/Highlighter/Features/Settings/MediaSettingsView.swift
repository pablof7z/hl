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
                        let projection = store.safeCore.projectBlossomServerList(
                            input: BlossomServerListProjectionInput(
                                servers: servers,
                                addUrl: nil,
                                removeIndexes: [],
                                moveIndexes: indices.map(UInt64.init),
                                moveToIndex: UInt64(newOffset)
                            )
                        )
                        servers = projection.servers
                        if projection.canSave {
                            Task { await save() }
                        }
                    }
                    .onDelete { indices in
                        let projection = store.safeCore.projectBlossomServerList(
                            input: BlossomServerListProjectionInput(
                                servers: servers,
                                addUrl: nil,
                                removeIndexes: indices.map(UInt64.init),
                                moveIndexes: [],
                                moveToIndex: nil
                            )
                        )
                        servers = projection.servers
                        if projection.canSave {
                            Task { await save() }
                        }
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
                let projection = store.safeCore.projectBlossomServerList(
                    input: BlossomServerListProjectionInput(
                        servers: servers,
                        addUrl: url,
                        removeIndexes: [],
                        moveIndexes: [],
                        moveToIndex: nil
                    )
                )
                servers = projection.servers
                if projection.canSave {
                    Task { await save() }
                }
            }
        }
        .task { await load() }
    }

    private func load() async {
        let snapshot = await store.safeCore.getBlossomServerSettingsSnapshot()
        servers = snapshot.servers
        isLoading = false
    }

    private func save() async {
        let projection = store.safeCore.projectBlossomServerList(
            input: BlossomServerListProjectionInput(
                servers: servers,
                addUrl: nil,
                removeIndexes: [],
                moveIndexes: [],
                moveToIndex: nil
            )
        )
        guard projection.canSave else { return }
        servers = projection.servers
        isSaving = true
        let snapshot = await store.safeCore.setBlossomServerSettings(servers)
        servers = snapshot.servers
        isSaving = false
    }
}

private struct AddBlossomServerSheet: View {
    let existingServers: [String]
    let onAdd: (String) -> Void

    @Environment(HighlighterStore.self) private var store
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
                        onAdd(entryProjection.submitUrl)
                        dismiss()
                    }
                    .disabled(!entryProjection.canAdd)
                }
            }
        }
        .presentationDetents([.medium])
    }

    private var entryProjection: BlossomServerEntryProjection {
        let submitUrl = urlText.trimmingCharacters(in: .whitespaces)
        let isValid = submitUrl.hasPrefix("https://") || submitUrl.hasPrefix("http://")
        let isDuplicate = existingServers.contains { $0.trimmingCharacters(in: .whitespaces) == submitUrl }
        return BlossomServerEntryProjection(submitUrl: submitUrl, isValid: isValid, isDuplicate: isDuplicate, canAdd: isValid && !isDuplicate)
    }
}
