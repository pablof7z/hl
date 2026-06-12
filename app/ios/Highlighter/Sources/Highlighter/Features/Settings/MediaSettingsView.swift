import SwiftUI

struct MediaSettingsView: View {
    @Environment(HighlighterStore.self) private var store
    @State private var showAddSheet = false

    var body: some View {
        List {
            Section {
                if store.mediaSettings.isLoading {
                    ProgressView()
                        .frame(maxWidth: .infinity, alignment: .center)
                        .padding(.vertical, 8)
                } else {
                    ForEach(store.mediaSettings.blossomServers, id: \.self) { server in
                        Text(server)
                            .lineLimit(1)
                            .truncationMode(.middle)
                    }
                    .onMove { indices, newOffset in
                        store.moveBlossomServers(fromOffsets: indices, toOffset: newOffset)
                    }
                    .onDelete { indices in
                        for idx in indices where idx < store.mediaSettings.blossomServers.count {
                            store.removeBlossomServer(url: store.mediaSettings.blossomServers[idx])
                        }
                    }
                    if let error = store.mediaSettings.errorMessage {
                        Text(error)
                            .font(.caption)
                            .foregroundStyle(.red)
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
                .disabled(store.mediaSettings.isSaving || store.mediaSettings.isLoading)
            }
            ToolbarItem(placement: .topBarLeading) {
                if !store.mediaSettings.isLoading {
                    EditButton()
                }
            }
        }
        .sheet(isPresented: $showAddSheet) {
            AddBlossomServerSheet()
        }
        .task { store.openMediaSettings() }
        .onDisappear { store.closeMediaSettings() }
    }
}

private struct AddBlossomServerSheet: View {
    @Environment(HighlighterStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var urlText = ""

    private var isValid: Bool {
        let t = urlText.trimmingCharacters(in: .whitespaces)
        return t.hasPrefix("https://") || t.hasPrefix("http://")
    }

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
                        let trimmed = urlText.trimmingCharacters(in: .whitespaces)
                        store.addBlossomServer(url: trimmed)
                        dismiss()
                    }
                    .disabled(!isValid)
                }
            }
        }
        .presentationDetents([.medium])
    }
}
