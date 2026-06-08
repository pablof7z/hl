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
    var loadError: String?
    /// Transient flash when a highlight the user just published echoes back.
    var lastPublishedHighlightId: String?

    // Plumbing.
    @ObservationIgnored let target: ArticleReaderTarget
    @ObservationIgnored let safeCore: SafeHighlighterCore
    @ObservationIgnored weak var eventBridge: EventBridge?
    @ObservationIgnored private var subscriptionHandle: UInt64?

    init(
        target: ArticleReaderTarget,
        safeCore: SafeHighlighterCore,
        eventBridge: EventBridge?
    ) {
        self.target = target
        self.safeCore = safeCore
        self.eventBridge = eventBridge
        self.article = target.seed
    }

    func start() async {
        await loadAll()
        isLoadingInitial = false
        await installSubscription()
    }

    func stop() {
        if let handle = subscriptionHandle {
            Task { [safeCore] in await safeCore.unsubscribe(handle) }
            eventBridge?.unregister(handle: handle)
            subscriptionHandle = nil
        }
    }

    // MARK: - Loads

    func loadAll() async {
        async let articleTask: ArticleRecord? = {
            let outcome = await safeCore.getArticle(pubkeyHex: target.pubkey, dTag: target.dTag)
            return outcome.error.isEmpty ? outcome.value : nil
        }()
        async let highlightsTask: [HighlightRecord] = {
            let outcome = await safeCore.getHighlightsForArticle(address: target.address)
            return outcome.error.isEmpty ? outcome.values : []
        }()
        async let profileTask: ProfileMetadata? = {
            let outcome = await safeCore.getUserProfile(pubkeyHex: target.pubkey)
            return outcome.error.isEmpty ? outcome.value : nil
        }()

        let (article, highlights, profile) = await (articleTask, highlightsTask, profileTask)
        if let article {
            self.article = article
        }
        self.highlights = highlights
        if let profile {
            self.authorProfile = profile
        }
    }

    /// Called by `EventBridge` when an `ArticleUpdated` delta arrives.
    /// Re-queries only the slice Rust says is affected by the event kind.
    func applyUpdate(kind: UInt32) async {
        switch safeCore.getArticleUpdateAction(kind: kind) {
        case .refreshArticle:
            let outcome = await safeCore.getArticle(
                pubkeyHex: target.pubkey,
                dTag: target.dTag
            )
            if outcome.error.isEmpty, let article = outcome.value {
                self.article = article
            }
        case .refreshHighlights:
            let outcome = await safeCore.getHighlightsForArticle(address: target.address)
            if outcome.error.isEmpty {
                self.highlights = outcome.values
            }
        case .ignore:
            break
        }
    }

    // MARK: - Writes

    /// Publish a solo NIP-84 highlight for the currently loaded article.
    /// Returns the record so the view can flash the new overlay without
    /// waiting for the subscription to echo back.
    ///
    /// Returns outcome state so the caller can surface publish failures in a toast.
    func publishHighlight(quote: String, note: String, context: String) async -> HighlightOutcome {
        guard let article else {
            return HighlightOutcome(value: nil, error: "Article not yet loaded.")
        }
        let artifactOutcome = safeCore.getArticleArtifactRecord(article: article)
        guard artifactOutcome.error.isEmpty, let artifact = artifactOutcome.value else {
            return HighlightOutcome(
                value: nil,
                error: artifactOutcome.error.isEmpty ? "Article source is unavailable." : artifactOutcome.error
            )
        }
        let draft = HighlightDraft(
            quote: quote,
            context: context,
            note: note,
            clipStartSeconds: nil,
            clipEndSeconds: nil,
            clipSpeaker: "",
            clipTranscriptSegmentIds: [],
            image: nil
        )
        let outcome = await safeCore.publishHighlight(draft: draft, artifact: artifact)
        guard outcome.error.isEmpty, let record = outcome.value else { return outcome }
        // Optimistically inject into the local list so the overlay appears
        // immediately; the subscription delta will reconcile shortly.
        if !highlights.contains(where: { $0.eventId == record.eventId }) {
            highlights.insert(record, at: 0)
        }
        lastPublishedHighlightId = record.eventId
        return outcome
    }

    // MARK: - Private

    private func installSubscription() async {
        guard subscriptionHandle == nil, let bridge = eventBridge else { return }
        let outcome = await safeCore.subscribeArticle(
            pubkeyHex: target.pubkey,
            dTag: target.dTag
        )
        guard outcome.error.isEmpty else {
            // Non-fatal: cold ndb path still shows the seeded article and
            // its cached highlights. Live updates will resume on the next
            // visit.
            return
        }
        subscriptionHandle = outcome.handle
        bridge.registerArticle(self, handle: outcome.handle)
    }
}
