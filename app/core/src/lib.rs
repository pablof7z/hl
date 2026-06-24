uniffi::setup_scaffolding!();

// ── New nmp-lane kernel (Phase 1) ────────────────────────────────────────────
pub mod capabilities;
pub mod ffi;
pub mod kernel;

pub use capabilities::{CapabilityRequest, CapabilityResult, KeychainOp, KeychainResult};
pub use ffi::HighlighterApp;
pub use kernel::{
    AppAction, AppConfig, AppRootSnapshot, HighlighterObserver, ManualClock, RootShellSnapshot,
    RootTab, RouteKind, SystemClock, ToastSnapshot, ViewId, ViewRoute, ViewSnapshot,
};
// ─────────────────────────────────────────────────────────────────────────────

pub mod article_reader;
pub mod articles;
pub mod artifacts;
pub mod blossom;
pub mod book_detail;
pub mod bookmarks;
pub mod capture;
pub mod chat;
pub mod client;
pub mod clock;
pub mod comments;
pub mod curation;
pub mod discussions;
pub mod errors;
pub mod events;
pub mod feedback;
pub mod follows;
pub mod groups;
pub mod highlights;
pub mod isbn_lookup;
pub mod lists;
pub mod models;
pub mod nip46;
pub mod nostr_entities;
pub mod nostr_runtime;
pub mod ocr;
pub mod onboarding;
pub mod outbox;
pub mod pictures;
pub mod podcast_playback;
pub mod podcast_position;
pub mod podcast_transcript;
pub mod profile;
pub mod reactions;
pub mod reads;
pub mod recent_books;
pub mod relays;
pub mod room_invites;
pub mod search;
pub mod session;
pub mod share_extension;
pub mod share_links;
pub mod share_targets;
pub mod subscriptions;
#[cfg(test)]
pub mod test_ndb;
pub mod time_labels;
pub mod waveform;
pub mod web_metadata;

pub use article_reader::{
    ArticleReaderHighlightPublishSnapshot, ArticleReaderPublishResultInput,
    ArticleReaderPublishResultProjection, ArticleReaderSnapshot, ArticleReaderSnapshotApplyInput,
    ArticleReaderSnapshotProjection,
};
pub use articles::{
    ArticleProfileCardProjection, ArticleProfileCardProjectionInput, ArticleReaderHeaderProjection,
    ArticleReaderHeaderProjectionInput,
};
pub use artifacts::ArtifactPublishSnapshot;
pub use blossom::{
    BlossomServerEntryProjection, BlossomServerEntryProjectionInput, BlossomServerListProjection,
    BlossomServerListProjectionInput, BlossomServerSettingsMutationSnapshot,
    BlossomServerSettingsSnapshot, BlossomUploadSnapshot,
};
pub use book_detail::{
    BookDetailSnapshot, BookDetailSnapshotApplyInput, BookDetailSnapshotApplyProjection,
};
pub use bookmarks::{
    ArticleBookmarkChromeProjection, ArticleBookmarkChromeProjectionInput,
    ArticleBookmarkStateProjection, ArticleBookmarkStateProjectionInput, ArticleBookmarksSnapshot,
    ArticleBookmarksSnapshotApplyInput, ArticleBookmarksSnapshotApplyProjection,
};
pub use capture::{
    CaptureBookDisplayProjection, CaptureBookDisplayProjectionInput,
    CaptureCommunitySelectionProjection, CaptureCommunitySelectionProjectionInput,
    CapturePublishInput, CapturePublishPhase, CapturePublishProjection,
    CapturePublishProjectionInput, CapturePublishResultProjection,
    CapturePublishResultProjectionInput, CapturePublishSnapshot, CaptureStashProjection,
    CaptureStashProjectionInput, CaptureUploadProjection, CaptureUploadProjectionInput,
};
pub use client::HighlighterCore;
pub use comments::{
    CommentActionChromeProjection, CommentActionChromeProjectionInput, CommentComposerProjection,
    CommentComposerProjectionInput, CommentInlineThreadSnapshotApplyInput,
    CommentInlineThreadSnapshotApplyProjection, CommentInteractionMutationSnapshot,
    CommentInteractionRow, CommentInteractionSnapshot, CommentNodeChromeProjection,
    CommentNodeChromeProjectionInput, CommentPublishResultInput, CommentPublishResultProjection,
    CommentPublishSnapshot, CommentScopeSnapshot, CommentSnapshotApplyInput,
    CommentSnapshotApplyProjection, CommentThreadSnapshot, CommentThreadViewProjection,
    CommentThreadViewProjectionInput, CommentToolbarProjection, CommentToolbarProjectionInput,
};
pub use errors::CoreError;
pub use events::{DataChangeType, Delta, EventCallback};
pub use groups::{
    CommunityRowProjection, CommunityRowProjectionInput, CreateRoomCoverUploadResultInput,
    CreateRoomCoverUploadResultProjection, CreateRoomProjection, CreateRoomProjectionInput,
    CreateRoomPublishResultInput, CreateRoomPublishResultProjection, CreateRoomPublishSnapshot,
    CreateRoomVisibilityOption, JoinRoomRequestSnapshot, JoinedCommunitiesSnapshot,
    JoinedCommunitiesSnapshotApplyInput, JoinedCommunitiesSnapshotApplyProjection, RoomAccess,
    RoomAvatarProjection, RoomAvatarProjectionInput, RoomCoverCardProjection,
    RoomCoverCardProjectionInput, RoomVisibility,
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
pub use isbn_lookup::{
    BookPickerQueryProjection, BookPickerQueryProjectionInput, BookPickerSnapshot,
    EditedBookPreviewProjection, IsbnManualPreviewProjection, IsbnManualPreviewProjectionInput,
    IsbnPreviewLookupApplyInput, IsbnPreviewLookupApplyProjection, IsbnPreviewLookupSnapshot,
    IsbnPreviewRequestProjection, IsbnPreviewRequestProjectionInput,
};
pub use lists::{
    BookmarkLibraryFilter, BookmarkLibraryFilterChipProjection, BookmarkLibraryPane,
    BookmarkLibraryProjection, BookmarkLibraryProjectionInput, BookmarkLibraryScope,
    BookmarkLibraryScopeOptionProjection, BookmarkLibrarySnapshot, BookmarkSetDetailSnapshot,
    BookmarkSetRowProjection, BookmarkSetRowProjectionInput, BookmarkedArticleRowProjection,
    BookmarkedArticleRowProjectionInput, CurationMenuSnapshot, CurationMenuSnapshotApplyInput,
    CurationMenuSnapshotApplyProjection, CurationSetCreateProjection,
    CurationSetCreateProjectionInput, WebBookmarkRowProjection, WebBookmarkRowProjectionInput,
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
pub use nip46::NostrConnectStartSnapshot;
pub use nostr_entities::{
    NostrEntityArticleCardProjection, NostrEntityArticleCardProjectionInput,
    NostrEntityRefSnapshot, NostrEntityResolutionSnapshot,
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
    ProfileImageUploadResultInput, ProfileImageUploadResultProjection,
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
pub use room_invites::{
    RoomInviteAvatarProjection, RoomInviteAvatarProjectionInput, RoomInviteCandidate,
    RoomInviteCandidateSource, RoomInviteChip, RoomInviteInputFormat, RoomInviteProjection,
    RoomInviteResolvedCandidate, RoomInviteSelectionAction, RoomInviteSelectionChromeInput,
    RoomInviteSelectionChromeProjection, RoomInviteSelectionInput, RoomInviteSelectionProjection,
    RoomInviteSendResultProjection, RoomInviteSnapshot, RoomInviteSnapshotInput,
    RoomInviteSuggestion,
};
pub use search::{
    SearchArticleResultsSnapshot, SearchChromeSnapshot, SearchCommunityRowProjection,
    SearchCommunityRowProjectionInput, SearchHighlightRowProjection,
    SearchHighlightRowProjectionInput, SearchQueryProjection, SearchQueryProjectionInput,
    SearchRelayArticlesApplyInput, SearchRelayArticlesApplyProjection, SearchRelayRefreshInput,
    SearchRelayRefreshProjection, SearchRelayStartResultInput, SearchRelayStartResultProjection,
    SearchRelayUpdateInput, SearchRelayUpdateProjection, SearchResultsApplyInput,
    SearchResultsApplyProjection, SearchResultsSnapshot, SearchScheduleInput,
    SearchScheduleProjection, SearchSuggestionsProjection, SearchSuggestionsProjectionInput,
    SearchTextMatchSpan, SearchTextMatchesProjection, SearchTextMatchesProjectionInput,
};
pub use session::{
    AccountGenerationSnapshot, AuthSessionRestoreSnapshot, AuthSessionSnapshot,
    PublicKeyDisplayProjection, PublicKeyDisplayProjectionInput, SecretKeyDisplayProjection,
    SecretKeyDisplayProjectionInput, SecretKeySettingsSnapshot, SessionStorageWriteInput,
    SessionStorageWriteSnapshot,
};
pub use share_extension::{
    ShareQueueAttempt, ShareQueueDrainProjection, ShareQueueDrainProjectionInput, ShareQueueItem,
};
pub use share_links::ArticleShareUrlSnapshot;
pub use share_targets::{
    ShareArticleTargetProjectionInput, ShareArtifactTargetProjection,
    ShareArtifactTargetProjectionInput, ShareHighlightArticleTargetProjectionInput,
    ShareHighlightTargetProjection, ShareHighlightTargetProjectionInput,
    ShareToCommunityPublishResultInput, ShareToCommunityPublishResultProjection,
    ShareWebReaderTargetProjectionInput, ShareWebReaderTargetSnapshot,
};
pub use time_labels::{
    RelativeTimeLabelInput, RelativeTimeLabelProjection, RelativeTimeLabelStyle,
};
pub use waveform::{
    WaveformCacheKeyProjection, WaveformCacheKeyProjectionInput, WaveformPeaksPlan,
    WaveformPeaksPlanInput, WaveformWifiStatus,
};
pub use web_metadata::{
    WebMetadata, WebMetadataRequestProjection, WebMetadataRequestProjectionInput,
};
