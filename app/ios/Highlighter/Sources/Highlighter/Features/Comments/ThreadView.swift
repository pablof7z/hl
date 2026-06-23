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

    @Environment(HighlighterStore.self) private var app
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
        let resolvedFocused = focused.map { node in
            findNode(in: store.tree, eventId: node.record.eventId) ?? node
        }
        let children = resolvedFocused?.children ?? store.tree
        let replyCount = resolvedFocused?.children.count ?? 0
        let totalCount = countAllNodes(store.tree)
        let navTitle: String = {
            guard resolvedFocused == nil else { return "Reply thread" }
            switch totalCount {
            case 0: return "Comments"
            case 1: return "1 comment"
            default: return "\(totalCount) comments"
            }
        }()
        let replyCountLabel: String = {
            switch replyCount {
            case 0: return "Be the first to reply"
            case 1: return "1 reply"
            default: return "\(replyCount) replies"
            }
        }()
        return CommentThreadViewProjection(
            focused: resolvedFocused,
            children: children,
            navTitle: navTitle,
            emptyStateLabel: resolvedFocused != nil ? "Be the first to reply." : "Start the conversation.",
            composerPlaceholder: resolvedFocused != nil ? "Reply…" : "Add to the conversation",
            replyCountLabel: replyCountLabel
        )
    }

    private func countAllNodes(_ nodes: [CommentNode]) -> Int {
        nodes.reduce(0) { $0 + 1 + countAllNodes($1.children) }
    }

    private func findNode(in nodes: [CommentNode], eventId: String) -> CommentNode? {
        for node in nodes {
            if node.record.eventId == eventId { return node }
            if let found = findNode(in: node.children, eventId: eventId) { return found }
        }
        return nil
    }

    // MARK: - Inline reply preview

    @ViewBuilder
    private func inlineReplyPreview(for parent: CommentNode) -> some View {
        let nodeChildren = parent.children
        let replyCount = nodeChildren.count
        let moreCount = replyCount > 1 ? replyCount - 1 : 0
        let mostRecentReply = nodeChildren.last
        let authorPubkey = artifactAuthorPubkey?.trimmingCharacters(in: .whitespaces) ?? ""
        let isMostRecentAuthorReply = !authorPubkey.isEmpty && (mostRecentReply.map { $0.record.pubkey == authorPubkey } ?? false)
        let moreLabel = moreCount == 0 ? "" : moreCount == 1 ? "View 1 more reply" : "View \(moreCount) more replies"
        let chrome = CommentNodeChromeProjection(replyCount: UInt32(replyCount), showsReplyChevron: replyCount > 0, mostRecentReply: mostRecentReply, hasMoreReplies: moreCount > 0, moreRepliesLabel: moreLabel, isMostRecentAuthorReply: isMostRecentAuthorReply)
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

    private func focusOn(_ node: CommentNode) {
        focusedNode = node
    }
}
