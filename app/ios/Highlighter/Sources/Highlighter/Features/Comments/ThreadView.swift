import SwiftUI

/// Destination view for both the sheet root (no `focused` node — list is
/// the artifact's top-level comments) and any pushed thread (`focused`
/// node — list is its direct children). Includes a header (artifact
/// preview at root, the focused comment as subject when pushed) and a
/// pinned-bottom composer that always replies to the current subject.
struct ThreadView: View {
    /// `nil` at root view; the focused comment when pushed.
    let focused: CommentNode?
    /// At root we display the artifact context header.
    let artifactHeader: AnyView?
    let store: CommentsStore
    let artifact: ArtifactRef
    /// Pubkey of the artifact's own author (article author, podcaster, …)
    /// — used to tint the inline-reply thread line gold when they engage.
    let artifactAuthorPubkey: String?
    /// Bound to the parent sheet's `NavigationPath` so row taps push.
    @Binding var path: NavigationPath

    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    if let focused {
                        focusedHeader(focused)
                            .padding(.bottom, 4)
                    } else if let artifactHeader {
                        artifactHeader
                            .padding(.bottom, 4)
                    }

                    if children.isEmpty {
                        emptyState
                    } else {
                        ForEach(children) { child in
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
                parentEventId: focused?.record.eventId,
                placeholder: composerPlaceholder,
                store: store
            )
        }
        .background(Color.highlighterPaper.ignoresSafeArea())
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .principal) {
                Text(navTitle)
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
            }
        }
    }

    // MARK: - Children resolution

    /// Cells to render at depth 0 of *this* view: top-level when at root,
    /// the focused comment's direct children when pushed.
    private var children: [CommentNode] {
        if let focused {
            return locate(eventId: focused.record.eventId)?.children ?? focused.children
        }
        return store.tree
    }

    /// Re-walk the tree to find the current node for a given id (so the
    /// pushed view always sees the freshest children after a publish).
    private func locate(eventId: String) -> CommentNode? {
        Self.locate(eventId: eventId, in: store.tree)
    }

    private static func locate(eventId: String, in nodes: [CommentNode]) -> CommentNode? {
        for n in nodes {
            if n.record.eventId == eventId { return n }
            if let hit = locate(eventId: eventId, in: n.children) { return hit }
        }
        return nil
    }

    // MARK: - Inline reply preview (depth-1 most-recent reply)

    @ViewBuilder
    private func inlineReplyPreview(for parent: CommentNode) -> some View {
        if let mostRecent = parent.mostRecentReply {
            let isAuthorReply = (artifactAuthorPubkey != nil)
                && (mostRecent.record.pubkey == artifactAuthorPubkey)
            CommentRow(
                node: mostRecent,
                depth: 1,
                isAuthorReply: isAuthorReply,
                onTap: { focusOn(mostRecent) },
                store: store
            )
            .padding(.leading, 18)
            .padding(.trailing, 18)

            if parent.children.count > 1 {
                moreRepliesChip(parent: parent)
            }
        }
    }

    private func moreRepliesChip(parent: CommentNode) -> some View {
        Button {
            focusOn(parent)
        } label: {
            HStack(spacing: 6) {
                Spacer()
                    .frame(width: 36 + 18 + 12, alignment: .leading)
                Text("View \(parent.children.count - 1) more \(parent.children.count - 1 == 1 ? "reply" : "replies")")
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

    // MARK: - Focused-comment header (when pushed)

    @ViewBuilder
    private func focusedHeader(_ node: CommentNode) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            CommentRow(
                node: node,
                depth: 0,
                isAuthorReply: false,
                onTap: {}, // already focused; tap is a no-op
                store: store
            )
            .allowsHitTesting(false)
            HStack(spacing: 6) {
                Image(systemName: "arrow.turn.down.right")
                    .font(.caption)
                    .foregroundStyle(Color.highlighterInkMuted)
                Text(replyCountLabel(for: node))
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

    private func replyCountLabel(for node: CommentNode) -> String {
        let count = (locate(eventId: node.record.eventId)?.children.count) ?? node.children.count
        if count == 0 { return "Be the first to reply" }
        if count == 1 { return "1 reply" }
        return "\(count) replies"
    }

    // MARK: - Empty state

    private var emptyState: some View {
        VStack(spacing: 8) {
            Image(systemName: "bubble.left.and.bubble.right")
                .font(.system(size: 28, weight: .light))
                .foregroundStyle(Color.highlighterInkMuted)
            Text(emptyStateLabel)
                .font(.subheadline)
                .foregroundStyle(Color.highlighterInkMuted)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 60)
    }

    private var emptyStateLabel: String {
        focused == nil ? "Start the conversation." : "Be the first to reply."
    }

    // MARK: - Helpers

    private var navTitle: String {
        if focused != nil { return "Reply thread" }
        let count = store.totalCount
        if count == 0 { return "Comments" }
        if count == 1 { return "1 comment" }
        return "\(count) comments"
    }

    private var composerPlaceholder: String {
        if focused == nil { return "Add to the conversation" }
        return "Reply…"
    }

    private func focusOn(_ node: CommentNode) {
        path.append(node.record.eventId)
    }
}
