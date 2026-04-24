import SwiftUI

/// NIP-29 chat tab content for a room. Owns its own `ChatStore` so each
/// room's chat lifetime tracks the view.
///
/// Visual language: paper background, ink text, sans typography. No iMessage
/// bubbles — messages are rendered in the magazine-comment style of the
/// rest of Highlighter, with consecutive messages from the same author
/// collapsing the avatar/name header (Slack/iMessage groups).
struct ChatView: View {
    let groupId: String

    @Environment(HighlighterStore.self) private var app
    @State private var store = ChatStore()
    @State private var draft: String = ""
    @FocusState private var inputFocused: Bool

    var body: some View {
        VStack(spacing: 0) {
            messageList
            Rectangle()
                .fill(Color.highlighterRule)
                .frame(height: 1)
            composer
        }
        .background(Color.highlighterPaper.ignoresSafeArea())
        .task {
            await store.start(groupId: groupId, core: app.safeCore, bridge: app.eventBridge)
            await prefetchProfiles()
        }
        .onDisappear { store.stop() }
        .onChange(of: store.messages.count) { _, _ in
            Task { await prefetchProfiles() }
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
        if store.isLoading && store.messages.isEmpty {
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if store.messages.isEmpty {
            ContentUnavailableView(
                "No messages yet",
                systemImage: "bubble.left",
                description: Text("Be the first to say hello.")
            )
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(Array(store.messages.enumerated()), id: \.element.eventId) { index, message in
                            ChatMessageRow(
                                message: message,
                                profile: app.profileCache[message.authorPubkey],
                                showHeader: shouldShowHeader(at: index)
                            )
                            .id(message.eventId)
                        }
                    }
                    .padding(.vertical, 12)
                }
                .onChange(of: store.messages.count) { _, _ in
                    if let last = store.messages.last {
                        withAnimation(.easeOut(duration: 0.2)) {
                            proxy.scrollTo(last.eventId, anchor: .bottom)
                        }
                    }
                }
                .onAppear {
                    if let last = store.messages.last {
                        proxy.scrollTo(last.eventId, anchor: .bottom)
                    }
                }
            }
        }
    }

    /// Show the avatar+name header when this row's author differs from the
    /// previous row's, OR when more than 5 minutes elapsed since the
    /// previous message (a fresh "burst" of conversation).
    private func shouldShowHeader(at index: Int) -> Bool {
        guard index > 0 else { return true }
        let prev = store.messages[index - 1]
        let curr = store.messages[index]
        if prev.authorPubkey != curr.authorPubkey { return true }
        // 5 minutes between messages → re-show header so timestamps stay
        // legible even in long monologues.
        if curr.createdAt > prev.createdAt + 300 { return true }
        return false
    }

    // MARK: - Composer

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

            Button {
                Task { await send() }
            } label: {
                Image(systemName: "arrow.up.circle.fill")
                    .font(.system(size: 30))
                    .foregroundStyle(canSend ? Color.highlighterAccent : Color.highlighterInkMuted.opacity(0.5))
            }
            .disabled(!canSend)
            .accessibilityLabel("Send message")
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(Color.highlighterPaper)
    }

    private var canSend: Bool {
        !draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private func send() async {
        let text = draft
        draft = ""
        await store.send(text: text)
    }

    // MARK: - Profile prefetch

    /// Walk the unique authors in the message list and ask `HighlighterStore`
    /// to surface a kind:0 + a live subscription for each. The store dedupes
    /// internally, so this is cheap to call on every delta.
    private func prefetchProfiles() async {
        let pubkeys = Set(store.messages.map(\.authorPubkey))
        for pubkey in pubkeys {
            await app.requestProfile(pubkeyHex: pubkey)
        }
    }
}

private struct ChatMessageRow: View {
    let message: ChatMessageRecord
    let profile: ProfileMetadata?
    let showHeader: Bool

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            if showHeader {
                ProfileAvatar(profile: profile, pubkey: message.authorPubkey, size: 32)
            } else {
                Color.clear.frame(width: 32, height: 1)
            }
            VStack(alignment: .leading, spacing: 2) {
                if showHeader {
                    HStack(alignment: .firstTextBaseline, spacing: 6) {
                        Text(displayName)
                            .font(.subheadline.weight(.semibold))
                            .foregroundStyle(Color.highlighterInkStrong)
                        Text(timeLabel(message.createdAt))
                            .font(.caption2)
                            .foregroundStyle(Color.highlighterInkMuted)
                    }
                }
                Text(message.content)
                    .font(.body)
                    .foregroundStyle(Color.highlighterInkStrong)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .textSelection(.enabled)
            }
        }
        .padding(.horizontal, 16)
        .padding(.top, showHeader ? 10 : 2)
    }

    private var displayName: String {
        if let p = profile {
            if !p.displayName.isEmpty { return p.displayName }
            if !p.name.isEmpty { return p.name }
        }
        // Short-pubkey fallback so unknown authors are still distinguishable.
        let prefix = String(message.authorPubkey.prefix(8))
        return prefix.isEmpty ? "Anonymous" : prefix
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
            // "Mon 14:32"
            fmt.dateFormat = "EEE HH:mm"
        } else {
            fmt.dateStyle = .short
            fmt.timeStyle = .short
        }
        return fmt.string(from: date)
    }
}

private struct ProfileAvatar: View {
    let profile: ProfileMetadata?
    let pubkey: String
    let size: CGFloat

    var body: some View {
        Group {
            if let urlString = profile?.picture, let url = URL(string: urlString) {
                AsyncImage(url: url) { phase in
                    switch phase {
                    case .success(let image):
                        image.resizable().scaledToFill()
                    case .empty, .failure:
                        placeholder
                    @unknown default:
                        placeholder
                    }
                }
            } else {
                placeholder
            }
        }
        .frame(width: size, height: size)
        .clipShape(Circle())
        .overlay(
            Circle().stroke(Color.highlighterRule, lineWidth: 0.5)
        )
    }

    private var placeholder: some View {
        ZStack {
            Color.highlighterRule.opacity(0.5)
            Text(initial)
                .font(.system(size: size * 0.42, weight: .semibold, design: .rounded))
                .foregroundStyle(Color.highlighterInkMuted)
        }
    }

    private var initial: String {
        if let name = profile?.displayName.first ?? profile?.name.first {
            return String(name).uppercased()
        }
        return String(pubkey.prefix(1)).uppercased()
    }
}
