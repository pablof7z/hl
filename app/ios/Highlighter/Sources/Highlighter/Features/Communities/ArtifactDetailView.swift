import SwiftUI

/// Dispatch view for an artifact row. Routes by `preview.source`:
/// - `podcast` → pushes `PodcastListeningView`, which loads the artifact
///   into the global player on appear. The MiniPlayer accessory still
///   surfaces (mounted on `MainTabView`); back chevron returns to the room.
/// - `article` → NIP-23 reader, built from the artifact's `a`-tag reference
///   (`30023:<pubkey>:<d>`).
/// - URL-backed sources → existing web reader.
struct ArtifactDetailView: View {
    let artifact: ArtifactRecord

    @Environment(HighlighterStore.self) private var app

    var body: some View {
        let route = app.core.getArtifactDetailRoute(artifact: artifact)

        Group {
            switch route.target {
            case .podcast:
                PodcastListeningView(presentation: .pushed, artifact: artifact)
            case .article:
                if let target = ArticleReaderTarget(artifactRoute: route) {
                    ArticleReaderView(target: target)
                } else {
                    missingReferenceView
                }
            case .book:
                BookView(catalogId: route.bookCatalogId)
                    .environment(app)
            case .web:
                if let url = URL(string: route.url) {
                    WebReaderView(target: WebReaderTarget(url: url, highlightQuote: ""))
                } else {
                    missingReferenceView
                }
            case .unavailable:
                missingReferenceView
            }
        }
        .navigationTitle(artifact.preview.title.isEmpty ? "Artifact" : artifact.preview.title)
        .navigationBarTitleDisplayMode(.inline)
    }

    private var missingReferenceView: some View {
        ContentUnavailableView(
            "Missing artifact reference",
            systemImage: "doc.text",
            description: Text("This share doesn't carry an openable artifact reference.")
        )
    }
}
