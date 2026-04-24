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
        .navigationTitle(thread.title ?? "Feedback")
        .navigationBarTitleDisplayMode(.inline)
        .task {
            await detailStore.start(
                rootEventId: thread.rootEventId,
                coordinate: FeedbackProject.coordinate,
                agentPubkey: listStore.cachedAgentPubkey,
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
                LazyVStack(alignment: .leading, spacing: 10) {
                    if let summary = thread.summary, !summary.isEmpty {
                        Text(summary)
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                            .padding(.horizontal, 12)
                            .padding(.top, 8)
                    }
                    ForEach(detailStore.events, id: \.eventId) { event in
                        FeedbackMessageBubble(
                            event: event,
                            isFromMe: event.authorPubkey == app.currentUser?.pubkey
                        )
                        .id(event.eventId)
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
                        .background(Color.accentColor.opacity(canSend ? 1 : 0.4), in: .circle)
                        .foregroundStyle(.white)
                }
                .disabled(!canSend)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }

    private var canSend: Bool {
        !draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !detailStore.isPublishing
    }

    private func send() async {
        sendError = nil
        do {
            _ = try await detailStore.sendReply(body: draft)
            draft = ""
            await listStore.refreshThreads()
        } catch {
            sendError = (error as? LocalizedError)?.errorDescription ?? "\(error)"
        }
    }
}

private struct FeedbackMessageBubble: View {
    let event: FeedbackEventRecord
    let isFromMe: Bool

    var body: some View {
        HStack {
            if isFromMe { Spacer(minLength: 40) }
            VStack(alignment: isFromMe ? .trailing : .leading, spacing: 2) {
                Text(event.content)
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
            }
            if !isFromMe { Spacer(minLength: 40) }
        }
        .padding(.horizontal, 12)
    }

    private func timeLabel(_ ts: UInt64) -> String {
        let date = Date(timeIntervalSince1970: TimeInterval(ts))
        let formatter = DateFormatter()
        formatter.dateStyle = .none
        formatter.timeStyle = .short
        return formatter.string(from: date)
    }
}
