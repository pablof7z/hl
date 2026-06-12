uniffi::setup_scaffolding!();

pub mod articles;
pub mod artifacts;
pub mod blossom;
pub mod bookmarks;
pub mod cache;
pub mod chat;
mod client;
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
pub mod nmp_app;
mod nmp_runtime;
pub mod nostr_entities;
pub mod nostr_runtime;
pub mod outbox;
pub mod pictures;
pub mod profile;
pub mod reactions;
pub mod reads;
pub mod recent_books;
pub mod recommendations;
pub mod relay_polish;
pub mod relays;
pub mod search;
pub mod session;
pub mod share_links;
pub mod subscriptions;
pub mod web_metadata;

pub use errors::CoreError;
pub use events::{DataChangeType, Delta, EventCallback};
pub use models::{
    ArticleRecord, ArtifactPreview, ArtifactRecord, BlossomUpload, BookmarkSetRecord,
    ChatMessageRecord, CommentRecord, CommunitySummary, CurrentUser, DiscussionAttachment,
    DiscussionRecord, FeedbackEventRecord, FeedbackThreadRecord, GeneratedAccount, HighlightDraft,
    HighlightRecord, HydratedHighlight, NostrConnectOptions, PictureDraft, PictureRecord,
    ProfileMetadata, ProfileUpdateDraft, ReadingFeedItem, RoomRecommendation,
    RoomRecommendationReason, WebBookmarkRecord,
};
pub use nmp_app::{
    HighlighterAppAction, HighlighterAppConfig, HighlighterAppReconciler, HighlighterAppState,
    HighlighterArticleReaderSnapshot, HighlighterBookPickerSnapshot,
    HighlighterBookmarkCollectionDetailSnapshot, HighlighterBookmarksSnapshot,
    HighlighterCaptureArtifact, HighlighterCaptureSnapshot, HighlighterChromeSnapshot,
    HighlighterConnectionState, HighlighterCurationMenuSnapshot, HighlighterEditProfileImageTarget,
    HighlighterEditProfileSnapshot, HighlighterFeedbackSnapshot, HighlighterHomeFeedItem,
    HighlighterHomeFeedItemKind, HighlighterHomeFeedSnapshot, HighlighterHomeReadItem,
    HighlighterIsbnPreview, HighlighterMediaSettingsSnapshot, HighlighterNetworkImportSnapshot,
    HighlighterNetworkSnapshot, HighlighterNmpApp, HighlighterOnboardingInterest,
    HighlighterOnboardingSnapshot, HighlighterProfile, HighlighterProfileViewSnapshot,
    HighlighterRelayNip11Snapshot, HighlighterRelayRemovalImpact, HighlighterSessionCredential,
    HighlighterShareComposerSnapshot, HighlighterToast, HighlighterToastKind,
    HighlighterWebMetadata,
};
pub use reactions::ReactionRecord;
pub use relays::RelayConfig;
pub use web_metadata::WebMetadata;

#[uniffi::export]
pub fn normalize_isbn(raw: String) -> Option<String> {
    isbn_lookup::normalize_isbn(&raw).ok()
}
