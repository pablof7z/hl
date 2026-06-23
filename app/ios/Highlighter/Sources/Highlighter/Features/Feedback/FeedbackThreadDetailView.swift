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
        .navigationTitle(threadPresentation.navigationTitle)
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
                    if let summary = threadPresentation.detailSummary {
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
                            projection: messagePresentation(for: row)
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
                            Color.accentColor.opacity(composerProjection.canSend ? 1 : 0.4),
                            in: .circle
                        )
                        .foregroundStyle(.white)
                }
                .disabled(!composerProjection.canSend)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }

    private var composerProjection: FeedbackComposerProjection {
        let submitBody = draft.trimmingCharacters(in: .whitespaces)
        return FeedbackComposerProjection(submitBody: submitBody, canSend: !submitBody.isEmpty && !detailStore.isPublishing)
    }

    private var threadPresentation: FeedbackThreadPresentationProjection {
        let rowTitle = thread.title ?? thread.preview
        let navigationTitle = thread.title ?? "Feedback"
        let summaryText = thread.summary.flatMap { $0.isEmpty ? nil : $0 }
        let rowSecondaryText = summaryText ?? (thread.title != nil && !thread.preview.isEmpty ? thread.preview : nil)
        let statusLabel = thread.statusLabel.flatMap { $0.isEmpty ? nil : $0 }
        return FeedbackThreadPresentationProjection(
            navigationTitle: navigationTitle,
            rowTitle: rowTitle,
            rowSecondaryText: rowSecondaryText,
            detailSummary: summaryText,
            statusLabel: statusLabel
        )
    }

    private func send() async {
        let projection = composerProjection
        guard projection.canSend else { return }

        // Kernel is the sole writer: dispatch hl.feedback.post_reply
        // fire-and-forget. The reply streams back into
        // kernel.feedbackThread[rootEventId] (and bumps the list's activity),
        // which the views re-apply. Clear the draft optimistically.
        sendError = nil
        await detailStore.sendReply(body: projection.submitBody)
        draft = ""
    }

    private func messagePresentation(
        for row: FeedbackMessageRowProjection
    ) -> FeedbackMessagePresentationProjection {
        let profile = app.profileSnapshots[row.event.authorPubkey]
        let isFromMe = app.currentUser?.pubkey == row.event.authorPubkey
        let dn = profile?.displayName ?? ""
        let n = profile?.name ?? ""
        let displayName = !dn.isEmpty ? dn : !n.isEmpty ? n : String(row.event.authorPubkey.prefix(8))
        let displayInitial = displayName.first.map { String($0).uppercased() } ?? ""
        let pictureUrl = profile?.picture ?? ""
        return FeedbackMessagePresentationProjection(
            isFromMe: isFromMe,
            showHeader: row.showHeader,
            displayName: displayName,
            displayInitial: displayInitial,
            pictureUrl: pictureUrl
        )
    }
}

private struct FeedbackMessageBubble: View {
    let event: FeedbackEventRecord
    let projection: FeedbackMessagePresentationProjection

    var body: some View {
        HStack(alignment: .bottom, spacing: 6) {
            if projection.isFromMe {
                Spacer(minLength: 40)
            } else {
                avatarSlot
            }

            VStack(alignment: projection.isFromMe ? .trailing : .leading, spacing: 2) {
                if projection.showHeader {
                    Text(projection.displayName)
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 4)
                        .padding(.top, 8)
                }
                Text(markdownContent)
                    .font(.body)
                    .foregroundStyle(projection.isFromMe ? Color.white : Color.primary)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .background(
                        projection.isFromMe
                            ? Color.accentColor
                            : Color(.secondarySystemBackground),
                        in: .rect(cornerRadius: 14)
                    )
                Text(timeLabel(event.createdAt))
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 4)
            }

            if projection.isFromMe {
                avatarSlot
            } else {
                Spacer(minLength: 40)
            }
        }
        .padding(.horizontal, 12)
        .padding(.top, projection.showHeader ? 4 : 1)
    }

    @ViewBuilder
    private var avatarSlot: some View {
        if projection.showHeader {
            AuthorAvatar(
                pubkey: event.authorPubkey,
                pictureURL: projection.pictureUrl,
                displayInitial: projection.displayInitial,
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
