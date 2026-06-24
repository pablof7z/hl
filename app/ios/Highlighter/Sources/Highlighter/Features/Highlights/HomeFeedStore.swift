import Foundation
import Observation

/// Home feed — composes friend highlights and friend-surfaced reads into
/// Rust-owned screen rows.
///
/// Phase 7: kernel-backed. The kernel owns ALL aggregation (grouping, dedupe,
/// suppression, stable ids, ordering, social surfacing, embedded enriched
/// highlights) and ships it as `KernelHomeFeedSnapshot`; Swift opens the
/// `ViewId.homeFeed` view and maps the raw rows into the bespoke render
/// view-models (`HomeFeedItem`/`HydratedHighlight`/`ReadingFeedItem`). The
/// live-lane `getHomeFeedSnapshot` / `subscribeFollowingReads|Highlights` path
/// is gone (kernel is the sole reader).
@MainActor
@Observable
final class HomeFeedStore {
    typealias Item = HomeFeedItem

    var items: [Item] = []
    var isLoadingInitial: Bool = true
    var loadError: String?

    @ObservationIgnored let kernel: HighlighterAppKernel

    init(kernel: HighlighterAppKernel) {
        self.kernel = kernel
    }

    func start() async {
        kernel.openHomeFeed()
        applyKernelSnapshot()
        isLoadingInitial = false
    }

    func stop() {
        kernel.closeHomeFeed()
    }

    /// Map the kernel home-feed snapshot into bespoke render rows. Called on
    /// `start()` and from the view's `.onChange(of: kernel.homeFeed)`.
    func applyKernelSnapshot() {
        guard let snapshot = kernel.homeFeed else { return }
        // Index the bounded artifact-preview slice by coordinate for O(1) lookup.
        var previews: [String: ArtifactPreviewRow] = [:]
        for p in snapshot.artifactPreviews {
            previews[p.coordinate] = p
        }
        items = snapshot.rows.map { row in
            Self.mapRow(row, previews: previews)
        }
        isLoadingInitial = false
    }

    // MARK: - Kernel row → bespoke view-model mapping (Phase 7)

    private static func mapRow(
        _ row: KernelHomeFeedRow,
        previews: [String: ArtifactPreviewRow]
    ) -> HomeFeedItem {
        switch row.kind {
        case .highlight:
            let preview = row.artifactCoordinate.flatMap { previews[$0] }
            let hydrated = row.highlights.map { hr in
                HydratedHighlight(
                    highlight: HighlightRecord(kernelRow: hr),
                    artifact: preview.flatMap(Self.artifactRecord(from:)),
                    sharedByEventId: nil,
                    sharedByPubkey: nil
                )
            }
            return HomeFeedItem(
                stableId: row.stableId,
                sortKey: row.sortKey,
                highlights: hydrated,
                read: nil
            )
        case .article:
            let preview = row.artifactCoordinate.flatMap { previews[$0] }
            let read = ReadingFeedItem(
                article: Self.articleRecord(from: row, preview: preview),
                authorFollowed: row.authorFollowed,
                interactorPubkeys: row.interactorPubkeys,
                latestActivityAt: row.latestActivityAt
            )
            return HomeFeedItem(
                stableId: row.stableId,
                sortKey: row.sortKey,
                highlights: [],
                read: read
            )
        }
    }

    /// Build a feed-card `ArticleRecord` from the kernel article row + its
    /// artifact preview. The card renders title/summary/image/author/date only —
    /// the body `content` is empty (the reader fetches the full document).
    private static func articleRecord(
        from row: KernelHomeFeedRow,
        preview: ArtifactPreviewRow?
    ) -> ArticleRecord {
        let address = row.articleAddress ?? ""
        return ArticleRecord(
            eventId: row.articleId ?? "",
            address: address,
            pubkey: row.articleAuthorPubkey ?? "",
            identifier: Self.dTag(fromAddress: address),
            title: preview?.title ?? "",
            summary: preview?.summary ?? "",
            image: preview?.imageUrl ?? "",
            content: "",
            hashtags: [],
            publishedAt: row.articleCreatedAt,
            createdAt: row.articleCreatedAt
        )
    }

    /// Minimal `ArtifactRecord` from a resolved preview, for the highlight
    /// share-to-room target. `nil` callers fall back to the inline NIP-23
    /// address parsing in `HighlightsTabView.shareTargetForHighlight(_:)`.
    private static func artifactRecord(from preview: ArtifactPreviewRow) -> ArtifactRecord? {
        guard !preview.pending else { return nil }
        return nil
    }

    /// Extract the NIP-33 `d` tag from a `kind:pubkey:d` coordinate.
    private static func dTag(fromAddress address: String) -> String {
        let parts = address.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false)
        return parts.count == 3 ? String(parts[2]) : ""
    }
}
