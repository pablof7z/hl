import SwiftUI

/// Chat-style view for a single feedback thread. Shows every kind:1
/// `e`-tagged to the root, regardless of author, with a composer pinned to
/// the bottom for replies.
struct FeedbackThreadDetailView: View {
    let thread: FeedbackThreadRecord
    let listStore: FeedbackStore

    @Environment(HighlighterStore.self) private var app
    @Environment(HighlighterAppKernel.self) private var kernel
    @State private var detailStore = FeedbackThreadStore()
    @State private var draft: String = ""
    @State private var sendError: String?

    var body: some View {
        VStack(spacing: 0) {
            messageList
            Divider()
            composer
        }
        .navigationTitle(navigationTitle)
        .navigationBarTitleDisplayMode(.inline)
        .task {
            await detailStore.start(rootEventId: thread.rootEventId, kernel: kernel)
        }
        .onChange(of: kernel.feedbackThread[thread.rootEventId]) { _, _ in
            detailStore.applyKernelSnapshot()
        }
        .onDisappear { detailStore.stop() }
    }

    @ViewBuilder
    private var messageList: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 2) {
                    if let summary = detailSummary {
                        Text(summary)
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                            .padding(.horizontal, 12)
                            .padding(.top, 8)
                            .padding(.bottom, 6)
                    }
                    ForEach(detailStore.rows, id: \.event.eventId) { row in
                        FeedbackMessageBubble(
                            event: row.event,
                            presentation: messagePresentation(for: row)
                        )
                        .id(row.event.eventId)
                        .task(id: row.event.authorPubkey) {
                            await app.requestProfile(pubkeyHex: row.event.authorPubkey)
                        }
                    }
                    if detailStore.isLoading && detailStore.rows.isEmpty {
                        ProgressView().padding()
                    }
                }
                .padding(.vertical, 8)
            }
            .onChange(of: detailStore.rows.count) { _, _ in
                if let last = detailStore.rows.last {
                    withAnimation(.easeOut(duration: 0.2)) {
                        proxy.scrollTo(last.event.eventId, anchor: .bottom)
                    }
                }
            }
        }
    }

    @ViewBuilder
    private var composer: some View {
        VStack(spacing: 6) {
            if let sendError {
                Text(sendError)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            HStack(alignment: .bottom, spacing: 8) {
                TextField("Reply…", text: $draft, axis: .vertical)
                    .textFieldStyle(.roundedBorder)
                    .lineLimit(1...5)
                Button {
                    Task { await send() }
                } label: {
                    Image(systemName: "paperplane.fill")
                        .font(.title3)
                        .frame(width: 36, height: 36)
                        .background(
                            Color.accentColor.opacity(composerCanSend ? 1 : 0.4),
                            in: .circle
                        )
                        .foregroundStyle(.white)
                }
                .disabled(!composerCanSend)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }

    // MARK: – Inline projections (no safeCore FFI calls)

    /// `submitBody` is the draft trimmed; `canSend` requires a non-empty body
    /// and no in-flight publish — mirrors feedback_composer_projection in Rust.
    private var composerSubmitBody: String { draft.trimmingCharacters(in: .whitespacesAndNewlines) }
    private var composerCanSend: Bool { !composerSubmitBody.isEmpty && !detailStore.isPublishing }

    /// Navigation title: thread's title when set, otherwise "Feedback".
    private var navigationTitle: String { thread.title ?? "Feedback" }

    /// One-liner detail summary: non-empty summary if present, otherwise nil.
    private var detailSummary: String? {
        guard let s = thread.summary, !s.isEmpty else { return nil }
        return s
    }

    private func send() async {
        let body = composerSubmitBody
        guard composerCanSend else { return }

        // Kernel is the sole writer: dispatch hl.feedback.post_reply
        // fire-and-forget. The reply streams back into
        // kernel.feedbackThread[rootEventId] (and bumps the list's activity),
        // which the views re-apply. Clear the draft optimistically.
        sendError = nil
        await detailStore.sendReply(body: body)
        draft = ""
    }

    /// Inline equivalent of feedback_message_presentation in Rust.
    private func messagePresentation(
        for row: FeedbackMessageRowProjection
    ) -> InlineFeedbackMessagePresentation {
        let event = row.event
        let profile = app.profileSnapshots[event.authorPubkey]
        let isFromMe = app.currentUser?.pubkey == event.authorPubkey
        let displayName: String = {
            if let p = profile {
                if !p.displayName.isEmpty { return p.displayName }
                if !p.name.isEmpty { return p.name }
            }
            return String(event.authorPubkey.prefix(8))
        }()
        let displayInitial = displayName.unicodeScalars.first.map { String($0).uppercased() } ?? ""
        let pictureUrl = profile?.picture ?? ""
        return InlineFeedbackMessagePresentation(
            isFromMe: isFromMe,
            showHeader: row.showHeader,
            displayName: displayName,
            displayInitial: displayInitial,
            pictureUrl: pictureUrl
        )
    }
}

/// Value type used in place of the FFI `FeedbackMessagePresentationProjection`.
private struct InlineFeedbackMessagePresentation {
    let isFromMe: Bool
    let showHeader: Bool
    let displayName: String
    let displayInitial: String
    let pictureUrl: String
}

private struct FeedbackMessageBubble: View {
    let event: FeedbackEventRecord
    let presentation: InlineFeedbackMessagePresentation

    var body: some View {
        HStack(alignment: .bottom, spacing: 6) {
            if presentation.isFromMe {
                Spacer(minLength: 40)
            } else {
                avatarSlot
            }

            VStack(alignment: presentation.isFromMe ? .trailing : .leading, spacing: 2) {
                if presentation.showHeader {
                    Text(presentation.displayName)
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 4)
                        .padding(.top, 8)
                }
                Text(markdownContent)
                    .font(.body)
                    .foregroundStyle(presentation.isFromMe ? Color.white : Color.primary)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .background(
                        presentation.isFromMe
                            ? Color.accentColor
                            : Color(.secondarySystemBackground),
                        in: .rect(cornerRadius: 14)
                    )
                Text(timeLabel(event.createdAt))
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 4)
            }

            if presentation.isFromMe {
                avatarSlot
            } else {
                Spacer(minLength: 40)
            }
        }
        .padding(.horizontal, 12)
        .padding(.top, presentation.showHeader ? 4 : 1)
    }

    @ViewBuilder
    private var avatarSlot: some View {
        if presentation.showHeader {
            AuthorAvatar(
                pubkey: event.authorPubkey,
                pictureURL: presentation.pictureUrl,
                displayInitial: presentation.displayInitial,
                size: 28
            )
        } else {
            Color.clear.frame(width: 28, height: 1)
        }
    }

    private var markdownContent: AttributedString {
        (try? AttributedString(
            markdown: event.content,
            options: AttributedString.MarkdownParsingOptions(
                interpretedSyntax: .inlineOnlyPreservingWhitespace
            )
        )) ?? AttributedString(event.content)
    }

    private func timeLabel(_ ts: UInt64) -> String {
        let date = Date(timeIntervalSince1970: TimeInterval(ts))
        let formatter = DateFormatter()
        formatter.dateStyle = .none
        formatter.timeStyle = .short
        return formatter.string(from: date)
    }
}
