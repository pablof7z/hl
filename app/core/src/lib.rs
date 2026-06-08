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
pub mod isbn_lookup;
pub mod lists;
pub mod models;
pub mod network_preferences;
pub mod nip05;
pub mod nip46;
pub mod nostr_entities;
pub mod nostr_runtime;
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
pub mod relay_polish;
pub mod relays;
pub mod room_explorer_config;
pub mod search;
pub mod session;
pub mod share_extension;
pub mod subscriptions;
pub mod web_metadata;
pub mod whats_new;

pub use client::HighlighterCore;
pub use errors::CoreError;
pub use events::{DataChangeType, Delta, EventCallback};
pub use models::{
    ArticleListOutcome, ArticleOutcome, ArticleReaderRoute, ArticleReaderRouteOutcome,
    ArticleRecord, ArtifactDetailRoute, ArtifactDetailTarget, ArtifactListOutcome, ArtifactOutcome,
    ArtifactPreview, ArtifactPreviewOutcome, ArtifactRecord, BlossomUpload, BlossomUploadOutcome,
    BookRoute, BookRouteOutcome, BookmarkSetListOutcome, BookmarkSetOutcome, BookmarkSetRecord,
    BoolOutcome, CacheStatsOutcome, ChatMessageListOutcome, ChatMessageOutcome, ChatMessageRecord,
    CommentListOutcome, CommentOutcome, CommentRecord, CommunityListOutcome, CommunitySummary,
    CurationMenuItem, CurationMenuItemListOutcome, CurrentUser, CurrentUserOutcome, DataOutcome,
    DiscussionAttachment, DiscussionListOutcome, DiscussionOutcome, DiscussionRecord,
    FeedbackEventListOutcome, FeedbackEventOutcome, FeedbackEventRecord, FeedbackThreadListOutcome,
    FeedbackThreadRecord, GeneratedAccount, GeneratedAccountOutcome, HighlightDraft,
    HighlightListOutcome, HighlightOutcome, HighlightRecord, HighlightSourceKind,
    HydratedHighlight, HydratedHighlightListOutcome, LoginInputAction, MutationOutcome,
    Nip05AvailabilityOutcome, Nip11DocumentOutcome, NostrEntityEventOutcome, NostrEntityRefOutcome,
    OptionalStringOutcome, PictureDraft, PictureOutcome, PictureRecord, PodcastPositionRecord,
    ProfileListOutcome, ProfileMetadata, ProfileOutcome, ProfileUpdateDraft, ReactionOutcome,
    ReactionSummaryOutcome, ReadingFeedItem, ReadingFeedListOutcome, RelayConfigListOutcome,
    RelayDiagnosticListOutcome, RoomRecommendation, RoomRecommendationListOutcome,
    RoomRecommendationReason, StringListOutcome, StringOutcome, SubscriptionOutcome,
    TranscriptSegmentListOutcome, WebBookmarkListOutcome, WebBookmarkRecord, WebMetadataOutcome,
    WhatsNewEntriesOutcome,
};
pub use nip05::Nip05Availability;
pub use podcast_transcript::TranscriptSegment;
pub use reactions::ReactionRecord;
pub use web_metadata::WebMetadata;
pub use whats_new::WhatsNewEntry;
