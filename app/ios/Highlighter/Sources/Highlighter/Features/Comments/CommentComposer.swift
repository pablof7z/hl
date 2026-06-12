import SwiftUI

/// Pinned-bottom composer. Replies to the current thread's subject; drafts
/// and publish state are owned by the Rust comments snapshot.
struct CommentComposer: View {
    let parentEventId: String?
    /// Display label for the composer placeholder — caller passes context
    /// like "Add to the conversation" at root, "Reply to @alice" inside
    /// a pushed thread.
    let placeholder: String

    @Environment(HighlighterStore.self) private var app

    @FocusState private var focused: Bool

    private var draft: Binding<String> {
        Binding(
            get: { app.commentDraft(parentEventId: parentEventId) },
            set: { value in
                app.setCommentDraft(parentEventId: parentEventId, body: value)
                app.clearCommentPublishError()
            }
        )
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let errorMessage = app.comments.publishErrorMessage {
                Text(errorMessage)
                    .font(.caption)
                    .foregroundStyle(Color.highlighterAccent)
                    .padding(.horizontal, 14)
                    .padding(.top, 6)
                    .transition(.opacity)
            }
            HStack(alignment: .bottom, spacing: 10) {
                TextField(placeholder, text: draft, axis: .vertical)
                    .focused($focused)
                    .lineLimit(1 ... 6)
                    .font(.body)
                    .foregroundStyle(Color.highlighterInkStrong)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 10)
                    .background(
                        Color.highlighterInkStrong.opacity(0.06),
                        in: RoundedRectangle(cornerRadius: 18, style: .continuous)
                    )
                    .submitLabel(.send)
                    .onSubmit { submit() }

                sendButton
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
        }
        .background(
            .regularMaterial,
            in: RoundedRectangle(cornerRadius: 0, style: .continuous)
        )
        .overlay(alignment: .top) {
            Rectangle()
                .fill(Color.highlighterRule.opacity(0.6))
                .frame(height: 0.5)
        }
    }

    private var sendButton: some View {
        Button(action: submit) {
            ZStack {
                Circle()
                    .fill(canSubmit ? Color.highlighterAccent : Color.highlighterInkMuted.opacity(0.35))
                if app.comments.isPublishing {
                    ProgressView()
                        .progressViewStyle(.circular)
                        .tint(.white)
                } else {
                    Image(systemName: "arrow.up")
                        .font(.system(size: 14, weight: .bold))
                        .foregroundStyle(.white)
                }
            }
            .frame(width: 36, height: 36)
        }
        .buttonStyle(.plain)
        .disabled(!canSubmit || app.comments.isPublishing)
        .animation(.easeInOut(duration: 0.18), value: canSubmit)
        .accessibilityLabel("Send comment")
    }

    private var canSubmit: Bool {
        !draft.wrappedValue.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private func submit() {
        guard canSubmit, !app.comments.isPublishing else { return }
        app.clearCommentPublishError()
        app.publishComment(parentEventId: parentEventId)
        focused = false
    }
}
