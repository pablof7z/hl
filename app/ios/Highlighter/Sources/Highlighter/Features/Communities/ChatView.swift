import SwiftUI

struct ChatView: View {
    let groupId: String

    @Environment(HighlighterStore.self) private var app
    @Environment(HighlighterAppKernel.self) private var kernel
    @State private var store = ChatStore()
    @State private var draft: String = ""
    @FocusState private var inputFocused: Bool
    @State private var replyTo: ChatMessageRecord? = nil
    @State private var isAtBottom: Bool = true
    @State private var pendingNewCount: Int = 0
    /// Incrementing this triggers a scroll-to-bottom inside the ScrollViewReader.
    @State private var scrollRevision: Int = 0
    /// eventId of the top-most visible message before a loadMore — used to
    /// restore scroll position after older messages are prepended.
    @State private var loadMoreAnchorId: String?

    var body: some View {
        VStack(spacing: 0) {
            ZStack(alignment: .bottom) {
                messageList
                if pendingNewCount > 0 {
                    newMessagesPill
                }
            }
            Rectangle()
                .fill(Color.highlighterRule)
                .frame(height: 1)
            composerArea
        }
        .background(Color.highlighterPaper.ignoresSafeArea())
        .task {
            let hostRelayUrl = kernel.roomHomeSnapshots[groupId]?.hostRelayUrl ?? ""
            await store.start(groupId: groupId, hostRelayUrl: hostRelayUrl, kernel: kernel)
        }
        .onDisappear { store.stop() }
        .onChange(of: kernel.roomChatSnapshots[groupId]) { _, _ in
            store.applyKernelSnapshot()
        }
        .onChange(of: store.activityRevision) { _, _ in
            let added = max(1, store.activityDelta)
            if isAtBottom {
                scrollRevision += 1
            } else {
                withAnimation(.spring(response: 0.35, dampingFraction: 0.8)) {
                    pendingNewCount += added
                }
            }
        }
        .alert("Couldn't send", isPresented: Binding(
            get: { store.sendError != nil },
            set: { if !$0 { store.sendError = nil } }
        )) {
            Button("OK", role: .cancel) { store.sendError = nil }
        } message: {
            Text(store.sendError ?? "")
        }
    }

    // MARK: - Message list

    @ViewBuilder
    private var messageList: some View {
        if store.isLoading && store.rows.isEmpty {
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if store.rows.isEmpty {
            emptyState
        } else {
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        // Load-more trigger — invisible when idle, spinner when fetching.
                        if store.hasMore || store.isLoadingMore {
                            Group {
                                if store.isLoadingMore {
                                    ProgressView()
                                        .frame(maxWidth: .infinity)
                                        .padding(.vertical, 12)
                                        .id("load-more-spinner")
                                } else {
                                    Color.clear
                                        .frame(height: 1)
                                        .id("load-more-trigger")
                                        .onAppear {
                                            loadMoreAnchorId = store.rows.first?.message.eventId
                                            Task { await store.loadMore() }
                                        }
                                }
                            }
                        }

                        ForEach(Array(store.rows.enumerated()), id: \.element.message.eventId) { index, row in
                            let message = row.message

                            ChatMessageRowView(
                                message: message,
                                authorDisplay: profileDisplay(for: message.authorPubkey),
                                showHeader: row.showHeader,
                                replyToMessage: row.replyToMessage,
                                replyToAuthorDisplay: row.replyToMessage.map { profileDisplay(for: $0.authorPubkey) },
                                onReply: { replyTo = message; inputFocused = true }
                            )
                            .id(message.eventId)
                            .task(id: message.authorPubkey) {
                                await app.requestProfile(pubkeyHex: message.authorPubkey)
                            }
                            .onAppear {
                                if index == store.rows.count - 1 {
                                    isAtBottom = true
                                    pendingNewCount = 0
                                }
                            }
                            .onDisappear {
                                if index == store.rows.count - 1 {
                                    isAtBottom = false
                                }
                            }
                        }
                    }
                    .padding(.vertical, 12)
                }
                .onAppear {
                    if let last = store.rows.last?.message {
                        proxy.scrollTo(last.eventId, anchor: .bottom)
                    }
                }
                .onChange(of: scrollRevision) { _, _ in
                    guard let last = store.rows.last?.message else { return }
                    withAnimation(.easeOut(duration: 0.2)) {
                        proxy.scrollTo(last.eventId, anchor: .bottom)
                    }
                }
                .onChange(of: store.isLoadingMore) { wasLoading, isLoading in
                    // After prepending older messages, snap back to where the
                    // user was so the view doesn't jump to the new top.
                    guard wasLoading, !isLoading, let anchorId = loadMoreAnchorId else { return }
                    proxy.scrollTo(anchorId, anchor: .top)
                    loadMoreAnchorId = nil
                }
            }
        }
    }

    private var emptyState: some View {
        VStack(spacing: 12) {
            Image(systemName: "bubble.left.and.bubble.right")
                .font(.system(size: 38))
                .foregroundStyle(Color.highlighterInkMuted.opacity(0.45))
            Text("No messages yet")
                .font(.headline)
                .foregroundStyle(Color.highlighterInkStrong)
            Text("Be the first to say something.")
                .font(.subheadline)
                .foregroundStyle(Color.highlighterInkMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var newMessagesPill: some View {
        Button {
            withAnimation(.spring(response: 0.3)) {
                pendingNewCount = 0
                isAtBottom = true
            }
            scrollRevision += 1
        } label: {
            HStack(spacing: 5) {
                Image(systemName: "arrow.down")
                    .font(.caption.weight(.semibold))
                Text(pendingNewCount == 1 ? "1 new message" : "\(pendingNewCount) new messages")
                    .font(.caption.weight(.semibold))
            }
            .foregroundStyle(Color.highlighterPaper)
            .padding(.horizontal, 14)
            .padding(.vertical, 7)
            .background(Capsule().fill(Color.highlighterAccent))
        }
        .padding(.bottom, 12)
        .transition(.scale.combined(with: .opacity))
    }

    // MARK: - Composer area

    private var composerArea: some View {
        VStack(spacing: 0) {
            if let reply = replyTo {
                replyBar(reply)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
            composer
        }
        .background(Color.highlighterPaper)
        .animation(.spring(response: 0.28, dampingFraction: 0.8), value: replyTo?.eventId)
    }

    private func replyBar(_ message: ChatMessageRecord) -> some View {
        HStack(alignment: .top, spacing: 10) {
            Rectangle()
                .fill(Color.highlighterAccent)
                .frame(width: 2)
                .clipShape(Capsule())
            VStack(alignment: .leading, spacing: 2) {
                Text(profileDisplay(for: message.authorPubkey).displayName)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(Color.highlighterAccent)
                Text(message.content)
                    .font(.caption)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(2)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            Button {
                replyTo = nil
            } label: {
                Image(systemName: "xmark")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkMuted)
                    .padding(4)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(Color.highlighterRule.opacity(0.25))
    }

    @ViewBuilder
    private var composer: some View {
        HStack(alignment: .bottom, spacing: 8) {
            TextField("Message", text: $draft, axis: .vertical)
                .textFieldStyle(.plain)
                .font(.body)
                .lineLimit(1...6)
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .background(
                    RoundedRectangle(cornerRadius: 18, style: .continuous)
                        .fill(Color.highlighterRule.opacity(0.35))
                )
                .focused($inputFocused)

            if composerProjection.canSend {
                Button {
                    Task { await send() }
                } label: {
                    Image(systemName: "arrow.up.circle.fill")
                        .font(.system(size: 30))
                        .foregroundStyle(Color.highlighterAccent)
                }
                .accessibilityLabel("Send message")
                .transition(.scale.combined(with: .opacity))
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .animation(
            .spring(response: 0.25, dampingFraction: 0.7),
            value: composerProjection.canSend
        )
    }

    // MARK: - Helpers

    private var composerProjection: ChatComposerProjection {
        let submitBody = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        return ChatComposerProjection(submitBody: submitBody, canSend: !submitBody.isEmpty)
    }

    private func send() async {
        let projection = composerProjection
        guard projection.canSend else { return }

        let reply = replyTo
        draft = ""
        replyTo = nil
        await store.send(text: projection.submitBody, replyTo: reply)
        scrollRevision += 1
    }

    private func profileDisplay(for pubkey: String) -> ProfileDisplayProjection {
        let profile = app.profileSnapshots[pubkey]
        let name: String
        if let p = profile {
            let candidate = p.displayName.isEmpty ? p.name : p.displayName
            name = candidate.isEmpty ? String(pubkey.prefix(8)) : candidate
        } else {
            name = String(pubkey.prefix(8))
        }
        let initial = String(name.prefix(1))
        let pictureUrl = profile.map(\.picture) ?? ""
        return ProfileDisplayProjection(displayName: name, displayInitial: initial, pictureUrl: pictureUrl)
    }


}

// MARK: - ChatMessageRowView

private struct ChatMessageRowView: View {
    let message: ChatMessageRecord
    let authorDisplay: ProfileDisplayProjection
    let showHeader: Bool
    let replyToMessage: ChatMessageRecord?
    let replyToAuthorDisplay: ProfileDisplayProjection?
    let onReply: () -> Void

    @State private var swipeOffset: CGFloat = 0
    @State private var swipeTriggered: Bool = false

    var body: some View {
        ZStack(alignment: .leading) {
            Image(systemName: "arrowshape.turn.up.left.fill")
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(Color.highlighterAccent)
                .opacity(Double(min(swipeOffset, 40)) / 40.0)
                .offset(x: max(-4, swipeOffset - 28))

            rowContent
                .offset(x: swipeOffset)
        }
        .contentShape(Rectangle())
        .sensoryFeedback(.impact(flexibility: .soft), trigger: swipeTriggered)
        .gesture(
            DragGesture(minimumDistance: 15, coordinateSpace: .local)
                .onChanged { value in
                    let dx = value.translation.width
                    let dy = value.translation.height
                    guard dx > 0, dx > abs(dy) else { return }
                    swipeOffset = min(dx * 0.55, 60)
                    if swipeOffset >= 40 && !swipeTriggered {
                        swipeTriggered = true
                        onReply()
                    }
                }
                .onEnded { _ in
                    withAnimation(.spring(response: 0.3, dampingFraction: 0.7)) {
                        swipeOffset = 0
                    }
                    swipeTriggered = false
                }
        )
        .contextMenu {
            Button {
                onReply()
            } label: {
                Label("Reply", systemImage: "arrowshape.turn.up.left")
            }
            Button {
                UIPasteboard.general.string = message.content
            } label: {
                Label("Copy", systemImage: "doc.on.doc")
            }
        }
    }

    private var rowContent: some View {
        HStack(alignment: .top, spacing: 10) {
            if showHeader {
                ProfileAvatar(display: authorDisplay, size: 28)
            } else {
                Color.clear.frame(width: 28, height: 1)
            }
            VStack(alignment: .leading, spacing: 3) {
                if showHeader {
                    HStack(alignment: .firstTextBaseline, spacing: 6) {
                        Text(authorDisplay.displayName)
                            .font(.subheadline.weight(.semibold))
                            .foregroundStyle(Color.highlighterInkStrong)
                        Text(timeLabel(message.createdAt))
                            .font(.caption2)
                            .foregroundStyle(Color.highlighterInkMuted)
                    }
                }
                if let replyMsg = replyToMessage {
                    replyChip(replyMsg)
                }
                NostrRichText(content: message.content, font: .body)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .textSelection(.enabled)
            }
        }
        .padding(.horizontal, 16)
        .padding(.top, showHeader ? 10 : 2)
    }

    private func replyChip(_ quoted: ChatMessageRecord) -> some View {
        HStack(spacing: 6) {
            Rectangle()
                .fill(Color.highlighterAccent.opacity(0.7))
                .frame(width: 2)
                .clipShape(Capsule())
            VStack(alignment: .leading, spacing: 1) {
                Text(replyToAuthorDisplay?.displayName ?? "")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(Color.highlighterAccent)
                Text(quoted.content)
                    .font(.caption)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(2)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .padding(.vertical, 5)
        .padding(.trailing, 8)
        .background(
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .fill(Color.highlighterAccent.opacity(0.06))
        )
    }

    private func timeLabel(_ ts: UInt64) -> String {
        let date = Date(timeIntervalSince1970: TimeInterval(ts))
        let now = Date()
        let cal = Calendar.current
        let fmt = DateFormatter()
        if cal.isDateInToday(date) {
            fmt.dateStyle = .none
            fmt.timeStyle = .short
        } else if cal.isDate(date, equalTo: now, toGranularity: .weekOfYear) {
            fmt.dateFormat = "EEE HH:mm"
        } else {
            fmt.dateStyle = .short
            fmt.timeStyle = .short
        }
        return fmt.string(from: date)
    }
}

// MARK: - ProfileAvatar

private struct ProfileAvatar: View {
    let display: ProfileDisplayProjection
    let size: CGFloat

    var body: some View {
        Group {
            if !display.pictureUrl.isEmpty, let url = URL(string: display.pictureUrl) {
                AsyncImage(url: url) { phase in
                    switch phase {
                    case .success(let image):
                        image.resizable().scaledToFill()
                    default:
                        placeholder
                    }
                }
            } else {
                placeholder
            }
        }
        .frame(width: size, height: size)
        .clipShape(Circle())
        .overlay(Circle().stroke(Color.highlighterRule, lineWidth: 0.5))
    }

    private var placeholder: some View {
        ZStack {
            Color.highlighterRule.opacity(0.5)
            Text(display.displayInitial)
                .font(.system(size: size * 0.42, weight: .semibold, design: .rounded))
                .foregroundStyle(Color.highlighterInkMuted)
        }
    }
}
