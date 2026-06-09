uniffi::setup_scaffolding!();

pub mod articles;
pub mod artifact_detail;
pub mod artifacts;
pub mod blossom;
pub mod bookmarks;
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
pub use client::HighlighterCore;
pub use errors::CoreError;
pub use events::{DataChangeType, Delta, EventCallback};
pub use feedback::{
    FeedbackComposerProjection, FeedbackComposerProjectionInput, FeedbackMessagePresentationInput,
    FeedbackMessagePresentationProjection, FeedbackThreadPresentationProjection,
};
pub use groups::{
    CreateRoomProjection, CreateRoomProjectionInput, CreateRoomVisibilityOption, RoomAccess,
    RoomAvatarProjection, RoomAvatarProjectionInput, RoomVisibility,
};
pub use highlights::{
    HighlightDetailContentProjection, HighlightDetailContentProjectionInput,
    HighlightDetailResourceProjection, HighlightDetailResourceProjectionInput,
    HighlightFeedContentProjection, HighlightFeedContentProjectionInput,
    HighlightGroupCardProjection, HighlightGroupCardProjectionInput,
    HighlightGroupHighlighterProfile, HighlightGroupHighlighterProjection,
    HighlightGroupLabelSegment, HighlightResourceAuthorProfile, HighlightResourceHeaderProjection,
    HighlightResourceHeaderProjectionInput,
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
pub use nip05::Nip05Availability;
pub use ocr::{OcrLine, OcrPageDetection, OcrPageSide, OcrRect, OcrWord};
pub use podcast_transcript::{
    PodcastClipComposerInput, PodcastClipComposerProjection, PodcastClipReference,
    PodcastClipSelection, PodcastListeningProjection, PodcastListeningProjectionInput,
    PodcastTimelineRow, PodcastTimelineRowKind, PodcastTimelineRowState, TranscriptSegment,
};
pub use profile::{
    ProfileDisplayFallback, ProfileDisplayProjection, ProfileDisplayProjectionInput,
    ProfileDisplayWithLabelProjectionInput, ProfileIdentityProjection,
    ProfileIdentityProjectionInput,
};
pub use reactions::ReactionRecord;
pub use reads::{
    ReadingFeedCardProjection, ReadingFeedCardProjectionInput, ReadingFeedInteractorProfile,
};
pub use recommendations::{
    RoomRecommendationAvatarProjection, RoomRecommendationCardProjection,
    RoomRecommendationCardProjectionInput, RoomRecommendationReasonProfile,
};
pub use relays::{
    AddRelayProbeStatus, AddRelaySheetProjection, AddRelaySheetProjectionInput, ImportRelayRow,
    ImportRelaysProjection, ImportRelaysProjectionInput, RelayAvatarProjection, RelayConfig,
    RelayDetailProjection, RelayDetailProjectionInput, RelayRemoveProjection,
    RelayRemoveProjectionInput, RelayRowProjection, RelayRowProjectionInput,
    RelaySettingsProjection, RelayStatusTone,
};
pub use room_invites::{
    RoomInviteAddDecision, RoomInviteAvatarProjection, RoomInviteAvatarProjectionInput,
    RoomInviteCandidate, RoomInviteCandidateSource, RoomInviteChip, RoomInviteInputFormat,
    RoomInviteProjection, RoomInviteProjectionInput, RoomInviteResolvedCandidate,
    RoomInviteSendResultProjection, RoomInviteSuggestion,
};
pub use room_library::{
    RoomLibraryArticleCardProjection, RoomLibraryArticleCardProjectionInput,
    RoomLibraryBookCardProjection, RoomLibraryBookCardProjectionInput,
    RoomLibraryGenericCardProjection, RoomLibraryGenericCardProjectionInput,
    RoomLibraryPodcastCardProjection, RoomLibraryPodcastCardProjectionInput,
};
pub use room_preview::{
    RoomPreviewArtifactRowProjection, RoomPreviewArtifactsProjection,
    RoomPreviewArtifactsProjectionInput,
};
pub use session::{
    PublicKeyDisplayProjection, PublicKeyDisplayProjectionInput, SecretKeyDisplayProjection,
    SecretKeyDisplayProjectionInput,
};
pub use share_targets::{
    ShareArticleTargetProjectionInput, ShareArtifactTargetProjection,
    ShareArtifactTargetProjectionInput, ShareCommunityRowProjection,
    ShareCommunityRowProjectionInput, ShareHighlightArticleTargetProjectionInput,
    ShareHighlightTargetProjection, ShareHighlightTargetProjectionInput,
};
pub use time_labels::{
    RelativeTimeLabelInput, RelativeTimeLabelProjection, RelativeTimeLabelStyle,
};
pub use web_metadata::WebMetadata;
pub use whats_new::WhatsNewEntry;
