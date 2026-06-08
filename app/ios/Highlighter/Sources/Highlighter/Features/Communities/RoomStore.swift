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
    /// Per-artifact highlights, keyed by `"<tagName>:<tagValue>"` (e.g.
    /// `"a:30023:pk:d"` for articles, `"i:isbn:…"` for books, `"r:<url>"`
    /// for podcasts). Populated by `get_highlights_for_reference` because
    /// the group-scoped `get_highlights(groupId:)` filters on `#h` which
    /// kind:9802 events don't carry.
    private(set) var highlightsByReference: [String: [HighlightRecord]] = [:]
    /// NIP-22 comments (kind:1111) per artifact, keyed by the UPPERCASE
    /// scope (`"A:30023:pk:d"` / `"I:isbn:…"` / `"E:<event-id>"`).
    private(set) var commentsByReference: [String: [CommentRecord]] = [:]
    private(set) var commentKeysByArtifactId: [String: String] = [:]
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

        let (artifactOutcome, highlightOutcome) = await (artifactsFetch, highlightsFetch)
        artifacts = artifactOutcome.values
        highlights = highlightOutcome.values
        if !artifactOutcome.error.isEmpty {
            loadError = artifactOutcome.error
        } else if !highlightOutcome.error.isEmpty {
            loadError = highlightOutcome.error
        }
        isLoading = false

        await refreshReferenceQueries()

        let outcome = await core.subscribeRoom(groupId: groupId)
        guard outcome.error.isEmpty else {
            // Subscription failure leaves cache-only rendering working.
            return
        }
        subscriptionHandle = outcome.handle
        bridge?.registerRoom(self, handle: outcome.handle)
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
        Task { await self.refreshReferenceQueries(for: artifact) }
    }

    func apply(highlight: HydratedHighlight) {
        if let i = highlights.firstIndex(where: { $0.highlight.eventId == highlight.highlight.eventId }) {
            highlights[i] = highlight
        } else {
            highlights.append(highlight)
        }
        // Merge into the reference-scoped bucket too so per-artifact lanes
        // reflect live arrivals without waiting for the next refresh.
        if let target = core?.getHighlightReferenceTarget(highlight: highlight.highlight) {
            let key = target.lookupKey
            var bucket = highlightsByReference[key] ?? []
            if let i = bucket.firstIndex(where: { $0.eventId == highlight.highlight.eventId }) {
                bucket[i] = highlight.highlight
            } else {
                bucket.append(highlight.highlight)
            }
            bucket.sort { ($0.createdAt ?? 0) > ($1.createdAt ?? 0) }
            highlightsByReference[key] = bucket
        }
    }

    // MARK: - Reference queries

    /// Runs `get_highlights_for_reference` + `get_comments_for_reference`
    /// for every artifact in `artifacts`. Each artifact dispatches both
    /// fetches in parallel; failures keep whatever was previously there.
    private func refreshReferenceQueries() async {
        guard let core else { return }
        let targets: [ArtifactReferenceTarget] = artifacts.compactMap {
            core.getArtifactReferenceTarget(artifact: $0)
        }
        guard !targets.isEmpty else { return }
        commentKeysByArtifactId = Dictionary(
            uniqueKeysWithValues: targets.compactMap { target in
                guard !target.commentKey.isEmpty else { return nil }
                return (target.artifactId, target.commentKey)
            }
        )

        struct FetchResult {
            let target: ArtifactReferenceTarget
            let highlights: [HighlightRecord]?
            let comments: [CommentRecord]?
        }

        await withTaskGroup(of: FetchResult.self) { group in
            for target in targets {
                group.addTask {
                    let highlightOutcome = await core.getHighlightsForReference(
                        tagName: target.lowercaseTag,
                        tagValue: target.value
                    )
                    let commentOutcome: CommentListOutcome?
                    if let scope = target.commentScope {
                        commentOutcome = await core.getCommentsForScope(scope: scope, limit: 128)
                    } else {
                        commentOutcome = nil
                    }
                    return FetchResult(
                        target: target,
                        highlights: highlightOutcome.error.isEmpty ? highlightOutcome.values : nil,
                        comments: commentOutcome?.error.isEmpty == true ? commentOutcome?.values : nil
                    )
                }
            }
            for await result in group {
                let t = result.target
                if let hl = result.highlights {
                    highlightsByReference[t.lookupKey] = hl
                }
                if let cm = result.comments, !t.commentKey.isEmpty {
                    commentsByReference[t.commentKey] = cm
                }
            }
        }
    }

    private func refreshReferenceQueries(for artifact: ArtifactRecord) async {
        guard let core, let target = core.getArtifactReferenceTarget(artifact: artifact) else { return }
        if !target.commentKey.isEmpty {
            commentKeysByArtifactId[target.artifactId] = target.commentKey
        } else {
            commentKeysByArtifactId.removeValue(forKey: target.artifactId)
        }
        let highlightOutcome = await core.getHighlightsForReference(
            tagName: target.lowercaseTag,
            tagValue: target.value
        )
        if highlightOutcome.error.isEmpty {
            highlightsByReference[target.lookupKey] = highlightOutcome.values
        }
        if let scope = target.commentScope, !target.commentKey.isEmpty {
            let commentOutcome = await core.getCommentsForScope(scope: scope, limit: 128)
            if commentOutcome.error.isEmpty {
                commentsByReference[target.commentKey] = commentOutcome.values
            }
        }
    }

    func commentCount(for artifact: ArtifactRecord) -> Int {
        guard let core,
              let target = core.getArtifactReferenceTarget(artifact: artifact),
              !target.commentKey.isEmpty else {
            return 0
        }
        return commentsByReference[target.commentKey]?.count ?? 0
    }
}
