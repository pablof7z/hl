import SwiftUI

/// Root comments screen pushed onto the enclosing NavigationStack.
/// No inner NavigationStack — thread drill-down is handled by
/// ThreadView's own `.navigationDestination(item:)`.
struct CommentsView: View {
    let artifact: ArtifactRef
    let artifactAuthorPubkey: String?
    let artifactHeader: AnyView?

    @Environment(HighlighterStore.self) private var app

    var body: some View {
        ThreadView(
            focused: nil,
            artifactHeader: artifactHeader,
            artifactAuthorPubkey: artifactAuthorPubkey
        )
        .task(id: artifact) {
            app.openComments(artifact: artifact)
        }
    }
}
