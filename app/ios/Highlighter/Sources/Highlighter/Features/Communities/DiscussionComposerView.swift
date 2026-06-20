import SwiftUI

/// New-discussion composer. A discussion is a kind:11 thread with the
/// `t=discussion` marker, optionally carrying an attached URL (rendered as
/// an artifact preview chip). Publishing is synchronous from the user's
/// POV — we hold the sheet open until the core confirms the publish so
/// callers can refresh their Rust-backed projection.
struct DiscussionComposerView: View {
    let groupId: String
    let navigationTitle: String
    let onPublished: () -> Void

    init(groupId: String, navigationTitle: String = "New discussion", onPublished: @escaping () -> Void) {
        self.groupId = groupId
        self.navigationTitle = navigationTitle
        self.onPublished = onPublished
    }

    @Environment(HighlighterStore.self) private var app
    @Environment(HighlighterAppKernel.self) private var kernel
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

        // Kernel is the sole writer: dispatch hl.discussion.post fire-and-forget.
        // The new kind:11 streams back into kernel.roomDiscussions, which the
        // list re-applies; dismiss optimistically (publish errors surface as
        // kernel toasts, same as other kernel actions).
        let attachment = projection.submitAttachmentUrl.flatMap { $0.isEmpty ? nil : $0 }
        kernel.app.dispatch(.postDiscussion(
            groupId: groupId,
            title: projection.submitTitle,
            body: projection.submitBody,
            attachmentUrl: attachment
        ))
        onPublished()
        dismiss()
    }
}
