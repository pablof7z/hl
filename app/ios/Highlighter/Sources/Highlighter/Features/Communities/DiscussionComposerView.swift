import SwiftUI

/// New-discussion composer. A discussion is a kind:11 thread with the
/// `t=discussion` marker, optionally carrying an attached URL (rendered as
/// an artifact preview chip). Publishing is synchronous from the user's
/// POV — we hold the sheet open until the core returns a signed record so
/// the caller can optimistically insert it into the list.
struct DiscussionComposerView: View {
    let groupId: String
    let navigationTitle: String
    let onPublished: (DiscussionRecord) -> Void

    init(groupId: String, navigationTitle: String = "New discussion", onPublished: @escaping (DiscussionRecord) -> Void) {
        self.groupId = groupId
        self.navigationTitle = navigationTitle
        self.onPublished = onPublished
    }

    @Environment(HighlighterStore.self) private var app
    @Environment(\.dismiss) private var dismiss

    @State private var title: String = ""
    @State private var messageBody: String = ""
    @State private var attachmentURL: String = ""
    @State private var isPublishing: Bool = false
    @State private var errorMessage: String?

    private var canPublish: Bool {
        composerProjection.canPublish
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
            .navigationTitle(navigationTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                        .disabled(isPublishing)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(isPublishing ? "Posting…" : "Post") {
                        Task { await publish() }
                    }
                    .disabled(!canPublish)
                }
            }
        }
    }

    private var composerProjection: DiscussionComposerProjection {
        app.safeCore.projectDiscussionComposer(
            input: DiscussionComposerProjectionInput(
                title: title,
                body: messageBody,
                attachmentUrl: attachmentURL,
                isPublishing: isPublishing
            )
        )
    }

    private func publish() async {
        let projection = composerProjection
        guard projection.canPublish else { return }
        isPublishing = true
        errorMessage = nil
        defer { isPublishing = false }

        let outcome = await app.safeCore.publishDiscussionFromComposer(
            input: DiscussionComposerPublishInput(
                groupId: groupId,
                title: projection.submitTitle,
                body: projection.submitBody,
                attachmentUrl: projection.submitAttachmentUrl
            )
        )
        guard outcome.error.isEmpty, let record = outcome.value else {
            errorMessage = outcome.error.isEmpty ? "Failed to publish." : outcome.error
            return
        }
        onPublished(record)
        dismiss()
    }
}
