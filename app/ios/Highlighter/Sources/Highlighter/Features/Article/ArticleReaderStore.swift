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
        let snapshot = await safeCore.getArticleReaderSnapshot(
            pubkeyHex: target.pubkey,
            dTag: target.dTag
        )
        apply(snapshot: snapshot)
    }

    private func apply(snapshot: ArticleReaderSnapshot) {
        if let loadedArticle = snapshot.article {
            article = loadedArticle
        }
        highlights = snapshot.highlights
        if let profile = snapshot.authorProfile {
            authorProfile = profile
        }
    }

    /// Called by `EventBridge` when an `ArticleUpdated` delta arrives.
    /// Re-queries Rust's full reader snapshot so native code does not branch
    /// on protocol event kinds.
    func applyUpdate() async {
        await loadAll()
    }

    // MARK: - Writes

    /// Publish a solo NIP-84 highlight for the currently loaded article.
    /// Returns the record so the view can flash the new overlay without
    /// waiting for the subscription to echo back.
    ///
    /// Returns outcome state so the caller can surface publish failures in a toast.
    func publishHighlight(
        quote: String,
        note: String,
        context: String
    ) async -> ArticleReaderHighlightPublishSnapshot {
        let outcome = await safeCore.publishArticleReaderHighlightSnapshot(
            pubkeyHex: target.pubkey,
            dTag: target.dTag,
            article: article,
            quote: quote,
            note: note,
            context: context
        )
        guard outcome.error.isEmpty else { return outcome }
        apply(snapshot: outcome.snapshot)
        lastPublishedHighlightId = outcome.publishedHighlightId
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
