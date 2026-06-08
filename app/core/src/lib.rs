uniffi::setup_scaffolding!();

pub mod articles;
pub mod artifact_detail;
pub mod artifacts;
pub mod blossom;
pub mod bookmarks;
pub mod lists;
pub mod chat;
pub mod clock;
pub mod client;
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
    ArticleListOutcome, ArticleRecord, ArtifactDetailRoute, ArtifactDetailTarget,
    ArtifactListOutcome, ArtifactOutcome, ArtifactPreview, ArtifactPreviewOutcome, ArtifactRecord,
    BlossomUpload, BlossomUploadOutcome, BookmarkSetListOutcome,
    BookmarkSetOutcome, BookmarkSetRecord, BoolOutcome, ChatMessageListOutcome,
    ChatMessageOutcome, ChatMessageRecord, CommentOutcome, CommentRecord, CommunityListOutcome,
    CommunitySummary, CurrentUser, CurrentUserOutcome, DataOutcome, DiscussionAttachment,
    DiscussionListOutcome, DiscussionOutcome, DiscussionRecord, FeedbackEventOutcome,
    FeedbackEventRecord, FeedbackThreadRecord, GeneratedAccount, GeneratedAccountOutcome,
    HighlightDraft, HighlightListOutcome,
    HighlightOutcome, HighlightRecord, HydratedHighlight, HydratedHighlightListOutcome,
    MutationOutcome, NostrConnectOptions, PictureDraft, PictureOutcome, PictureRecord,
    PodcastPositionRecord, ProfileListOutcome, ProfileMetadata, ReactionListOutcome,
    ReactionOutcome, ReadingFeedItem, RoomRecommendation, RoomRecommendationReason,
    StringListOutcome, StringOutcome, SubscriptionOutcome, TranscriptSegmentListOutcome,
    WebBookmarkListOutcome, WebBookmarkRecord, WebMetadataOutcome, WhatsNewEntriesOutcome,
};
pub use nip05::Nip05Availability;
pub use podcast_transcript::TranscriptSegment;
pub use reactions::ReactionRecord;
pub use web_metadata::WebMetadata;
pub use whats_new::WhatsNewEntry;
