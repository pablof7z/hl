uniffi::setup_scaffolding!();

pub mod articles;
pub mod artifacts;
pub mod blossom;
pub mod bookmarks;
pub mod cache;
pub mod chat;
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
pub mod nip46;
pub mod nostr_entities;
pub mod nostr_runtime;
pub mod outbox;
pub mod pictures;
pub mod profile;
pub mod reads;
pub mod recent_books;
pub mod recommendations;
pub mod relay_polish;
pub mod relays;
pub mod search;
pub mod session;
pub mod subscriptions;
pub mod web_metadata;

pub use client::HighlighterCore;
pub use errors::CoreError;
pub use events::{DataChangeType, Delta, EventCallback};
pub use models::{
    ArticleRecord, ArtifactPreview, ArtifactRecord, BlossomUpload, ChatMessageRecord,
    CommentRecord, CommunitySummary, CurrentUser, DiscussionAttachment, DiscussionRecord,
    FeedbackEventRecord, FeedbackThreadRecord, HighlightDraft, HighlightRecord, HydratedHighlight,
    NostrConnectOptions, PictureDraft, PictureRecord, ProfileMetadata, ReadingFeedItem,
    RoomRecommendation, RoomRecommendationReason,
};
pub use web_metadata::WebMetadata;
