import SwiftUI

/// Chat-style view for a single feedback thread. Shows every kind:1
/// `e`-tagged to the root, regardless of author, with a composer pinned to
/// the bottom for replies.
struct FeedbackThreadDetailView: View {
    let thread: FeedbackThreadRecord
    let listStore: FeedbackStore

    @Environment(HighlighterStore.self) private var app
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
            let agentPubkey = await listStore.resolveAgentPubkey()
            await detailStore.start(
                rootEventId: thread.rootEventId,
                coordinate: FeedbackProject.coordinate,
                agentPubkey: agentPubkey,
                core: app.safeCore,
                bridge: app.eventBridge
            )
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
                    ForEach(Array(detailStore.events.enumerated()), id: \.element.eventId) { index, event in
                        FeedbackMessageBubble(
                            event: event,
                            projection: messagePresentation(for: event, at: index)
                        )
                        .id(event.eventId)
                        .task(id: event.authorPubkey) {
                            await app.requestProfile(pubkeyHex: event.authorPubkey)
                        }
                    }
                    if detailStore.isLoading && detailStore.events.isEmpty {
                        ProgressView().padding()
                    }
                }
                .padding(.vertical, 8)
            }
            .onChange(of: detailStore.events.count) { _, _ in
                if let last = detailStore.events.last {
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
        app.safeCore.projectFeedbackComposer(
            input: FeedbackComposerProjectionInput(
                body: draft,
                isPublishing: detailStore.isPublishing
            )
        )
    }

    private var threadPresentation: FeedbackThreadPresentationProjection {
        app.safeCore.projectFeedbackThreadPresentation(thread: thread)
    }

    private func send() async {
        let projection = composerProjection
        guard projection.canSend else { return }

        sendError = nil
        let outcome = await detailStore.sendReply(body: projection.submitBody)
        if outcome.error.isEmpty {
            draft = ""
            await listStore.refreshThreads()
        } else {
            sendError = outcome.error
        }
    }

    private func messagePresentation(
        for event: FeedbackEventRecord,
        at index: Int
    ) -> FeedbackMessagePresentationProjection {
        app.safeCore.projectFeedbackMessagePresentation(
            input: FeedbackMessagePresentationInput(
                event: event,
                previousEvent: index > 0 ? detailStore.events[index - 1] : nil,
                currentUserPubkey: app.currentUser?.pubkey,
                profile: app.profileSnapshots[event.authorPubkey]
            )
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
