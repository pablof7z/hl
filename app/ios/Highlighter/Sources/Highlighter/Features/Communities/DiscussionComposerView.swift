import SwiftUI

/// New-discussion composer. Swift owns the transient form text; Rust owns URL
/// validation, publishing, error state, and the room projection refresh.
struct DiscussionComposerView: View {
    let groupId: String
    let navigationTitle: String

    init(groupId: String, navigationTitle: String = "New discussion") {
        self.groupId = groupId
        self.navigationTitle = navigationTitle
    }

    @Environment(HighlighterStore.self) private var app
    @Environment(\.dismiss) private var dismiss

    @State private var title: String = ""
    @State private var messageBody: String = ""
    @State private var attachmentURL: String = ""
    @State private var waitingForPublish: Bool = false

    private var canPublish: Bool {
        !title.trimmingCharacters(in: .whitespaces).isEmpty && !isPublishing
    }

    var body: some View {
        NavigationStack {
            Form {
                Section("Title") {
                    TextField("What do you want to talk about?", text: $title)
                        .textInputAutocapitalization(.sentences)
                }
                Section("Body") {
                    TextEditor(text: $messageBody)
                        .frame(minHeight: 140)
                }
                Section {
                    TextField("https://…", text: $attachmentURL)
                        .keyboardType(.URL)
                        .textContentType(.URL)
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                } header: {
                    Text("Attach URL (optional)")
                } footer: {
                    Text("Paste a podcast, article, or book link to propose it to the room.")
                }
                if let errorMessage {
                    Section {
                        Text(errorMessage).foregroundStyle(.red)
                    }
                }
            }
            .onChange(of: publishedDiscussionId) { _, eventId in
                guard waitingForPublish, eventId != nil else { return }
                waitingForPublish = false
                dismiss()
            }
            .onChange(of: errorMessage) { _, message in
                if message != nil {
                    waitingForPublish = false
                }
            }
            .navigationTitle(navigationTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                        .disabled(isPublishing)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(isPublishing ? "Posting…" : "Post") {
                        publish()
                    }
                    .disabled(!canPublish)
                }
            }
        }
    }

    private var activeRoomDetail: HighlighterRoomDetailSnapshot {
        app.roomDetail
    }

    private var isPublishing: Bool {
        activeRoomDetail.groupId == groupId && activeRoomDetail.isPublishingDiscussion
    }

    private var errorMessage: String? {
        activeRoomDetail.groupId == groupId ? activeRoomDetail.discussionErrorMessage : nil
    }

    private var publishedDiscussionId: String? {
        activeRoomDetail.groupId == groupId ? activeRoomDetail.lastPublishedDiscussionId : nil
    }

    private func publish() {
        waitingForPublish = true
        app.clearRoomDiscussionError()
        app.publishRoomDiscussion(
            title: title,
            body: messageBody,
            attachmentUrl: attachmentURL
        )
    }
}
