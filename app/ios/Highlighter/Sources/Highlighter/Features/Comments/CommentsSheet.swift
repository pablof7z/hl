import SwiftUI

/// The premium NIP-22 comments sheet. Hosts a NavigationStack so tapping
/// any comment row pushes a thread view focused on that comment — push
/// recurses arbitrarily, dismisses by drag-down regardless of depth.
///
/// Detents: 140pt peek (intermediate dismissal state), 52% half (default
/// landing), large (top-safe-area) for deep reading + active typing.
struct CommentsSheet: View {
    let artifact: ArtifactRef
    /// Pubkey of the artifact's own author (article author, podcaster, …)
    let artifactAuthorPubkey: String?
    /// Optional header card rendered at the top of the root list.
    let artifactHeader: AnyView?

    @Bindable var store: CommentsStore

    @Environment(\.dismiss) private var dismiss
    @State private var path = NavigationPath()

    var body: some View {
        NavigationStack(path: $path) {
            ThreadView(
                focused: nil,
                artifactHeader: artifactHeader,
                store: store,
                artifact: artifact,
                artifactAuthorPubkey: artifactAuthorPubkey,
                path: $path
            )
            .navigationDestination(for: String.self) { eventId in
                let node = locate(eventId: eventId, in: store.tree)
                    ?? CommentNode(
                        record: store.records.first(where: { $0.eventId == eventId })
                            ?? makeMissingPlaceholder(eventId: eventId),
                        children: []
                    )
                ThreadView(
                    focused: node,
                    artifactHeader: nil,
                    store: store,
                    artifact: artifact,
                    artifactAuthorPubkey: artifactAuthorPubkey,
                    path: $path
                )
            }
        }
        .presentationDetents([.height(140), .fraction(0.52), .large])
        .presentationDragIndicator(.visible)
        .presentationCornerRadius(28)
        .presentationContentInteraction(.scrolls)
        .presentationBackgroundInteraction(.disabled)
    }

    private func locate(eventId: String, in nodes: [CommentNode]) -> CommentNode? {
        for n in nodes {
            if n.record.eventId == eventId { return n }
            if let hit = locate(eventId: eventId, in: n.children) { return hit }
        }
        return nil
    }

    /// If we land on a thread whose record isn't in our cache yet (the
    /// destination was pushed before a refresh completed), surface a
    /// placeholder so navigation doesn't crash.
    private func makeMissingPlaceholder(eventId: String) -> CommentRecord {
        CommentRecord(
            eventId: eventId,
            pubkey: "",
            body: "",
            rootTagName: artifact.rootTagName,
            rootTagValue: artifact.rootTagValue,
            parentTagName: artifact.rootTagName.lowercased(),
            parentTagValue: artifact.rootTagValue,
            rootKind: "\(artifact.rootKind)",
            createdAt: nil
        )
    }
}
