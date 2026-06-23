import SwiftUI

/// Root and pushed destination for NIP-22 thread navigation.
///
/// At root (`focused == nil`) renders the artifact's top-level comments.
/// When pushed (`focused != nil`) renders that comment's direct children
/// as its heading and lists its replies. Recursive drill-down is handled
/// by the local `focusedNode` state + `navigationDestination(item:)` so
/// every level lives in the enclosing NavigationStack — no nested stacks.
struct ThreadView: View {
    let focused: CommentNode?
    let artifactHeader: AnyView?
    let store: CommentsStore
    let scope: CommentScope
    let artifactAuthorPubkey: String?

    @Environment(\.dismiss) private var dismiss
    @State private var focusedNode: CommentNode? = nil

    var body: some View {
        let projection = threadProjection

        VStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    if let focused = projection.focused {
                        focusedHeader(focused, replyCountLabel: projection.replyCountLabel)
                            .padding(.bottom, 4)
                    } else if let artifactHeader {
                        artifactHeader
                            .padding(.bottom, 4)
                    }

                    if projection.children.isEmpty {
                        emptyState(label: projection.emptyStateLabel)
                    } else {
                        ForEach(projection.children) { child in
                            VStack(spacing: 0) {
                                CommentRow(
                                    node: child,
                                    depth: 0,
                                    isAuthorReply: false,
                                    onTap: { focusOn(child) },
                                    store: store
                                )
                                inlineReplyPreview(for: child)
                                Divider()
                                    .background(Color.highlighterRule.opacity(0.4))
                            }
                        }
                    }
                }
            }
            .scrollDismissesKeyboard(.interactively)

            CommentComposer(
                parentEventId: projection.focused?.record.eventId,
                placeholder: projection.composerPlaceholder,
                store: store
            )
        }
        .background(Color.highlighterPaper.ignoresSafeArea())
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .principal) {
                Text(projection.navTitle)
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
            }
        }
        .navigationDestination(item: $focusedNode) { node in
            ThreadView(
                focused: node,
                artifactHeader: nil,
                store: store,
                scope: scope,
                artifactAuthorPubkey: artifactAuthorPubkey
            )
        }
    }

    // MARK: - Projection

    private var threadProjection: CommentThreadViewProjection {
        let tree = store.tree
        let focusedNode: CommentNode? = focused.flatMap { f in
            findNode(in: tree, eventId: f.record.eventId) ?? f
        }
        let children: [CommentNode] = focusedNode?.children ?? tree
        let replyCount = focusedNode?.children.count ?? 0
        let totalCount = countNodes(tree)
        let navTitle: String = focusedNode != nil
            ? "Reply thread"
            : (totalCount == 0 ? "Comments" : (totalCount == 1 ? "1 comment" : "\(totalCount) comments"))
        let emptyStateLabel = focusedNode != nil ? "Be the first to reply." : "Start the conversation."
        let composerPlaceholder = focusedNode != nil ? "Reply…" : "Add to the conversation"
        let replyCountLabel: String = {
            switch replyCount {
            case 0: return "Be the first to reply"
            case 1: return "1 reply"
            default: return "\(replyCount) replies"
            }
        }()
        return CommentThreadViewProjection(
            focused: focusedNode,
            children: children,
            navTitle: navTitle,
            emptyStateLabel: emptyStateLabel,
            composerPlaceholder: composerPlaceholder,
            replyCountLabel: replyCountLabel
        )
    }

    private func findNode(in nodes: [CommentNode], eventId: String) -> CommentNode? {
        for node in nodes {
            if node.record.eventId == eventId { return node }
            if let hit = findNode(in: node.children, eventId: eventId) { return hit }
        }
        return nil
    }

    private func countNodes(_ nodes: [CommentNode]) -> Int {
        nodes.reduce(0) { $0 + 1 + countNodes($1.children) }
    }

    // MARK: - Inline reply preview

    @ViewBuilder
    private func inlineReplyPreview(for parent: CommentNode) -> some View {
        let chrome = nodeChrome(for: parent)
        if let mostRecent = chrome.mostRecentReply {
            CommentRow(
                node: mostRecent,
                depth: 1,
                isAuthorReply: chrome.isMostRecentAuthorReply,
                onTap: { focusOn(mostRecent) },
                store: store
            )
            .padding(.leading, 18)
            .padding(.trailing, 18)

            if chrome.hasMoreReplies {
                moreRepliesChip(parent: parent, label: chrome.moreRepliesLabel)
            }
        }
    }

    private func moreRepliesChip(parent: CommentNode, label: String) -> some View {
        Button {
            focusOn(parent)
        } label: {
            HStack(spacing: 6) {
                Spacer()
                    .frame(width: 36 + 18 + 12, alignment: .leading)
                Text(label)
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(Color.highlighterAccent)
                Image(systemName: "chevron.right")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(Color.highlighterAccent)
                Spacer()
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.vertical, 6)
        }
        .buttonStyle(.plain)
    }

    // MARK: - Focused-comment header

    @ViewBuilder
    private func focusedHeader(_ node: CommentNode, replyCountLabel: String) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            CommentRow(
                node: node,
                depth: 0,
                isAuthorReply: false,
                onTap: {},
                store: store
            )
            .allowsHitTesting(false)
            HStack(spacing: 6) {
                Image(systemName: "arrow.turn.down.right")
                    .font(.caption)
                    .foregroundStyle(Color.highlighterInkMuted)
                Text(replyCountLabel)
                    .font(.caption.weight(.medium))
                    .foregroundStyle(Color.highlighterInkMuted)
                    .textCase(.uppercase)
                    .tracking(0.6)
                Spacer()
            }
            .padding(.horizontal, 18)
            .padding(.bottom, 6)
            Rectangle()
                .fill(Color.highlighterRule.opacity(0.4))
                .frame(height: 0.5)
        }
    }

    // MARK: - Empty state

    private func emptyState(label: String) -> some View {
        VStack(spacing: 8) {
            Image(systemName: "bubble.left.and.bubble.right")
                .font(.system(size: 28, weight: .light))
                .foregroundStyle(Color.highlighterInkMuted)
            Text(label)
                .font(.subheadline)
                .foregroundStyle(Color.highlighterInkMuted)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 60)
    }

    // MARK: - Helpers

    private func nodeChrome(for parent: CommentNode) -> CommentNodeChromeProjection {
        let children = parent.children
        let replyCount = UInt32(children.count)
        let mostRecentReply = children.last
        let moreCount = children.count > 1 ? children.count - 1 : 0
        let moreRepliesLabel: String
        switch moreCount {
        case 0: moreRepliesLabel = ""
        case 1: moreRepliesLabel = "View 1 more reply"
        default: moreRepliesLabel = "View \(moreCount) more replies"
        }
        let authorPubkey = artifactAuthorPubkey.flatMap { $0.isEmpty ? nil : $0 }
        let isMostRecentAuthorReply: Bool = {
            guard let reply = mostRecentReply, let author = authorPubkey else { return false }
            return reply.record.pubkey == author
        }()
        return CommentNodeChromeProjection(
            replyCount: replyCount,
            showsReplyChevron: replyCount > 0,
            mostRecentReply: mostRecentReply,
            hasMoreReplies: moreCount > 0,
            moreRepliesLabel: moreRepliesLabel,
            isMostRecentAuthorReply: isMostRecentAuthorReply
        )
    }

    private func focusOn(_ node: CommentNode) {
        focusedNode = node
    }
}
