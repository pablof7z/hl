import SwiftUI

/// Attaches the premium NIP-22 comments experience to any reader: mounts
/// the Liquid Glass count toolbar via `safeAreaInset(.bottom)`, owns the
/// `CommentsStore` lifecycle, and presents the multi-detent comments
/// sheet (NavigationStack + recursive thread push) on tap.
///
/// Generic over artifacts — caller passes an `ArtifactRef` (article,
/// event, external URL/podcast/book). Optionally passes the artifact's
/// own-author pubkey for the gold thread-line tint when they engage.
struct CommentsAttachment: ViewModifier {
    let artifact: ArtifactRef
    let artifactAuthorPubkey: String?
    let artifactHeader: AnyView?

    @Environment(HighlighterStore.self) private var app
    @State private var store = CommentsStore()
    @State private var isSheetPresented: Bool = false
    @State private var didStart: Bool = false

    func body(content: Content) -> some View {
        content
            .safeAreaInset(edge: .bottom) {
                CommentsToolbar(
                    count: store.totalCount,
                    recentCommenterPubkeys: store.recentCommenterPubkeys(limit: 3),
                    shrunk: false,
                    onTap: { isSheetPresented = true }
                )
            }
            .task(id: artifact) {
                guard !didStart else { return }
                didStart = true
                await store.start(
                    artifact: artifact,
                    core: app.safeCore,
                    currentUserPubkey: app.currentUser?.pubkey
                )
            }
            .sheet(isPresented: $isSheetPresented) {
                CommentsSheet(
                    artifact: artifact,
                    artifactAuthorPubkey: artifactAuthorPubkey,
                    artifactHeader: artifactHeader,
                    store: store
                )
                .environment(app)
            }
    }
}

extension View {
    /// Mounts the comments toolbar + sheet for an artifact. Pass the
    /// artifact's own-author pubkey when known (drives the gold
    /// thread-line tint on author replies).
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
