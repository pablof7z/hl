import SwiftUI

struct MediaSettingsView: View {
    var body: some View {
        List {
            Section {
                Text("Default server")
                    .lineLimit(1)
                    .truncationMode(.middle)
            } header: {
                Text("Blossom Servers")
            } footer: {
                Text("Files are uploaded to the default media server.")
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle("Media")
        .navigationBarTitleDisplayMode(.inline)
    }
}
