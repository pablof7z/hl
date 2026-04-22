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
    }

    func apply(highlight: HydratedHighlight) {
        if let i = highlights.firstIndex(where: { $0.highlight.eventId == highlight.highlight.eventId }) {
            highlights[i] = highlight
        } else {
            highlights.append(highlight)
        }
    }
}
