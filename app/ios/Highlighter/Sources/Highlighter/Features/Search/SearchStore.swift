import Foundation
import Observation

enum SearchDirectNavigation: Equatable {
    case article(ArticleReaderTarget)
    case profile(String)
    case entity(NostrEntityRef)
    case group(String)
}

/// Drives `SearchView`. Owns the query state + the result buckets.
///
/// Phase 7 cutover (complete): the KERNEL owns ALL search buckets.
/// `query` changes dispatch `hl.search.run` (scope = articles + highlights, one
/// NIP-50 query); results stream back via `kernel.searchSnapshot` →
/// `applyKernelSnapshot()` (wired from the View's `.onChange`). Swift buckets the
/// mixed hits by kind. The PEOPLE bucket is now kernel-owned (#1697): a local
/// kind:0 cache scan via `project_profile_search_rows`. `searchRelays` / recent
/// queries stay on the bespoke chrome path.
///
/// Omnibox (#1865): input classification is delegated to the NMP kernel via
/// `AppAction.runOmnibox`. The resolved `OmniboxOutcome` in the `ViewId.search`
/// snapshot is fed back via `applyOmniboxOutcome` from `SearchView.onChange`.
@MainActor
@Observable
final class SearchStore {
    // MARK: - Inputs

    /// Raw text from the search field. Writes schedule a query.
    var query: String = "" {
        didSet {
            guard query != oldValue else { return }
            scheduleSearch(for: query)
        }
    }

    // MARK: - Outputs (reactive)

    private(set) var highlights: [HighlightRecord] = []
    private(set) var articles: [ArticleRecord] = []
    private(set) var communities: [CommunitySummary] = []
    /// Kernel-owned local kind:0 scan; populated via `applyKernelSnapshot()` from
    /// `ProfileSearchRow` rows in `searchSnapshot`.
    private(set) var profiles: [ProfileSearchRow] = []
    private(set) var hasQuery: Bool = false

    /// True between dispatching the kernel search and the first snapshot.
    private(set) var isLocalLoading: Bool = false

    /// True while the kernel NIP-50 relay subscription is in flight.
    private(set) var isRelayLoading: Bool = false

    /// The resolved set of relays the NIP-50 query hits, shown as a footnote.
    private(set) var searchRelays: [String] = []

    /// Set when the omnibox resolver routes to a specific Nostr entity or group.
    private(set) var directNavigation: SearchDirectNavigation?
    private(set) var directOpenMessage: String?

    /// Set when the omnibox resolver classified the current input as a secret key
    /// (`nsec` / `ncryptsec`). The view renders a safe notice and suppresses the
    /// normal result echo — the secret is NEVER displayed.
    private(set) var secretRejected: Bool = false

    // MARK: - Dependencies

    /// Phase 7: the kernel owns article/highlight/community search.
    @ObservationIgnored private let kernel: HighlighterAppKernel

    // MARK: - Internal state

    /// The query whose results currently populate the buckets.
    private var appliedQuery: String = ""
    /// Gate for the omnibox outcome (#1865). Armed on each fresh `runOmnibox`
    /// dispatch and consumed once the resolved outcome is routed.
    private var omniboxArmed: Bool = false

    // MARK: - Init

    init(kernel: HighlighterAppKernel) {
        self.kernel = kernel
    }

    // MARK: - Lifecycle

    func start() async {
        kernel.openSearch()
    }

    func stop() {
        kernel.closeSearch()
    }

    // MARK: - Query orchestration

    /// Re-applies a search explicitly (e.g. tapping a recent search chip).
    func submit(_ query: String) {
        if self.query != query {
            self.query = query
        } else {
            scheduleSearch(for: query)
        }
    }

    func clear() {
        if query.isEmpty {
            scheduleSearch(for: "")
        } else {
            query = ""
        }
    }

    func consumeDirectNavigation() {
        directNavigation = nil
    }

    /// Disarm the omnibox gate so a retained outcome pushed on Search re-open
    /// is not re-routed. The next genuine query change re-arms it.
    func disarmOmnibox() {
        omniboxArmed = false
    }

    private func scheduleSearch(for raw: String) {
        directNavigation = nil
        directOpenMessage = nil
        secretRejected = false
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)

        guard !trimmed.isEmpty else {
            clearResultsForEmptyQuery()
            return
        }

        hasQuery = true
        isLocalLoading = true
        isRelayLoading = true

        // Single brain (#1865): hand the raw input to NMP's omnibox resolver.
        // The kernel classifies the input and surfaces an `OmniboxOutcome` in
        // the `ViewId.search` snapshot which the view feeds back via
        // `applyOmniboxOutcome`. Also dispatch the content search so article/
        // highlight/community buckets populate concurrently.
        omniboxArmed = true
        kernel.app.dispatch(.runOmnibox(query: trimmed))
        kernel.app.dispatch(.runSearch(query: trimmed, scope: .articlesHighlightsAndUsers))
        appliedQuery = trimmed
    }

    private func clearResultsForEmptyQuery() {
        hasQuery = false
        appliedQuery = ""
        highlights = []
        articles = []
        communities = []
        profiles = []
        isLocalLoading = false
        isRelayLoading = false
        directOpenMessage = nil
        secretRejected = false
    }

    // MARK: - Omnibox routing (#1865)

    /// Route the kernel's classification of the current input. Called by
    /// `SearchView` whenever the `ViewId.search` snapshot's omnibox outcome changes.
    func applyOmniboxOutcome(_ outcome: OmniboxOutcome?) {
        guard let outcome, omniboxArmed else { return }
        omniboxArmed = false
        switch outcome {
        case .navigate:
            // Nostr entity paste-nav: entity resolution is deferred to Wave 2
            // (kernel will expose a typed resolve-entity action). For now the
            // search buckets still run and the user can tap a result.
            secretRejected = false

        case .resolveNip05(let identifier):
            secretRejected = false
            if directNavigation == nil {
                directOpenMessage = "Looking up \(identifier)…"
            }

        case .openGroup(_, let localId):
            secretRejected = false
            directOpenMessage = nil
            directNavigation = .group(localId)

        case .rejectSecret:
            directNavigation = nil
            directOpenMessage = nil
            secretRejected = true
            highlights = []
            articles = []
            communities = []
            profiles = []

        case .freeText, .relayUrl, .noMatch:
            secretRejected = false
        }
    }

    // MARK: - Kernel snapshot (article / highlight / community buckets)

    /// Apply the kernel search snapshot. Called from `SearchView.onChange(of:
    /// kernel.searchSnapshot)`. Buckets the mixed `hits` by kind for Articles,
    /// reads the kernel-decoded `highlights` directly, maps `communities` rows,
    /// and reads the kernel `profiles` local-scan bucket.
    func applyKernelSnapshot() {
        guard let snap = kernel.searchSnapshot else { return }
        articles = snap.hits
            .filter { $0.kind == 30023 }
            .map(Self.articleRecord(from:))
        highlights = snap.hits
            .filter { $0.kind == 9802 }
            .map(Self.highlightRecord(from:))
        profiles = snap.hits
            .filter { $0.kind == 0 }
            .compactMap(Self.profileRow(from:))
        isLocalLoading = false
        isRelayLoading = false
    }

    // MARK: - Kernel row → bespoke record mapping

    private static func articleRecord(from hit: KernelSearchHitRow) -> ArticleRecord {
        let identifier = tagValue(hit.tags, "d") ?? ""
        let publishedAt = tagValue(hit.tags, "published_at").flatMap(UInt64.init)
        return ArticleRecord(
            eventId: hit.id,
            address: "30023:\(hit.author):\(identifier)",
            pubkey: hit.author,
            identifier: identifier,
            title: tagValue(hit.tags, "title") ?? "",
            summary: tagValue(hit.tags, "summary") ?? "",
            image: tagValue(hit.tags, "image") ?? "",
            content: hit.content,
            hashtags: hit.tags.filter { $0.first == "t" && $0.count > 1 }.map { $0[1] },
            publishedAt: publishedAt,
            createdAt: hit.createdAt
        )
    }

    private static func highlightRecord(from hit: KernelSearchHitRow) -> HighlightRecord {
        HighlightRecord(
            eventId: hit.id,
            pubkey: hit.author,
            quote: hit.content,
            context: tagValue(hit.tags, "context") ?? "",
            note: tagValue(hit.tags, "comment") ?? "",
            artifactAddress: tagValue(hit.tags, "a") ?? "",
            eventReference: tagValue(hit.tags, "e") ?? "",
            externalReference: tagValue(hit.tags, "i") ?? "",
            sourceUrl: tagValue(hit.tags, "r") ?? "",
            sourceReferenceKey: "",
            clipStartSeconds: nil,
            clipEndSeconds: nil,
            clipSpeaker: "",
            clipTranscriptSegmentIds: [],
            imageUrl: "",
            createdAt: hit.createdAt
        )
    }

    private static func profileRow(from hit: KernelSearchHitRow) -> ProfileSearchRow? {
        guard let data = hit.content.data(using: .utf8),
              let json = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else {
            return nil
        }
        func str(_ key: String) -> String {
            (json[key] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        }
        let displayName: String = {
            let dn = str("display_name")
            if !dn.isEmpty { return dn }
            let alias = str("displayName")
            if !alias.isEmpty { return alias }
            return str("displayname")
        }()
        let picture: String = {
            let p = str("picture")
            return p.isEmpty ? str("image") : p
        }()
        return ProfileSearchRow(
            pubkey: hit.author,
            name: str("name"),
            displayName: displayName,
            nip05: str("nip05"),
            picture: picture,
            about: str("about"),
            createdAt: hit.createdAt
        )
    }

    private static func tagValue(_ tags: [[String]], _ name: String) -> String? {
        tags.first { $0.first == name && $0.count > 1 }?[1]
    }
}
