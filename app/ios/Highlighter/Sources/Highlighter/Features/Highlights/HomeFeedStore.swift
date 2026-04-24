import Foundation
import Observation

/// Home feed — the merge of friend highlights and friend-surfaced reads into
/// a single chronological stream. Composes `HighlightsStore` and `ReadsStore`
/// (owning both for the lifetime of the view) and recomputes a deduped,
/// sorted `items` array whenever either side changes.
///
/// Dedup rule: if a friend highlighted an article, that article is dropped
/// from the reads side — the highlight row already represents the piece
/// (and more richly, with a friend's voice). An article only ever gets the
/// bare article-card treatment when no friend has highlighted it.
@MainActor
@Observable
final class HomeFeedStore {
    enum Item: Hashable {
        case highlight(HydratedHighlight)
        case read(ReadingFeedItem)

        var sortKey: UInt64 {
            switch self {
            case .highlight(let h): return h.highlight.createdAt ?? 0
            case .read(let r): return r.latestActivityAt
            }
        }

        var stableId: String {
            switch self {
            case .highlight(let h): return "h:\(h.highlight.eventId)"
            case .read(let r):
                return "r:30023:\(r.article.pubkey):\(r.article.identifier)"
            }
        }
    }

    var items: [Item] = []
    var isLoadingInitial: Bool = true

    @ObservationIgnored let highlights: HighlightsStore
    @ObservationIgnored let reads: ReadsStore

    @ObservationIgnored private var observing: Bool = false

    init(safeCore: SafeHighlighterCore, eventBridge: EventBridge?) {
        self.highlights = HighlightsStore(safeCore: safeCore, eventBridge: eventBridge)
        self.reads = ReadsStore(safeCore: safeCore, eventBridge: eventBridge)
    }

    func start() async {
        async let h: Void = highlights.start()
        async let r: Void = reads.start()
        _ = await (h, r)
        recompute()
        isLoadingInitial = false
        observing = true
        observeHighlights()
        observeReads()
    }

    func stop() {
        observing = false
        highlights.stop()
        reads.stop()
    }

    private func observeHighlights() {
        withObservationTracking {
            _ = highlights.items
        } onChange: { [weak self] in
            Task { @MainActor in
                guard let self, self.observing else { return }
                self.recompute()
                self.observeHighlights()
            }
        }
    }

    private func observeReads() {
        withObservationTracking {
            _ = reads.items
        } onChange: { [weak self] in
            Task { @MainActor in
                guard let self, self.observing else { return }
                self.recompute()
                self.observeReads()
            }
        }
    }

    private func recompute() {
        let highlightedAddresses: Set<String> = Set(
            highlights.items.compactMap { hydrated in
                let addr = hydrated.highlight.artifactAddress
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                return addr.isEmpty ? nil : addr
            }
        )

        var merged: [Item] = []
        merged.reserveCapacity(highlights.items.count + reads.items.count)
        for h in highlights.items {
            merged.append(.highlight(h))
        }
        for r in reads.items {
            let addr = "30023:\(r.article.pubkey):\(r.article.identifier)"
            if highlightedAddresses.contains(addr) { continue }
            merged.append(.read(r))
        }
        merged.sort { $0.sortKey > $1.sortKey }
        items = merged
    }
}
