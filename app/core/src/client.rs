//! Top-level UniFFI-exposed object. Swift holds one `HighlighterCore` for
//! the life of the app.
//!
//! State discipline: async methods never hold the `parking_lot` guard across
//! an `.await` point (the guard isn't `Send`). Long-running protocol work
//! happens in `Session` / feature modules, which own their own async state.

use std::{collections::BTreeMap, sync::Arc};

use nostr_sdk::prelude::*;
use parking_lot::RwLock;

use crate::articles;
use crate::blossom;
use crate::clock::{Clock, SystemClock};
use crate::comments;
use crate::curation;
use crate::discovery;
use crate::errors::CoreError;
use crate::events::{DataChangeType, Delta, EventCallback};
use crate::feedback;
use crate::follows;
use crate::groups;
use crate::highlights;
use crate::isbn_lookup;
use crate::models::{
    ArticleListOutcome, ArticleOutcome, ArticleReaderRoute, ArticleReaderRouteOutcome,
    ArticleRecord, ArticleUpdateAction, ArtifactDetailRoute, ArtifactListOutcome, ArtifactOutcome,
    ArtifactPreview, ArtifactPreviewOutcome, ArtifactRecord, ArtifactReferenceTarget,
    BlossomUpload, BlossomUploadOutcome, BookRoute, BookRouteOutcome, BookmarkSetListOutcome,
    BookmarkSetOutcome, BookmarkSetRecord, BoolOutcome, CacheStatsOutcome, ChatMessageListOutcome,
    ChatMessageOutcome, ChatMessageRecord, CommentListOutcome, CommentOutcome, CommentRecord,
    CommentReferenceBucket, CommentScope, CommentScopeOutcome, CommentThreadNode,
    CommentThreadProjection, CommunityListOutcome, CommunitySummary, CurationMenuItem,
    CurationMenuItemListOutcome, CurrentUser, CurrentUserOutcome, DataOutcome,
    DiscussionListOutcome, DiscussionOutcome, DiscussionRecord, FeedbackEventListOutcome,
    FeedbackEventOutcome, FeedbackEventRecord, FeedbackThreadListOutcome, FeedbackThreadRecord,
    GeneratedAccountOutcome, HighlightDraft, HighlightListOutcome, HighlightOutcome,
    HighlightRecord, HighlightReferenceBucket, HighlightReferenceTarget, HighlightSourceKind,
    HomeFeedItem, HydratedHighlight, HydratedHighlightListOutcome, LoginInputAction,
    MutationOutcome, Nip05AvailabilityOutcome, Nip11DocumentOutcome, NostrConnectOptions,
    NostrEntityEventOutcome, NostrEntityRefOutcome, OnboardingInterest,
    OnboardingInterestProjection, OnboardingInterestSelection, OptionalStringOutcome, PictureDraft,
    PictureOutcome, PictureRecord, PodcastPositionRecord, ProfileListOutcome, ProfileMetadata,
    ProfileOutcome, ProfileUpdateAction, ProfileUpdateDraft, ReactionOutcome,
    ReactionSummaryOutcome, ReadingFeedItem, ReadingFeedListOutcome, RelayConfigListOutcome,
    RelayDiagnostic, RelayDiagnosticListOutcome, RoomLane, RoomRecommendation,
    RoomRecommendationListOutcome, StringListOutcome, StringOutcome, SubscriptionOutcome,
    TranscriptSegmentListOutcome, WebBookmarkListOutcome, WebBookmarkRecord, WebMetadataOutcome,
    WhatsNewEntriesOutcome,
};
use crate::network_preferences;
use crate::nip05::{self, Nip05Availability};
use crate::nip46::{self, BunkerSigner};
use crate::nostr_runtime::NostrRuntime;
use crate::onboarding;
use crate::podcast_position;
use crate::podcast_transcript::{
    self, PodcastClipComposerInput, PodcastClipComposerProjection, PodcastClipReference,
    PodcastClipSelection, PodcastListeningProjection, PodcastListeningProjectionInput,
    TranscriptSegment,
};
use crate::profile;
use crate::reads;
use crate::recent_searches;
use crate::recommendations;
use crate::reference_targets;
use crate::relays::nostr_connect_relay;
use crate::room_explorer_config;
use crate::room_lanes;
use crate::room_state;
use crate::session::{current_user_from_pubkey, Session};
use crate::subscriptions::{SubscriptionKind, SubscriptionRegistry};
use crate::web_metadata::{self, WebMetadata, WebMetadataStore};
use crate::whats_new;

#[derive(uniffi::Object)]
pub struct HighlighterCore {
    inner: Arc<RwLock<Inner>>,
    runtime: Arc<NostrRuntime>,
    /// Shared with every pump task so `set_event_callback` can replace the
    /// callback atomically mid-flight.
    callback_slot: Arc<RwLock<Option<Arc<dyn EventCallback>>>>,
    subscriptions: Arc<SubscriptionRegistry>,
    /// OG/favicon cache shared across all `get_web_metadata` calls. Lives
    /// on the core so concurrent fetches for the same URL coalesce.
    web_metadata: Arc<WebMetadataStore>,
    /// Rust-owned persistent ISBN preview cache. Native shells render these
    /// previews but do not mirror them in platform storage.
    isbn_previews: Arc<isbn_lookup::IsbnPreviewCache>,
    /// Rust-owned recent search history shared by every native shell.
    recent_searches: Arc<recent_searches::RecentSearchesStore>,
    /// Rust-owned rooms explorer curator config and NIP-11 refresh path.
    room_explorer_config: Arc<room_explorer_config::RoomExplorerConfigStore>,
    /// Rust-owned What's New entries and seen marker.
    whats_new: Arc<whats_new::WhatsNewStore>,
    /// Rust-owned durable onboarding completion flag.
    onboarding: Arc<onboarding::OnboardingStore>,
    /// Rust-owned network preference state.
    network_preferences: Arc<network_preferences::NetworkPreferencesStore>,
    /// Rust-owned durable podcast playback position.
    podcast_position: Arc<podcast_position::PodcastPositionStore>,
    /// Kernel-owned clock shared by feature modules that need timestamps.
    clock: Arc<dyn Clock>,
}

struct Inner {
    session: Session,
    pending_joins: BTreeMap<String, String>,
}

fn mutation_outcome(result: Result<(), CoreError>) -> MutationOutcome {
    match result {
        Ok(()) => MutationOutcome {
            applied: true,
            error: String::new(),
        },
        Err(error) => MutationOutcome {
            applied: false,
            error: error.to_string(),
        },
    }
}

fn bool_outcome(result: Result<bool, CoreError>) -> BoolOutcome {
    match result {
        Ok(value) => BoolOutcome {
            value,
            error: String::new(),
        },
        Err(error) => BoolOutcome {
            value: false,
            error: error.to_string(),
        },
    }
}

fn book_route_outcome(result: Result<Option<BookRoute>, CoreError>) -> BookRouteOutcome {
    match result {
        Ok(value) => BookRouteOutcome {
            value,
            error: String::new(),
        },
        Err(error) => BookRouteOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn string_list_outcome(result: Result<Vec<String>, CoreError>) -> StringListOutcome {
    match result {
        Ok(values) => StringListOutcome {
            values,
            error: String::new(),
        },
        Err(error) => StringListOutcome {
            values: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn string_outcome(result: Result<String, CoreError>) -> StringOutcome {
    match result {
        Ok(value) => StringOutcome {
            value,
            error: String::new(),
        },
        Err(error) => StringOutcome {
            value: String::new(),
            error: error.to_string(),
        },
    }
}

fn nip05_availability_outcome(
    result: Result<Nip05Availability, CoreError>,
) -> Nip05AvailabilityOutcome {
    match result {
        Ok(value) => Nip05AvailabilityOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => Nip05AvailabilityOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn join_room_display_name(room_name: &str) -> String {
    let trimmed = room_name.trim();
    if trimmed.is_empty() {
        "this room".to_string()
    } else {
        trimmed.to_string()
    }
}

fn optional_string_outcome(result: Result<Option<String>, CoreError>) -> OptionalStringOutcome {
    match result {
        Ok(value) => OptionalStringOutcome {
            value,
            error: String::new(),
        },
        Err(error) => OptionalStringOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn data_outcome(result: Result<Vec<u8>, CoreError>) -> DataOutcome {
    match result {
        Ok(value) => DataOutcome {
            value,
            error: String::new(),
        },
        Err(error) => DataOutcome {
            value: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn transcript_segment_list_outcome(
    result: Result<Vec<TranscriptSegment>, CoreError>,
) -> TranscriptSegmentListOutcome {
    match result {
        Ok(values) => TranscriptSegmentListOutcome {
            values,
            error: String::new(),
        },
        Err(error) => TranscriptSegmentListOutcome {
            values: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn current_user_outcome(result: Result<CurrentUser, CoreError>) -> CurrentUserOutcome {
    match result {
        Ok(value) => CurrentUserOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => CurrentUserOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn generated_account_outcome(
    result: Result<crate::models::GeneratedAccount, CoreError>,
) -> GeneratedAccountOutcome {
    match result {
        Ok(value) => GeneratedAccountOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => GeneratedAccountOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn subscription_outcome(result: Result<u64, CoreError>) -> SubscriptionOutcome {
    match result {
        Ok(handle) => SubscriptionOutcome {
            handle,
            error: String::new(),
        },
        Err(error) => SubscriptionOutcome {
            handle: 0,
            error: error.to_string(),
        },
    }
}

fn bookmark_set_list_outcome(
    result: Result<Vec<BookmarkSetRecord>, CoreError>,
) -> BookmarkSetListOutcome {
    match result {
        Ok(values) => BookmarkSetListOutcome {
            values,
            error: String::new(),
        },
        Err(error) => BookmarkSetListOutcome {
            values: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn curation_menu_item_list_outcome(
    result: Result<Vec<CurationMenuItem>, CoreError>,
) -> CurationMenuItemListOutcome {
    match result {
        Ok(values) => CurationMenuItemListOutcome {
            values,
            error: String::new(),
        },
        Err(error) => CurationMenuItemListOutcome {
            values: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn bookmark_set_outcome(result: Result<BookmarkSetRecord, CoreError>) -> BookmarkSetOutcome {
    match result {
        Ok(value) => BookmarkSetOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => BookmarkSetOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn web_bookmark_list_outcome(
    result: Result<Vec<WebBookmarkRecord>, CoreError>,
) -> WebBookmarkListOutcome {
    match result {
        Ok(values) => WebBookmarkListOutcome {
            values,
            error: String::new(),
        },
        Err(error) => WebBookmarkListOutcome {
            values: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn article_list_outcome(result: Result<Vec<ArticleRecord>, CoreError>) -> ArticleListOutcome {
    match result {
        Ok(values) => ArticleListOutcome {
            values,
            error: String::new(),
        },
        Err(error) => ArticleListOutcome {
            values: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn optional_article_outcome(result: Result<Option<ArticleRecord>, CoreError>) -> ArticleOutcome {
    match result {
        Ok(value) => ArticleOutcome {
            value,
            error: String::new(),
        },
        Err(error) => ArticleOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn article_reader_route_outcome(
    result: Result<Option<ArticleReaderRoute>, CoreError>,
) -> ArticleReaderRouteOutcome {
    match result {
        Ok(value) => ArticleReaderRouteOutcome {
            value,
            error: String::new(),
        },
        Err(error) => ArticleReaderRouteOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn artifact_outcome(result: Result<ArtifactRecord, CoreError>) -> ArtifactOutcome {
    match result {
        Ok(value) => ArtifactOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => ArtifactOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn artifact_preview_outcome(result: Result<ArtifactPreview, CoreError>) -> ArtifactPreviewOutcome {
    match result {
        Ok(value) => ArtifactPreviewOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => ArtifactPreviewOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn artifact_list_outcome(result: Result<Vec<ArtifactRecord>, CoreError>) -> ArtifactListOutcome {
    match result {
        Ok(values) => ArtifactListOutcome {
            values,
            error: String::new(),
        },
        Err(error) => ArtifactListOutcome {
            values: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn web_metadata_outcome(result: Result<WebMetadata, CoreError>) -> WebMetadataOutcome {
    match result {
        Ok(value) => WebMetadataOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => WebMetadataOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn blossom_upload_outcome(result: Result<BlossomUpload, CoreError>) -> BlossomUploadOutcome {
    match result {
        Ok(value) => BlossomUploadOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => BlossomUploadOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn chat_message_outcome(result: Result<ChatMessageRecord, CoreError>) -> ChatMessageOutcome {
    match result {
        Ok(value) => ChatMessageOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => ChatMessageOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn chat_message_list_outcome(
    result: Result<Vec<ChatMessageRecord>, CoreError>,
) -> ChatMessageListOutcome {
    match result {
        Ok(values) => ChatMessageListOutcome {
            values,
            error: String::new(),
        },
        Err(error) => ChatMessageListOutcome {
            values: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn comment_outcome(result: Result<CommentRecord, CoreError>) -> CommentOutcome {
    match result {
        Ok(value) => CommentOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => CommentOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn comment_list_outcome(result: Result<Vec<CommentRecord>, CoreError>) -> CommentListOutcome {
    match result {
        Ok(values) => CommentListOutcome {
            values,
            error: String::new(),
        },
        Err(error) => CommentListOutcome {
            values: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn comment_scope_outcome(result: Result<CommentScope, CoreError>) -> CommentScopeOutcome {
    match result {
        Ok(value) => CommentScopeOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => CommentScopeOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn community_list_outcome(
    result: Result<Vec<CommunitySummary>, CoreError>,
) -> CommunityListOutcome {
    match result {
        Ok(values) => CommunityListOutcome {
            values,
            error: String::new(),
        },
        Err(error) => CommunityListOutcome {
            values: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn relay_config_list_outcome(
    result: Result<Vec<crate::relays::RelayConfig>, CoreError>,
) -> RelayConfigListOutcome {
    match result {
        Ok(values) => RelayConfigListOutcome {
            values,
            error: String::new(),
        },
        Err(error) => RelayConfigListOutcome {
            values: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn relay_diagnostic_list_outcome(
    result: Result<Vec<crate::models::RelayDiagnostic>, CoreError>,
) -> RelayDiagnosticListOutcome {
    match result {
        Ok(values) => RelayDiagnosticListOutcome {
            values,
            error: String::new(),
        },
        Err(error) => RelayDiagnosticListOutcome {
            values: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn nip11_document_outcome(
    result: Result<crate::models::Nip11Document, CoreError>,
) -> Nip11DocumentOutcome {
    match result {
        Ok(value) => Nip11DocumentOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => Nip11DocumentOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn cache_stats_outcome(result: Result<crate::models::CacheStats, CoreError>) -> CacheStatsOutcome {
    match result {
        Ok(value) => CacheStatsOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => CacheStatsOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn room_recommendation_list_outcome(
    result: Result<Vec<RoomRecommendation>, CoreError>,
) -> RoomRecommendationListOutcome {
    match result {
        Ok(values) => RoomRecommendationListOutcome {
            values,
            error: String::new(),
        },
        Err(error) => RoomRecommendationListOutcome {
            values: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn discussion_outcome(result: Result<DiscussionRecord, CoreError>) -> DiscussionOutcome {
    match result {
        Ok(value) => DiscussionOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => DiscussionOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn discussion_list_outcome(
    result: Result<Vec<DiscussionRecord>, CoreError>,
) -> DiscussionListOutcome {
    match result {
        Ok(values) => DiscussionListOutcome {
            values,
            error: String::new(),
        },
        Err(error) => DiscussionListOutcome {
            values: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn feedback_event_outcome(result: Result<FeedbackEventRecord, CoreError>) -> FeedbackEventOutcome {
    match result {
        Ok(value) => FeedbackEventOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => FeedbackEventOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn feedback_event_list_outcome(
    result: Result<Vec<FeedbackEventRecord>, CoreError>,
) -> FeedbackEventListOutcome {
    match result {
        Ok(values) => FeedbackEventListOutcome {
            values,
            error: String::new(),
        },
        Err(error) => FeedbackEventListOutcome {
            values: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn feedback_thread_list_outcome(
    result: Result<Vec<FeedbackThreadRecord>, CoreError>,
) -> FeedbackThreadListOutcome {
    match result {
        Ok(values) => FeedbackThreadListOutcome {
            values,
            error: String::new(),
        },
        Err(error) => FeedbackThreadListOutcome {
            values: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn highlight_list_outcome(result: Result<Vec<HighlightRecord>, CoreError>) -> HighlightListOutcome {
    match result {
        Ok(values) => HighlightListOutcome {
            values,
            error: String::new(),
        },
        Err(error) => HighlightListOutcome {
            values: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn hydrated_highlight_list_outcome(
    result: Result<Vec<HydratedHighlight>, CoreError>,
) -> HydratedHighlightListOutcome {
    match result {
        Ok(values) => HydratedHighlightListOutcome {
            values,
            error: String::new(),
        },
        Err(error) => HydratedHighlightListOutcome {
            values: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn reading_feed_list_outcome(
    result: Result<Vec<ReadingFeedItem>, CoreError>,
) -> ReadingFeedListOutcome {
    match result {
        Ok(values) => ReadingFeedListOutcome {
            values,
            error: String::new(),
        },
        Err(error) => ReadingFeedListOutcome {
            values: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn highlight_outcome(result: Result<HighlightRecord, CoreError>) -> HighlightOutcome {
    match result {
        Ok(value) => HighlightOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => HighlightOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn profile_list_outcome(result: Result<Vec<ProfileMetadata>, CoreError>) -> ProfileListOutcome {
    match result {
        Ok(values) => ProfileListOutcome {
            values,
            error: String::new(),
        },
        Err(error) => ProfileListOutcome {
            values: Vec::new(),
            error: error.to_string(),
        },
    }
}

fn profile_outcome(result: Result<ProfileMetadata, CoreError>) -> ProfileOutcome {
    match result {
        Ok(value) => ProfileOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => ProfileOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn optional_profile_outcome(result: Result<Option<ProfileMetadata>, CoreError>) -> ProfileOutcome {
    match result {
        Ok(value) => ProfileOutcome {
            value,
            error: String::new(),
        },
        Err(error) => ProfileOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn nostr_entity_ref_outcome(
    result: Result<crate::nostr_entities::NostrEntityRef, CoreError>,
) -> NostrEntityRefOutcome {
    match result {
        Ok(value) => NostrEntityRefOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => NostrEntityRefOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn nostr_entity_event_outcome(
    result: Result<Option<crate::nostr_entities::NostrEntityEvent>, CoreError>,
) -> NostrEntityEventOutcome {
    match result {
        Ok(value) => NostrEntityEventOutcome {
            value,
            error: String::new(),
        },
        Err(error) => NostrEntityEventOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn reaction_outcome(
    result: Result<crate::reactions::ReactionRecord, CoreError>,
) -> ReactionOutcome {
    match result {
        Ok(value) => ReactionOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => ReactionOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn reaction_summary_outcome(
    result: Result<crate::reactions::ReactionSummary, CoreError>,
) -> ReactionSummaryOutcome {
    match result {
        Ok(value) => ReactionSummaryOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => ReactionSummaryOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn picture_outcome(result: Result<PictureRecord, CoreError>) -> PictureOutcome {
    match result {
        Ok(value) => PictureOutcome {
            value: Some(value),
            error: String::new(),
        },
        Err(error) => PictureOutcome {
            value: None,
            error: error.to_string(),
        },
    }
}

fn whats_new_entries_outcome(
    result: Result<Vec<whats_new::WhatsNewEntry>, CoreError>,
) -> WhatsNewEntriesOutcome {
    match result {
        Ok(entries) => WhatsNewEntriesOutcome {
            entries,
            error: String::new(),
        },
        Err(error) => WhatsNewEntriesOutcome {
            entries: Vec::new(),
            error: error.to_string(),
        },
    }
}

impl HighlighterCore {
    async fn start_nostr_connect_with_options(
        &self,
        options: NostrConnectOptions,
        callback: &str,
    ) -> Result<String, CoreError> {
        // Local ephemeral keypair. The remote signer uses this pubkey to
        // address its messages to us over the relay; after pair completion
        // the user's pubkey comes from the remote signer via GetPublicKey.
        let local_keys = Keys::generate();
        let secret = nip46::random_secret();

        let pairing_relay = nostr_connect_relay();
        let uri = nip46::build_nostr_connect_uri(
            local_keys.public_key(),
            pairing_relay,
            &options.name,
            &options.url,
            &options.image,
            &options.perms,
            &secret,
        )?;

        // Ensure the NIP-46 relay is part of the pool before we start
        // listening for the inbound `connect` request. `add_relay` is a
        // no-op if the relay is already known, but we can't rely on the
        // initial pool reconcile having completed yet.
        let client = self.runtime.client().clone();
        if let Err(e) = client.add_relay(pairing_relay).await {
            tracing::warn!(relay = %pairing_relay, error = %e, "add_relay");
        }
        client.connect().await;

        // Spawn a background task that waits for the remote signer to
        // connect and then installs the resulting BunkerSigner. The task
        // must own: the client, callback slot, Session slot, and local keys.
        let inner = self.inner.clone();
        let runtime = self.runtime.clone();
        let callback_slot = self.callback_slot.clone();
        let clock = self.clock.clone();
        self.runtime.runtime_handle().spawn(async move {
            let result = BunkerSigner::await_inbound_with_clock(
                client.clone(),
                local_keys,
                Some(secret),
                clock,
            )
            .await;
            match result {
                Ok((signer, user_pubkey)) => {
                    let user = match current_user_from_pubkey(&user_pubkey) {
                        Ok(u) => u,
                        Err(e) => {
                            tracing::warn!(error = %e, "npub encode after bunker pair");
                            return;
                        }
                    };
                    let signer = Arc::new(signer);
                    // We're inside NostrRuntime's tokio runtime here
                    // (spawned via `runtime_handle().spawn`). The sync
                    // `runtime.set_signer` wrapper uses `block_on`, which
                    // panics when called from that same runtime, so talk to
                    // the client directly instead.
                    client.set_signer((*signer).clone()).await;
                    runtime.spawn_apply_user_relay_config(user_pubkey.to_hex());
                    let sub_id = runtime.spawn_membership_subscription(user_pubkey);
                    let contacts_id = runtime.spawn_contacts_subscription(user_pubkey);
                    {
                        let mut guard = inner.write();
                        guard.session.set_bunker(signer, user.clone());
                        guard.session.set_membership_subscription(sub_id);
                        guard.session.set_contacts_subscription(contacts_id);
                    }
                    let cb = { callback_slot.read().clone() };
                    if let Some(cb) = cb {
                        cb.on_data_changed(Delta {
                            subscription_id: 0,
                            change: DataChangeType::SignerConnected { user },
                        });
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "nostrconnect inbound pairing failed");
                }
            }
        });

        Ok(nip46::append_callback_to_uri(&uri, callback))
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl HighlighterCore {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        let runtime =
            Arc::new(NostrRuntime::new().expect("nostr runtime initialization must succeed"));
        Self::assemble(runtime)
    }

    // -- Auth (sync) --

    pub fn classify_login_input(&self, input: String) -> LoginInputAction {
        crate::session::classify_login_input(&input)
    }

    pub fn project_public_key_display(
        &self,
        input: crate::session::PublicKeyDisplayProjectionInput,
    ) -> crate::session::PublicKeyDisplayProjection {
        crate::session::public_key_display_projection(input)
    }

    pub fn project_secret_key_display(
        &self,
        input: crate::session::SecretKeyDisplayProjectionInput,
    ) -> crate::session::SecretKeyDisplayProjection {
        crate::session::secret_key_display_projection(input)
    }

    pub fn project_relative_time_label(
        &self,
        input: crate::time_labels::RelativeTimeLabelInput,
    ) -> crate::time_labels::RelativeTimeLabelProjection {
        crate::time_labels::relative_time_label_projection(input, self.clock.now_unix_seconds())
    }

    pub fn login_nsec(&self, nsec: String) -> CurrentUserOutcome {
        let result: Result<CurrentUser, CoreError> = (|| {
            // Do the session mutation + keys extraction in a single write-guard
            // scope. Binding both values to locals ensures the guard drops
            // before the subsequent `self.inner.write()` call — without this,
            // Rust keeps the guard alive for the whole expression chain and
            // parking_lot deadlocks on re-entry.
            let (user, keys) = {
                let mut guard = self.inner.write();
                let user = guard.session.login_nsec(&nsec)?;
                let keys = guard.session.keys().cloned();
                (user, keys)
            };

            if let Some(keys) = keys {
                self.runtime.set_signer(keys.clone());
                let pubkey = keys.public_key();
                // First-pass: apply whatever's in cache so subscriptions have a
                // pool to talk to immediately. The bootstrap below races to
                // fetch the user's actual NIP-65 from the network and re-apply
                // — without it, a fresh install with cold cache stays on
                // seed_defaults forever.
                self.runtime.spawn_apply_user_relay_config(pubkey.to_hex());
                let user_relay_config_id = self.runtime.spawn_user_relay_config_bootstrap(pubkey);
                let sub_id = self.runtime.spawn_membership_subscription(pubkey);
                let contacts_id = self.runtime.spawn_contacts_subscription(pubkey);
                // Eagerly fetch 39000 metadata for any groups already in the
                // nostrdb cache. Without this, the first `getJoinedCommunities`
                // call on a warm cache would return summaries with name=id because
                // the stage-2 metadata sub would only be installed after the pump
                // sees a live membership delta.
                let cached_ids =
                    crate::subscriptions::collect_cached_group_ids(self.runtime.ndb(), &pubkey);
                if !cached_ids.is_empty() {
                    self.runtime
                        .spawn_group_metadata_subscription(cached_ids.into_iter().collect());
                }
                // Best-effort outbox bootstrap: fetch follows' kind:10002 so the
                // home-feed planner has data to work with. Empty on first login
                // (no kind:3 cached yet) — `subscribe_following_*` will re-arm
                // this whenever it's called, picking up follows discovered since.
                //
                // Also kick off a NIP-77 negentropy sync against purplepag.es
                // for the social trio (kind:0/3/10002) of the same set. Live
                // subscriptions catch incremental updates; negentropy sync is
                // the cheap cold-start path that closes the "no kind:10002
                // cached" gap so the planner stops dumping authors into the
                // fallback shard.
                let cached_follows = current_followed_pubkeys(self.runtime.ndb(), &pubkey);
                self.runtime
                    .spawn_negentropy_sync_for_follows(cached_follows.clone());
                let follows_nip65_id = self
                    .runtime
                    .spawn_follows_relay_lists_subscription(cached_follows);

                let mut guard = self.inner.write();
                guard.session.set_membership_subscription(sub_id);
                guard.session.set_contacts_subscription(contacts_id);
                guard
                    .session
                    .set_user_relay_config_subscription(user_relay_config_id);
                if let Some(id) = follows_nip65_id {
                    guard.session.set_follows_nip65_subscription(id);
                }
            }

            Ok(user)
        })();
        current_user_outcome(result)
    }

    pub fn generate_account(&self) -> GeneratedAccountOutcome {
        let result: Result<crate::models::GeneratedAccount, CoreError> = (|| {
            let keys = Keys::generate();
            let nsec = keys
                .secret_key()
                .to_bech32()
                .map_err(|e| CoreError::Other(format!("nsec encoding failed: {e}")))?;
            let outcome = self.login_nsec(nsec.clone());
            if !outcome.error.is_empty() {
                return Err(CoreError::Other(outcome.error));
            }
            let user = outcome
                .value
                .ok_or_else(|| CoreError::Other("login did not return a user".into()))?;
            Ok(crate::models::GeneratedAccount { user, nsec })
        })();
        generated_account_outcome(result)
    }

    pub fn logout(&self) {
        self.subscriptions.clear(&self.runtime);
        {
            let mut guard = self.inner.write();
            if let Some(sub_id) = guard.session.take_membership_subscription() {
                self.runtime.drop_subscription(sub_id);
            }
            if let Some(sub_id) = guard.session.take_contacts_subscription() {
                self.runtime.drop_subscription(sub_id);
            }
            if let Some(sub_id) = guard.session.take_discovery_subscription() {
                self.runtime.drop_subscription(sub_id);
            }
            if let Some(sub_id) = guard.session.take_curation_subscription() {
                self.runtime.drop_subscription(sub_id);
            }
            if let Some(sub_id) = guard.session.take_friends_memberships_subscription() {
                self.runtime.drop_subscription(sub_id);
            }
            if let Some(sub_id) = guard.session.take_follows_nip65_subscription() {
                self.runtime.drop_subscription(sub_id);
            }
            if let Some(sub_id) = guard.session.take_user_relay_config_subscription() {
                self.runtime.drop_subscription(sub_id);
            }
            guard.pending_joins.clear();
            guard.session.logout();
        }
        self.runtime.unset_signer();
    }

    pub fn current_user(&self) -> Option<CurrentUser> {
        self.inner.read().session.current_user()
    }

    pub fn is_onboarding_complete(&self) -> bool {
        self.onboarding.is_complete()
    }

    pub fn set_onboarding_complete(&self, complete: bool) -> MutationOutcome {
        mutation_outcome(self.onboarding.set_complete(complete))
    }

    pub fn get_onboarding_interests(&self) -> Vec<OnboardingInterest> {
        onboarding::interest_catalog()
    }

    pub fn get_onboarding_interest_selection(
        &self,
        selected_ids: Vec<String>,
    ) -> OnboardingInterestSelection {
        onboarding::interest_selection(selected_ids)
    }

    pub fn get_onboarding_interest_projection(
        &self,
        selected_ids: Vec<String>,
    ) -> OnboardingInterestProjection {
        onboarding::interest_projection(selected_ids)
    }

    pub async fn complete_onboarding_interests(
        &self,
        selected_ids: Vec<String>,
    ) -> MutationOutcome {
        let result: Result<(), CoreError> = async {
            let selection = onboarding::interest_selection(selected_ids);
            if !selection.can_continue {
                return Err(CoreError::InvalidInput(
                    "choose at least three interests".into(),
                ));
            }
            let follower = {
                let guard = self.inner.read();
                guard
                    .session
                    .current_user()
                    .ok_or(CoreError::NotAuthenticated)?
                    .pubkey
            };
            follows::publish_follow_additions(&self.runtime, &follower, &selection.follow_pubkeys)
                .await?;
            self.onboarding.set_complete(true)?;
            Ok(())
        }
        .await;
        mutation_outcome(result)
    }

    pub fn is_wifi_only_enabled(&self) -> bool {
        self.network_preferences.wifi_only_enabled()
    }

    pub fn set_wifi_only_enabled(&self, enabled: bool) -> MutationOutcome {
        mutation_outcome(self.network_preferences.set_wifi_only_enabled(enabled))
    }

    pub fn get_podcast_position(&self) -> Option<PodcastPositionRecord> {
        self.podcast_position.current()
    }

    pub fn get_podcast_position_seconds(&self, guid: String) -> Option<f64> {
        self.podcast_position.position_for_guid(&guid)
    }

    pub fn save_podcast_position(
        &self,
        guid: String,
        position_seconds: f64,
        artifact: ArtifactRecord,
    ) -> MutationOutcome {
        mutation_outcome(self.podcast_position.save(guid, position_seconds, artifact))
    }

    pub async fn load_podcast_transcript(&self, url: String) -> TranscriptSegmentListOutcome {
        transcript_segment_list_outcome(podcast_transcript::fetch_transcript(&url).await)
    }

    /// Build a podcast clip highlight draft from transcript selection inputs.
    /// Rust owns selected segment matching, chronological quote assembly, and
    /// protocol payload fields.
    pub fn get_podcast_clip_highlight_draft(
        &self,
        segments: Vec<TranscriptSegment>,
        selected_segment_ids: Vec<String>,
        note: String,
        clip_start_seconds: Option<f64>,
        clip_end_seconds: Option<f64>,
        clip_speaker: String,
    ) -> HighlightDraft {
        podcast_transcript::clip_highlight_draft(
            &segments,
            &selected_segment_ids,
            note,
            clip_start_seconds,
            clip_end_seconds,
            clip_speaker,
        )
    }

    pub fn get_podcast_clip_composer_projection(
        &self,
        input: PodcastClipComposerInput,
    ) -> PodcastClipComposerProjection {
        podcast_transcript::clip_composer_projection(input)
    }

    pub fn get_podcast_clip_composer_draft(
        &self,
        segments: Vec<TranscriptSegment>,
        transcript_available: bool,
        context: String,
        clip_start_seconds: f64,
        clip_end_seconds: f64,
    ) -> HighlightDraft {
        podcast_transcript::clip_composer_highlight_draft(
            &segments,
            transcript_available,
            context,
            clip_start_seconds,
            clip_end_seconds,
        )
    }

    pub fn get_podcast_listening_projection(
        &self,
        input: PodcastListeningProjectionInput,
    ) -> PodcastListeningProjection {
        podcast_transcript::listening_projection(input)
    }

    pub fn get_podcast_clip_reference(&self, artifact: ArtifactRecord) -> PodcastClipReference {
        podcast_transcript::podcast_clip_reference(&artifact)
    }

    pub fn clear_podcast_clip_selection(&self) -> PodcastClipSelection {
        podcast_transcript::clear_clip_selection()
    }

    pub fn mark_podcast_clip_in(
        &self,
        selection: PodcastClipSelection,
        current_time: f64,
    ) -> PodcastClipSelection {
        podcast_transcript::mark_clip_in(&selection, current_time)
    }

    pub fn mark_podcast_clip_out(
        &self,
        selection: PodcastClipSelection,
        current_time: f64,
    ) -> PodcastClipSelection {
        podcast_transcript::mark_clip_out(&selection, current_time)
    }

    pub fn extend_podcast_clip_to_segment(
        &self,
        selection: PodcastClipSelection,
        segment: TranscriptSegment,
    ) -> PodcastClipSelection {
        podcast_transcript::extend_clip_to_segment(&selection, &segment)
    }

    pub fn set_podcast_clip_start(
        &self,
        selection: PodcastClipSelection,
        value: f64,
    ) -> PodcastClipSelection {
        podcast_transcript::set_clip_start(&selection, value)
    }

    pub fn set_podcast_clip_end(
        &self,
        selection: PodcastClipSelection,
        value: f64,
        duration_seconds: f64,
    ) -> PodcastClipSelection {
        podcast_transcript::set_clip_end(&selection, value, duration_seconds)
    }

    pub async fn download_podcast_artwork(&self, url: String) -> DataOutcome {
        data_outcome(podcast_transcript::download_artwork(&url).await)
    }

    pub fn get_artifact_detail_route(&self, artifact: ArtifactRecord) -> ArtifactDetailRoute {
        crate::artifact_detail::route_for_artifact(&artifact)
    }

    pub fn share_extension_communities_snapshot(
        &self,
        communities: Vec<CommunitySummary>,
    ) -> Vec<u8> {
        crate::share_extension::communities_snapshot_json(communities)
    }

    pub async fn prepare_whats_new(&self) -> WhatsNewEntriesOutcome {
        whats_new_entries_outcome(self.whats_new.prepare().await)
    }

    pub async fn mark_whats_new_seen(&self, shipped_at_unix_seconds: u64) -> MutationOutcome {
        mutation_outcome(self.whats_new.mark_seen(shipped_at_unix_seconds).await)
    }

    // -- Auth (async) --
    // Async auth flows delegate without holding the parking_lot guard across
    // await. The session module is responsible for thread-safe internal state.

    pub async fn start_default_nostr_connect(&self, callback: String) -> StringOutcome {
        let result = self
            .start_nostr_connect_with_options(NostrConnectOptions::default(), &callback)
            .await;
        string_outcome(result)
    }

    pub async fn pair_bunker(&self, uri: String) -> CurrentUserOutcome {
        let result: Result<CurrentUser, CoreError> = async {
            let normalized = normalize_bunker_uri(&uri);
            if normalized.is_empty() {
                Err(CoreError::InvalidInput("empty bunker URI".into()))?;
            }

            let client = self.runtime.client().clone();
            let (signer, user_pubkey) =
                BunkerSigner::pair_with_clock(client, &normalized, self.clock.clone()).await?;
            let user = current_user_from_pubkey(&user_pubkey)?;

            let signer = Arc::new(signer);
            self.runtime.set_signer((*signer).clone());
            self.runtime
                .spawn_apply_user_relay_config(user_pubkey.to_hex());

            let sub_id = self.runtime.spawn_membership_subscription(user_pubkey);
            let contacts_id = self.runtime.spawn_contacts_subscription(user_pubkey);
            let cached_ids =
                crate::subscriptions::collect_cached_group_ids(self.runtime.ndb(), &user_pubkey);
            if !cached_ids.is_empty() {
                self.runtime
                    .spawn_group_metadata_subscription(cached_ids.into_iter().collect());
            }
            {
                let mut guard = self.inner.write();
                guard.session.set_bunker(signer, user.clone());
                guard.session.set_membership_subscription(sub_id);
                guard.session.set_contacts_subscription(contacts_id);
            }

            let cb = { self.callback_slot.read().clone() };
            if let Some(cb) = cb {
                cb.on_data_changed(Delta {
                    subscription_id: 0,
                    change: DataChangeType::SignerConnected { user: user.clone() },
                });
            }

            Ok(user)
        }
        .await;
        current_user_outcome(result)
    }

    // -- Subscriptions --

    pub fn set_event_callback(&self, callback: Arc<dyn EventCallback>) {
        *self.callback_slot.write() = Some(callback.clone());

        // One-shot app-scope seed: if a user is already logged in, broadcast
        // `SignerConnected` so any freshly-registered Swift store bootstraps
        // its `currentUser` without racing a Swift-side cache read.
        let seed_user = self.inner.read().session.current_user();
        if let Some(user) = seed_user {
            callback.on_data_changed(Delta {
                subscription_id: 0,
                change: DataChangeType::SignerConnected { user },
            });
        }
    }

    /// Consume a pending join when a matching NIP-29 membership delta arrives.
    /// Swift routes the delta; Rust owns whether it was pending and what toast
    /// should be shown.
    pub fn confirm_pending_join(&self, group_id: String) {
        if let Some(room_name) = self.remove_pending_join(&group_id) {
            self.emit_app_toast(format!("You're in {room_name} ✓"));
        }
    }

    /// App-scope subscription for the joined-communities view. Returns a
    /// handle; fires CommunityUpserted / MembershipChanged deltas tagged
    /// with that handle. Re-uses the relay sub installed at login; this
    /// call is about setting up the nostrdb notification pump.
    pub async fn subscribe_joined_communities(&self) -> SubscriptionOutcome {
        subscription_outcome((|| {
            let user_pubkey = self.require_user_pubkey()?;
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::JoinedCommunities { user_pubkey },
            )
        })())
    }

    /// Per-room view-scope subscription. Returns a handle; fires
    /// ArtifactUpserted / HighlightUpserted / HighlightShared for this
    /// specific group.
    pub async fn subscribe_room(&self, group_id: String) -> SubscriptionOutcome {
        subscription_outcome((|| {
            if group_id.trim().is_empty() {
                return Err(CoreError::InvalidInput("group_id must not be empty".into()));
            }
            self.subscriptions
                .register(&self.runtime, SubscriptionKind::Room { group_id })
        })())
    }

    /// Per-room Discussions view-scope subscription. Returns a handle; fires
    /// `DiscussionUpserted` deltas for kind:11 threads in this group that
    /// carry the `t=discussion` marker.
    pub async fn subscribe_room_discussions(&self, group_id: String) -> SubscriptionOutcome {
        subscription_outcome((|| {
            if group_id.trim().is_empty() {
                return Err(CoreError::InvalidInput("group_id must not be empty".into()));
            }
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::RoomDiscussions { group_id },
            )
        })())
    }

    /// Per-room Chat view-scope subscription. Returns a handle; fires
    /// `ChatMessageUpserted` deltas for kind:9 messages tagged
    /// `#h=<group_id>`.
    pub async fn subscribe_room_chat(&self, group_id: String) -> SubscriptionOutcome {
        subscription_outcome((|| {
            if group_id.trim().is_empty() {
                return Err(CoreError::InvalidInput("group_id must not be empty".into()));
            }
            self.subscriptions
                .register(&self.runtime, SubscriptionKind::RoomChat { group_id })
        })())
    }

    /// Vault view-scope subscription for the current user's own highlights.
    pub async fn subscribe_vault(&self) -> SubscriptionOutcome {
        subscription_outcome((|| {
            let user_pubkey = self.require_user_pubkey()?;
            self.subscriptions
                .register(&self.runtime, SubscriptionKind::Vault { user_pubkey })
        })())
    }

    /// Profile view-scope subscription. Fires `UserProfileUpdated` deltas
    /// when any event relevant to `pubkey_hex`'s profile arrives. Install on
    /// profile view appearance; `unsubscribe(handle)` on disappearance.
    pub async fn subscribe_user_profile(&self, pubkey_hex: String) -> SubscriptionOutcome {
        subscription_outcome((|| {
            let pubkey = PublicKey::from_hex(pubkey_hex.trim())
                .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;
            self.subscriptions
                .register(&self.runtime, SubscriptionKind::UserProfile { pubkey })
        })())
    }

    /// Following Reads view-scope subscription. Snapshots the user's current
    /// follow list, then listens for: (a) new articles authored by a follow,
    /// (b) interactions by a follow against any kind:30023 content. Fires
    /// `FollowingReadsUpdated` deltas; the Swift store re-queries the feed.
    /// Install on tab appearance; `unsubscribe(handle)` on disappearance.
    pub async fn subscribe_following_reads(&self) -> SubscriptionOutcome {
        subscription_outcome((|| {
            let user_pubkey = self.require_user_pubkey()?;
            let follow_hex_strings =
                follows::query_follows(self.runtime.ndb(), &user_pubkey.to_hex())?;
            let follows_pks: Vec<PublicKey> = follow_hex_strings
                .iter()
                .filter_map(|s| PublicKey::from_hex(s.trim()).ok())
                .collect();
            self.refresh_follows_nip65_subscription(&follows_pks);
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::FollowingReads {
                    follows: follows_pks,
                },
            )
        })())
    }

    /// Highlights home-feed view-scope subscription. Snapshots the user's
    /// current follow list (plus self — nobody lists themselves in kind:3
    /// but we want our own highlights in the home feed) and joined-group
    /// ids, then listens for kind:9802 events authored by anyone in that
    /// set or tagged into any joined room.
    pub async fn subscribe_following_highlights(&self) -> SubscriptionOutcome {
        subscription_outcome((|| {
            let user_pubkey = self.require_user_pubkey()?;
            let follow_hex_strings =
                follows::query_follows(self.runtime.ndb(), &user_pubkey.to_hex())?;
            let mut follows_pks: Vec<PublicKey> = follow_hex_strings
                .iter()
                .filter_map(|s| PublicKey::from_hex(s.trim()).ok())
                .collect();
            if !follows_pks.contains(&user_pubkey) {
                follows_pks.push(user_pubkey);
            }
            self.refresh_follows_nip65_subscription(&follows_pks);
            let joined = groups::query_joined_communities_from_ndb(
                self.runtime.ndb(),
                &user_pubkey.to_hex(),
            )?;
            let group_ids: Vec<String> = joined.into_iter().map(|c| c.id).collect();
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::FollowingHighlights {
                    follows: follows_pks,
                    group_ids,
                },
            )
        })())
    }

    /// Article-reader view-scope subscription. Fires `ArticleUpdated` deltas
    /// whenever the article's replaceable body supersedes OR a new kind:9802
    /// highlighting this article's `a`-tag arrives. Install on reader view
    /// appearance; `unsubscribe(handle)` on disappearance.
    pub async fn subscribe_article(
        &self,
        pubkey_hex: String,
        d_tag: String,
    ) -> SubscriptionOutcome {
        subscription_outcome((|| {
            let pubkey_hex = pubkey_hex.trim();
            let d_tag = d_tag.trim();
            if pubkey_hex.is_empty() || d_tag.is_empty() {
                return Err(CoreError::InvalidInput(
                    "pubkey_hex and d_tag must not be empty".into(),
                ));
            }
            let author = PublicKey::from_hex(pubkey_hex)
                .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;
            let address = format!("30023:{}:{}", pubkey_hex, d_tag);
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::Article {
                    author,
                    d_tag: d_tag.to_string(),
                    address,
                },
            )
        })())
    }

    /// Feedback-threads subscription for the shake-to-share surface. Fires
    /// `FeedbackThreadsUpdated` deltas whenever a kind:1 root authored by
    /// the current user (with the project `a` tag) or any kind:513 metadata
    /// for the same project arrives. Swift re-queries on each.
    pub async fn subscribe_feedback_threads(&self, coordinate: String) -> SubscriptionOutcome {
        subscription_outcome((|| {
            let coordinate = coordinate.trim();
            if coordinate.is_empty() {
                return Err(CoreError::InvalidInput(
                    "coordinate must not be empty".into(),
                ));
            }
            let user_pubkey = self.require_user_pubkey()?;
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::FeedbackThreads {
                    coordinate: coordinate.to_string(),
                    current_user_pubkey: user_pubkey,
                },
            )
        })())
    }

    /// Per-thread feedback subscription. Fires `FeedbackThreadEventUpserted`
    /// deltas for every kind:1 `e`-tagged to the root (regardless of author).
    pub async fn subscribe_feedback_thread(&self, root_event_id: String) -> SubscriptionOutcome {
        subscription_outcome((|| {
            let root_event_id = root_event_id.trim();
            if root_event_id.is_empty() {
                return Err(CoreError::InvalidInput(
                    "root_event_id must not be empty".into(),
                ));
            }
            let root = EventId::from_hex(root_event_id)
                .map_err(|e| CoreError::InvalidInput(format!("invalid event id: {e}")))?;
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::FeedbackThread {
                    root_event_id: root,
                },
            )
        })())
    }

    /// Drop a subscription by handle. Idempotent.
    pub fn unsubscribe(&self, handle: u64) {
        self.subscriptions.remove(&self.runtime, handle);
    }

    // -- Reads --

    pub async fn get_joined_communities(&self) -> CommunityListOutcome {
        community_list_outcome((|| {
            let Some(user) = self.inner.read().session.current_user() else {
                return Err(CoreError::NotAuthenticated);
            };
            groups::query_joined_communities_from_ndb(self.runtime.ndb(), &user.pubkey)
        })())
    }

    pub async fn get_joined_room_names_for_relay(&self, url: String) -> StringListOutcome {
        string_list_outcome((|| {
            let Some(user) = self.inner.read().session.current_user() else {
                return Err(CoreError::NotAuthenticated);
            };
            groups::query_joined_room_names_for_relay_from_ndb(
                self.runtime.ndb(),
                &user.pubkey,
                &url,
            )
        })())
    }

    pub async fn get_artifacts(&self, group_id: String, limit: u32) -> ArtifactListOutcome {
        artifact_list_outcome(crate::artifacts::query_for_group(
            self.runtime.ndb(),
            &group_id,
            limit,
        ))
    }

    /// Classify a highlight source for native icon/label rendering. Rust owns
    /// the source/reference interpretation; native shells only render the enum.
    pub fn get_highlight_source_kind(
        &self,
        preview_source: String,
        external_reference: String,
        artifact_address: String,
        source_url: String,
    ) -> HighlightSourceKind {
        highlights::source_kind(
            &preview_source,
            &external_reference,
            &artifact_address,
            &source_url,
        )
    }

    pub async fn get_highlights(
        &self,
        group_id: String,
        limit: u32,
    ) -> HydratedHighlightListOutcome {
        hydrated_highlight_list_outcome(crate::highlights::query_for_group(
            self.runtime.ndb(),
            &group_id,
            limit,
        ))
    }

    pub async fn get_my_highlights(&self, limit: u32) -> HighlightListOutcome {
        highlight_list_outcome((|| {
            let Some(user) = self.inner.read().session.current_user() else {
                return Err(CoreError::NotAuthenticated);
            };
            highlights::query_highlights_by_author(self.runtime.ndb(), &user.pubkey, limit)
        })())
    }

    /// Following Reads feed — articles surfaced through the user's follow
    /// graph. See `reads::query_following_reads` for semantics. Returns an
    /// empty list if the user isn't logged in or has no follows cached yet.
    pub async fn get_following_reads(&self, limit: u32) -> ReadingFeedListOutcome {
        let Some(user) = self.inner.read().session.current_user() else {
            return ReadingFeedListOutcome {
                values: Vec::new(),
                error: CoreError::NotAuthenticated.to_string(),
            };
        };
        reading_feed_list_outcome(reads::query_following_reads(
            self.runtime.ndb(),
            &user.pubkey,
            limit,
        ))
    }

    pub fn project_reading_feed_card(
        &self,
        input: reads::ReadingFeedCardProjectionInput,
    ) -> reads::ReadingFeedCardProjection {
        reads::reading_feed_card_projection(input)
    }

    /// Highlights home feed — kind:9802 events authored by follows plus
    /// highlights tagged into joined rooms. See
    /// `highlights::query_following_highlights` for semantics.
    pub async fn get_following_highlights(&self, limit: u32) -> HydratedHighlightListOutcome {
        let result: Result<Vec<HydratedHighlight>, CoreError> = (|| {
            let Some(user) = self.inner.read().session.current_user() else {
                return Err(CoreError::NotAuthenticated);
            };
            let joined =
                groups::query_joined_communities_from_ndb(self.runtime.ndb(), &user.pubkey)?;
            let group_ids: Vec<String> = joined.into_iter().map(|c| c.id).collect();
            highlights::query_following_highlights(
                self.runtime.ndb(),
                &user.pubkey,
                &group_ids,
                limit,
            )
        })();
        hydrated_highlight_list_outcome(result)
    }

    pub fn project_highlight_group_card(
        &self,
        input: highlights::HighlightGroupCardProjectionInput,
    ) -> highlights::HighlightGroupCardProjection {
        highlights::highlight_group_card_projection(input)
    }

    /// Compose following highlights and following reads into the home feed.
    /// Rust owns grouping, stable identity, duplicate suppression, and merged
    /// ordering; native shells render the returned rows.
    pub fn build_home_feed_items(
        &self,
        highlights: Vec<HydratedHighlight>,
        reads: Vec<ReadingFeedItem>,
    ) -> Vec<HomeFeedItem> {
        crate::home_feed::build_items(&highlights, &reads)
    }

    // -- Profile reads (per-pubkey, no auth required) --

    pub async fn get_user_profile(&self, pubkey_hex: String) -> ProfileOutcome {
        optional_profile_outcome(profile::query_profile_from_ndb(
            self.runtime.ndb(),
            pubkey_hex.trim(),
        ))
    }

    /// Profile/avatar presentation projection. Rust owns profile-name
    /// precedence, pubkey fallback, and avatar URL selection; native shells
    /// render the resulting values without reimplementing business rules.
    pub fn project_profile_display(
        &self,
        input: profile::ProfileDisplayProjectionInput,
    ) -> profile::ProfileDisplayProjection {
        profile::profile_display_projection(input)
    }

    /// Profile/avatar presentation projection for bylines that include an
    /// artifact-provided author label.
    pub fn project_profile_display_with_label(
        &self,
        input: profile::ProfileDisplayWithLabelProjectionInput,
    ) -> profile::ProfileDisplayProjection {
        profile::profile_display_with_label_projection(input)
    }

    /// Compact profile handle projection for social proof surfaces. Rust owns
    /// handle precedence and pubkey fallback length; native shells render it.
    pub fn project_profile_handle(
        &self,
        input: profile::ProfileDisplayProjectionInput,
    ) -> profile::ProfileDisplayProjection {
        profile::profile_handle_projection(input)
    }

    /// Profile header identity projection. Rust owns display fallbacks and
    /// NIP-05 label normalization; native shells render the returned fields.
    pub fn project_profile_identity(
        &self,
        input: profile::ProfileIdentityProjectionInput,
    ) -> profile::ProfileIdentityProjection {
        profile::profile_identity_projection(input)
    }

    /// Publish a new kind:0 metadata event for the current user. Preserves
    /// any unknown JSON fields the user had set via other clients —
    /// only the canonical fields the edit form drives get overwritten.
    /// Empty strings clear the corresponding field. Returns the parsed
    /// metadata so the caller's UI can swap to the new state without
    /// waiting for the relay echo.
    pub async fn update_profile(&self, draft: ProfileUpdateDraft) -> ProfileOutcome {
        let result: Result<ProfileMetadata, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            profile::publish_profile(&self.runtime, &draft).await
        }
        .await;
        profile_outcome(result)
    }

    pub fn normalize_nip05_username(&self, input: String) -> String {
        nip05::normalize_username(&input)
    }

    pub fn suggest_nip05_username(&self, display_name: String) -> String {
        nip05::suggest_username(&display_name)
    }

    pub fn is_nip05_username_valid(&self, input: String) -> bool {
        nip05::is_valid_username(&input)
    }

    pub async fn check_nip05_availability(&self, name: String) -> Nip05AvailabilityOutcome {
        nip05_availability_outcome(nip05::check_availability(&name).await)
    }

    pub async fn register_nip05(&self, name: String, domain: String) -> StringOutcome {
        let result: Result<String, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            nip05::register_username(&self.runtime, &name, &domain).await
        }
        .await;
        string_outcome(result)
    }

    pub async fn get_user_articles(&self, pubkey_hex: String, limit: u32) -> ArticleListOutcome {
        article_list_outcome(articles::query_articles_by_author(
            self.runtime.ndb(),
            pubkey_hex.trim(),
            limit,
        ))
    }

    /// Read a single NIP-23 article by author + `d` tag from nostrdb. `None`
    /// if ndb hasn't cached it yet — the reader's `subscribe_article` pump
    /// backfills via relays, and a later call returns `Some`.
    pub async fn get_article(&self, pubkey_hex: String, d_tag: String) -> ArticleOutcome {
        optional_article_outcome(articles::query_article(
            self.runtime.ndb(),
            pubkey_hex.trim(),
            d_tag.trim(),
        ))
    }

    /// Read a single NIP-23 article by its full NIP-33 address
    /// (`30023:<pubkey>:<d>`) from nostrdb.
    pub async fn get_article_by_address(&self, address: String) -> ArticleOutcome {
        optional_article_outcome(articles::query_article_by_address(
            self.runtime.ndb(),
            address.trim(),
        ))
    }

    /// Return the author pubkey from a valid NIP-23 article address
    /// (`30023:<pubkey>:<d>`).
    pub async fn get_article_address_author(&self, address: String) -> OptionalStringOutcome {
        optional_string_outcome(Ok(articles::article_author_from_address(address.trim())))
    }

    /// Resolve a full NIP-23 article address into the native reader route.
    /// Invalid or non-article addresses produce an empty value without error.
    pub fn get_article_reader_route(&self, address: String) -> ArticleReaderRouteOutcome {
        article_reader_route_outcome(Ok(articles::article_reader_route_from_address(
            address.trim(),
        )))
    }

    /// Resolve author + `d` tag into the native reader route. Rust owns the
    /// canonical `30023:<pubkey>:<d>` address construction.
    pub fn get_article_reader_route_for_article(
        &self,
        pubkey_hex: String,
        d_tag: String,
    ) -> ArticleReaderRouteOutcome {
        article_reader_route_outcome(Ok(articles::article_reader_route(
            pubkey_hex.trim(),
            d_tag.trim(),
        )))
    }

    pub fn project_article_reader_header(
        &self,
        input: articles::ArticleReaderHeaderProjectionInput,
    ) -> articles::ArticleReaderHeaderProjection {
        articles::article_reader_header_projection(input)
    }

    /// Project a cached NIP-23 article into the artifact preview shape used by
    /// kind:11 sharing. Rust owns the `a`/`k`/highlight reference fields.
    pub fn get_article_artifact_preview(&self, article: ArticleRecord) -> ArtifactPreviewOutcome {
        artifact_preview_outcome(Ok(articles::article_artifact_preview(&article)))
    }

    /// Project a NIP-23 article address into a minimal artifact preview for
    /// share flows that only have the address cached.
    pub fn get_article_artifact_preview_for_address(
        &self,
        address: String,
    ) -> ArtifactPreviewOutcome {
        artifact_preview_outcome(
            articles::article_artifact_preview_from_address(address.trim()).ok_or_else(|| {
                CoreError::InvalidInput("invalid NIP-23 article address".to_string())
            }),
        )
    }

    /// Project a cached NIP-23 article into the artifact record shape expected
    /// by highlight publishing.
    pub fn get_article_artifact_record(&self, article: ArticleRecord) -> ArtifactOutcome {
        artifact_outcome(Ok(articles::article_artifact_record(&article)))
    }

    /// Read all highlights referencing the given NIP-23 article address
    /// (`30023:<pubkey>:<d>`) from nostrdb, newest first.
    pub async fn get_highlights_for_article(
        &self,
        address: String,
        limit: u32,
    ) -> HighlightListOutcome {
        highlight_list_outcome(highlights::query_for_article(
            self.runtime.ndb(),
            address.trim(),
            limit,
        ))
    }

    /// Read highlights whose `tag_name` tag holds `tag_value`, newest
    /// first. Generalizes `get_highlights_for_article`: pass `("a", "30023:pk:d")`
    /// for articles, `("i", "isbn:…")` for ISBN books, `("r", "<url>")` for
    /// podcasts. `tag_name` must be a single character.
    pub async fn get_highlights_for_reference(
        &self,
        tag_name: String,
        tag_value: String,
        limit: u32,
    ) -> HighlightListOutcome {
        let Some(ch) = tag_name.trim().chars().next() else {
            return HighlightListOutcome {
                values: Vec::new(),
                error: String::new(),
            };
        };
        highlight_list_outcome(highlights::query_for_reference(
            self.runtime.ndb(),
            ch,
            tag_value.trim(),
            limit,
        ))
    }

    /// Resolve a book catalog id into the canonical ISBN route used by native
    /// book screens. Accepts raw ISBNs and `isbn:<digits>` values.
    pub fn get_book_route(&self, catalog_id: String) -> BookRouteOutcome {
        book_route_outcome(Ok(highlights::book_route_for_catalog(catalog_id.trim())))
    }

    /// Resolve a highlight's book reference from its external reference or
    /// artifact address. Rust owns the precedence and canonical catalog id.
    pub fn get_highlight_book_route(
        &self,
        external_reference: String,
        artifact_address: String,
    ) -> BookRouteOutcome {
        book_route_outcome(Ok(highlights::book_route_for_highlight(
            external_reference.trim(),
            artifact_address.trim(),
        )))
    }

    /// Read book passages from a catalog id. Rust owns the NIP-73 ISBN
    /// reference derivation used by the book detail screen.
    pub async fn get_book_highlights(
        &self,
        catalog_id: String,
        limit: u32,
    ) -> HighlightListOutcome {
        highlight_list_outcome(highlights::query_for_book_catalog(
            self.runtime.ndb(),
            catalog_id.trim(),
            limit,
        ))
    }

    /// Classify a subscription event kind into the exact article reader slice
    /// that native shells should refresh.
    pub fn get_article_update_action(&self, kind: u32) -> ArticleUpdateAction {
        crate::events::article_update_action(kind)
    }

    /// Project an optimistically published highlight into the current visible
    /// article highlight list. Rust owns duplicate suppression and ordering.
    pub fn insert_unique_highlight_front(
        &self,
        highlights: Vec<HighlightRecord>,
        highlight: HighlightRecord,
    ) -> Vec<HighlightRecord> {
        highlights::insert_unique_front(&highlights, &highlight)
    }

    /// Classify a subscription event kind into the exact profile slice that
    /// native shells should refresh.
    pub fn get_profile_update_action(&self, kind: u32) -> ProfileUpdateAction {
        crate::events::profile_update_action(kind)
    }

    /// Project a NIP-23 article address into the NIP-22 root scope used by
    /// comment reads/writes.
    pub fn get_article_comment_scope(&self, address: String) -> CommentScopeOutcome {
        comment_scope_outcome(comments::article_scope(&address))
    }

    /// Project a NIP-84 highlight event id into the NIP-22 root scope used by
    /// comment reads/writes.
    pub fn get_highlight_comment_scope(&self, event_id_hex: String) -> CommentScopeOutcome {
        comment_scope_outcome(comments::highlight_scope(&event_id_hex))
    }

    /// Project a kind:11 discussion event id into the NIP-22 root scope used
    /// by comment reads/writes.
    pub fn get_discussion_comment_scope(&self, event_id_hex: String) -> CommentScopeOutcome {
        comment_scope_outcome(comments::discussion_scope(&event_id_hex))
    }

    /// Project a web URL into the external NIP-22 root scope used by comment
    /// reads/writes.
    pub fn get_web_comment_scope(&self, url: String) -> CommentScopeOutcome {
        comment_scope_outcome(comments::web_scope(&url))
    }

    /// Project an artifact preview into a NIP-22 root scope using the
    /// preview's Rust-owned protocol reference fields.
    pub fn get_artifact_comment_scope(&self, preview: ArtifactPreview) -> CommentScopeOutcome {
        comment_scope_outcome(comments::scope_from_preview(&preview))
    }

    /// Project an artifact record into the room lane reference target used for
    /// highlight and comment reads. Rust owns reference precedence, artifact
    /// identity, and NIP-22 comment keys.
    pub fn get_artifact_reference_target(
        &self,
        artifact: ArtifactRecord,
    ) -> Option<ArtifactReferenceTarget> {
        reference_targets::artifact_reference_target(&artifact)
    }

    /// Project a highlight into the room lane reference bucket used for live
    /// updates. Native shells only use this to place the raw delta.
    pub fn get_highlight_reference_target(
        &self,
        highlight: HighlightRecord,
    ) -> Option<HighlightReferenceTarget> {
        reference_targets::highlight_reference_target(&highlight)
    }

    /// Build the visible community-home artifact lanes from bounded screen
    /// inputs. Rust owns artifact/highlight matching, de-duplication, dormant
    /// filtering, and activity ordering.
    pub fn build_visible_room_lanes(
        &self,
        artifacts: Vec<ArtifactRecord>,
        highlights: Vec<HydratedHighlight>,
        highlights_by_reference: Vec<HighlightReferenceBucket>,
        comments_by_reference: Vec<CommentReferenceBucket>,
    ) -> Vec<RoomLane> {
        room_lanes::build_visible_room_lanes(
            &artifacts,
            &highlights,
            &highlights_by_reference,
            &comments_by_reference,
        )
    }

    /// Build the visible NIP-22 comment thread from a bounded screen record
    /// set. Rust owns parent resolution, orphan promotion, and chronological
    /// child ordering.
    pub fn build_comment_thread(
        &self,
        records: Vec<CommentRecord>,
        root_tag_value: String,
    ) -> Vec<CommentThreadNode> {
        comments::build_thread(&records, &root_tag_value)
    }

    /// Project an optimistically published comment into the current bounded
    /// thread state. Rust owns comment duplicate suppression and tree rebuild.
    pub fn insert_comment_and_build_thread(
        &self,
        records: Vec<CommentRecord>,
        comment: CommentRecord,
        root_tag_value: String,
    ) -> CommentThreadProjection {
        comments::insert_comment_and_build_thread(&records, &comment, root_tag_value.trim())
    }

    /// Upsert a live room artifact delta into a bounded screen collection.
    /// Rust owns replacement identity and newest-first ordering.
    pub fn upsert_room_artifact(
        &self,
        artifacts: Vec<ArtifactRecord>,
        artifact: ArtifactRecord,
    ) -> Vec<ArtifactRecord> {
        room_state::upsert_room_artifact(&artifacts, &artifact)
    }

    /// Upsert a live room highlight delta into a bounded screen collection.
    /// Rust owns replacement identity and newest-first ordering.
    pub fn upsert_room_highlight(
        &self,
        highlights: Vec<HydratedHighlight>,
        highlight: HydratedHighlight,
    ) -> Vec<HydratedHighlight> {
        room_state::upsert_room_highlight(&highlights, &highlight)
    }

    /// Upsert a raw highlight into a per-reference bucket. Rust owns
    /// replacement identity and newest-first ordering.
    pub fn upsert_highlight_reference_bucket(
        &self,
        bucket: Vec<HighlightRecord>,
        highlight: HighlightRecord,
    ) -> Vec<HighlightRecord> {
        room_state::upsert_highlight_reference_bucket(&bucket, &highlight)
    }

    /// Count comments for an artifact using Rust-owned reference keys.
    pub fn count_artifact_comments(
        &self,
        artifact: ArtifactRecord,
        comments_by_reference: Vec<CommentReferenceBucket>,
    ) -> u32 {
        room_state::artifact_comment_count(&artifact, &comments_by_reference)
    }

    /// Upsert a live discussion delta into a bounded room discussion list.
    /// Rust owns replacement identity and newest-first ordering.
    pub fn upsert_room_discussion(
        &self,
        discussions: Vec<DiscussionRecord>,
        discussion: DiscussionRecord,
    ) -> Vec<DiscussionRecord> {
        room_state::upsert_room_discussion(&discussions, &discussion)
    }

    /// Upsert a live chat delta into a bounded room chat list. Rust owns
    /// replacement identity and oldest-first ordering.
    pub fn upsert_chat_message(
        &self,
        messages: Vec<ChatMessageRecord>,
        message: ChatMessageRecord,
    ) -> Vec<ChatMessageRecord> {
        room_state::upsert_chat_message(&messages, &message)
    }

    /// Read NIP-22 comments (kind:1111) rooted at a Rust-owned scope.
    pub async fn get_comments_for_scope(
        &self,
        scope: CommentScope,
        limit: u32,
    ) -> CommentListOutcome {
        comment_list_outcome(comments::query_for_scope(self.runtime.ndb(), &scope, limit))
    }

    /// Publish a NIP-22 kind:1111 comment scoped to a Rust-owned root.
    /// `parent_event_id` is `None` for top-level comments and `Some(id)` for
    /// replies (the parent kind:1111 comment).
    pub async fn publish_comment_for_scope(
        &self,
        scope: CommentScope,
        parent_event_id: Option<String>,
        content: String,
    ) -> CommentOutcome {
        let result: Result<CommentRecord, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            let parent = parent_event_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty());
            comments::publish_comment_for_scope(&self.runtime, &scope, parent, content.trim()).await
        }
        .await;
        comment_outcome(result)
    }

    // -- Reactions (NIP-25 kind:7) ---------------------------------------

    /// Rust-owned like summary for a target event. The core classifies the
    /// NIP-25 reaction content and resolves the current user's own like.
    pub async fn get_like_summary_for_event(
        &self,
        target_event_id: String,
        limit: u32,
    ) -> ReactionSummaryOutcome {
        let current_user = self
            .inner
            .read()
            .session
            .current_user()
            .map(|user| user.pubkey);
        reaction_summary_outcome(crate::reactions::query_like_summary_for_event(
            self.runtime.ndb(),
            target_event_id.trim(),
            current_user.as_deref(),
            limit,
        ))
    }

    /// Publish a like targeting a NIP-22 comment. Rust owns both the target
    /// kind and the NIP-25 content marker for "like".
    pub async fn publish_comment_like(
        &self,
        event_id: String,
        author_pubkey_hex: String,
    ) -> ReactionOutcome {
        let result: Result<crate::reactions::ReactionRecord, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            crate::reactions::publish_comment_like(
                &self.runtime,
                event_id.trim(),
                author_pubkey_hex.trim(),
            )
            .await
        }
        .await;
        reaction_outcome(result)
    }

    /// Delete one of the user's own kind:7 reactions via NIP-09.
    pub async fn unpublish_reaction(&self, reaction_event_id: String) -> StringOutcome {
        let result: Result<String, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            crate::reactions::unpublish_reaction(&self.runtime, reaction_event_id.trim()).await
        }
        .await;
        string_outcome(result)
    }

    pub async fn get_user_highlights(
        &self,
        pubkey_hex: String,
        limit: u32,
    ) -> HighlightListOutcome {
        highlight_list_outcome(highlights::query_highlights_by_author(
            self.runtime.ndb(),
            pubkey_hex.trim(),
            limit,
        ))
    }

    pub async fn get_user_communities(&self, pubkey_hex: String) -> CommunityListOutcome {
        community_list_outcome(groups::query_joined_communities_from_ndb(
            self.runtime.ndb(),
            pubkey_hex.trim(),
        ))
    }

    // -- Follow state (kind:3) --

    /// Returns true if the logged-in user's cached contact list currently
    /// includes `target_pubkey_hex`.
    pub async fn is_following(&self, target_pubkey_hex: String) -> BoolOutcome {
        bool_outcome((|| {
            let Some(user) = self.inner.read().session.current_user() else {
                return Err(CoreError::NotAuthenticated);
            };
            follows::is_following(self.runtime.ndb(), &user.pubkey, target_pubkey_hex.trim())
        })())
    }

    /// Publish a new kind:3 that adds (`follow=true`) or removes
    /// (`follow=false`) `target_pubkey_hex` from the logged-in user's contact
    /// list. Returns the new event id, or `None` if already in the desired
    /// state (no republish).
    pub async fn set_follow(
        &self,
        target_pubkey_hex: String,
        follow: bool,
    ) -> OptionalStringOutcome {
        let result: Result<Option<String>, CoreError> = async {
            let follower = {
                let guard = self.inner.read();
                guard
                    .session
                    .current_user()
                    .ok_or(CoreError::NotAuthenticated)?
                    .pubkey
            };
            follows::publish_follow_toggle(
                &self.runtime,
                &follower,
                target_pubkey_hex.trim(),
                follow,
            )
            .await
        }
        .await;
        optional_string_outcome(result)
    }

    /// Recent books across the user's joined communities — drives the
    /// capture-flow book picker. Returns `[]` if no books are cached or the
    /// user isn't logged in.
    pub async fn get_recent_books(&self, limit: u32) -> ArtifactListOutcome {
        artifact_list_outcome((|| {
            let Some(user) = self.inner.read().session.current_user() else {
                return Ok(Vec::new());
            };
            crate::recent_books::query_recent_books(self.runtime.ndb(), &user.pubkey, limit)
        })())
    }

    pub async fn search_artifacts(&self, query: String, limit: u32) -> ArtifactListOutcome {
        artifact_list_outcome(crate::artifacts::search_cached(
            self.runtime.ndb(),
            &query,
            limit,
        ))
    }

    // -- Search: across local nostrdb (all four surfaces) + NIP-50 relay ---

    pub async fn search_highlights(&self, query: String, limit: u32) -> HighlightListOutcome {
        highlight_list_outcome(crate::search::search_highlights(
            self.runtime.ndb(),
            &query,
            limit,
        ))
    }

    pub async fn search_articles(&self, query: String, limit: u32) -> ArticleListOutcome {
        article_list_outcome(crate::search::search_articles(
            self.runtime.ndb(),
            &query,
            limit,
        ))
    }

    pub async fn search_communities(&self, query: String, limit: u32) -> CommunityListOutcome {
        community_list_outcome(crate::search::search_communities(
            self.runtime.ndb(),
            &query,
            limit,
        ))
    }

    pub async fn search_profiles(&self, query: String, limit: u32) -> ProfileListOutcome {
        profile_list_outcome(crate::search::search_profiles(
            self.runtime.ndb(),
            &query,
            limit,
        ))
    }

    /// Resolve the merged set of NIP-50 search relays for the current user —
    /// always includes `wss://relay.highlighter.com`, plus every `relay` tag
    /// from the newest cached kind:10007 (NIP-51 search relay list).
    pub async fn get_search_relays(&self) -> StringListOutcome {
        let user_hex = self
            .inner
            .read()
            .session
            .current_user()
            .map(|u| u.pubkey)
            .unwrap_or_default();
        string_list_outcome(crate::search::query_search_relays(
            self.runtime.ndb(),
            &user_hex,
        ))
    }

    pub async fn get_recent_searches(&self) -> StringListOutcome {
        string_list_outcome(self.recent_searches.all().await)
    }

    pub async fn record_recent_search(&self, query: String) -> StringListOutcome {
        string_list_outcome(self.recent_searches.record(&query).await)
    }

    pub async fn clear_recent_searches(&self) -> StringListOutcome {
        string_list_outcome(self.recent_searches.clear().await)
    }

    /// Open a NIP-50 relay subscription for kind:30023 against the user's
    /// search relays. Returns a handle; the pump fires
    /// `SearchArticlesUpdated { query }` deltas as matching events ingest,
    /// and the Swift store responds by re-running `search_articles` locally
    /// to merge the new events into its Articles bucket.
    pub async fn subscribe_article_search(&self, query: String) -> SubscriptionOutcome {
        let result: Result<u64, CoreError> = async {
            let trimmed = query.trim().to_string();
            if trimmed.is_empty() {
                return Err(CoreError::InvalidInput(
                    "search query must not be empty".into(),
                ));
            }
            let user_hex = self
                .inner
                .read()
                .session
                .current_user()
                .map(|u| u.pubkey)
                .unwrap_or_default();
            let relays = crate::search::query_search_relays(self.runtime.ndb(), &user_hex)?;
            if relays.is_empty() {
                return Err(CoreError::InvalidInput("no search relays resolved".into()));
            }
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::SearchArticles {
                    query: trimmed,
                    relays,
                },
            )
        }
        .await;
        subscription_outcome(result)
    }

    // -- Bookmarks (NIP-51 kind:10003) -----------------------------------

    /// Return the set of article addresses the user has bookmarked in their
    /// newest kind:10003 list (empty when not logged in or no list cached).
    pub async fn get_bookmarked_article_addresses(&self) -> StringListOutcome {
        let user_hex = self
            .inner
            .read()
            .session
            .current_user()
            .map(|u| u.pubkey)
            .unwrap_or_default();
        string_list_outcome(
            crate::bookmarks::query_bookmarks(self.runtime.ndb(), &user_hex)
                .map(|list| list.addresses),
        )
    }

    /// Read-only predicate: is `address` currently bookmarked for the logged-in
    /// user? Always `false` when no user is logged in.
    pub async fn is_article_bookmarked(&self, address: String) -> BoolOutcome {
        let user_hex = self
            .inner
            .read()
            .session
            .current_user()
            .map(|u| u.pubkey)
            .unwrap_or_default();
        if user_hex.is_empty() {
            return bool_outcome(Ok(false));
        }
        bool_outcome(crate::bookmarks::is_bookmarked(
            self.runtime.ndb(),
            &user_hex,
            &address,
        ))
    }

    /// Toggle `address` in the user's kind:10003 list. Returns the new
    /// membership state — `true` if the address is now bookmarked, `false`
    /// if it was removed.
    pub async fn toggle_article_bookmark(&self, address: String) -> BoolOutcome {
        let user_hex = match self.inner.read().session.current_user().map(|u| u.pubkey) {
            Some(user_hex) => user_hex,
            None => return bool_outcome(Err(CoreError::NotInitialized)),
        };
        bool_outcome(crate::bookmarks::toggle_bookmark(&self.runtime, &user_hex, &address).await)
    }

    /// Read-only predicate: is `event_id_hex` currently bookmarked for
    /// the logged-in user? Always `false` when no user is logged in.
    pub async fn is_event_bookmarked(&self, event_id_hex: String) -> BoolOutcome {
        let user_hex = self
            .inner
            .read()
            .session
            .current_user()
            .map(|u| u.pubkey)
            .unwrap_or_default();
        if user_hex.is_empty() {
            return bool_outcome(Ok(false));
        }
        bool_outcome(crate::bookmarks::is_event_bookmarked(
            self.runtime.ndb(),
            &user_hex,
            &event_id_hex,
        ))
    }

    /// Toggle `event_id_hex` in the user's kind:10003 list (for comments
    /// and other event-id-addressed targets). Returns the new membership
    /// state.
    pub async fn toggle_event_bookmark(&self, event_id_hex: String) -> BoolOutcome {
        let user_hex = match self.inner.read().session.current_user().map(|u| u.pubkey) {
            Some(user_hex) => user_hex,
            None => return bool_outcome(Err(CoreError::NotInitialized)),
        };
        bool_outcome(
            crate::bookmarks::toggle_event_bookmark(&self.runtime, &user_hex, &event_id_hex).await,
        )
    }

    /// Open a live subscription on the current user's kind:10003 bookmark
    /// events. Deltas land on the app-scope bus (`BookmarksUpdated`); the
    /// Swift bookmarks store re-queries on each.
    pub async fn subscribe_bookmarks(&self) -> SubscriptionOutcome {
        subscription_outcome((|| {
            let user_hex = self
                .inner
                .read()
                .session
                .current_user()
                .map(|u| u.pubkey)
                .ok_or(CoreError::NotInitialized)?;
            let pk = PublicKey::from_hex(&user_hex)
                .map_err(|e| CoreError::InvalidInput(format!("invalid user pubkey: {e}")))?;
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::Bookmarks { user_pubkey: pk },
            )
        })())
    }

    // -- NIP-51 Bookmark sets (kind:30003) / Curation sets (kind:30004) -----

    /// Return all kind:30003 bookmark sets authored by the current user.
    pub async fn get_my_bookmark_sets(&self) -> BookmarkSetListOutcome {
        let user_hex = self
            .inner
            .read()
            .session
            .current_user()
            .map(|u| u.pubkey)
            .unwrap_or_default();
        bookmark_set_list_outcome(crate::lists::query_user_sets(
            self.runtime.ndb(),
            &user_hex,
            crate::lists::KIND_BOOKMARK_SETS,
        ))
    }

    /// Resolve the cached article rows referenced by a bookmark/curation set.
    /// Rust owns NIP-33 address parsing and collection ordering.
    pub async fn get_bookmark_set_articles(&self, record: BookmarkSetRecord) -> ArticleListOutcome {
        article_list_outcome(articles::query_articles_for_addresses(
            self.runtime.ndb(),
            &record.article_addresses,
        ))
    }

    /// Resolve cached article rows for the current user's kind:10003 article
    /// bookmark addresses. Rust owns NIP-33 parsing and newest-first ordering.
    pub async fn get_bookmarked_articles(&self, addresses: Vec<String>) -> ArticleListOutcome {
        article_list_outcome(articles::query_articles_for_addresses(
            self.runtime.ndb(),
            &addresses,
        ))
    }

    /// Return all kind:30004 curation sets authored by the current user.
    pub async fn get_my_curation_sets(&self) -> BookmarkSetListOutcome {
        let user_hex = self
            .inner
            .read()
            .session
            .current_user()
            .map(|u| u.pubkey)
            .unwrap_or_default();
        bookmark_set_list_outcome(crate::lists::query_user_sets(
            self.runtime.ndb(),
            &user_hex,
            crate::lists::KIND_CURATION_SETS,
        ))
    }

    /// Return current user's curation sets projected for the bookmark menu.
    /// Rust owns display fallback, current membership, and ordering.
    pub async fn get_curation_menu_items(&self, address: String) -> CurationMenuItemListOutcome {
        let user_hex = self
            .inner
            .read()
            .session
            .current_user()
            .map(|u| u.pubkey)
            .unwrap_or_default();
        curation_menu_item_list_outcome((|| {
            let sets = crate::lists::query_user_sets(
                self.runtime.ndb(),
                &user_hex,
                crate::lists::KIND_CURATION_SETS,
            )?;
            Ok(crate::lists::curation_menu_items_for_address(
                sets, &address,
            ))
        })())
    }

    /// Return kind:30004 curation sets from users the current user follows.
    pub async fn get_following_curation_sets(&self) -> BookmarkSetListOutcome {
        bookmark_set_list_outcome((|| {
            let user_hex = self
                .inner
                .read()
                .session
                .current_user()
                .map(|u| u.pubkey)
                .unwrap_or_default();
            let follows = crate::follows::query_follows(self.runtime.ndb(), &user_hex)?;
            let sets = crate::lists::query_following_curation_sets(self.runtime.ndb(), &follows)?;
            Ok(crate::lists::explorable_curation_sets(
                self.runtime.ndb(),
                sets,
            ))
        })())
    }

    /// Create a new empty kind:30004 curation set with `title`. Returns
    /// the freshly published record so the UI can immediately use its
    /// `id` (d-tag) to add items.
    pub async fn create_curation_set(&self, title: String) -> BookmarkSetOutcome {
        let result: Result<BookmarkSetRecord, CoreError> = async {
            let user_hex = self
                .inner
                .read()
                .session
                .current_user()
                .map(|u| u.pubkey)
                .ok_or(CoreError::NotAuthenticated)?;
            crate::lists::create_curation_set(
                &self.runtime,
                &user_hex,
                title.trim(),
                self.clock.as_ref(),
            )
            .await
        }
        .await;
        bookmark_set_outcome(result)
    }

    /// Idempotently set membership of `address` (NIP-33 a-tag value, e.g.
    /// `"30023:<pubkey>:<d>"`) in the current user's curation set keyed
    /// by `d_tag`. `member == true` ensures presence; `false` ensures
    /// absence. Returns the new membership state.
    pub async fn set_address_in_curation_set(
        &self,
        d_tag: String,
        address: String,
        member: bool,
    ) -> BoolOutcome {
        let result: Result<bool, CoreError> = async {
            let user_hex = self
                .inner
                .read()
                .session
                .current_user()
                .map(|u| u.pubkey)
                .ok_or(CoreError::NotAuthenticated)?;
            crate::lists::set_address_in_curation_set(
                &self.runtime,
                &user_hex,
                d_tag.trim(),
                address.trim(),
                member,
            )
            .await
        }
        .await;
        bool_outcome(result)
    }

    /// Toggle membership of `address` (NIP-33 a-tag value, e.g.
    /// `"30023:<pubkey>:<d>"`) in the current user's curation set keyed
    /// by `d_tag`. Rust owns the current-membership read and returns the
    /// new membership state.
    pub async fn toggle_address_in_curation_set(
        &self,
        d_tag: String,
        address: String,
    ) -> BoolOutcome {
        let result: Result<bool, CoreError> = async {
            let user_hex = self
                .inner
                .read()
                .session
                .current_user()
                .map(|u| u.pubkey)
                .ok_or(CoreError::NotAuthenticated)?;
            crate::lists::toggle_address_in_curation_set(
                &self.runtime,
                &user_hex,
                d_tag.trim(),
                address.trim(),
            )
            .await
        }
        .await;
        bool_outcome(result)
    }

    /// Open a live subscription for the current user's kind:30003/30004 sets.
    /// Delivers `BookmarkSetsUpdated` (view-scoped) on each delta.
    pub async fn subscribe_bookmark_sets(&self) -> SubscriptionOutcome {
        subscription_outcome((|| {
            let user_hex = self
                .inner
                .read()
                .session
                .current_user()
                .map(|u| u.pubkey)
                .ok_or(CoreError::NotInitialized)?;
            let pk = PublicKey::from_hex(&user_hex)
                .map_err(|e| CoreError::InvalidInput(format!("invalid user pubkey: {e}")))?;
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::BookmarkSets { user_pubkey: pk },
            )
        })())
    }

    /// Open a live subscription for kind:30004 sets from followed authors.
    /// Delivers `FollowingCurationSetsUpdated` (view-scoped) on each delta.
    pub async fn subscribe_following_curation_sets(&self) -> SubscriptionOutcome {
        subscription_outcome((|| {
            let user_hex = self
                .inner
                .read()
                .session
                .current_user()
                .map(|u| u.pubkey)
                .ok_or(CoreError::NotInitialized)?;
            let follow_hexes = crate::follows::query_follows(self.runtime.ndb(), &user_hex)?;
            let follows: Vec<PublicKey> = follow_hexes
                .iter()
                .filter_map(|h| PublicKey::from_hex(h).ok())
                .collect();
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::FollowingCurationSets { follows },
            )
        })())
    }

    // -- NIP-B0 Web bookmarks (kind:39701) -----------------------------------

    /// Return all NIP-B0 kind:39701 web bookmarks authored by the current user.
    pub async fn get_my_web_bookmarks(&self) -> WebBookmarkListOutcome {
        let user_hex = self
            .inner
            .read()
            .session
            .current_user()
            .map(|u| u.pubkey)
            .unwrap_or_default();
        web_bookmark_list_outcome(crate::lists::query_user_web_bookmarks(
            self.runtime.ndb(),
            &user_hex,
        ))
    }

    /// Open a live subscription for the current user's NIP-B0 kind:39701 events.
    /// Delivers `WebBookmarksUpdated` (view-scoped) on each delta.
    pub async fn subscribe_web_bookmarks(&self) -> SubscriptionOutcome {
        subscription_outcome((|| {
            let user_hex = self
                .inner
                .read()
                .session
                .current_user()
                .map(|u| u.pubkey)
                .ok_or(CoreError::NotInitialized)?;
            let pk = PublicKey::from_hex(&user_hex)
                .map_err(|e| CoreError::InvalidInput(format!("invalid user pubkey: {e}")))?;
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::WebBookmarks { user_pubkey: pk },
            )
        })())
    }

    pub async fn lookup_isbn(&self, isbn: String) -> ArtifactPreviewOutcome {
        artifact_preview_outcome(self.isbn_previews.lookup(&isbn).await)
    }

    /// Normalize user-entered or scanned ISBN input. Native shells use this
    /// only to enable/route capture UI; Rust remains the source of truth for
    /// ISBN validity and canonical ISBN-13 conversion.
    pub fn normalize_isbn_input(&self, raw: String) -> Option<String> {
        isbn_lookup::normalize_isbn(&raw).ok()
    }

    /// Resolve an ISBN against the bounded recent-book projection already
    /// rendered by the native picker. Rust owns the canonical ISBN reference
    /// matching; native shells only decide how to present the selected record.
    pub fn find_existing_book_for_isbn(
        &self,
        isbn: String,
        recents: Vec<ArtifactRecord>,
    ) -> Option<ArtifactRecord> {
        isbn_lookup::existing_record_for_isbn(&isbn, &recents)
    }

    /// Build the edited ISBN book preview after scan/manual entry. Rust owns
    /// ISBN normalization and the NIP-73 reference fields; native supplies
    /// only the user's edited title/author and optional lookup metadata.
    pub fn build_edited_book_preview(
        &self,
        isbn: String,
        base_preview: Option<ArtifactPreview>,
        title: String,
        author: String,
    ) -> ArtifactPreviewOutcome {
        artifact_preview_outcome(isbn_lookup::edited_book_preview(
            isbn.trim(),
            base_preview,
            &title,
            &author,
        ))
    }

    pub fn reconstruct_ocr_markdown(&self, lines: Vec<crate::ocr::OcrLine>) -> String {
        crate::ocr::reconstruct_markdown(&lines)
    }

    pub fn detect_ocr_active_page(
        &self,
        lines: Vec<crate::ocr::OcrLine>,
    ) -> Option<crate::ocr::OcrPageDetection> {
        crate::ocr::detect_active_page(&lines)
    }

    pub fn crop_ocr_lines(
        &self,
        lines: Vec<crate::ocr::OcrLine>,
        page_rect: crate::ocr::OcrRect,
    ) -> Vec<crate::ocr::OcrLine> {
        crate::ocr::crop_lines(&lines, page_rect)
    }

    pub fn default_highlight_crop_box(
        &self,
        highlight_boxes: Vec<crate::ocr::OcrRect>,
        image_width: f64,
        image_height: f64,
        margin_fraction: f64,
    ) -> Option<crate::ocr::OcrRect> {
        crate::ocr::default_highlight_crop_box(
            &highlight_boxes,
            image_width,
            image_height,
            margin_fraction,
        )
    }

    pub fn sanitize_highlight_crop_box(
        &self,
        crop_box: crate::ocr::OcrRect,
        fallback: Option<crate::ocr::OcrRect>,
    ) -> crate::ocr::OcrRect {
        crate::ocr::sanitize_highlight_crop_box(crop_box, fallback)
    }

    pub fn selectable_ocr_words(
        &self,
        lines: Vec<crate::ocr::OcrLine>,
    ) -> Vec<crate::ocr::OcrWord> {
        crate::ocr::selectable_words(&lines)
    }

    pub fn join_ocr_quote(&self, words: Vec<crate::ocr::OcrWord>) -> String {
        crate::ocr::join_quote(&words)
    }

    pub fn ocr_alt_text(&self, markdown: String) -> String {
        crate::ocr::alt_text_from_markdown(&markdown)
    }

    /// Build an `ArtifactPreview` from a bare URL. Used by the iOS Share
    /// Extension flow — the main app drains the share queue, normalizes each
    /// URL through this, then calls `publish_artifact` to post the kind:11.
    pub async fn build_preview_from_url(&self, url: String) -> ArtifactPreviewOutcome {
        artifact_preview_outcome(crate::artifacts::build_preview(&url))
    }

    /// Fetch OpenGraph + favicon metadata for a web URL. Backed by a
    /// JSON-on-disk cache (7-day positive TTL, 1-hour negative TTL) and
    /// in-flight coalescing — concurrent calls for the same URL share one
    /// HTTP request. Returns `CoreError::NotFound` when the page 404s,
    /// `CoreError::Network` on transport failure.
    pub async fn get_web_metadata(&self, url: String) -> WebMetadataOutcome {
        web_metadata_outcome(web_metadata::get_or_fetch(self.web_metadata.clone(), &url).await)
    }

    pub async fn get_discussions(&self, group_id: String, limit: u32) -> DiscussionListOutcome {
        discussion_list_outcome(crate::discussions::query_for_group(
            self.runtime.ndb(),
            &group_id,
            limit,
        ))
    }

    /// NIP-29 chat messages (kind:9) cached for `group_id`, ordered ascending
    /// by `created_at`. UI can also peek with `limit=1` to detect chat
    /// activity and decide whether to expose the chat tab at all.
    pub async fn get_chat_messages(&self, group_id: String, limit: u32) -> ChatMessageListOutcome {
        chat_message_list_outcome(crate::chat::query_chat_messages(
            self.runtime.ndb(),
            &group_id,
            limit,
        ))
    }

    // -- Feedback (shake-to-share) --

    /// Threads scoped to `coordinate` authored by the current user. Returns
    /// an empty list if not logged in.
    pub async fn get_feedback_threads(&self, coordinate: String) -> FeedbackThreadListOutcome {
        let result: Result<Vec<FeedbackThreadRecord>, CoreError> = (|| {
            let user = match self.inner.read().session.current_user() {
                Some(u) => u,
                None => return Ok(Vec::new()),
            };
            feedback::query_threads(self.runtime.ndb(), &coordinate, &user.pubkey)
        })();
        feedback_thread_list_outcome(result)
    }

    /// Every message in a feedback thread, ordered ascending by `created_at`.
    pub async fn get_feedback_thread_events(
        &self,
        root_event_id: String,
    ) -> FeedbackEventListOutcome {
        feedback_event_list_outcome(feedback::query_thread_events(
            self.runtime.ndb(),
            &root_event_id,
        ))
    }

    /// First `p` tag of the project's kind:31933 event by addressable
    /// coordinate. The shake-to-share composer uses this to pick the agent
    /// pubkey for the root note's `p` tag. `None` if the project event isn't
    /// cached or has no agents.
    pub async fn get_project_first_agent_pubkey(
        &self,
        coordinate: String,
    ) -> OptionalStringOutcome {
        optional_string_outcome(feedback::query_first_agent_pubkey(
            self.runtime.ndb(),
            &coordinate,
        ))
    }

    /// Feedback composer projection shared by new-thread and reply surfaces.
    /// Rust owns submit trimming and send eligibility so each platform shell
    /// renders the same enabled/disabled state.
    pub fn project_feedback_composer(
        &self,
        input: feedback::FeedbackComposerProjectionInput,
    ) -> feedback::FeedbackComposerProjection {
        feedback::feedback_composer_projection(input)
    }

    /// Feedback thread row/detail presentation projection. Rust owns title,
    /// preview, summary, and status fallback rules; native shells keep
    /// localized relative-time formatting and rendering.
    pub fn project_feedback_thread_presentation(
        &self,
        thread: FeedbackThreadRecord,
    ) -> feedback::FeedbackThreadPresentationProjection {
        feedback::feedback_thread_presentation(thread)
    }

    /// Feedback message bubble presentation projection. Rust owns current-user
    /// classification, header grouping, and profile fallback semantics; native
    /// shells keep markdown and time rendering.
    pub fn project_feedback_message_presentation(
        &self,
        input: feedback::FeedbackMessagePresentationInput,
    ) -> feedback::FeedbackMessagePresentationProjection {
        feedback::feedback_message_presentation(input)
    }

    /// Optimistically insert a newly-published feedback root into the thread
    /// list. Rust owns root validation, preview text, dedupe, and ordering.
    pub fn optimistically_insert_feedback_root_thread(
        &self,
        threads: Vec<FeedbackThreadRecord>,
        root_event: FeedbackEventRecord,
    ) -> Vec<FeedbackThreadRecord> {
        feedback::optimistically_insert_root_thread(&threads, &root_event)
    }

    /// Upsert a streamed feedback thread event into the open thread list.
    /// Rust owns replacement identity and oldest-first ordering.
    pub fn upsert_feedback_thread_event(
        &self,
        events: Vec<FeedbackEventRecord>,
        event: FeedbackEventRecord,
    ) -> Vec<FeedbackEventRecord> {
        feedback::upsert_thread_event(&events, &event)
    }

    // -- Writes --

    /// Wrap a local preview for highlight/picture publish paths before a
    /// kind:11 share exists. Rust owns the empty record sentinel fields.
    pub fn get_unpublished_artifact_record(&self, preview: ArtifactPreview) -> ArtifactOutcome {
        artifact_outcome(Ok(crate::artifacts::unpublished_record(preview)))
    }

    pub async fn publish_artifact(
        &self,
        preview: ArtifactPreview,
        group_id: String,
        note: Option<String>,
    ) -> ArtifactOutcome {
        let result: Result<ArtifactRecord, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            crate::artifacts::publish(&self.runtime, preview, &group_id, note.as_deref()).await
        }
        .await;
        artifact_outcome(result)
    }

    pub async fn publish_discussion(
        &self,
        group_id: String,
        title: String,
        body: String,
        attachment: Option<ArtifactPreview>,
    ) -> DiscussionOutcome {
        let result: Result<DiscussionRecord, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            crate::discussions::publish(
                &self.runtime,
                &group_id,
                &title,
                &body,
                attachment,
                self.clock.as_ref(),
            )
            .await
        }
        .await;
        discussion_outcome(result)
    }

    /// Publish a NIP-29 kind:9 chat message into `group_id`. When
    /// `reply_to_event_id` is set, the published event carries a marked
    /// NIP-10 `["e", <id>, "", "reply"]` tag.
    pub async fn publish_chat_message(
        &self,
        group_id: String,
        content: String,
        reply_to_event_id: Option<String>,
    ) -> ChatMessageOutcome {
        let result: Result<ChatMessageRecord, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            crate::chat::publish_chat_message(
                &self.runtime,
                &group_id,
                &content,
                reply_to_event_id.as_deref(),
            )
            .await
        }
        .await;
        chat_message_outcome(result)
    }

    /// Publish a feedback note (kind:1) for the shake-to-share surface. When
    /// `parent_event_id` is `Some`, the note is a reply marked NIP-10 root;
    /// otherwise it's a brand-new thread. `agent_pubkey` is optional — pass
    /// `None` when the project event isn't cached yet (the note still ships,
    /// just without a `p` tag).
    pub async fn publish_feedback_note(
        &self,
        coordinate: String,
        agent_pubkey: Option<String>,
        parent_event_id: Option<String>,
        body: String,
    ) -> FeedbackEventOutcome {
        let result: Result<FeedbackEventRecord, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            feedback::publish_note(
                &self.runtime,
                &coordinate,
                agent_pubkey.as_deref(),
                parent_event_id.as_deref(),
                &body,
            )
            .await
        }
        .await;
        feedback_event_outcome(result)
    }

    pub async fn publish_highlights_and_share(
        &self,
        artifact: ArtifactRecord,
        drafts: Vec<HighlightDraft>,
        target_group_id: String,
    ) -> HighlightListOutcome {
        let result: Result<Vec<HighlightRecord>, CoreError> = async {
            // Guard: user must be logged in.
            let _ = self.require_user_pubkey()?;
            crate::highlights::publish_and_share(&self.runtime, artifact, drafts, &target_group_id)
                .await
        }
        .await;
        highlight_list_outcome(result)
    }

    /// Re-share an existing kind:9802 highlight into a NIP-29 room as a
    /// kind:16 generic repost. Used to surface a friend's highlight (or
    /// your own old one) into a community without re-publishing the
    /// underlying highlight event. The repost carries `["e", id]`,
    /// `["k", "9802"]`, `["p", author]`, and `["h", target_group_id]`
    /// per NIP-18 + NIP-29 conventions. Empty `relay_url` falls back
    /// to the Highlighter relay as the e-tag relay hint.
    pub async fn share_highlight_to_room(
        &self,
        highlight_id: String,
        highlight_author_pubkey_hex: String,
        highlight_relay_url: String,
        target_group_id: String,
    ) -> MutationOutcome {
        let result: Result<(), CoreError> = async {
            let _ = self.require_user_pubkey()?;
            crate::highlights::share_to_community(
                &self.runtime,
                highlight_id.trim(),
                highlight_author_pubkey_hex.trim(),
                highlight_relay_url.trim(),
                target_group_id.trim(),
            )
            .await
        }
        .await;
        mutation_outcome(result)
    }

    /// Publish a solo NIP-84 highlight without a NIP-29 repost. Used by the
    /// article reader's text-selection flow: user highlights → event lands in
    /// their vault; sharing into a community is a later explicit action.
    pub async fn publish_highlight(
        &self,
        draft: HighlightDraft,
        artifact: ArtifactRecord,
    ) -> HighlightOutcome {
        let result: Result<HighlightRecord, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            crate::highlights::publish(&self.runtime, draft, artifact).await
        }
        .await;
        highlight_outcome(result)
    }

    // -- Rooms explorer (discovery + curation + recommendations) --

    /// Install (if not already installed) a long-lived relay sub for every
    /// kind:39000 metadata event. Call once on explorer appear from iOS.
    /// Idempotent; the sub rides until logout.
    pub async fn start_room_discovery(&self) {
        let already = self.inner.read().session.has_discovery_subscription();
        if already {
            return;
        }
        let sub_id = self.runtime.spawn_all_rooms_subscription();
        self.inner
            .write()
            .session
            .set_discovery_subscription(sub_id);
    }

    /// Install (if not already installed) two relay subs that together
    /// power the "Friends are here" explorer shelf:
    ///
    /// 1. kind:10009 authored by any of the user's follows — NIP-51
    ///    user-owned "simple groups" list (denser, always-public signal).
    /// 2. kind:39001 / 39002 where any follow appears in a `p` tag —
    ///    relay-owned membership fallback for groups whose members haven't
    ///    published a 10009 yet.
    ///
    /// No-op if the user isn't logged in or has no follows cached yet.
    /// Idempotent; both subs ride until logout.
    pub async fn start_friends_rooms_discovery(&self) -> MutationOutcome {
        mutation_outcome((|| {
            let (have_memberships, have_groups_list) = {
                let guard = self.inner.read();
                (
                    guard.session.has_friends_memberships_subscription(),
                    guard.session.has_friends_groups_list_subscription(),
                )
            };
            if have_memberships && have_groups_list {
                return Ok(());
            }
            let Some(user) = self.inner.read().session.current_user() else {
                return Ok(());
            };
            let follows_hex = follows::query_follows(self.runtime.ndb(), &user.pubkey)?;
            let follows: Vec<PublicKey> = follows_hex
                .iter()
                .filter_map(|s| PublicKey::from_hex(s.trim()).ok())
                .collect();
            if follows.is_empty() {
                return Ok(());
            }

            if !have_groups_list {
                if let Some(sub_id) = self
                    .runtime
                    .spawn_friends_groups_list_subscription(follows.clone())
                {
                    self.inner
                        .write()
                        .session
                        .set_friends_groups_list_subscription(sub_id);
                }
            }

            if !have_memberships {
                if let Some(sub_id) = self.runtime.spawn_friends_memberships_subscription(follows) {
                    self.inner
                        .write()
                        .session
                        .set_friends_memberships_subscription(sub_id);
                }
            }
            Ok(())
        })())
    }

    /// Install (if not already installed) the kind:10012 curated-list sub for
    /// `curator_pubkey_hex`. Once the list lands in ndb, this method also
    /// spawns a metadata backfill for every group the list references, so a
    /// subsequent `get_featured_rooms` returns rich summaries rather than
    /// bare ids. Idempotent; the sub rides until logout.
    pub async fn start_featured_rooms(&self, curator_pubkey_hex: String) -> MutationOutcome {
        mutation_outcome((|| {
            let curator = PublicKey::from_hex(curator_pubkey_hex.trim())
                .map_err(|e| CoreError::InvalidInput(format!("invalid curator pubkey: {e}")))?;

            let already = self.inner.read().session.has_curation_subscription();
            if !already {
                let sub_id = self.runtime.spawn_curated_list_subscription(curator);
                self.inner.write().session.set_curation_subscription(sub_id);
            }

            // Even if the sub was already installed, ensure any groups the
            // currently-cached list references have their 39000s backfilled —
            // the relay may have delivered the list but not the metadata.
            let group_ids_from_list = {
                let ndb = self.runtime.ndb();
                // Reuse fetch_curated_rooms' internals indirectly by asking for
                // the list's ids. A full fetch is cheap; we only need ids here.
                match curation::fetch_curated_rooms_from_ndb(ndb, curator_pubkey_hex.trim()) {
                    Ok(summaries) => summaries.into_iter().map(|c| c.id).collect::<Vec<_>>(),
                    Err(_) => Vec::new(),
                }
            };
            if !group_ids_from_list.is_empty() {
                self.runtime
                    .spawn_group_metadata_subscription(group_ids_from_list);
            }
            Ok(())
        })())
    }

    pub async fn get_room_explorer_curator_pubkey(&self) -> StringOutcome {
        string_outcome(self.room_explorer_config.curator_pubkey().await)
    }

    pub async fn start_room_explorer_featured_rooms(&self) -> MutationOutcome {
        let result: Result<(), CoreError> = async {
            let curator_pubkey = self.room_explorer_config.refresh_curator_pubkey().await?;
            let outcome = self.start_featured_rooms(curator_pubkey).await;
            if outcome.error.is_empty() {
                Ok(())
            } else {
                Err(CoreError::Other(outcome.error))
            }
        }
        .await;
        mutation_outcome(result)
    }

    /// Curator's latest kind:10012 list, resolved into `CommunitySummary`
    /// items in curator-chosen order. Rooms without cached metadata are
    /// dropped; the next call after `start_featured_rooms` has backfilled
    /// metadata returns the full list.
    pub async fn get_featured_rooms(&self, curator_pubkey_hex: String) -> CommunityListOutcome {
        community_list_outcome(curation::fetch_curated_rooms_from_ndb(
            self.runtime.ndb(),
            curator_pubkey_hex.trim(),
        ))
    }

    /// Every cached room, newest first, truncated to `limit`. Powers the
    /// explorer's "Browse all" grid.
    pub async fn get_all_rooms(&self, limit: u32) -> CommunityListOutcome {
        community_list_outcome(discovery::query_all_rooms_from_ndb(
            self.runtime.ndb(),
            limit,
        ))
    }

    /// The N most-recently-seen rooms. Same underlying query as
    /// `get_all_rooms` with a tighter limit — kept as a distinct method so
    /// the Swift explorer store's shelves remain single-purpose.
    pub async fn get_new_rooms(&self, limit: u32) -> CommunityListOutcome {
        community_list_outcome(discovery::query_all_rooms_from_ndb(
            self.runtime.ndb(),
            limit,
        ))
    }

    /// Remove rooms the user has already joined while preserving discovery
    /// order. Rust owns explorer shelf duplicate suppression.
    pub fn exclude_joined_rooms(
        &self,
        rooms: Vec<CommunitySummary>,
        joined: Vec<CommunitySummary>,
    ) -> Vec<CommunitySummary> {
        discovery::exclude_joined_rooms(&rooms, &joined)
    }

    /// Filter already-projected rooms by user query. Rust owns the search
    /// normalization and match fields for the browse-all room grid.
    pub fn search_rooms(
        &self,
        rooms: Vec<CommunitySummary>,
        query: String,
    ) -> Vec<CommunitySummary> {
        discovery::search_rooms(&rooms, &query)
    }

    /// Rooms where 2+ of the user's follows are members. Empty when the user
    /// isn't logged in, has no follows cached, or no room satisfies the
    /// threshold.
    pub async fn get_rooms_with_friends(&self, limit: u32) -> RoomRecommendationListOutcome {
        let Some(user) = self.inner.read().session.current_user() else {
            return RoomRecommendationListOutcome {
                values: Vec::new(),
                error: String::new(),
            };
        };
        room_recommendation_list_outcome(recommendations::query_rooms_with_friends(
            self.runtime.ndb(),
            &user.pubkey,
            limit,
        ))
    }

    /// Rooms where authors of articles the user has highlighted post
    /// artifacts. Empty when the user hasn't highlighted any articles yet.
    pub async fn get_rooms_from_read_authors(&self, limit: u32) -> RoomRecommendationListOutcome {
        let Some(user) = self.inner.read().session.current_user() else {
            return RoomRecommendationListOutcome {
                values: Vec::new(),
                error: String::new(),
            };
        };
        room_recommendation_list_outcome(recommendations::query_rooms_from_read_authors(
            self.runtime.ndb(),
            &user.pubkey,
            limit,
        ))
    }

    /// Publish a NIP-29 kind:9021 join-request for `group_id`. Rust owns the
    /// pending-join state and emits app toast deltas for request sent,
    /// request failure, and later membership confirmation.
    pub async fn request_join_room(&self, group_id: String, room_name: String) -> StringOutcome {
        let group_id = group_id.trim().to_string();
        self.record_pending_join(&group_id, &room_name);
        let result: Result<String, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            groups::publish_join_request(&self.runtime, &group_id).await
        }
        .await;
        match &result {
            Ok(_) if self.has_pending_join(&group_id) => {
                self.emit_app_toast("Join requested".to_string());
            }
            Ok(_) => {}
            Err(error) => {
                self.remove_pending_join(&group_id);
                self.emit_app_toast(error.to_string());
            }
        }
        string_outcome(result)
    }

    /// Create a brand-new NIP-29 room. Publishes kind:9007 (create-group) and
    /// kind:9002 (edit-metadata) signed by the current user. Returns the
    /// freshly-generated group id on success — the relay's 39000/39001/39002
    /// follow-up events drive the iOS membership stream automatically.
    pub fn project_create_room(
        &self,
        input: groups::CreateRoomProjectionInput,
    ) -> groups::CreateRoomProjection {
        groups::create_room_projection(input)
    }

    pub fn project_room_avatar(
        &self,
        input: groups::RoomAvatarProjectionInput,
    ) -> groups::RoomAvatarProjection {
        groups::room_avatar_projection(input)
    }

    pub fn project_room_recommendation_card(
        &self,
        input: recommendations::RoomRecommendationCardProjectionInput,
    ) -> recommendations::RoomRecommendationCardProjection {
        recommendations::room_recommendation_card_projection(input)
    }

    pub fn project_room_preview_artifacts(
        &self,
        input: crate::room_preview::RoomPreviewArtifactsProjectionInput,
    ) -> crate::room_preview::RoomPreviewArtifactsProjection {
        crate::room_preview::room_preview_artifacts_projection(input)
    }

    pub async fn create_room(
        &self,
        name: String,
        about: String,
        picture: String,
        visibility: groups::RoomVisibility,
        access: groups::RoomAccess,
    ) -> StringOutcome {
        let result: Result<String, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            groups::create_room(
                &self.runtime,
                name.trim(),
                about.trim(),
                picture.trim(),
                visibility,
                access,
            )
            .await
        }
        .await;
        string_outcome(result)
    }

    /// Add a Nostr user (by hex pubkey) to a room as a member. Must be
    /// signed by a room admin — the relay enforces this. Returns the
    /// kind:9000 event id on success.
    pub async fn add_room_member(&self, group_id: String, pubkey_hex: String) -> StringOutcome {
        let result: Result<String, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            groups::add_member(&self.runtime, group_id.trim(), pubkey_hex.trim()).await
        }
        .await;
        string_outcome(result)
    }

    /// Mint `count` single-use invite codes for `group_id` by publishing a
    /// kind:9009 event. Must be signed by an admin — the relay rejects
    /// non-admin attempts. Returns the minted codes in order.
    pub async fn create_room_invite_codes(
        &self,
        group_id: String,
        count: u32,
    ) -> StringListOutcome {
        let result: Result<Vec<String>, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            groups::create_invite_codes(&self.runtime, group_id.trim(), count).await
        }
        .await;
        string_list_outcome(result)
    }

    /// Decode a Nostr identifier (`npub1…`, `nprofile1…`, optionally with a
    /// `nostr:` URI prefix) to a 64-char hex pubkey. Returns
    /// `CoreError::InvalidInput` if the input isn't a recognised pubkey
    /// reference. Used by the room-invite picker to resolve a pasted handle.
    pub fn decode_npub(&self, input: String) -> StringOutcome {
        let result =
            crate::room_invites::decode_pubkey_reference(&input).map(|(pubkey_hex, _)| pubkey_hex);
        string_outcome(result)
    }

    pub fn get_room_invite_projection(
        &self,
        input: crate::room_invites::RoomInviteProjectionInput,
    ) -> crate::room_invites::RoomInviteProjection {
        crate::room_invites::project_invite(input)
    }

    pub fn get_room_invite_avatar_projection(
        &self,
        input: crate::room_invites::RoomInviteAvatarProjectionInput,
    ) -> crate::room_invites::RoomInviteAvatarProjection {
        crate::room_invites::avatar_projection(input)
    }

    pub fn get_room_invite_add_decision(
        &self,
        pubkey_hex: String,
        selected_pubkeys: Vec<String>,
        current_user_pubkey: String,
    ) -> crate::room_invites::RoomInviteAddDecision {
        crate::room_invites::add_decision(&pubkey_hex, &selected_pubkeys, &current_user_pubkey)
    }

    pub fn get_room_invite_send_result(
        &self,
        selected: Vec<crate::room_invites::RoomInviteCandidate>,
        failed_pubkeys: Vec<String>,
    ) -> crate::room_invites::RoomInviteSendResultProjection {
        crate::room_invites::project_send_result(&selected, &failed_pubkeys)
    }

    /// Classify a NIP-19 entity (`npub1…`, `nprofile1…`, `note1…`,
    /// `nevent1…`, `naddr1…`) into a renderable variant. Strips an
    /// optional `nostr:` URI prefix. Used by the iOS rich-text renderer
    /// to walk event content for inline mentions and event-ref cards.
    pub fn decode_nostr_entity(&self, input: String) -> NostrEntityRefOutcome {
        nostr_entity_ref_outcome(crate::nostr_entities::decode_nostr_entity(&input))
    }

    pub fn nostr_entity_fallback_label(
        &self,
        entity: crate::nostr_entities::NostrEntityRef,
    ) -> String {
        crate::nostr_entities::fallback_label(&entity)
    }

    pub fn nostr_entity_inline_render(
        &self,
        entity: crate::nostr_entities::NostrEntityRef,
    ) -> crate::nostr_entities::NostrEntityInlineRender {
        crate::nostr_entities::inline_render(&entity)
    }

    pub fn nostr_entity_identity_key(
        &self,
        entity: crate::nostr_entities::NostrEntityRef,
    ) -> String {
        crate::nostr_entities::identity_key(&entity)
    }

    pub fn tokenize_nostr_content(
        &self,
        content: String,
    ) -> Vec<crate::nostr_entities::NostrContentRun> {
        crate::nostr_entities::tokenize_nostr_content(&content)
    }

    pub fn tokenize_nostr_markdown_inline(
        &self,
        content: String,
    ) -> Vec<crate::nostr_entities::NostrContentRun> {
        crate::nostr_entities::tokenize_nostr_markdown_inline(&content)
    }

    pub fn standalone_nostr_entity(
        &self,
        content: String,
    ) -> Option<crate::nostr_entities::NostrEntityRef> {
        crate::nostr_entities::standalone_nostr_entity(&content)
    }

    pub fn extract_nostr_event_refs(
        &self,
        content: String,
    ) -> Vec<crate::nostr_entities::NostrEntityRef> {
        crate::nostr_entities::extract_event_refs(&content)
    }

    /// Mint a NIP-19 `nevent` for a kind:9802 highlight share link. The
    /// canonical relay hint is Rust policy; native shells provide only the
    /// event id and author hint they are already rendering.
    pub fn encode_highlight_share_nevent(
        &self,
        event_id_hex: String,
        author_pubkey_hex: String,
    ) -> StringOutcome {
        string_outcome(crate::nostr_entities::encode_event_to_nevent(
            event_id_hex,
            Some(author_pubkey_hex),
            vec![crate::relays::highlighter_relay().to_string()],
            Some(9802),
        ))
    }

    /// Best-effort cache lookup for a [`NostrEntityRef`]. Returns the
    /// resolved event when nostrdb already has it, `None` otherwise.
    /// The caller should pair this with `subscribe_nostr_entity` so a
    /// cold-cache reference warms up over the wire.
    pub async fn resolve_nostr_entity(
        &self,
        entity: crate::nostr_entities::NostrEntityRef,
    ) -> NostrEntityEventOutcome {
        nostr_entity_event_outcome(crate::nostr_entities::resolve_from_cache(
            self.runtime.ndb(),
            &entity,
        ))
    }

    /// Install a view-scoped subscription for the missing event behind an
    /// entity. Routes to relay hints first (when the bech32 carried any) plus
    /// the indexer pool. Events received are persisted to nostrdb via the
    /// `NdbDatabase` bridge; the subscription pump emits
    /// `NostrEntityResolved` when the target lands.
    pub async fn subscribe_nostr_entity(
        &self,
        entity: crate::nostr_entities::NostrEntityRef,
    ) -> SubscriptionOutcome {
        subscription_outcome((|| {
            let _ = self.require_user_pubkey()?;
            let _ = crate::nostr_entities::relay_filter(&entity)?;
            self.subscriptions
                .register(&self.runtime, SubscriptionKind::NostrEntity { entity })
        })())
    }

    /// Pubkeys (hex) the current user follows per their cached kind:3 contact
    /// list. Empty if the user isn't logged in or the cache hasn't seen a
    /// kind:3 yet. Used by the room-invite picker to surface "people you know"
    /// before any typing happens.
    pub async fn get_follows(&self) -> StringListOutcome {
        string_list_outcome((|| {
            let user_pubkey = self.require_user_pubkey()?;
            crate::follows::query_follows(self.runtime.ndb(), &user_pubkey.to_hex())
        })())
    }

    // -- Blossom (BUD-03, kind:10063) --

    /// Return the user's ordered Blossom server list from nostrdb. Empty if no
    /// kind:10063 has been cached yet (relay hasn't delivered it).
    pub async fn get_blossom_servers(&self) -> StringListOutcome {
        string_list_outcome((|| {
            let user = self
                .inner
                .read()
                .session
                .current_user()
                .ok_or(CoreError::NotAuthenticated)?;
            blossom::query_blossom_servers(self.runtime.ndb(), &user.pubkey)
        })())
    }

    /// Replace the user's Blossom server list with `servers` (must be
    /// non-empty). Order is preserved — first server is the upload default.
    pub async fn set_blossom_servers(&self, servers: Vec<String>) -> StringOutcome {
        let result: Result<String, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            blossom::publish_blossom_servers(&self.runtime, servers).await
        }
        .await;
        string_outcome(result)
    }

    /// Publish the default Blossom server list only if the user has no cached
    /// kind:10063. Called once after login; no-op when the list already exists.
    pub async fn init_default_blossom_servers(&self) -> MutationOutcome {
        let result: Result<(), CoreError> = async {
            let user = self
                .inner
                .read()
                .session
                .current_user()
                .ok_or(CoreError::NotAuthenticated)?;
            blossom::init_default_blossom_servers(&self.runtime, &user.pubkey).await
        }
        .await;
        mutation_outcome(result)
    }

    // -- Capture flow (BUD-01 upload + kind:20 picture publish) --

    /// Upload a photo to the default Blossom server (`blossom.primal.net`)
    /// using BUD-01 auth. The caller (iOS) is responsible for stripping EXIF
    /// metadata and recompressing the image before sending bytes — Rust does
    /// not decode the image. `width`/`height` are stamped onto the returned
    /// descriptor for use in the publishing event's `imeta` tag.
    /// `alt` is the recognized OCR text, or empty if none.
    pub async fn upload_photo(
        &self,
        bytes: Vec<u8>,
        mime: String,
        width: u32,
        height: u32,
        alt: String,
    ) -> BlossomUploadOutcome {
        let result: Result<BlossomUpload, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            blossom::upload_blob(
                &self.runtime,
                bytes,
                mime,
                width,
                height,
                alt,
                self.clock.as_ref(),
            )
            .await
        }
        .await;
        blossom_upload_outcome(result)
    }

    /// Publish a NIP-68 `kind:20` picture event into a NIP-29 community.
    /// Used by the capture flow when the user opts not to (or can't) extract
    /// a highlight quote — the photo still ships to the community with all
    /// the imeta metadata.
    pub async fn publish_picture(&self, draft: PictureDraft) -> PictureOutcome {
        let result: Result<PictureRecord, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            crate::pictures::publish_picture(&self.runtime, draft).await
        }
        .await;
        picture_outcome(result)
    }

    // -- Relay config (NIP-65 read/write + NIP-78 rooms/indexer) --

    /// Return the user's effective relay list, merging NIP-65 (read/write)
    /// with NIP-78 app-data (rooms/indexer). Falls back to `seed_defaults()`
    /// when neither has been cached yet (first login).
    pub async fn get_relays(&self) -> RelayConfigListOutcome {
        relay_config_list_outcome((|| {
            let user = self
                .inner
                .read()
                .session
                .current_user()
                .ok_or(CoreError::NotAuthenticated)?;
            crate::relays::query_relays(self.runtime.ndb(), &user.pubkey)
        })())
    }

    /// Insert-or-update a single relay. Replaces the row with matching URL or
    /// appends a new one, re-publishes kind:10002 + kind:30078, and reconciles
    /// the live relay pool so the change takes effect immediately.
    pub async fn upsert_relay(&self, cfg: crate::relays::RelayConfig) -> MutationOutcome {
        let result: Result<(), CoreError> = async {
            let user = self
                .inner
                .read()
                .session
                .current_user()
                .ok_or(CoreError::NotAuthenticated)?;
            crate::relays::upsert_relay(&self.runtime, &user.pubkey, cfg).await?;
            self.runtime.spawn_apply_user_relay_config(user.pubkey);
            Ok(())
        }
        .await;
        mutation_outcome(result)
    }

    /// Remove a relay by URL.
    pub async fn remove_relay(&self, url: String) -> MutationOutcome {
        let result: Result<(), CoreError> = async {
            let user = self
                .inner
                .read()
                .session
                .current_user()
                .ok_or(CoreError::NotAuthenticated)?;
            crate::relays::remove_relay(&self.runtime, &user.pubkey, url).await?;
            self.runtime.spawn_apply_user_relay_config(user.pubkey);
            Ok(())
        }
        .await;
        mutation_outcome(result)
    }

    /// Atomically update a single relay's role flags.
    pub async fn set_relay_roles(
        &self,
        url: String,
        read: bool,
        write: bool,
        rooms: bool,
        indexer: bool,
    ) -> MutationOutcome {
        let result: Result<(), CoreError> = async {
            let user = self
                .inner
                .read()
                .session
                .current_user()
                .ok_or(CoreError::NotAuthenticated)?;
            crate::relays::set_relay_roles(
                &self.runtime,
                &user.pubkey,
                url,
                read,
                write,
                rooms,
                indexer,
            )
            .await?;
            self.runtime.spawn_apply_user_relay_config(user.pubkey);
            Ok(())
        }
        .await;
        mutation_outcome(result)
    }

    // -- Relay telemetry --

    /// Snapshot of the live per-relay diagnostics map. One row per URL
    /// currently in the client's pool. Refreshed by the background
    /// diagnostics poller at least once per second.
    pub async fn get_relay_diagnostics(&self) -> RelayDiagnosticListOutcome {
        relay_diagnostic_list_outcome(Ok(self.runtime.relay_diagnostics_snapshot()))
    }

    pub fn auto_connected_relay_config(&self, url: String) -> crate::relays::RelayConfig {
        crate::relays::auto_connected_display_config(url)
    }

    pub fn project_relay_settings(
        &self,
        configured_relays: Vec<crate::relays::RelayConfig>,
        diagnostics: Vec<RelayDiagnostic>,
    ) -> crate::relays::RelaySettingsProjection {
        crate::relays::settings_projection(&configured_relays, &diagnostics)
    }

    pub fn project_relay_row(
        &self,
        input: crate::relays::RelayRowProjectionInput,
    ) -> crate::relays::RelayRowProjection {
        crate::relays::relay_row_projection(input)
    }

    pub fn project_relay_detail(
        &self,
        input: crate::relays::RelayDetailProjectionInput,
    ) -> crate::relays::RelayDetailProjection {
        crate::relays::relay_detail_projection(input)
    }

    pub fn project_relay_remove(
        &self,
        input: crate::relays::RelayRemoveProjectionInput,
    ) -> crate::relays::RelayRemoveProjection {
        crate::relays::relay_remove_projection(input)
    }

    pub fn default_add_relay_config(&self) -> crate::relays::RelayConfig {
        crate::relays::default_add_relay_config()
    }

    pub fn project_add_relay_sheet(
        &self,
        input: crate::relays::AddRelaySheetProjectionInput,
    ) -> crate::relays::AddRelaySheetProjection {
        crate::relays::add_relay_sheet_projection(input)
    }

    pub fn default_import_relay_selection(
        &self,
        relays: Vec<crate::relays::RelayConfig>,
    ) -> Vec<String> {
        crate::relays::default_import_relay_selection(relays)
    }

    pub fn project_import_relays(
        &self,
        input: crate::relays::ImportRelaysProjectionInput,
    ) -> crate::relays::ImportRelaysProjection {
        crate::relays::import_relays_projection(input)
    }

    /// Handle the Swift side uses to match `RelayStatusChanged` deltas on the
    /// event bus. Relay status changes are app-scoped and ride
    /// `subscription_id == 0`, so this returns `0` unconditionally — the
    /// value is a stable contract, not a unique sub id.
    pub async fn subscribe_relay_status(&self) -> SubscriptionOutcome {
        subscription_outcome(Ok(0))
    }

    /// Nudge the relay pool to attempt a reconnect on every disconnected
    /// relay. `Client::connect` is idempotent — already-connected relays
    /// are unaffected; disconnected / terminated / banned relays get a
    /// fresh WebSocket attempt.
    pub async fn reconnect_all(&self) -> MutationOutcome {
        self.runtime.client().connect().await;
        mutation_outcome(Ok(()))
    }

    /// Close every WebSocket in the pool. Used by the Wi-Fi-only toggle
    /// when the device drops off Wi-Fi — the Swift side re-enables by
    /// calling `reconnect_all` once the path monitor reports Wi-Fi back.
    pub async fn disconnect_all(&self) -> MutationOutcome {
        self.runtime.client().disconnect().await;
        mutation_outcome(Ok(()))
    }

    /// Fetch the target relay's NIP-11 information document via an HTTPS
    /// GET to the `ws[s]://` URL's HTTP equivalent with
    /// `Accept: application/nostr+json`. Fails fast on timeout.
    pub async fn probe_relay_nip11(&self, url: String) -> Nip11DocumentOutcome {
        nip11_document_outcome(crate::relay_polish::probe_nip11(&url).await)
    }

    /// Fetch another user's kind:10002 via the indexer pool and return the
    /// parsed `RelayConfig` rows. Useful for "adopt someone else's relay
    /// setup" flows — the Swift caller shows the list with checkboxes
    /// and upserts the selected subset through `upsert_relay`.
    pub async fn import_relays_from_npub(&self, npub: String) -> RelayConfigListOutcome {
        relay_config_list_outcome(crate::relay_polish::import_from_npub(&self.runtime, &npub).await)
    }

    /// Size + event-count snapshot of the local nostrdb cache. Order-of-
    /// magnitude figures used by the Network Settings "Local cache" card.
    pub async fn get_cache_stats(&self) -> CacheStatsOutcome {
        cache_stats_outcome(crate::relay_polish::cache_stats(
            self.runtime.ndb(),
            self.runtime.data_dir(),
        ))
    }
}

impl HighlighterCore {
    fn emit_app_toast(&self, message: String) {
        if message.trim().is_empty() {
            return;
        }
        let cb = { self.callback_slot.read().clone() };
        if let Some(cb) = cb {
            cb.on_data_changed(Delta {
                subscription_id: 0,
                change: DataChangeType::AppToastRequested { message },
            });
        }
    }

    fn record_pending_join(&self, group_id: &str, room_name: &str) {
        let group_id = group_id.trim();
        if group_id.is_empty() {
            return;
        }
        self.inner
            .write()
            .pending_joins
            .insert(group_id.to_string(), join_room_display_name(room_name));
    }

    fn remove_pending_join(&self, group_id: &str) -> Option<String> {
        self.inner.write().pending_joins.remove(group_id.trim())
    }

    fn has_pending_join(&self, group_id: &str) -> bool {
        self.inner
            .read()
            .pending_joins
            .contains_key(group_id.trim())
    }

    /// Internal access for feature modules (artifacts, groups, highlights,
    /// recent_books) to the shared Client + Ndb. Not exposed to Swift.
    #[allow(dead_code)]
    pub(crate) fn runtime(&self) -> &NostrRuntime {
        &self.runtime
    }

    /// Construct with an isolated nostrdb path. Used by tests to avoid
    /// polluting the real application data dir. Not annotated with
    /// `#[uniffi::export]`, so it stays out of the Swift surface.
    #[doc(hidden)]
    pub fn new_with_data_dir(data_dir: std::path::PathBuf) -> Arc<Self> {
        let runtime = Arc::new(
            NostrRuntime::with_data_dir(data_dir)
                .expect("nostr runtime initialization must succeed"),
        );
        Self::assemble(runtime)
    }

    fn assemble(runtime: Arc<NostrRuntime>) -> Arc<Self> {
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        Self::assemble_with_clock(runtime, clock)
    }

    fn assemble_with_clock(runtime: Arc<NostrRuntime>, clock: Arc<dyn Clock>) -> Arc<Self> {
        let callback_slot: Arc<RwLock<Option<Arc<dyn EventCallback>>>> =
            Arc::new(RwLock::new(None));
        let subscriptions = Arc::new(SubscriptionRegistry::new(callback_slot.clone()));
        // Start the diagnostics poller before handing out the Arc<Self>.
        // The callback slot starts empty; the poller updates its in-memory
        // map regardless, and fires deltas once Swift installs a callback
        // via `set_event_callback`.
        runtime.spawn_diagnostics_poller(callback_slot.clone());
        let web_metadata = Arc::new(WebMetadataStore::open_with_clock(
            runtime.data_dir(),
            clock.clone(),
        ));
        let isbn_previews = Arc::new(isbn_lookup::IsbnPreviewCache::new(runtime.data_dir()));
        let recent_searches = Arc::new(recent_searches::RecentSearchesStore::new(
            runtime.data_dir(),
        ));
        let room_explorer_config = Arc::new(room_explorer_config::RoomExplorerConfigStore::new(
            runtime.data_dir(),
        ));
        let whats_new = Arc::new(whats_new::WhatsNewStore::new(runtime.data_dir()));
        let onboarding = Arc::new(onboarding::OnboardingStore::new(runtime.data_dir()));
        let network_preferences = Arc::new(network_preferences::NetworkPreferencesStore::new(
            runtime.data_dir(),
        ));
        let podcast_position = Arc::new(podcast_position::PodcastPositionStore::new(
            runtime.data_dir(),
        ));
        Arc::new(Self {
            inner: Arc::new(RwLock::new(Inner {
                session: Session::new(),
                pending_joins: BTreeMap::new(),
            })),
            runtime,
            callback_slot,
            subscriptions,
            web_metadata,
            isbn_previews,
            recent_searches,
            room_explorer_config,
            whats_new,
            onboarding,
            network_preferences,
            podcast_position,
            clock,
        })
    }

    /// Drop the previous follows-NIP-65 sub (if any) and install a new one
    /// covering `follows`. Also fires a fresh purplepag.es negentropy sync
    /// for kind:0/3/10002 — cheap when most events are already cached
    /// (negentropy only ships the deltas) and the right thing to do when
    /// the follow set may have grown since last call. No-op when `follows`
    /// is empty.
    fn refresh_follows_nip65_subscription(&self, follows: &[PublicKey]) {
        if follows.is_empty() {
            return;
        }
        self.runtime
            .spawn_negentropy_sync_for_follows(follows.to_vec());
        let new_id = match self
            .runtime
            .spawn_follows_relay_lists_subscription(follows.to_vec())
        {
            Some(id) => id,
            None => return,
        };
        let prev = {
            let mut guard = self.inner.write();
            let prev = guard.session.take_follows_nip65_subscription();
            guard.session.set_follows_nip65_subscription(new_id);
            prev
        };
        if let Some(prev) = prev {
            self.runtime.drop_subscription(prev);
        }
    }

    fn require_user_pubkey(&self) -> Result<PublicKey, CoreError> {
        let guard = self.inner.read();
        let user = guard
            .session
            .current_user()
            .ok_or(CoreError::NotAuthenticated)?;
        PublicKey::from_hex(&user.pubkey)
            .map_err(|e| CoreError::Other(format!("invalid current user pubkey: {e}")))
    }
}

/// Read the cached kind:3 contact list for `user_pubkey` and return the
/// `p`-tag pubkeys that successfully parse. Used at login to seed the
/// follows-NIP-65 subscription before the user touches a home feed.
fn current_followed_pubkeys(ndb: &nostrdb::Ndb, user_pubkey: &PublicKey) -> Vec<PublicKey> {
    let user_hex = user_pubkey.to_hex();
    let hexes = match crate::follows::query_follows(ndb, &user_hex) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "load cached follows for outbox bootstrap");
            return Vec::new();
        }
    };
    hexes
        .iter()
        .filter_map(|s| PublicKey::from_hex(s.trim()).ok())
        .collect()
}

/// Strip a leading `nostr:` prefix, trim whitespace. Olas does this before
/// handing a URI to `NDKBunkerSigner.bunker(...)`.
pub(crate) fn normalize_bunker_uri(input: &str) -> String {
    let t = input.trim();
    t.strip_prefix("nostr:").unwrap_or(t).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{channel, Sender};
    use std::time::Duration;

    struct ChannelCallback {
        tx: Sender<Delta>,
    }

    impl EventCallback for ChannelCallback {
        fn on_data_changed(&self, delta: Delta) {
            self.tx.send(delta).expect("send delta");
        }
    }

    #[test]
    fn confirm_pending_join_emits_rust_owned_toast_once() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let core = HighlighterCore::new_with_data_dir(tmp.path().join("ndb"));
        let (tx, rx) = channel();
        core.set_event_callback(Arc::new(ChannelCallback { tx }));

        core.record_pending_join("alpha", " Alpha ");
        core.confirm_pending_join("alpha".to_string());

        let delta = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("toast delta");
        match delta.change {
            DataChangeType::AppToastRequested { message } => {
                assert_eq!(message, "You're in Alpha ✓");
            }
            other => panic!("expected toast, got {other:?}"),
        }

        core.confirm_pending_join("alpha".to_string());
        assert!(
            rx.recv_timeout(Duration::from_millis(100)).is_err(),
            "pending join should be consumed once"
        );
    }
}
