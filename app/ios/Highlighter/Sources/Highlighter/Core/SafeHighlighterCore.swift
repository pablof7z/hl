import Foundation

/// Actor-isolated wrapper around the UniFFI-generated `HighlighterCore` so
/// Swift call sites get serialized access without worrying about FFI thread
/// safety. Mirrors TENEX's `SafeTenexCore`.
///
/// Remaining surface: podcast-domain only (hl product — not ported to NMP/kernel).
/// All other calls have been migrated to kernel actions or uniffi free functions.
actor SafeHighlighterCore {
    private let core: HighlighterCore

    init(core: HighlighterCore) {
        self.core = core
    }

    // MARK: - Podcast (hl product domain — permanent bespoke lane)

    nonisolated func planPodcastPlaybackSession(
        input: PodcastPlaybackSessionInput
    ) -> PodcastPlaybackSessionPlan {
        core.planPodcastPlaybackSession(input: input)
    }

    func recordPodcastPlaybackPosition(
        artifact: ArtifactRecord,
        positionSeconds: Double
    ) -> MutationSnapshot {
        core.recordPodcastPlaybackPosition(
            input: PodcastPlaybackPositionInput(
                artifact: artifact,
                positionSeconds: positionSeconds
            )
        )
    }

    func getPodcastPlaybackRehydrationSnapshot(
        hasCurrentArtifact: Bool
    ) -> PodcastPlaybackRehydrationSnapshot {
        core.getPodcastPlaybackRehydrationSnapshot(hasCurrentArtifact: hasCurrentArtifact)
    }

    func getPodcastListeningClipsSnapshot(
        artifact: ArtifactRecord?,
        limit: UInt32 = 128
    ) async -> PodcastListeningClipsSnapshot {
        await core.getPodcastListeningClipsSnapshot(artifact: artifact, limit: limit)
    }

    func publishPodcastClipHighlight(
        input: PodcastClipPublishInput
    ) async -> PodcastClipPublishSnapshot {
        await core.publishPodcastClipHighlight(input: input)
    }

    func publishPodcastComposerClip(
        input: PodcastClipComposerPublishInput
    ) async -> PodcastClipPublishSnapshot {
        await core.publishPodcastComposerClip(input: input)
    }

    // MARK: - Relay config (bespoke nostrdb + network fetch paths)
    // Relay config is sourced from kind:10002 + kind:30078 until the kernel
    // exposes relay management directly.

    func queryUserRelayConfigs(pubkeyHex: String) -> [RelayConfig] {
        core.queryUserRelayConfigs(pubkeyHex: pubkeyHex)
    }

    /// Fetch another user's kind:10002 relay list from the indexer relay pool.
    /// Used by the "Import from npub" flow in Network Settings.
    func fetchRelaysForPubkey(pubkeyHex: String) async -> [RelayConfig] {
        await core.fetchRelaysForPubkey(pubkeyHex: pubkeyHex)
    }

}
