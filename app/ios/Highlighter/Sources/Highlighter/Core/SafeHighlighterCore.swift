import Foundation

/// Actor-isolated wrapper around the UniFFI-generated `HighlighterCore` so
/// Swift call sites get serialized access without worrying about FFI thread
/// safety. Mirrors TENEX's `SafeTenexCore`.
actor SafeHighlighterCore {
    private let core: HighlighterCore

    init(core: HighlighterCore) {
        self.core = core
    }

    // MARK: - Auth (read-only helpers; sign-in/restore owned by kernel)

    func completeOnboardingInterests(selectedIds: [String]) async -> MutationSnapshot {
        await core.completeOnboardingInterests(selectedIds: selectedIds)
    }

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

    nonisolated func defaultHighlightCropBox(
        highlightBoxes: [OcrRect],
        imageWidth: Double,
        imageHeight: Double,
        marginFraction: Double
    ) -> OcrRect? {
        core.defaultHighlightCropBox(
            highlightBoxes: highlightBoxes,
            imageWidth: imageWidth,
            imageHeight: imageHeight,
            marginFraction: marginFraction
        )
    }

    nonisolated func sanitizeHighlightCropBox(_ cropBox: OcrRect, fallback: OcrRect?) -> OcrRect {
        core.sanitizeHighlightCropBox(cropBox: cropBox, fallback: fallback)
    }

    // MARK: - Profile reads

    nonisolated func decodeNostrEntity(_ input: String) -> NostrEntityRefSnapshot {
        core.decodeNostrEntity(input: input)
    }

    func updateProfile(draft: ProfileUpdateDraft) async -> ProfileUpdateSnapshot {
        await core.updateProfile(draft: draft)
    }

    func updateProfile(
        name: String,
        displayName: String,
        about: String,
        picture: String,
        banner: String,
        nip05: String,
        website: String,
        lud16: String
    ) async -> ProfileUpdateSnapshot {
        let draft = ProfileUpdateDraft(
            name: name,
            displayName: displayName,
            about: about,
            picture: picture,
            banner: banner,
            nip05: nip05,
            website: website,
            lud16: lud16
        )
        return await updateProfile(draft: draft)
    }

    // MARK: - Rooms explorer

    func startRoomDiscovery() async {
        await core.startRoomDiscovery()
    }

    func createRoom(
        name: String,
        about: String,
        picture: String,
        visibility: RoomVisibility,
        access: RoomAccess
    ) async -> CreateRoomPublishSnapshot {
        await core.createRoom(
            name: name,
            about: about,
            picture: picture,
            visibility: visibility,
            access: access
        )
    }

    // MARK: - Writes

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

    // MARK: - Relay config

    func removeRelay(_ url: String) async -> NetworkSettingsMutationSnapshot {
        await core.removeRelay(url: url)
    }

    // MARK: - Relay telemetry (PR 4)

    func reconnectAll() async -> NetworkSettingsMutationSnapshot {
        await core.reconnectAll()
    }

}
