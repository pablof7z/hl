import SwiftUI

/// Thread display + reply composer rendered inside an expanded `MemberClipRow`.
struct ClipThreadView: View {
    @Environment(HighlighterStore.self) private var app

    let clipEventId: String

    @State private var replyText: String = ""
    @State private var isSending: Bool = false
    @State private var sendError: String? = nil

    private var comments: [CommentRecord]? {
        app.podcastPlayer.comments[clipEventId]
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Divider()
                .padding(.horizontal, 16)

            if comments == nil {
                HStack {
                    Spacer()
                    ProgressView()
                    Spacer()
                }
                .padding(.vertical, 16)
            } else if let list = comments, list.isEmpty {
                Text("No replies yet")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 12)
            } else if let list = comments {
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

                if isSending {
                    ProgressView()
                        .scaleEffect(0.8)
                } else {
                    Button("Send") {
                        send()
                    }
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(composerProjection.canSubmit
                        ? Color.highlighterAccent
                        : Color.secondary)
                    .disabled(!composerProjection.canSubmit)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)

            if let error = sendError {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .padding(.horizontal, 16)
                    .padding(.bottom, 8)
            }
        }
    }

    private var composerProjection: CommentComposerProjection {
        app.safeCore.projectCommentComposer(
            input: CommentComposerProjectionInput(
                body: replyText,
                isPublishing: isSending
            )
        )
    }

    private func send() {
        let projection = composerProjection
        guard projection.canSubmit else { return }
        isSending = true
        sendError = nil
        let id = clipEventId
        Task {
            let scopeSnapshot = app.safeCore.getHighlightCommentScope(eventIdHex: id)
            guard scopeSnapshot.attach, let scope = scopeSnapshot.scope else {
                sendError = scopeSnapshot.errorMessage
                isSending = false
                return
            }
            let outcome = await app.safeCore.publishCommentForScopeSnapshot(
                scope: scope,
                content: projection.submitBody,
                limit: 200
            )
            let result = app.safeCore.projectCommentPublishResult(
                input: CommentPublishResultInput(error: outcome.error)
            )
            guard result.didPublish else {
                sendError = result.errorMessage
                isSending = false
                return
            }
            let applyProjection = app.safeCore.projectCommentInlineThreadSnapshotApply(
                input: CommentInlineThreadSnapshotApplyInput(
                    records: outcome.snapshot.records,
                    error: outcome.snapshot.error
                )
            )
            app.podcastPlayer.comments[id] = applyProjection.records
            sendError = applyProjection.errorMessage
            replyText = ""
            isSending = false
        }
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
        ProfileDisplayProjection.from(pubkey: comment.pubkey, profile: app.profileSnapshots[comment.pubkey])
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
