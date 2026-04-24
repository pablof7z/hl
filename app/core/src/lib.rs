uniffi::setup_scaffolding!();

pub mod articles;
pub mod artifacts;
pub mod blossom;
pub mod cache;
pub mod client;
pub mod curation;
pub mod discovery;
pub mod discussions;
pub mod errors;
pub mod events;
pub mod follows;
pub mod groups;
pub mod highlights;
pub mod isbn_lookup;
pub mod models;
pub mod nip46;
pub mod nostr_runtime;
pub mod pictures;
pub mod profile;
pub mod reads;
pub mod recent_books;
pub mod recommendations;
pub mod relays;
pub mod session;
pub mod subscriptions;

pub use client::HighlighterCore;
pub use errors::CoreError;
pub use events::{DataChangeType, Delta, EventCallback};
pub use models::{
    ArticleRecord, ArtifactPreview, ArtifactRecord, BlossomUpload, CommunitySummary, CurrentUser,
    DiscussionAttachment, DiscussionRecord, HighlightDraft, HighlightRecord, HydratedHighlight,
    NostrConnectOptions, PictureDraft, PictureRecord, ProfileMetadata, ReadingFeedItem,
    RoomRecommendation, RoomRecommendationReason,
};
