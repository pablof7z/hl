import Foundation
import Observation

/// Canonical identity for the article reader. Pass this to
/// `ArticleReaderView`; the store derives everything from `pubkey` + `dTag`
/// and falls back to the seed for the first paint while ndb catches up.
struct ArticleReaderTarget: Hashable, Sendable {
    let address: String
    let pubkey: String
    let dTag: String
    /// Optional seed used for the first paint (article cards already hold an
    /// `ArticleRecord`; reusing it avoids a blank flash while ndb answers).
    let seed: ArticleRecord?

    init(route: ArticleReaderRoute, seed: ArticleRecord? = nil) {
        self.address = route.address
        self.pubkey = route.pubkey
        self.dTag = route.dTag
        self.seed = seed
    }

    init?(artifactRoute route: ArtifactDetailRoute, seed: ArticleRecord? = nil) {
        guard route.target == .article,
              !route.articleAddress.isEmpty,
              !route.articlePubkey.isEmpty,
              !route.articleDTag.isEmpty else {
            return nil
        }
        self.address = route.articleAddress
        self.pubkey = route.articlePubkey
        self.dTag = route.articleDTag
        self.seed = seed
    }

    init(article: ArticleRecord, seed: ArticleRecord? = nil) {
        self.address = article.address
        self.pubkey = article.pubkey
        self.dTag = article.identifier
        self.seed = seed
    }

    static func == (lhs: ArticleReaderTarget, rhs: ArticleReaderTarget) -> Bool {
        lhs.address == rhs.address
    }

    func hash(into hasher: inout Hasher) {
        hasher.combine(address)
    }
}

/// View-scoped store for the article reader. Lifetime matches the
/// `ArticleReaderView` that owns it — created in `.task`, torn down in
/// `.onDisappear`. The kernel owns the subscription (via `openArticleReader`);
/// live updates arrive through the observable `kernel.articleReader[address]`
/// dict — no bespoke EventBridge subscription needed (Phase 7 complete cut).
///
/// Architecture: **kernel snapshot is the source of truth.** The store maps
/// `KernelArticleReaderSnapshot` → bespoke records so the existing view
/// hierarchy keeps working without changes.
@MainActor
@Observable
final class ArticleReaderStore {
    // Reactive state driving the view.
    var article: ArticleRecord?
    var highlights: [HighlightRecord] = []
    var isLoadingInitial: Bool = true
    /// Transient flash when a highlight the user just published echoes back.
    var lastPublishedHighlightId: String?

    // Plumbing.
    @ObservationIgnored let target: ArticleReaderTarget
    @ObservationIgnored let kernel: HighlighterAppKernel

    init(
        target: ArticleReaderTarget,
        kernel: HighlighterAppKernel
    ) {
        self.target = target
        self.kernel = kernel
        self.article = target.seed
    }

    func start() async {
        // Open the kernel article-reader view (registers the per-article
        // highlight feed; pushes KernelArticleReaderSnapshot).
        kernel.openArticleReader(address: target.address)
        // Eagerly apply whatever snapshot the kernel already holds.  On a
        // cold start this is typically nil and the seed article covers the
        // gap; the view's onChange fires once the snapshot lands.
        applyKernelSnapshot()
        isLoadingInitial = false
    }

    func stop() {
        kernel.closeArticleReader(address: target.address)
    }

    /// Apply the kernel article-reader snapshot: article metadata + overlay
    /// highlights. Called from `ArticleReaderView.onChange(of: kernel.articleReader[address])`
    /// and from `start()`. The kernel is authoritative for both the article
    /// body and the overlay highlights (per-article kind:9802 feed) —
    /// Phase 7 complete cut-over.
    func applyKernelSnapshot() {
        guard let snap = kernel.articleReader[target.address] else { return }
        // Map kernel snapshot → bespoke ArticleRecord so the existing view
        // hierarchy (ReaderScroll → Header) keeps working without changes.
        article = ArticleRecord(
            eventId: snap.id,
            address: snap.address,
            pubkey: snap.authorPubkey,
            identifier: snap.dTag,
            title: snap.title ?? "",
            summary: snap.summary ?? "",
            image: snap.heroImageUrl ?? "",
            content: "",   // Body rendered via contentTreeJson; markdown unused.
            hashtags: [],  // Phase 7: kernel snapshot omits hashtags; empty fallback.
            publishedAt: nil,
            createdAt: snap.createdAt
        )
        highlights = snap.highlights.map(HighlightRecord.init(kernelRow:))
    }

    /// Called by `EventBridge` when an `ArticleUpdated` delta arrives.
    /// With the kernel cut-over the live snapshot arrives via `articleReader`
    /// already; this re-applies it so any in-flight EventBridge registration
    /// (e.g. from a previous session) still converges correctly.
    func applyUpdate() async {
        applyKernelSnapshot()
    }

    // MARK: - Writes

    /// Publish a solo NIP-84 highlight for the currently loaded article.
    ///
    /// Phase 7: the KERNEL is the sole writer. Dispatches `hl.highlight.publish`
    /// (fire-and-forget) with the article address as the NIP-84 source reference;
    /// the new kind:9802 echoes back through the per-article highlight feed →
    /// `KernelArticleReaderSnapshot.highlights` → `applyKernelSnapshot()`, so the
    /// overlay updates without a live-lane publish. Returns `nil` on dispatch
    /// (no synchronous error surface — fire-and-forget, D6).
    func publishHighlight(
        quote: String,
        note: String,
        context: String
    ) async -> String? {
        kernel.app.dispatch(
            .publishHighlight(
                content: quote,
                sourceReference: target.address,
                relayHint: nil,
                note: note.isEmpty ? nil : note,
                context: context.isEmpty ? nil : context
            )
        )
        return nil
    }
}

// MARK: - Kernel row → bespoke record mapping (Phase 7)

extension HighlightRecord {
    /// Map an enriched kernel `HighlightRow` (from `KernelArticleReaderSnapshot.highlights`)
    /// into the bespoke `HighlightRecord` the overlay UI renders. 1:1 — the kernel
    /// HighlightRow was enriched (1c3c5cd9) to carry every field the record needs.
    init(kernelRow row: HighlightRow) {
        self.init(
            eventId: row.eventId,
            pubkey: row.authorPubkey,
            quote: row.content,
            context: row.context,
            note: row.note ?? "",
            artifactAddress: row.artifactAddress,
            eventReference: row.eventReference,
            externalReference: row.externalReference,
            sourceUrl: row.sourceUrl,
            sourceReferenceKey: row.sourceReferenceKey,
            clipStartSeconds: row.clipStartSeconds,
            clipEndSeconds: row.clipEndSeconds,
            clipSpeaker: row.clipSpeaker,
            clipTranscriptSegmentIds: row.clipTranscriptSegmentIds,
            imageUrl: row.imageUrl,
            createdAt: row.createdAt
        )
    }
}
