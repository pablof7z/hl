uniffi::setup_scaffolding!();

// ── New nmp-lane kernel (Phase 1) ────────────────────────────────────────────
pub mod capabilities;
pub mod ffi;
pub mod kernel;

pub use capabilities::{CapabilityRequest, CapabilityResult, KeychainOp, KeychainResult};
pub use ffi::HighlighterApp;
pub use kernel::{
    AppAction, AppConfig, AppRootSnapshot, HighlighterObserver, ManualClock, NmpKeyringHandler,
    RootShellSnapshot, RootTab, RouteKind, SystemClock, ToastSnapshot, ViewId, ViewRoute,
    ViewSnapshot,
};
// ─────────────────────────────────────────────────────────────────────────────

pub mod artifacts;
pub mod client;
pub mod clock;
pub mod errors;
pub mod events;
pub mod feedback;
pub mod highlights;
pub mod models;
pub mod nostr_runtime;
pub mod ocr;
pub mod onboarding;

pub mod podcast_playback;
pub mod podcast_position;
pub mod podcast_transcript;
pub mod profile;
pub mod relays;
pub mod share_links;
#[cfg(test)]
pub mod test_ndb;
pub mod time_labels;
pub mod waveform;

pub use artifacts::ArtifactPublishSnapshot;
pub use client::HighlighterCore;
pub use errors::CoreError;
pub use events::{
    DataChangeType, Delta, EventCallback, NostrContentRun, NostrEntityEvent,
    NostrEntityInlineRender, NostrEntityRef, NostrEntityRefSnapshot, NostrEntityRenderKind,
    NostrEntityResolutionSnapshot,
};
pub use highlights::{
    ArticleHighlightPublishProjection, ArticleHighlightPublishProjectionInput,
    ArticleReaderSelectionProjection, ArticleReaderSelectionProjectionInput,
    HighlightDetailContentProjection, HighlightDetailContentProjectionInput,
    HighlightDetailResourceProjection, HighlightDetailResourceProjectionInput,
    HighlightFeedContentProjection, HighlightFeedContentProjectionInput,
    HighlightGroupCardProjection, HighlightGroupCardProjectionInput,
    HighlightGroupHighlighterProfile, HighlightGroupHighlighterProjection,
    HighlightGroupLabelSegment, HighlightResourceAuthorProfile, HighlightResourceHeaderProjection,
    HighlightResourceHeaderProjectionInput, HighlightShareUrlSnapshot,
};
pub use models::{
    AppSubscriptionStartProjection, AppSubscriptionStartProjectionInput, ArticleReaderRoute,
    ArticleRecord, ArtifactDetailRoute, ArtifactDetailTarget, ArtifactPreview, ArtifactRecord,
    ArtifactReferenceTarget, BlossomUpload, BookRoute, BookmarkSetRecord, ChatMessageRecord,
    CommentRecord, CommentReferenceBucket, CommentThreadNode, CommentThreadProjection,
    CommunitySummary, CurationMenuItem, CurrentUser, DiscussionAttachment, DiscussionRecord,
    FeedbackEventRecord, FeedbackThreadRecord, GeneratedAccount, HighlightRecord,
    HighlightReferenceBucket, HighlightReferenceTarget, HighlightSourceKind, HomeFeedItem,
    HydratedHighlight, LoginInputAction, MutationSnapshot, OnboardingInterest,
    OnboardingInterestChip, OnboardingInterestProjection, OnboardingInterestSelection,
    ProfileMetadata, ProfileUpdateDraft, ReadingFeedItem, RoomLane, RoomRecommendation,
    RoomRecommendationReason, SubscriptionStartSnapshot, ViewSubscriptionStartProjection,
    ViewSubscriptionStartProjectionInput, WebBookmarkRecord,
};
pub use ocr::{OcrLine, OcrPageDetection, OcrPageSide, OcrRect, OcrWord};
pub use podcast_playback::{
    PodcastPlaybackPositionInput, PodcastPlaybackRehydrationSnapshot, PodcastPlaybackSeekInput,
    PodcastPlaybackSeekProjection, PodcastPlaybackSessionApplyInput,
    PodcastPlaybackSessionApplyProjection, PodcastPlaybackSessionInput, PodcastPlaybackSessionPlan,
    PodcastPlaybackTickInput, PodcastPlaybackTickProjection,
};
pub use podcast_transcript::{
    PodcastClipComposerInput, PodcastClipComposerProjection, PodcastClipComposerPublishInput,
    PodcastClipPublishInput, PodcastClipPublishResultInput, PodcastClipPublishResultProjection,
    PodcastClipPublishSnapshot, PodcastClipSelection, PodcastListeningClipsSnapshot,
    PodcastListeningProjection, PodcastListeningProjectionInput, PodcastNowPlayingProjection,
    PodcastNowPlayingProjectionInput, PodcastTimelineRow, PodcastTimelineRowKind,
    PodcastTimelineRowState, PodcastTranscriptAvailability, PodcastTranscriptLoadApplyInput,
    PodcastTranscriptLoadApplyProjection, PodcastTranscriptLoadSnapshot, TranscriptSegment,
};
pub use profile::{
    ProfileDisplayFallback, ProfileDisplayProjection, ProfileDisplayProjectionInput,
    ProfileDisplayWithLabelProjectionInput, ProfileFollowActionInput,
    ProfileFollowActionProjection, ProfileFollowMutationApplyInput,
    ProfileFollowMutationApplyProjection, ProfileFollowMutationInput,
    ProfileFollowMutationSnapshot, ProfileIdentityProjection, ProfileIdentityProjectionInput,
    ProfileRelationshipProjection, ProfileRelationshipProjectionInput, ProfileUpdateProjection,
    ProfileUpdateProjectionInput, ProfileUpdateResultInput, ProfileUpdateResultProjection,
    ProfileUpdateSnapshot,
};
pub use relays::{
    AddRelayProbeStatus, AddRelaySheetProjection, AddRelaySheetProjectionInput, ImportRelayRow,
    ImportRelaysFetchApplyInput, ImportRelaysFetchApplyProjection, ImportRelaysFetchSnapshot,
    ImportRelaysProjection, ImportRelaysProjectionInput, ImportRelaysSourceProjection,
    ImportRelaysSourceProjectionInput, NetworkCacheStatsSnapshot, NetworkDiagnosticsSnapshot,
    NetworkDiagnosticsSnapshotApplyInput, NetworkDiagnosticsSnapshotApplyProjection,
    NetworkPathPolicySnapshot, NetworkRelayConnectionPolicyAction,
    NetworkSettingsMutationApplyInput, NetworkSettingsMutationApplyProjection,
    NetworkSettingsMutationSnapshot, NetworkSettingsSnapshot, NetworkSettingsSnapshotApplyInput,
    NetworkSettingsSnapshotApplyProjection, NetworkWifiOnlyPreferenceApplyInput,
    NetworkWifiOnlyPreferenceApplyProjection, NetworkWifiOnlyPreferenceSnapshot,
    RelayAvatarProjection, RelayConfig, RelayDetailProjection, RelayDetailProjectionInput,
    RelayHostedRoomsApplyInput, RelayHostedRoomsApplyProjection, RelayHostedRoomsSnapshot,
    RelayNip11ProbeSnapshot, RelayRemoveProjection, RelayRemoveProjectionInput, RelayRowProjection,
    RelayRowProjectionInput, RelaySettingsProjection, RelayStatusTone,
};
pub use share_links::ArticleShareUrlSnapshot;
pub use time_labels::{
    RelativeTimeLabelInput, RelativeTimeLabelProjection, RelativeTimeLabelStyle,
};
pub use waveform::{
    WaveformCacheKeyProjection, WaveformCacheKeyProjectionInput, WaveformPeaksPlan,
    WaveformPeaksPlanInput, WaveformWifiStatus,
};
