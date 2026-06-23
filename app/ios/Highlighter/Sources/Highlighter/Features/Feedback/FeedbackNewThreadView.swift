import SwiftUI

/// Body-only composer for a new feedback thread. Title arrives from the
/// agent's kind:513 metadata; until then the thread row falls back to the
/// trimmed body content.
struct FeedbackNewThreadView: View {
    let store: FeedbackStore
    /// Called after a successful send. Caller decides whether to also dismiss
    /// the parent threads sheet (currently always `false` — we stay in the
    /// list so the user sees their new thread arrive).
    let onSent: (Bool) -> Void

    @Environment(HighlighterStore.self) private var app
    @Environment(\.dismiss) private var dismiss

    @State private var draft: String = ""
    @State private var isPublishing = false
    @State private var errorMessage: String?

    private var composerProjection: FeedbackComposerProjection {
        let submitBody = draft.trimmingCharacters(in: .whitespaces)
        return FeedbackComposerProjection(submitBody: submitBody, canSend: !submitBody.isEmpty && !isPublishing)
    }

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 0) {
                TextEditor(text: $draft)
                    .font(.body)
                    .padding(.horizontal, 12)
                    .padding(.top, 8)
                    .overlay(alignment: .topLeading) {
                        if draft.isEmpty {
                            Text("What's on your mind?")
                                .font(.body)
                                .foregroundStyle(.tertiary)
                                .padding(.horizontal, 17)
                                .padding(.top, 16)
                                .allowsHitTesting(false)
                        }
                    }
                if let errorMessage {
                    Text(errorMessage)
                        .font(.caption)
                        .foregroundStyle(.red)
                        .padding(.horizontal, 16)
                        .padding(.bottom, 8)
                }
            }
            .navigationTitle("New feedback")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                        .disabled(isPublishing)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(isPublishing ? "Sending…" : "Send") {
                        Task { await publish() }
                    }
                    .disabled(!composerProjection.canSend)
                }
            }
        }
    }

    private func publish() async {
        let projection = composerProjection
        guard projection.canSend else { return }

        // Kernel is the sole writer: dispatch hl.feedback.post_root
        // fire-and-forget. The new root note streams back into
        // kernel.feedbackThreads, which the list re-applies. Dismiss
        // optimistically (publish errors surface as kernel toasts).
        store.postRoot(content: projection.submitBody)
        dismiss()
        onSent(false)
    }
}
