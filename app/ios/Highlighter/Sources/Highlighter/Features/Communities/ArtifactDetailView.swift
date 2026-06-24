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
        Group {
            switch artifactSource {
            case "podcast":
                PodcastListeningView(presentation: .pushed, artifact: artifact)
            case "article":
                if let target = articleReaderTarget {
                    ArticleReaderView(target: target)
                } else if let url = artifactWebURL {
                    WebReaderView(target: WebReaderTarget(url: url, highlightQuote: ""))
                } else {
                    missingReferenceView
                }
            case "book":
                if let catalogId = bookCatalogId {
                    BookView(catalogId: catalogId).environment(app)
                } else if let url = artifactWebURL {
                    WebReaderView(target: WebReaderTarget(url: url, highlightQuote: ""))
                } else {
                    missingReferenceView
                }
            default:
                if let url = artifactWebURL {
                    WebReaderView(target: WebReaderTarget(url: url, highlightQuote: ""))
                } else {
                    missingReferenceView
                }
            }
        }
        .navigationTitle(artifact.preview.title.isEmpty ? "Artifact" : artifact.preview.title)
        .navigationBarTitleDisplayMode(.inline)
    }

    // MARK: - D1 routing (mirrors artifact_detail.rs)

    private var artifactSource: String {
        artifact.preview.source.trimmingCharacters(in: .whitespaces).lowercased()
    }

    private var articleReaderTarget: ArticleReaderTarget? {
        let preview = artifact.preview
        // mirrors reference_value_for(preview, "a") + parse_nip23_address
        let raw: String
        if preview.highlightTagName.caseInsensitiveCompare("a") == .orderedSame,
           !preview.highlightTagValue.trimmingCharacters(in: .whitespaces).isEmpty {
            raw = preview.highlightTagValue.trimmingCharacters(in: .whitespaces)
        } else if preview.referenceTagName.caseInsensitiveCompare("a") == .orderedSame,
                  !preview.referenceTagValue.trimmingCharacters(in: .whitespaces).isEmpty {
            raw = preview.referenceTagValue.trimmingCharacters(in: .whitespaces)
        } else {
            return nil
        }
        let parts = raw.split(separator: ":", maxSplits: 2).map(String.init)
        guard parts.count == 3, parts[0] == "30023", !parts[1].isEmpty, !parts[2].isEmpty else {
            return nil
        }
        let route = ArtifactDetailRoute(
            target: .article,
            articleAddress: raw,
            articlePubkey: parts[1],
            articleDTag: parts[2],
            bookCatalogId: "",
            url: ""
        )
        return ArticleReaderTarget(artifactRoute: route)
    }

    private var bookCatalogId: String? {
        let preview = artifact.preview
        // mirrors book_route + is_book_catalog_id + first_non_empty
        let candidates = [
            preview.catalogId,
            preview.referenceTagName.caseInsensitiveCompare("i") == .orderedSame ? preview.referenceTagValue : "",
            preview.highlightTagName.caseInsensitiveCompare("i") == .orderedSame ? preview.highlightTagValue : "",
        ]
        guard let candidate = candidates.first(where: { !$0.trimmingCharacters(in: .whitespaces).isEmpty }) else {
            return nil
        }
        let trimmed = candidate.trimmingCharacters(in: .whitespaces)
        let lowered = trimmed.lowercased()
        guard lowered.hasPrefix("isbn:") || lowered.hasPrefix("openlibrary:") || lowered.hasPrefix("goodreads:") else {
            return nil
        }
        return trimmed
    }

    private var artifactWebURL: URL? {
        let preview = artifact.preview
        // mirrors url_for_preview (no UTM stripping — acceptable simplification)
        let urlTagNames: Set<String> = ["r", "u", "i"]
        let candidates = [
            preview.url,
            urlTagNames.contains(preview.referenceTagName.lowercased()) ? preview.referenceTagValue : "",
            urlTagNames.contains(preview.highlightTagName.lowercased()) ? preview.highlightTagValue : "",
            preview.audioUrl,
            preview.audioPreviewUrl,
        ]
        for candidate in candidates {
            var raw = candidate.trimmingCharacters(in: .whitespaces)
            guard !raw.isEmpty else { continue }
            if raw.hasPrefix("url:") { raw = String(raw.dropFirst(4)) }
            if let url = URL(string: raw), url.scheme == "http" || url.scheme == "https" {
                return url
            }
        }
        return nil
    }

    private var missingReferenceView: some View {
        ContentUnavailableView(
            "Missing artifact reference",
            systemImage: "doc.text",
            description: Text("This share doesn't carry an openable artifact reference.")
        )
    }
}
