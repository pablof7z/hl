import SwiftUI

/// Whisper-quiet cell. The whole row is a tap target; the parent owns
/// the push semantics (`onTap`). Long-press surfaces the action menu
/// (Like, Bookmark, Copy, …) via the system contextMenu.
///
/// Renders top-level (depth 0) at full size; depth 1 is rendered with a
/// smaller avatar and an indented thread line. Past depth 1, the parent
/// thread view delegates to a pushed thread instead of nesting visually.
struct CommentRow: View {
    let node: CommentNode
    /// 0 = top-level, 1 = inline reply preview. The row itself never
    /// renders deeper indents — recursion happens via thread push.
    let depth: Int
    /// Tints the thread line gold when this reply is by the artifact's
    /// own author (article author, podcaster, …).
    let isAuthorReply: Bool
    let onTap: () -> Void

    let store: CommentsStore

    @Environment(HighlighterStore.self) private var app
    @State private var showProfile = false

    var body: some View {
        Button(action: onTap) {
            let author = authorDisplay

            HStack(alignment: .top, spacing: 0) {
                if depth > 0 {
                    threadRail
                        .padding(.trailing, 10)
                }

                HStack(alignment: .top, spacing: 12) {
                    AuthorAvatar(
                        pubkey: node.record.pubkey,
                        pictureURL: author.pictureUrl,
                        displayInitial: author.displayInitial,
                        size: depth == 0 ? 40 : 30,
                        ringWidth: 1.5
                    )

                    VStack(alignment: .leading, spacing: 6) {
                        headerLine
                        NostrRichText(
                            content: node.record.body,
                            font: depth == 0 ? .body : .subheadline,
                            ink: Color.highlighterInkStrong
                        )
                        .multilineTextAlignment(.leading)
                        .fixedSize(horizontal: false, vertical: true)
                        footer
                    }
                    Spacer(minLength: 0)
                }
            }
            .padding(.horizontal, depth == 0 ? 18 : 0)
            .padding(.vertical, 10)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .contextMenu {
            actionMenu
        }
        .navigationDestination(isPresented: $showProfile) {
            ProfileView(pubkey: node.record.pubkey)
        }
        .task(id: node.record.pubkey) {
            await app.requestProfile(pubkeyHex: node.record.pubkey)
        }
    }

    // MARK: - Header line (name · time · trailing reply chevron)

    @ViewBuilder
    private var headerLine: some View {
        let author = authorDisplay
        let chrome = nodeChrome

        HStack(spacing: 6) {
            Text(author.displayName)
                .font(.system(size: depth == 0 ? 15 : 13, weight: .semibold))
                .foregroundStyle(Color.highlighterInkStrong)
                .lineLimit(1)
            if let rel = relativeTime {
                Text("·").foregroundStyle(Color.highlighterInkMuted)
                Text(rel)
                    .font(.system(size: depth == 0 ? 13 : 12))
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(1)
            }
            Spacer(minLength: 0)
            if chrome.showsReplyChevron {
                replyChevron(count: Int(chrome.replyCount))
            }
        }
    }

    private func replyChevron(count: Int) -> some View {
        HStack(spacing: 2) {
            Text("\(count)")
                .font(.system(size: 12, weight: .medium, design: .rounded))
                .foregroundStyle(Color.highlighterInkMuted)
                .monospacedDigit()
            Image(systemName: "chevron.right")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(Color.highlighterInkMuted)
        }
    }

    // MARK: - Footer (heart + count)

    @ViewBuilder
    private var footer: some View {
        let chrome = actionChrome
        if chrome.showsFooter {
            HStack(spacing: 6) {
                Image(systemName: chrome.footerSystemImage)
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(
                        chrome.footerIsAccented ? Color.highlighterAccent : Color.highlighterInkMuted
                    )
                if chrome.showsFooterCount {
                    Text(chrome.footerCountLabel)
                        .font(.system(size: 12, weight: .medium, design: .rounded))
                        .foregroundStyle(Color.highlighterInkMuted)
                        .monospacedDigit()
                }
                Spacer(minLength: 0)
            }
            .padding(.top, 2)
            .opacity(chrome.footerIsAccented ? 1.0 : 0.65)
        }
    }

    // MARK: - Thread rail (inline reply only)

    private var threadRail: some View {
        Rectangle()
            .fill(
                isAuthorReply
                    ? Color.highlighterAccent
                    : Color.highlighterAccent.opacity(0.30)
            )
            .frame(width: 2)
            .frame(maxHeight: .infinity)
            .padding(.leading, 36)
    }

    // MARK: - Long-press menu

    @ViewBuilder
    private var actionMenu: some View {
        let chrome = actionChrome
        Button {
            showProfile = true
        } label: {
            Label("View profile", systemImage: "person.crop.circle")
        }
        Button {
            Task { await store.toggleLike(node.record) }
        } label: {
            Label(
                chrome.likeTitle,
                systemImage: chrome.likeSystemImage
            )
        }
        Button {
            Task { await store.toggleBookmark(node.record) }
        } label: {
            Label(
                chrome.bookmarkTitle,
                systemImage: chrome.bookmarkSystemImage
            )
        }
        Button {
            UIPasteboard.general.string = node.record.body
        } label: {
            Label("Copy text", systemImage: "doc.on.doc")
        }
    }

    // MARK: - Helpers

    private var actionChrome: CommentActionChromeProjection {
        let isLiked = store.isLiked(node.record.eventId)
        let isBookmarked = store.isBookmarked(node.record.eventId)
        let likeCount = UInt32(store.likeCount(node.record.eventId))
        return CommentActionChromeProjection(
            showsFooter: isLiked || likeCount > 0,
            footerSystemImage: isLiked ? "heart.fill" : "heart",
            footerIsAccented: isLiked,
            showsFooterCount: likeCount > 0,
            footerCountLabel: likeCount > 0 ? "\(likeCount)" : "",
            likeTitle: isLiked ? "Unlike" : "Like",
            likeSystemImage: isLiked ? "heart.slash" : "heart",
            bookmarkTitle: isBookmarked ? "Remove bookmark" : "Bookmark",
            bookmarkSystemImage: isBookmarked ? "bookmark.slash" : "bookmark"
        )
    }

    private var nodeChrome: CommentNodeChromeProjection {
        app.safeCore.projectCommentNodeChrome(
            input: CommentNodeChromeProjectionInput(
                node: node,
                artifactAuthorPubkey: nil
            )
        )
    }

    private var authorDisplay: ProfileDisplayProjection {
        {
            let profile = app.profileSnapshots[node.record.pubkey]
            let name = (profile?.displayName ?? "").isEmpty
                ? ((profile?.name ?? "").isEmpty ? String(node.record.pubkey.prefix(10)) : profile!.name)
                : profile!.displayName
            return ProfileDisplayProjection(
                displayName: name,
                displayInitial: name.first.map { String($0).uppercased() } ?? "?",
                pictureUrl: profile?.picture ?? ""
            )
        }()
    }

    private var relativeTime: String? {
        guard let s = node.record.createdAt, s > 0 else { return nil }
        let date = Date(timeIntervalSince1970: TimeInterval(s))
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        formatter.dateTimeStyle = .numeric
        return formatter.localizedString(for: date, relativeTo: Date())
    }
}
