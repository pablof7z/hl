import SwiftUI

/// Body-only composer for a new feedback thread. Title comes later from the
/// agent's kind:513 metadata; until then the thread row falls back to the
/// trimmed body content.
struct FeedbackNewThreadView: View {
    @Environment(HighlighterStore.self) private var app
    @Environment(\.dismiss) private var dismiss

    @State private var seenPublishedRoot: String?

    private var canPublish: Bool {
        !app.feedback.newThreadDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && !app.feedback.isPublishingNewThread
    }

    private var draft: Binding<String> {
        Binding(
            get: { app.feedback.newThreadDraft },
            set: { app.setFeedbackNewThreadDraft($0) }
        )
    }

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 0) {
                TextEditor(text: draft)
                    .font(.body)
                    .padding(.horizontal, 12)
                    .padding(.top, 8)
                    .overlay(alignment: .topLeading) {
                        if app.feedback.newThreadDraft.isEmpty {
                            Text("What's on your mind?")
                                .font(.body)
                                .foregroundStyle(.tertiary)
                                .padding(.horizontal, 17)
                                .padding(.top, 16)
                                .allowsHitTesting(false)
                        }
                    }
                if let errorMessage = app.feedback.publishErrorMessage {
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
                        .disabled(app.feedback.isPublishingNewThread)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(app.feedback.isPublishingNewThread ? "Sending…" : "Send") {
                        app.publishFeedbackNewThread()
                    }
                    .disabled(!canPublish)
                }
            }
        }
        .onAppear {
            seenPublishedRoot = app.feedback.lastPublishedRootEventId
            app.clearFeedbackPublishError()
        }
        .onChange(of: app.feedback.lastPublishedRootEventId) { _, next in
            guard let next, next != seenPublishedRoot else { return }
            seenPublishedRoot = next
            if !app.feedback.isPublishingNewThread {
                dismiss()
            }
        }
    }
}
