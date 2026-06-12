import SwiftUI

/// Attaches NIP-22 comments to any reader by injecting a top-bar toolbar
/// button (bubble icon + count) that pushes a CommentsView onto the
/// enclosing NavigationStack. Opens the Rust comments slice so the
/// count is live before the user ever taps.
struct CommentsAttachment: ViewModifier {
    let artifact: ArtifactRef
    let artifactAuthorPubkey: String?
    let artifactHeader: AnyView?

    @Environment(HighlighterStore.self) private var app
    @State private var showComments = false

    func body(content: Content) -> some View {
        content
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button { showComments = true } label: {
                        commentsLabel
                    }
                    .accessibilityLabel(
                        commentCount == 0
                            ? "Start the thread"
                            : "\(commentCount) comments"
                    )
                }
            }
            .navigationDestination(isPresented: $showComments) {
                CommentsView(
                    artifact: artifact,
                    artifactAuthorPubkey: artifactAuthorPubkey,
                    artifactHeader: artifactHeader
                )
            }
            .task(id: artifact) {
                app.openComments(artifact: artifact)
            }
    }

    private var commentsLabel: some View {
        HStack(spacing: 4) {
            Image(systemName: "bubble.left")
                .font(.system(size: 15, weight: .medium))
            if commentCount > 0 {
                Text("\(commentCount)")
                    .font(.system(size: 13, weight: .semibold, design: .rounded))
                    .monospacedDigit()
            }
        }
    }

    private var commentCount: Int {
        guard app.comments.rootTagName == artifact.rootTagName,
              app.comments.rootTagValue == artifact.rootTagValue
        else { return 0 }
        return Int(app.comments.recordCount)
    }
}

extension View {
    func commentsAttachment(
        artifact: ArtifactRef,
        artifactAuthorPubkey: String? = nil,
        artifactHeader: AnyView? = nil
    ) -> some View {
        modifier(CommentsAttachment(
            artifact: artifact,
            artifactAuthorPubkey: artifactAuthorPubkey,
            artifactHeader: artifactHeader
        ))
    }
}
