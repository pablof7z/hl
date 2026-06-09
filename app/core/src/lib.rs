uniffi::setup_scaffolding!();

pub mod articles;
pub mod artifact_detail;
pub mod artifacts;
pub mod blossom;
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
pub mod reactions;
pub mod reads;
pub mod recent_books;
pub mod recent_searches;
pub mod recommendations;
pub mod reference_targets;
pub mod relay_polish;
pub mod relays;
pub mod room_explorer_config;
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

pub use articles::{
    ArticleProfileCardProjection, ArticleProfileCardProjectionInput, ArticleReaderHeaderProjection,
    ArticleReaderHeaderProjectionInput,
};
pub use artifact_detail::ArtifactDetailProjection;
pub use blossom::{
    BlossomServerEntryProjection, BlossomServerEntryProjectionInput, BlossomServerListProjection,
    BlossomServerListProjectionInput,
};
pub use bookmarks::{
    ArticleBookmarkStateProjection, ArticleBookmarkStateProjectionInput,
    EventBookmarkStateProjection, EventBookmarkStateProjectionInput,
};
pub use capture::{
    CaptureBookDisplayProjection, CaptureBookDisplayProjectionInput,
    CaptureCommunitySelectionProjection, CaptureCommunitySelectionProjectionInput,
    CaptureHighlightDraftInput, CaptureHighlightDraftProjection, CapturePictureDraftInput,
    CapturePublishPhase, CapturePublishProjection, CapturePublishProjectionInput,
    CaptureStashProjection, CaptureStashProjectionInput,
};
pub use chat::{ChatComposerProjection, ChatComposerProjectionInput};
pub use client::HighlighterCore;
pub use comments::{
    CommentComposerProjection, CommentComposerProjectionInput, CommentNodeChromeProjection,
    CommentNodeChromeProjectionInput, CommentThreadViewProjection,
    CommentThreadViewProjectionInput,
};
pub use discussions::{
    DiscussionAttachmentProjection, DiscussionAttachmentProjectionInput,
    DiscussionComposerProjection, DiscussionComposerProjectionInput,
    DiscussionComposerPublishInput,
};
pub use errors::CoreError;
pub use events::{DataChangeType, Delta, EventCallback};
pub use feedback::{
    FeedbackComposerProjection, FeedbackComposerProjectionInput, FeedbackMessagePresentationInput,
    FeedbackMessagePresentationProjection, FeedbackThreadPresentationProjection,
};
pub use groups::{
    CommunityRowProjection, CommunityRowProjectionInput, CreateRoomProjection,
    CreateRoomProjectionInput, CreateRoomVisibilityOption, RoomAccess, RoomAvatarProjection,
    RoomAvatarProjectionInput, RoomVisibility,
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
    HighlightResourceHeaderProjectionInput,
};
pub use isbn_lookup::{
    BookPickerQueryProjection, BookPickerQueryProjectionInput, IsbnManualPreviewProjection,
    IsbnManualPreviewProjectionInput, IsbnPreviewRequestProjection,
    IsbnPreviewRequestProjectionInput,
};
pub use lists::{
    BookmarkSetDetailProjection, BookmarkSetDetailProjectionInput, BookmarkSetRowProjection,
    BookmarkSetRowProjectionInput, BookmarkedArticleRowProjection,
    BookmarkedArticleRowProjectionInput, CurationSetCreateProjection,
    CurationSetCreateProjectionInput, WebBookmarkRowProjection, WebBookmarkRowProjectionInput,
};
pub use models::{
    ArticleListOutcome, ArticleOutcome, ArticleReaderRoute, ArticleReaderRouteOutcome,
    ArticleRecord, ArtifactDetailRoute, ArtifactDetailTarget, ArtifactListOutcome, ArtifactOutcome,
    ArtifactPreview, ArtifactPreviewOutcome, ArtifactRecord, ArtifactReferenceTarget,
    BlossomUpload, BlossomUploadOutcome, BookRoute, BookRouteOutcome, BookmarkSetListOutcome,
    BookmarkSetOutcome, BookmarkSetRecord, BoolOutcome, CacheStatsOutcome, ChatMessageListOutcome,
    ChatMessageOutcome, ChatMessageRecord, CommentListOutcome, CommentOutcome, CommentRecord,
    CommentReferenceBucket, CommentThreadNode, CommentThreadProjection, CommunityListOutcome,
    CommunitySummary, CurationMenuItem, CurationMenuItemListOutcome, CurrentUser,
    CurrentUserOutcome, DataOutcome, DiscussionAttachment, DiscussionListOutcome,
    DiscussionOutcome, DiscussionRecord, FeedbackEventListOutcome, FeedbackEventOutcome,
    FeedbackEventRecord, FeedbackThreadListOutcome, FeedbackThreadRecord, GeneratedAccount,
    GeneratedAccountOutcome, HighlightDraft, HighlightListOutcome, HighlightOutcome,
    HighlightRecord, HighlightReferenceBucket, HighlightReferenceTarget, HighlightSourceKind,
    HomeFeedItem, HydratedHighlight, HydratedHighlightListOutcome, LoginInputAction,
    MutationOutcome, Nip05AvailabilityOutcome, Nip11DocumentOutcome, NostrEntityEventOutcome,
    NostrEntityRefOutcome, OnboardingInterest, OnboardingInterestChip,
    OnboardingInterestProjection, OnboardingInterestSelection, OptionalStringOutcome, PictureDraft,
    PictureOutcome, PictureRecord, PodcastPositionRecord, ProfileListOutcome, ProfileMetadata,
    ProfileOutcome, ProfileUpdateDraft, ReactionOutcome, ReactionSummaryOutcome, ReadingFeedItem,
    ReadingFeedListOutcome, RelayConfigListOutcome, RelayDiagnosticListOutcome, RoomLane,
    RoomRecommendation, RoomRecommendationListOutcome, RoomRecommendationReason, StringListOutcome,
    StringOutcome, SubscriptionOutcome, TranscriptSegmentListOutcome, WebBookmarkListOutcome,
    WebBookmarkRecord, WebMetadataOutcome, WhatsNewEntriesOutcome,
};
pub use nip05::{
    Nip05Availability, OnboardingCreateAccountProjection, OnboardingCreateAccountProjectionInput,
    OnboardingUsernameCheckProjection,
};
pub use nostr_entities::{NostrEntityArticleCardProjection, NostrEntityArticleCardProjectionInput};
pub use ocr::{OcrLine, OcrPageDetection, OcrPageSide, OcrRect, OcrWord};
pub use podcast_transcript::{
    PodcastClipComposerInput, PodcastClipComposerProjection, PodcastClipReference,
    PodcastClipSelection, PodcastListeningProjection, PodcastListeningProjectionInput,
    PodcastNowPlayingProjection, PodcastNowPlayingProjectionInput, PodcastTimelineRow,
    PodcastTimelineRowKind, PodcastTimelineRowState, TranscriptSegment,
};
pub use profile::{
    ProfileDisplayFallback, ProfileDisplayProjection, ProfileDisplayProjectionInput,
    ProfileDisplayWithLabelProjectionInput, ProfileIdentityProjection,
    ProfileIdentityProjectionInput, ProfileRelationshipProjection,
    ProfileRelationshipProjectionInput, ProfileUpdateProjection, ProfileUpdateProjectionInput,
};
pub use reactions::{CommentLikeStateProjection, CommentLikeStateProjectionInput, ReactionRecord};
pub use reads::{
    ReadingFeedCardProjection, ReadingFeedCardProjectionInput, ReadingFeedInteractorProfile,
};
pub use recommendations::{
    RoomRecommendationAvatarProjection, RoomRecommendationCardProjection,
    RoomRecommendationCardProjectionInput, RoomRecommendationReasonProfile,
};
pub use relays::{
    AddRelayProbeStatus, AddRelaySheetProjection, AddRelaySheetProjectionInput, ImportRelayRow,
    ImportRelaysProjection, ImportRelaysProjectionInput, ImportRelaysSourceProjection,
    ImportRelaysSourceProjectionInput, RelayAvatarProjection, RelayConfig, RelayDetailProjection,
    RelayDetailProjectionInput, RelayRemoveProjection, RelayRemoveProjectionInput,
    RelayRowProjection, RelayRowProjectionInput, RelaySettingsProjection, RelayStatusTone,
};
pub use room_invites::{
    RoomInviteAvatarProjection, RoomInviteAvatarProjectionInput, RoomInviteCandidate,
    RoomInviteCandidateSource, RoomInviteChip, RoomInviteInputFormat, RoomInviteProjection,
    RoomInviteProjectionInput, RoomInviteResolvedCandidate, RoomInviteSelectionAction,
    RoomInviteSelectionInput, RoomInviteSelectionProjection, RoomInviteSendResultProjection,
    RoomInviteSuggestion,
};
pub use room_library::{
    RoomLibraryArticleCardProjection, RoomLibraryArticleCardProjectionInput,
    RoomLibraryBookCardProjection, RoomLibraryBookCardProjectionInput,
    RoomLibraryGenericCardProjection, RoomLibraryGenericCardProjectionInput,
    RoomLibraryPodcastCardProjection, RoomLibraryPodcastCardProjectionInput,
};
pub use room_preview::{
    RoomPreviewActionProjection, RoomPreviewActionProjectionInput,
    RoomPreviewArtifactRowProjection, RoomPreviewArtifactsProjection,
    RoomPreviewArtifactsProjectionInput, RoomPreviewSecondaryAction,
};
pub use search::{
    SearchHighlightRowProjection, SearchHighlightRowProjectionInput, SearchQueryProjection,
    SearchQueryProjectionInput, SearchSuggestionsProjection, SearchSuggestionsProjectionInput,
    SearchTextMatchSpan, SearchTextMatchesProjection, SearchTextMatchesProjectionInput,
};
pub use session::{
    PublicKeyDisplayProjection, PublicKeyDisplayProjectionInput, SecretKeyDisplayProjection,
    SecretKeyDisplayProjectionInput,
};
pub use share_extension::{
    ShareQueueAttempt, ShareQueueDrainProjection, ShareQueueDrainProjectionInput, ShareQueueItem,
};
pub use share_targets::{
    ShareArticleTargetProjectionInput, ShareArtifactTargetProjection,
    ShareArtifactTargetProjectionInput, ShareHighlightArticleTargetProjectionInput,
    ShareHighlightTargetProjection, ShareHighlightTargetProjectionInput,
    ShareWebReaderTargetProjectionInput,
};
pub use time_labels::{
    RelativeTimeLabelInput, RelativeTimeLabelProjection, RelativeTimeLabelStyle,
};
pub use web_metadata::{
    WebMetadata, WebMetadataRequestProjection, WebMetadataRequestProjectionInput,
};
pub use whats_new::WhatsNewEntry;
