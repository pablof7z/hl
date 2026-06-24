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
    /// The article BODY as the nmp `content_tree`, decoded from the kernel
    /// `KernelArticleReaderSnapshot.contentTreeJson` (#22). This is the body
    /// render source — replacing the bespoke `ArticleRecord.content` markdown
    /// read. `nil` until the full document arrives (cold-start window, D6).
    /// The native select-to-highlight overlay (`ArticleBodyView`) renders on top
    /// of this tree via `ContentTreeBodyRenderer`.
    var contentTree: ContentTreeWire?

    // Plumbing.
    @ObservationIgnored let target: ArticleReaderTarget
    /// The kernel is the SOLE data source for the reader (Phase 7 C1). It owns
    /// the overlay highlights (per-article kind:9802 feed), is the SOLE WRITER
    /// for publishing highlights (D5), and `KernelArticleReaderSnapshot` carries
    /// the COMPLETE article metadata + author profile (#22 already moved the
    /// BODY to the snapshot's `contentTreeJson`). The bespoke
    /// subscribeArticle/getArticleReaderSnapshot/projectArticleReaderSnapshot
    /// path is gone — `applyKernelSnapshot()` populates everything.
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
        // highlight feed; pushes KernelArticleReaderSnapshot.highlights).
        kernel.openArticleReader(address: target.address)
        applyKernelSnapshot()   // apply any seed already present in the kernel
        isLoadingInitial = false
    }

    func stop() {
        kernel.closeArticleReader(address: target.address)
    }

    /// Apply the kernel article-reader snapshot's overlay highlights AND the nmp
    /// `content_tree` body. Called from
    /// `ArticleReaderView.onChange(of: kernel.articleReader[address])`. The kernel
    /// is authoritative for the overlay (enriched kind:9802 rows) and the body
    /// (raw `content_tree`, D1); #22 cut the bespoke markdown body read.
    func applyKernelSnapshot() {
        guard let snap = kernel.articleReader[target.address] else { return }
        highlights = snap.highlights.map(HighlightRecord.init(kernelRow:))
        // #22: the article BODY is now the nmp `content_tree` the kernel emits
        // (D1 — kernel emits raw content_tree, Swift renders). Decode the
        // snapshot's `contentTreeJson` into the vendored nmp `ContentTreeWire`.
        // Keep a non-empty tree once decoded so a transient empty snapshot tick
        // (e.g. a highlight-only delta before the body arrives) doesn't blank
        // the body mid-read.
        if let tree = ContentTreeBodyRenderer.decodeTree(json: snap.contentTreeJson) {
            contentTree = tree
        }
        // C1: article metadata + author profile now come straight from the
        // kernel snapshot, replacing the bespoke
        // getArticleReaderSnapshot/projectArticleReaderSnapshot path. The kernel
        // snapshot is enriched (author kind:0 display name + picture, title,
        // hero image, etc.), so the reader header + author chip render from it.
        // Only overwrite once the snapshot carries a real event (`id` non-empty)
        // so a transient highlight-only tick doesn't clobber the seed.
        if !snap.id.isEmpty {
            article = ArticleRecord(kernelSnapshot: snap)
            authorProfile = ProfileMetadata(kernelSnapshot: snap)
        }
    }

    /// Called by `EventBridge` when an `ArticleUpdated` delta arrives. Phase 7
    /// C1: re-applies the kernel snapshot (the sole data source) rather than
    /// re-querying the retired bespoke reader snapshot.
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

// MARK: - Kernel snapshot → bespoke record mapping (Phase 7)

extension ArticleRecord {
    /// Build the bespoke `ArticleRecord` the reader header + share sheet render
    /// from the enriched `KernelArticleReaderSnapshot` (Phase 7 C1). The kernel
    /// snapshot carries the complete article metadata, so the bespoke
    /// getArticleReaderSnapshot path is no longer needed. The BODY is rendered
    /// from `snapshot.contentTreeJson` (#22), so `content` stays empty here —
    /// markdown is no longer the body source. `hashtags`/`publishedAt` aren't on
    /// the snapshot yet, so they default empty/`createdAt`.
    init(kernelSnapshot snap: KernelArticleReaderSnapshot) {
        self.init(
            eventId: snap.id,
            address: snap.address,
            pubkey: snap.authorPubkey,
            identifier: snap.dTag,
            title: snap.title ?? "",
            summary: snap.summary ?? "",
            image: snap.heroImageUrl ?? "",
            content: "",
            hashtags: [],
            publishedAt: snap.createdAt,
            createdAt: snap.createdAt
        )
    }
}

extension ProfileMetadata {
    /// Build the author `ProfileMetadata` chip from the enriched
    /// `KernelArticleReaderSnapshot` (Phase 7 C1). The kernel enriches the
    /// snapshot with the author's kind:0 display name + picture; the remaining
    /// profile fields aren't carried on the article snapshot, so they default
    /// empty (the profile feed fills them in via `app.profileSnapshots`).
    init(kernelSnapshot snap: KernelArticleReaderSnapshot) {
        self.init(
            pubkey: snap.authorPubkey,
            name: "",
            displayName: snap.authorDisplayName ?? "",
            about: "",
            picture: snap.authorPictureUrl ?? "",
            banner: "",
            nip05: "",
            website: "",
            lud16: "",
            createdAt: nil
        )
    }
}

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
