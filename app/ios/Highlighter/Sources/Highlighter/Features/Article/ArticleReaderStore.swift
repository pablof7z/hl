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
/// `.onDisappear`. Subscribes via `subscribe_article` so live article and
/// highlight deltas trigger Rust-classified re-queries.
///
/// Architecture: **nostrdb is the source of truth.** The store never holds
/// data that isn't already in (or en-route to) ndb.
@MainActor
@Observable
final class ArticleReaderStore {
    // Reactive state driving the view.
    var article: ArticleRecord?
    var authorProfile: ProfileMetadata?
    var highlights: [HighlightRecord] = []
    var isLoadingInitial: Bool = true
    /// Transient flash when a highlight the user just published echoes back.
    var lastPublishedHighlightId: String?

    // Plumbing.
    @ObservationIgnored let target: ArticleReaderTarget
    @ObservationIgnored let safeCore: SafeHighlighterCore
    @ObservationIgnored weak var eventBridge: EventBridge?
    @ObservationIgnored private var subscriptionHandle: UInt64?
    /// Phase 7: the kernel owns the overlay highlights (per-article kind:9802
    /// feed) and is the SOLE WRITER for publishing highlights. The article BODY
    /// is still read from the live lane (reads coexist until Part C); the kernel
    /// snapshot's `highlights` win when the view is open.
    @ObservationIgnored let kernel: HighlighterAppKernel

    init(
        target: ArticleReaderTarget,
        safeCore: SafeHighlighterCore,
        eventBridge: EventBridge?,
        kernel: HighlighterAppKernel
    ) {
        self.target = target
        self.safeCore = safeCore
        self.eventBridge = eventBridge
        self.kernel = kernel
        self.article = target.seed
    }

    func start() async {
        // Open the kernel article-reader view (registers the per-article
        // highlight feed; pushes KernelArticleReaderSnapshot.highlights).
        kernel.openArticleReader(address: target.address)
        await loadAll()
        isLoadingInitial = false
        await installSubscription()
    }

    func stop() {
        kernel.closeArticleReader(address: target.address)
        if let handle = subscriptionHandle {
            Task { [safeCore] in await safeCore.unsubscribe(handle) }
            eventBridge?.unregister(handle: handle)
            subscriptionHandle = nil
        }
    }

    /// Apply the kernel article-reader snapshot's overlay highlights. Called from
    /// `ArticleReaderView.onChange(of: kernel.articleReader[address])`. The kernel
    /// is authoritative for the overlay (enriched kind:9802 rows); the body stays
    /// from the live-lane read for now (Part C completes the cut).
    func applyKernelSnapshot() {
        guard let snap = kernel.articleReader[target.address] else { return }
        highlights = snap.highlights.map(HighlightRecord.init(kernelRow:))
    }

    // MARK: - Loads

    func loadAll() async {
        let snapshot = await safeCore.getArticleReaderSnapshot(
            pubkeyHex: target.pubkey,
            dTag: target.dTag
        )
        apply(snapshot: snapshot)
    }

    private func apply(snapshot: ArticleReaderSnapshot) {
        let projection = safeCore.projectArticleReaderSnapshot(
            input: ArticleReaderSnapshotApplyInput(
                snapshot: snapshot,
                currentArticle: article,
                currentAuthorProfile: authorProfile
            )
        )
        apply(projection: projection)
    }

    private func apply(projection: ArticleReaderSnapshotProjection) {
        article = projection.article
        authorProfile = projection.authorProfile
        // The live-lane projection seeds highlights for the cold-start window;
        // the kernel overlay (per-article kind:9802 feed) is authoritative and
        // overrides as soon as its snapshot is present (Phase 7).
        highlights = projection.highlights
        applyKernelSnapshot()
    }

    /// Called by `EventBridge` when an `ArticleUpdated` delta arrives.
    /// Re-queries Rust's full reader snapshot so native code does not branch
    /// on protocol event kinds.
    func applyUpdate() async {
        await loadAll()
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

    // MARK: - Private

    private func installSubscription() async {
        guard subscriptionHandle == nil, let bridge = eventBridge else { return }
        let outcome = await safeCore.subscribeArticle(
            pubkeyHex: target.pubkey,
            dTag: target.dTag
        )
        let projection = safeCore.projectViewSubscriptionStart(
            input: ViewSubscriptionStartProjectionInput(start: outcome)
        )
        guard projection.shouldRegister else {
            // Non-fatal: cold ndb path still shows the seeded article and
            // its cached highlights. Live updates will resume on the next
            // visit.
            return
        }
        subscriptionHandle = projection.handle
        bridge.registerArticle(self, handle: projection.handle)
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
