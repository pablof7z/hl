import Foundation
import Observation

/// View-scoped reactive state for a single community's room home.
/// Lifetime is tied to the SwiftUI view that creates it — allocated on
/// `.task { }`, deallocated on view disappear. Owns its subscription
/// handle so granular Observation tracks only this room's data.
///
/// Data comes from nostrdb via the Rust core; this class never fabricates
/// or caches data that isn't also in nostrdb.
@MainActor
@Observable
final class RoomStore {
    private(set) var artifacts: [ArtifactRecord] = []
    private(set) var highlights: [HydratedHighlight] = []
    /// Highlights fetched per-article via `get_highlights_for_article(address:)`.
    /// Keyed on the NIP-23 address (e.g. `30023:<pubkey>:<d>`). This is the
    /// path that actually yields article highlights — the group-scoped
    /// `get_highlights(groupId:)` filters by `#h:groupId`, which kind:9802
    /// events don't carry (the community association is on kind:16 reposts,
    /// not on the highlight itself).
    private(set) var highlightsByAddress: [String: [HighlightRecord]] = [:]
    private(set) var isLoading: Bool = true
    private(set) var loadError: String?

    @ObservationIgnored private var groupId: String?
    @ObservationIgnored private var core: SafeHighlighterCore?
    @ObservationIgnored private weak var bridge: EventBridge?
    @ObservationIgnored private var subscriptionHandle: UInt64?

    /// Called from the View's `.task { }`. Reads nostrdb immediately for
    /// instant offline rendering, then installs a live subscription so
    /// incoming events flow in as deltas routed by `EventBridge`.
    func start(groupId: String, core: SafeHighlighterCore, bridge: EventBridge?) async {
        self.groupId = groupId
        self.core = core
        self.bridge = bridge
        isLoading = true
        loadError = nil

        async let artifactsFetch = core.getArtifacts(groupId: groupId)
        async let highlightsFetch = core.getHighlights(groupId: groupId)

        do {
            artifacts = try await artifactsFetch
            highlights = try await highlightsFetch
        } catch {
            // Stubs return .NotInitialized until Phase 2 #4 lands; absence
            // of cached data is the expected pre-impl state.
            loadError = (error as? CoreError).map { "\($0)" }
        }
        isLoading = false

        await refreshArticleHighlights()

        // Install the per-room pump and register this store as the delta
        // sink for its handle.
        do {
            let handle = try await core.subscribeRoom(groupId: groupId)
            subscriptionHandle = handle
            bridge?.registerRoom(self, handle: handle)
        } catch {
            // Subscription failure leaves cache-only rendering working;
            // surface nothing for now (matches the get*Empty state UX).
        }
    }

    func stop() {
        if let handle = subscriptionHandle, let core {
            Task { await core.unsubscribe(handle) }
            bridge?.unregister(handle: handle)
        }
        subscriptionHandle = nil
    }

    // MARK: - Delta application (called by EventBridge)

    func apply(artifact: ArtifactRecord) {
        if let i = artifacts.firstIndex(where: { $0.shareEventId == artifact.shareEventId }) {
            artifacts[i] = artifact
        } else {
            let inserted = artifacts + [artifact]
            artifacts = inserted.sorted { ($0.createdAt ?? 0) > ($1.createdAt ?? 0) }
        }
        Task { await self.refreshArticleHighlights(for: artifact) }
    }

    func apply(highlight: HydratedHighlight) {
        if let i = highlights.firstIndex(where: { $0.highlight.eventId == highlight.highlight.eventId }) {
            highlights[i] = highlight
        } else {
            highlights.append(highlight)
        }
        let address = highlight.highlight.artifactAddress
        if !address.isEmpty {
            var bucket = highlightsByAddress[address] ?? []
            if let i = bucket.firstIndex(where: { $0.eventId == highlight.highlight.eventId }) {
                bucket[i] = highlight.highlight
            } else {
                bucket.append(highlight.highlight)
            }
            highlightsByAddress[address] = bucket
        }
    }

    // MARK: - Article highlight fetching

    /// Parallel `get_highlights_for_article` fetch for every article-style
    /// artifact in `artifacts`. Call after an initial load and again on new
    /// artifact arrivals.
    private func refreshArticleHighlights() async {
        guard let core else { return }
        let addresses = articleAddresses(in: artifacts)
        guard !addresses.isEmpty else { return }

        await withTaskGroup(of: (String, [HighlightRecord])?.self) { group in
            for addr in addresses {
                group.addTask {
                    do {
                        let records = try await core.getHighlightsForArticle(address: addr)
                        return (addr, records)
                    } catch {
                        return nil
                    }
                }
            }
            for await result in group {
                if let (addr, records) = result {
                    highlightsByAddress[addr] = records
                }
            }
        }
    }

    /// Incremental refresh for a single artifact that just arrived via a
    /// delta. Keeps the existing bucket if the fetch fails (don't wipe).
    private func refreshArticleHighlights(for artifact: ArtifactRecord) async {
        guard let core, let addr = articleAddress(for: artifact) else { return }
        do {
            let records = try await core.getHighlightsForArticle(address: addr)
            highlightsByAddress[addr] = records
        } catch {
            // Keep whatever was there.
        }
    }

    private func articleAddresses(in artifacts: [ArtifactRecord]) -> [String] {
        var seen = Set<String>()
        var out: [String] = []
        for art in artifacts {
            guard let addr = articleAddress(for: art) else { continue }
            if seen.insert(addr).inserted { out.append(addr) }
        }
        return out
    }

    private func articleAddress(for artifact: ArtifactRecord) -> String? {
        let name = artifact.preview.referenceTagName
        let value = artifact.preview.referenceTagValue
        if name == "a", !value.isEmpty { return value }
        // Some shares carry the NIP-23 address on the highlight tag only.
        let hName = artifact.preview.highlightTagName
        let hValue = artifact.preview.highlightTagValue
        if hName == "a", !hValue.isEmpty { return hValue }
        return nil
    }
}
