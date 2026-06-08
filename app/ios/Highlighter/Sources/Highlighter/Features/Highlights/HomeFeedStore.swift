import Foundation
import Observation

/// Home feed — composes friend highlights and friend-surfaced reads into
/// Rust-owned screen rows. This store owns child-store lifetimes and
/// observation; Rust owns grouping, dedupe, stable ids, and ordering.
@MainActor
@Observable
final class HomeFeedStore {
    typealias Item = HomeFeedItem

    var items: [Item] = []
    var isLoadingInitial: Bool = true

    @ObservationIgnored private let core: SafeHighlighterCore
    @ObservationIgnored let highlights: HighlightsStore
    @ObservationIgnored let reads: ReadsStore

    @ObservationIgnored private var observing: Bool = false

    init(safeCore: SafeHighlighterCore, eventBridge: EventBridge?) {
        self.core = safeCore
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
        items = core.buildHomeFeedItems(
            highlights: highlights.items,
            reads: reads.items
        )
    }
}
