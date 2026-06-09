uniffi::setup_scaffolding!();

pub mod article_reader;
pub mod articles;
pub mod artifact_detail;
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
pub mod discovery;
pub mod discussions;
pub mod errors;
pub mod events;
pub mod feedback;
pub mod follows;
pub mod groups;
pub mod highlights;
pub mod home_feed;
pub mod isbn_lookup;
pub mod lists;
pub mod models;
pub mod network_preferences;
pub mod nip05;
pub mod nip46;
pub mod nostr_entities;
pub mod nostr_runtime;
pub mod ocr;
pub mod onboarding;
pub mod outbox;
pub mod pictures;
pub mod podcast_position;
pub mod podcast_transcript;
pub mod profile;
pub mod profile_page;
pub mod reactions;
pub mod reads;
pub mod recent_books;
pub mod recent_searches;
pub mod recommendations;
pub mod reference_targets;
pub mod relay_polish;
pub mod relays;
pub mod room_explorer;
pub mod room_explorer_config;
pub mod room_home;
pub mod room_invites;
pub mod room_lanes;
pub mod room_library;
pub mod room_preview;
pub mod room_state;
pub mod search;
pub mod session;
pub mod share_extension;
pub mod share_targets;
pub mod subscriptions;
pub mod time_labels;
pub mod web_metadata;
pub mod whats_new;

pub use article_reader::{ArticleReaderHighlightPublishSnapshotOutcome, ArticleReaderSnapshot};
pub use articles::{
    ArticleProfileCardProjection, ArticleProfileCardProjectionInput, ArticleReaderHeaderProjection,
    ArticleReaderHeaderProjectionInput,
};
pub use artifact_detail::ArtifactDetailProjection;
pub use artifacts::ArtifactPublishSnapshot;
pub use blossom::{
    BlossomServerEntryProjection, BlossomServerEntryProjectionInput, BlossomServerListProjection,
    BlossomServerListProjectionInput, BlossomServerSettingsMutationSnapshot,
    BlossomServerSettingsSnapshot, BlossomUploadSnapshot,
};
pub use book_detail::BookDetailSnapshot;
pub use bookmarks::{
    ArticleBookmarkChromeProjection, ArticleBookmarkChromeProjectionInput,
    ArticleBookmarkStateProjection, ArticleBookmarkStateProjectionInput, ArticleBookmarksSnapshot,
};
pub use capture::{
    CaptureBookDisplayProjection, CaptureBookDisplayProjectionInput,
    CaptureCommunitySelectionProjection, CaptureCommunitySelectionProjectionInput,
    CapturePublishInput, CapturePublishPhase, CapturePublishProjection,
    CapturePublishProjectionInput, CapturePublishSnapshot, CaptureStashProjection,
    CaptureStashProjectionInput,
};
pub use chat::{
    ChatComposerProjection, ChatComposerProjectionInput, ChatMessageRowProjection,
    ChatPresenceSnapshot, ChatPublishSnapshot, ChatSnapshot,
};
pub use client::HighlighterCore;
pub use comments::{
    CommentActionChromeProjection, CommentActionChromeProjectionInput, CommentComposerProjection,
    CommentComposerProjectionInput, CommentInteractionMutationOutcome, CommentInteractionRow,
    CommentInteractionSnapshot, CommentNodeChromeProjection, CommentNodeChromeProjectionInput,
    CommentPublishSnapshotOutcome, CommentScopeSnapshot, CommentThreadSnapshot,
    CommentThreadViewProjection, CommentThreadViewProjectionInput, CommentToolbarProjection,
    CommentToolbarProjectionInput,
};
pub use discussions::{
    DiscussionAttachmentProjection, DiscussionAttachmentProjectionInput,
    DiscussionComposerProjection, DiscussionComposerProjectionInput,
    DiscussionComposerPublishInput, DiscussionPublishSnapshot, RoomDiscussionSnapshot,
};
pub use errors::CoreError;
pub use events::{DataChangeType, Delta, EventCallback};
pub use feedback::{
    FeedbackComposerProjection, FeedbackComposerProjectionInput, FeedbackMessagePresentationInput,
    FeedbackMessagePresentationProjection, FeedbackMessageRowProjection,
    FeedbackReplyPublishSnapshot, FeedbackRootPublishSnapshot,
    FeedbackThreadPresentationProjection, FeedbackThreadSnapshot, FeedbackThreadsSnapshot,
};
pub use groups::{
    CommunityRowProjection, CommunityRowProjectionInput, CreateRoomProjection,
    CreateRoomProjectionInput, CreateRoomPublishSnapshot, CreateRoomVisibilityOption,
    JoinRoomRequestSnapshot, JoinedCommunitiesSnapshot, RoomAccess, RoomAvatarProjection,
    RoomAvatarProjectionInput, RoomCoverCardProjection, RoomCoverCardProjectionInput,
    RoomVisibility,
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
pub use home_feed::HomeFeedSnapshot;
pub use isbn_lookup::{
    BookPickerQueryProjection, BookPickerQueryProjectionInput, BookPickerSnapshot,
    EditedBookPreviewProjection, IsbnManualPreviewProjection, IsbnManualPreviewProjectionInput,
    IsbnPreviewLookupSnapshot, IsbnPreviewRequestProjection, IsbnPreviewRequestProjectionInput,
};
pub use lists::{
    BookmarkLibraryFilter, BookmarkLibraryFilterChipProjection, BookmarkLibraryPane,
    BookmarkLibraryProjection, BookmarkLibraryProjectionInput, BookmarkLibraryScope,
    BookmarkLibraryScopeOptionProjection, BookmarkLibrarySnapshot, BookmarkSetDetailSnapshot,
    BookmarkSetRowProjection, BookmarkSetRowProjectionInput, BookmarkedArticleRowProjection,
    BookmarkedArticleRowProjectionInput, CurationMenuSnapshot, CurationSetCreateProjection,
    CurationSetCreateProjectionInput, WebBookmarkRowProjection, WebBookmarkRowProjectionInput,
};
pub use models::{
    ArticleReaderRoute, ArticleRecord, ArtifactDetailRoute, ArtifactDetailTarget, ArtifactPreview,
    ArtifactRecord, ArtifactReferenceTarget, BlossomUpload, BookRoute, BookmarkSetRecord,
    ChatMessageRecord, CommentRecord, CommentReferenceBucket, CommentThreadNode,
    CommentThreadProjection, CommunitySummary, CurationMenuItem, CurrentUser, DiscussionAttachment,
    DiscussionRecord, FeedbackEventRecord, FeedbackThreadRecord, GeneratedAccount, HighlightRecord,
    HighlightReferenceBucket, HighlightReferenceTarget, HighlightSourceKind, HomeFeedItem,
    HydratedHighlight, LoginInputAction, MutationOutcome, OnboardingInterest,
    OnboardingInterestChip, OnboardingInterestProjection, OnboardingInterestSelection,
    PodcastPositionRecord, ProfileMetadata, ProfileUpdateDraft, ReadingFeedItem, RoomLane,
    RoomRecommendation, RoomRecommendationReason, SubscriptionOutcome, WebBookmarkRecord,
};
pub use nip05::{
    Nip05Availability, Nip05AvailabilitySnapshot, Nip05AvailabilityState,
    Nip05RegistrationSnapshot, OnboardingCreateAccountProjection,
    OnboardingCreateAccountProjectionInput, OnboardingUsernameCheckProjection,
};
pub use nip46::NostrConnectStartSnapshot;
pub use nostr_entities::{
    NostrEntityArticleCardProjection, NostrEntityArticleCardProjectionInput,
    NostrEntityRefSnapshot, NostrEntityResolutionSnapshot,
};
pub use ocr::{OcrLine, OcrPageDetection, OcrPageSide, OcrRect, OcrWord};
pub use podcast_transcript::{
    PodcastClipComposerInput, PodcastClipComposerProjection, PodcastClipComposerPublishInput,
    PodcastClipPublishInput, PodcastClipPublishSnapshot, PodcastClipSelection,
    PodcastListeningClipsSnapshot, PodcastListeningProjection, PodcastListeningProjectionInput,
    PodcastNowPlayingProjection, PodcastNowPlayingProjectionInput, PodcastTimelineRow,
    PodcastTimelineRowKind, PodcastTimelineRowState, PodcastTranscriptAvailability,
    PodcastTranscriptLoadSnapshot, TranscriptSegment,
};
pub use profile::{
    ProfileDisplayFallback, ProfileDisplayProjection, ProfileDisplayProjectionInput,
    ProfileDisplayWithLabelProjectionInput, ProfileFollowActionInput,
    ProfileFollowActionProjection, ProfileFollowMutationInput, ProfileFollowMutationSnapshot,
    ProfileIdentityProjection, ProfileIdentityProjectionInput, ProfileRelationshipProjection,
    ProfileRelationshipProjectionInput, ProfileUpdateProjection, ProfileUpdateProjectionInput,
    ProfileUpdateSnapshot,
};
pub use profile_page::ProfilePageSnapshot;
pub use reads::{
    ReadingFeedCardProjection, ReadingFeedCardProjectionInput, ReadingFeedInteractorProfile,
};
pub use recommendations::{
    RoomRecommendationAvatarProjection, RoomRecommendationCardProjection,
    RoomRecommendationCardProjectionInput, RoomRecommendationReasonProfile,
};
pub use relays::{
    AddRelayProbeStatus, AddRelaySheetProjection, AddRelaySheetProjectionInput, ImportRelayRow,
    ImportRelaysFetchSnapshot, ImportRelaysProjection, ImportRelaysProjectionInput,
    ImportRelaysSourceProjection, ImportRelaysSourceProjectionInput, NetworkCacheStatsSnapshot,
    NetworkDiagnosticsSnapshot, NetworkPathPolicySnapshot, NetworkRelayConnectionPolicyAction,
    NetworkSettingsMutationSnapshot, NetworkSettingsSnapshot, NetworkWifiOnlyPreferenceSnapshot,
    RelayAvatarProjection, RelayConfig, RelayDetailProjection, RelayDetailProjectionInput,
    RelayHostedRoomsSnapshot, RelayNip11ProbeSnapshot, RelayRemoveProjection,
    RelayRemoveProjectionInput, RelayRowProjection, RelayRowProjectionInput,
    RelaySettingsProjection, RelayStatusTone,
};
pub use room_explorer::{RoomBrowseSnapshot, RoomExplorerSnapshot};
pub use room_home::RoomHomeSnapshot;
pub use room_invites::{
    RoomInviteAvatarProjection, RoomInviteAvatarProjectionInput, RoomInviteCandidate,
    RoomInviteCandidateSource, RoomInviteChip, RoomInviteInputFormat, RoomInviteProjection,
    RoomInviteResolvedCandidate, RoomInviteSelectionAction, RoomInviteSelectionChromeInput,
    RoomInviteSelectionChromeProjection, RoomInviteSelectionInput, RoomInviteSelectionProjection,
    RoomInviteSendResultProjection, RoomInviteSnapshot, RoomInviteSnapshotInput,
    RoomInviteSuggestion, RoomShareLinkSnapshot,
};
pub use room_library::{
    RoomLibraryArticleCardProjection, RoomLibraryArticleCardProjectionInput,
    RoomLibraryBookCardProjection, RoomLibraryBookCardProjectionInput, RoomLibraryCardKind,
    RoomLibraryCardKindProjection, RoomLibraryCardKindProjectionInput,
    RoomLibraryGenericCardProjection, RoomLibraryGenericCardProjectionInput,
    RoomLibraryPodcastCardProjection, RoomLibraryPodcastCardProjectionInput,
};
pub use room_preview::{
    RoomPreviewActionProjection, RoomPreviewActionProjectionInput,
    RoomPreviewArtifactRowProjection, RoomPreviewArtifactsProjection,
    RoomPreviewArtifactsProjectionInput, RoomPreviewHeaderProjection,
    RoomPreviewHeaderProjectionInput, RoomPreviewSecondaryAction,
};
pub use search::{
    SearchArticleResultsSnapshot, SearchChromeSnapshot, SearchCommunityRowProjection,
    SearchCommunityRowProjectionInput, SearchHighlightRowProjection,
    SearchHighlightRowProjectionInput, SearchQueryProjection, SearchQueryProjectionInput,
    SearchResultsSnapshot, SearchSuggestionsProjection, SearchSuggestionsProjectionInput,
    SearchTextMatchSpan, SearchTextMatchesProjection, SearchTextMatchesProjectionInput,
};
pub use session::{
    AccountGenerationSnapshot, AuthSessionSnapshot, PublicKeyDisplayProjection,
    PublicKeyDisplayProjectionInput, SecretKeyDisplayProjection, SecretKeyDisplayProjectionInput,
};
pub use share_extension::{
    ShareQueueAttempt, ShareQueueDrainProjection, ShareQueueDrainProjectionInput, ShareQueueItem,
};
pub use share_targets::{
    ShareArticleTargetProjectionInput, ShareArtifactTargetProjection,
    ShareArtifactTargetProjectionInput, ShareHighlightArticleTargetProjectionInput,
    ShareHighlightTargetProjection, ShareHighlightTargetProjectionInput,
    ShareWebReaderTargetProjectionInput, ShareWebReaderTargetSnapshot,
};
pub use time_labels::{
    RelativeTimeLabelInput, RelativeTimeLabelProjection, RelativeTimeLabelStyle,
};
pub use web_metadata::{
    WebMetadata, WebMetadataRequestProjection, WebMetadataRequestProjectionInput,
};
pub use whats_new::{WhatsNewEntry, WhatsNewPresentationSnapshot};
