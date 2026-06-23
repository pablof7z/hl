import SwiftUI

/// Attaches NIP-22 comments to any reader by injecting a top-bar toolbar
/// button (bubble icon + count) that pushes a CommentsView onto the
/// enclosing NavigationStack. Owns the CommentsStore lifecycle so the
/// count is live before the user ever taps.
struct CommentsAttachment: ViewModifier {
    let scope: CommentScope
    let artifactAuthorPubkey: String?
    let artifactHeader: AnyView?

    @Environment(HighlighterStore.self) private var app
    @Environment(HighlighterAppKernel.self) private var kernel
    @State private var store = CommentsStore()
    @State private var showComments = false
    @State private var didStart = false

    func body(content: Content) -> some View {
        let projection = toolbarProjection

        content
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button { showComments = true } label: {
                        commentsLabel(projection)
                    }
                    .accessibilityLabel(projection.accessibilityLabel)
                }
            }
            .navigationDestination(isPresented: $showComments) {
                CommentsView(
                    scope: scope,
                    artifactAuthorPubkey: artifactAuthorPubkey,
                    artifactHeader: artifactHeader,
                    store: store
                )
            }
            .task(id: scope) {
                guard !didStart else { return }
                didStart = true
                await store.start(scope: scope, kernel: kernel)
            }
            .onChange(of: kernel.commentThreads[scope.rootTagValue]) { _, _ in
                store.applyKernelSnapshot()
            }
            .onDisappear { store.stop() }
    }

    private func commentsLabel(_ projection: CommentToolbarProjection) -> some View {
        HStack(spacing: 4) {
            Image(systemName: "bubble.left")
                .font(.system(size: 15, weight: .medium))
            if projection.showsCount {
                Text(projection.countLabel)
                    .font(.system(size: 13, weight: .semibold, design: .rounded))
                    .monospacedDigit()
            }
        }
    }

    private var toolbarProjection: CommentToolbarProjection {
        let count = UInt32(store.records.count)
        let countLabel = count == 1 ? "1 Comment" : "\(count) Comments"
        return CommentToolbarProjection(
            count: count,
            showsCount: count > 0,
            countLabel: countLabel,
            accessibilityLabel: countLabel
        )
    }
}

extension View {
    func commentsAttachment(
        scope: CommentScope,
        artifactAuthorPubkey: String? = nil,
        artifactHeader: AnyView? = nil
    ) -> some View {
        modifier(CommentsAttachment(
            scope: scope,
            artifactAuthorPubkey: artifactAuthorPubkey,
            artifactHeader: artifactHeader
        ))
    }
}
