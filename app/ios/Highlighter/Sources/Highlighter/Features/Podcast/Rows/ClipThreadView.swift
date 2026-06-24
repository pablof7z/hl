import SwiftUI

/// Thread display + reply composer rendered inside an expanded `MemberClipRow`.
struct ClipThreadView: View {
    @Environment(HighlighterStore.self) private var app
    @Environment(HighlighterAppKernel.self) private var kernel

    let clipEventId: String

    @State private var replyText: String = ""
    @State private var localComments: [CommentRecord]? = nil

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Divider()
                .padding(.horizontal, 16)

            if localComments == nil {
                HStack {
                    Spacer()
                    ProgressView()
                    Spacer()
                }
                .padding(.vertical, 16)
            } else if let list = localComments, list.isEmpty {
                Text("No replies yet")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 12)
            } else if let list = localComments {
                VStack(alignment: .leading, spacing: 12) {
                    ForEach(list.reversed(), id: \.eventId) { comment in
                        CommentRowView(comment: comment)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
            }

            Divider()
                .padding(.horizontal, 16)

            HStack(spacing: 10) {
                TextField("Reply...", text: $replyText)
                    .font(.subheadline)
                    .tint(Color.highlighterAccent)

                Button("Send") {
                    send()
                }
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(canSubmit
                    ? Color.highlighterAccent
                    : Color.secondary)
                .disabled(!canSubmit)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
        }
        .task {
            kernel.openCommentThread(rootTagValue: clipEventId)
            applySnapshot()
        }
        .onChange(of: kernel.commentThreads[clipEventId]) { _, _ in
            applySnapshot()
        }
        .onDisappear {
            kernel.closeCommentThread(rootTagValue: clipEventId)
        }
    }

    private var canSubmit: Bool {
        !replyText.trimmingCharacters(in: .whitespaces).isEmpty
    }

    private func applySnapshot() {
        guard let snapshot = kernel.commentThreads[clipEventId] else { return }
        localComments = snapshot.records.map(CommentTreeBuilder.record(from:))
    }

    private func send() {
        let trimmed = replyText.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return }
        kernel.app.dispatch(.postComment(
            rootTagName: "e",
            rootTagValue: clipEventId,
            rootKind: 9802,
            parentEventId: nil,
            rootAuthorPubkey: nil,
            parentAuthorPubkey: nil,
            content: trimmed
        ))
        replyText = ""
    }
}

private struct CommentRowView: View {
    @Environment(HighlighterStore.self) private var app
    let comment: CommentRecord

    var body: some View {
        let author = authorDisplay

        HStack(alignment: .top, spacing: 10) {
            AuthorAvatar(
                pubkey: comment.pubkey,
                pictureURL: author.pictureUrl,
                displayInitial: author.displayInitial,
                size: 26
            )

            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 6) {
                    Text(author.displayName)
                        .font(.footnote.weight(.semibold))
                        .foregroundStyle(.primary)
                        .lineLimit(1)
                    if let t = relativeTime {
                        Text("·").foregroundStyle(.secondary)
                        Text(t)
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                    }
                    Spacer(minLength: 0)
                }
                Text(comment.body)
                    .font(.subheadline)
                    .foregroundStyle(.primary)
                    .multilineTextAlignment(.leading)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .task(id: comment.pubkey) {
            await app.requestProfile(pubkeyHex: comment.pubkey)
        }
    }

    private var authorDisplay: ProfileDisplayProjection {
        let profile = app.profileSnapshots[comment.pubkey]
        let name = (profile?.displayName ?? "").isEmpty
            ? ((profile?.name ?? "").isEmpty ? String(comment.pubkey.prefix(10)) : profile!.name)
            : profile!.displayName
        return ProfileDisplayProjection(
            displayName: name,
            displayInitial: name.first.map { String($0).uppercased() } ?? "?",
            pictureUrl: profile?.picture ?? ""
        )
    }

    private var relativeTime: String? {
        guard let s = comment.createdAt, s > 0 else { return nil }
        let date = Date(timeIntervalSince1970: TimeInterval(s))
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        formatter.dateTimeStyle = .numeric
        return formatter.localizedString(for: date, relativeTo: Date())
    }
}
