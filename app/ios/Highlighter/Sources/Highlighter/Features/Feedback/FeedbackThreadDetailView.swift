import SwiftUI

/// Chat-style view for a single feedback thread. Shows every kind:1
/// `e`-tagged to the root, regardless of author, with a composer pinned to
/// the bottom for replies.
struct FeedbackThreadDetailView: View {
    let thread: FeedbackThreadRecord

    @Environment(HighlighterStore.self) private var app

    var body: some View {
        VStack(spacing: 0) {
            messageList
            Divider()
            composer
        }
        .navigationTitle(thread.title ?? "Feedback")
        .navigationBarTitleDisplayMode(.inline)
        .task(id: thread.rootEventId) {
            app.openFeedbackThread(rootEventId: thread.rootEventId)
        }
        .onDisappear { app.closeFeedbackThread() }
    }

    @ViewBuilder
    private var messageList: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 2) {
                    if let summary = thread.summary, !summary.isEmpty {
                        Text(summary)
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                            .padding(.horizontal, 12)
                            .padding(.top, 8)
                            .padding(.bottom, 6)
                    }
                    ForEach(Array(events.enumerated()), id: \.element.eventId) { index, event in
                        FeedbackMessageBubble(
                            event: event,
                            isFromMe: event.authorPubkey == app.currentUser?.pubkey,
                            showHeader: shouldShowHeader(at: index),
                            profile: app.profile(pubkeyHex: event.authorPubkey)
                        )
                        .id(event.eventId)
                        .task(id: event.authorPubkey) {
                            app.requestProfile(pubkeyHex: event.authorPubkey)
                        }
                    }
                    if app.feedback.isLoadingThread && events.isEmpty {
                        ProgressView().padding()
                    }
                    if let error = app.feedback.threadErrorMessage, events.isEmpty {
                        Text(error)
                            .font(.caption)
                            .foregroundStyle(.red)
                            .padding()
                    }
                }
                .padding(.vertical, 8)
            }
            .onChange(of: events.count) { _, _ in
                if let last = events.last {
                    withAnimation(.easeOut(duration: 0.2)) {
                        proxy.scrollTo(last.eventId, anchor: .bottom)
                    }
                }
            }
        }
    }

    @ViewBuilder
    private var composer: some View {
        VStack(spacing: 6) {
            if let sendError = app.feedback.publishErrorMessage {
                Text(sendError)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            HStack(alignment: .bottom, spacing: 8) {
                TextField("Reply…", text: replyDraft, axis: .vertical)
                    .textFieldStyle(.roundedBorder)
                    .lineLimit(1 ... 5)
                Button {
                    app.publishFeedbackReply()
                } label: {
                    Image(systemName: "paperplane.fill")
                        .font(.title3)
                        .frame(width: 36, height: 36)
                        .background(Color.accentColor.opacity(canSend ? 1 : 0.4), in: .circle)
                        .foregroundStyle(.white)
                }
                .accessibilityLabel("Send reply")
                .disabled(!canSend)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }

    private var canSend: Bool {
        !app.feedback.replyDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && !app.feedback.isPublishingReply
    }

    private var events: [FeedbackEventRecord] {
        app.feedback.selectedRootEventId == thread.rootEventId
            ? app.feedback.selectedEvents
            : []
    }

    private var replyDraft: Binding<String> {
        Binding(
            get: { app.feedback.replyDraft },
            set: { app.setFeedbackReplyDraft($0) }
        )
    }

    private func shouldShowHeader(at index: Int) -> Bool {
        guard index > 0 else { return true }
        let prev = events[index - 1]
        let curr = events[index]
        if prev.authorPubkey != curr.authorPubkey { return true }
        if curr.createdAt > prev.createdAt + 300 { return true }
        return false
    }
}

private struct FeedbackMessageBubble: View {
    let event: FeedbackEventRecord
    let isFromMe: Bool
    let showHeader: Bool
    let profile: ProfileMetadata?

    var body: some View {
        HStack(alignment: .bottom, spacing: 6) {
            if isFromMe {
                Spacer(minLength: 40)
            } else {
                avatarSlot
            }

            VStack(alignment: isFromMe ? .trailing : .leading, spacing: 2) {
                if showHeader {
                    Text(displayName)
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 4)
                        .padding(.top, 8)
                }
                Text(markdownContent)
                    .font(.body)
                    .foregroundStyle(isFromMe ? Color.white : Color.primary)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .background(
                        isFromMe
                            ? Color.accentColor
                            : Color(.secondarySystemBackground),
                        in: .rect(cornerRadius: 14)
                    )
                Text(timeLabel(event.createdAt))
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 4)
            }

            if isFromMe {
                avatarSlot
            } else {
                Spacer(minLength: 40)
            }
        }
        .padding(.horizontal, 12)
        .padding(.top, showHeader ? 4 : 1)
    }

    @ViewBuilder
    private var avatarSlot: some View {
        if showHeader {
            AuthorAvatar(
                pubkey: event.authorPubkey,
                pictureURL: profile?.picture ?? "",
                displayInitial: displayInitial,
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

    private var displayName: String {
        if let p = profile {
            if !p.displayName.isEmpty { return p.displayName }
            if !p.name.isEmpty { return p.name }
        }
        return String(event.authorPubkey.prefix(8))
    }

    private var displayInitial: String {
        displayName.first.map { String($0).uppercased() } ?? ""
    }

    private func timeLabel(_ ts: UInt64) -> String {
        let date = Date(timeIntervalSince1970: TimeInterval(ts))
        let formatter = DateFormatter()
        formatter.dateStyle = .none
        formatter.timeStyle = .short
        return formatter.string(from: date)
    }
}
