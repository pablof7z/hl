//! NMP-style app facade for Highlighter.
//!
//! This is the canonical app boundary for platform shells: one opaque handle,
//! fire-and-forget actions, and bounded snapshots. The lower-level
//! [`HighlighterCore`](crate::client::HighlighterCore) is an internal runtime
//! engine owned by this actor, not a platform API.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use parking_lot::RwLock;

use crate::client::HighlighterCore;
use crate::events::{DataChangeType, Delta, EventCallback};
use crate::groups::{RoomAccess, RoomVisibility};
use crate::models::{
    ArticleRecord, ArtifactPreview, ArtifactRecord, BlossomUpload, BookmarkSetRecord, CacheStats,
    ChatMessageRecord, CommentRecord, CommunitySummary, CurrentUser, DiscussionRecord,
    FeedbackEventRecord, FeedbackThreadRecord, HighlightDraft, HighlightRecord, HydratedHighlight,
    Nip11Document, NostrConnectOptions, PictureDraft, ProfileMetadata, ProfileUpdateDraft,
    ReadingFeedItem, RelayDiagnostic, RelayStatus, RoomRecommendation, WebBookmarkRecord,
};
use crate::nmp_runtime::SignerRequestDrain;
use crate::relays::RelayConfig;
use crate::web_metadata::WebMetadata;

const ACTION_QUEUE_CAPACITY: usize = 256;
const DEFAULT_VISIBLE_LIMIT: u32 = 64;
const DEFAULT_EMIT_HZ: u32 = 30;
const NOSTR_CONNECT_NAME: &str = "Highlighter";
const NOSTR_CONNECT_URL: &str = "https://highlighter.com";
const NOSTR_CONNECT_IMAGE: &str = "https://highlighter.com/icon.png";
const NOSTR_CONNECT_PERMS: &str = "sign_event:11,sign_event:1111,sign_event:9802,sign_event:16,nip04_encrypt,nip04_decrypt,nip44_encrypt,nip44_decrypt";
const SEARCH_HIGHLIGHT_LIMIT: u32 = 30;
const SEARCH_ARTICLE_LIMIT: u32 = 30;
const SEARCH_COMMUNITY_LIMIT: u32 = 20;
const SEARCH_PROFILE_LIMIT: u32 = 20;
const NIP05_API_URL: &str = "https://beta.highlighter.com/api/nip05";
const ROOM_EXPLORER_CURATOR_PUBKEY_HEX: &str =
    "7e1eabe25256545cfe0c534a99bfa5c6cd224e04b614182a9993feff54196c95";
const ROOM_EXPLORER_FEATURED_LIMIT: u32 = 16;
const ROOM_EXPLORER_NEW_LIMIT: u32 = 24;
const ROOM_EXPLORER_RECOMMENDATION_LIMIT: u32 = 16;
const ROOM_EXPLORER_BROWSE_LIMIT: u32 = 200;
const HOME_FEED_ITEM_LIMIT: usize = 80;
const HOME_FEED_HIGHLIGHT_QUERY_LIMIT: u32 = 120;
const HOME_FEED_READ_QUERY_LIMIT: u32 = 80;
const HOME_FEED_GROUP_HIGHLIGHT_LIMIT: usize = 6;
const ROOM_DETAIL_ARTIFACT_LIMIT: u32 = 48;
const ROOM_DETAIL_HIGHLIGHT_LIMIT: u32 = 96;
const ROOM_DETAIL_DISCUSSION_LIMIT: u32 = 96;
const ROOM_DETAIL_REFERENCE_LIMIT: u32 = 128;
const ROOM_DETAIL_CHAT_PAGE_SIZE: u32 = 50;
const ROOM_DETAIL_CHAT_MAX_LIMIT: u32 = 250;
const ROOM_INVITE_VISIBLE_FOLLOW_LIMIT: usize = 50;
const ROOM_INVITE_PROFILE_PREFETCH_LIMIT: usize = 40;
const COMMENTS_LIMIT: u32 = 128;
const COMMENT_REACTION_LIMIT: u32 = 128;
const FEEDBACK_THREAD_LIMIT: u32 = 64;
const FEEDBACK_EVENT_LIMIT: u32 = 256;
const NETWORK_NIP11_LIMIT: usize = 80;
const NETWORK_IMPORT_LIMIT: usize = 128;
const BOOKMARK_ARTICLE_LIMIT: usize = 80;
const BOOKMARK_COLLECTION_LIMIT: usize = 80;
const BOOKMARK_WEB_LIMIT: usize = 80;
const BOOKMARK_DETAIL_ARTICLE_LIMIT: usize = 120;
const CURATION_MENU_SET_LIMIT: usize = 64;
const RECENT_SEARCH_LIMIT: usize = 8;
const PROFILE_ARTICLE_LIMIT: usize = 48;
const PROFILE_HIGHLIGHT_LIMIT: usize = 64;
const PROFILE_COMMUNITY_LIMIT: usize = 64;
const ARTICLE_READER_HIGHLIGHT_LIMIT: usize = 128;
const RELAY_REMOVAL_ROOM_NAME_LIMIT: usize = 5;
const WHATS_NEW_VISIBLE_LIMIT: usize = 8;
const WHATS_NEW_JSON: &str = include_str!("whats_new.json");

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterAppConfig {
    /// Platform-resolved app-private data directory. When absent, Rust uses
    /// the platform default used by the compatibility core.
    pub data_dir: Option<String>,
    /// Upper bound for app-chrome lists in snapshots.
    pub visible_limit: u32,
    /// Maximum update cadence requested by the shell. This facade only emits
    /// after state changes; the value is carried so shells can share the same
    /// config vocabulary as NMP.
    pub emit_hz: u32,
    /// Test/diagnostic seam (design §4.6): override the bundled relay policy
    /// JSON so a harness can point every relay role at a controlled URL (e.g.
    /// a black-hole relay). Additive uniffi field with a `None` default so
    /// existing Swift/Kotlin constructors keep compiling. Applied once,
    /// process-wide, before the relay policy is first read.
    #[uniffi(default = None)]
    pub relay_policy_json: Option<String>,
}

impl HighlighterAppConfig {
    fn normalized_visible_limit(&self) -> usize {
        self.visible_limit
            .clamp(1, 250)
            .try_into()
            .unwrap_or(DEFAULT_VISIBLE_LIMIT as usize)
    }

    fn normalized_emit_hz(&self) -> u32 {
        self.emit_hz.clamp(1, 60)
    }
}

impl Default for HighlighterAppConfig {
    fn default() -> Self {
        Self {
            data_dir: None,
            visible_limit: DEFAULT_VISIBLE_LIMIT,
            emit_hz: DEFAULT_EMIT_HZ,
            relay_policy_json: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum HighlighterConnectionState {
    Unknown,
    Connecting,
    Online,
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum HighlighterToastKind {
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterToast {
    pub kind: HighlighterToastKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum HighlighterUsernameStatus {
    Idle,
    Checking,
    Available,
    Taken,
    Invalid,
    Error,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterCreateAccountSnapshot {
    pub display_name: String,
    pub username: String,
    pub username_status: HighlighterUsernameStatus,
    pub username_identifier: String,
    pub username_domain: String,
    pub is_creating: bool,
    pub can_submit: bool,
    pub error_message: Option<String>,
    pub created_user: Option<CurrentUser>,
}

impl HighlighterCreateAccountSnapshot {
    fn empty() -> Self {
        Self {
            display_name: String::new(),
            username: String::new(),
            username_status: HighlighterUsernameStatus::Idle,
            username_identifier: String::new(),
            username_domain: String::new(),
            is_creating: false,
            can_submit: false,
            error_message: None,
            created_user: None,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterCreateRoomSnapshot {
    pub cover_upload: Option<BlossomUpload>,
    pub is_cover_uploading: bool,
    pub is_creating: bool,
    pub created_group_id: Option<String>,
    pub error_message: Option<String>,
}

impl HighlighterCreateRoomSnapshot {
    fn empty() -> Self {
        Self {
            cover_upload: None,
            is_cover_uploading: false,
            is_creating: false,
            created_group_id: None,
            error_message: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum HighlighterRoomInviteCandidateSource {
    Follow,
    Paste,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum HighlighterRoomInvitePastedKind {
    Npub,
    Nprofile,
    Hex,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterRoomInviteCandidate {
    pub pubkey_hex: String,
    pub source: HighlighterRoomInviteCandidateSource,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterRoomInviteResolvedCandidate {
    pub pubkey_hex: String,
    pub kind: HighlighterRoomInvitePastedKind,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterRoomInviteSnapshot {
    pub group_id: String,
    pub query: String,
    pub visible_follows: Vec<String>,
    pub follow_count: u64,
    pub pasted_candidate: Option<HighlighterRoomInviteResolvedCandidate>,
    pub selected: Vec<HighlighterRoomInviteCandidate>,
    pub is_loading_follows: bool,
    pub is_minting_invite_link: bool,
    pub invite_url: Option<String>,
    pub invite_link_error_message: Option<String>,
    pub is_adding_members: bool,
    pub add_error_message: Option<String>,
    pub toast_message: Option<String>,
}

impl HighlighterRoomInviteSnapshot {
    fn empty() -> Self {
        Self {
            group_id: String::new(),
            query: String::new(),
            visible_follows: Vec::new(),
            follow_count: 0,
            pasted_candidate: None,
            selected: Vec::new(),
            is_loading_follows: false,
            is_minting_invite_link: false,
            invite_url: None,
            invite_link_error_message: None,
            is_adding_members: false,
            add_error_message: None,
            toast_message: None,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterCommentInteraction {
    pub event_id: String,
    pub like_count: u64,
    pub my_like_event_id: Option<String>,
    pub is_bookmarked: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterCommentDraft {
    pub parent_event_id: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterCommentChildLinks {
    pub event_id: String,
    pub child_event_ids: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterCommentsSnapshot {
    pub root_tag_name: String,
    pub root_tag_value: String,
    pub root_kind: u16,
    pub records: Vec<CommentRecord>,
    pub record_count: u64,
    pub top_level_event_ids: Vec<String>,
    pub child_links: Vec<HighlighterCommentChildLinks>,
    pub interactions: Vec<HighlighterCommentInteraction>,
    pub drafts: Vec<HighlighterCommentDraft>,
    pub is_loading: bool,
    pub error_message: Option<String>,
    pub is_publishing: bool,
    pub publish_error_message: Option<String>,
    pub last_published_event_id: Option<String>,
    pub interaction_error_message: Option<String>,
}

impl HighlighterCommentsSnapshot {
    fn empty() -> Self {
        Self {
            root_tag_name: String::new(),
            root_tag_value: String::new(),
            root_kind: 0,
            records: Vec::new(),
            record_count: 0,
            top_level_event_ids: Vec::new(),
            child_links: Vec::new(),
            interactions: Vec::new(),
            drafts: Vec::new(),
            is_loading: false,
            error_message: None,
            is_publishing: false,
            publish_error_message: None,
            last_published_event_id: None,
            interaction_error_message: None,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterFeedbackSnapshot {
    pub coordinate: String,
    pub threads: Vec<FeedbackThreadRecord>,
    pub thread_count: u64,
    pub is_loading_threads: bool,
    pub threads_error_message: Option<String>,
    pub selected_root_event_id: Option<String>,
    pub selected_events: Vec<FeedbackEventRecord>,
    pub selected_event_count: u64,
    pub is_loading_thread: bool,
    pub thread_error_message: Option<String>,
    pub new_thread_draft: String,
    pub reply_draft: String,
    pub is_publishing_new_thread: bool,
    pub is_publishing_reply: bool,
    pub publish_error_message: Option<String>,
    pub last_published_root_event_id: Option<String>,
}

impl HighlighterFeedbackSnapshot {
    fn empty() -> Self {
        Self {
            coordinate: String::new(),
            threads: Vec::new(),
            thread_count: 0,
            is_loading_threads: false,
            threads_error_message: None,
            selected_root_event_id: None,
            selected_events: Vec::new(),
            selected_event_count: 0,
            is_loading_thread: false,
            thread_error_message: None,
            new_thread_draft: String::new(),
            reply_draft: String::new(),
            is_publishing_new_thread: false,
            is_publishing_reply: false,
            publish_error_message: None,
            last_published_root_event_id: None,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterMediaSettingsSnapshot {
    pub blossom_servers: Vec<String>,
    pub blossom_server_count: u64,
    pub is_loading: bool,
    pub is_saving: bool,
    pub error_message: Option<String>,
}

impl HighlighterMediaSettingsSnapshot {
    fn empty() -> Self {
        Self {
            blossom_servers: Vec::new(),
            blossom_server_count: 0,
            is_loading: false,
            is_saving: false,
            error_message: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum HighlighterEditProfileImageTarget {
    Picture,
    Banner,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterEditProfileSnapshot {
    pub display_name: String,
    pub name: String,
    pub about: String,
    pub picture: String,
    pub banner: String,
    pub nip05: String,
    pub website: String,
    pub lud16: String,
    pub is_picture_uploading: bool,
    pub is_banner_uploading: bool,
    pub is_saving: bool,
    pub error_message: Option<String>,
    pub saved_profile: Option<ProfileMetadata>,
}

impl HighlighterEditProfileSnapshot {
    fn empty() -> Self {
        Self {
            display_name: String::new(),
            name: String::new(),
            about: String::new(),
            picture: String::new(),
            banner: String::new(),
            nip05: String::new(),
            website: String::new(),
            lud16: String::new(),
            is_picture_uploading: false,
            is_banner_uploading: false,
            is_saving: false,
            error_message: None,
            saved_profile: None,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterAuthSnapshot {
    pub is_signing_in: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterChromeSnapshot {
    pub current_user: Option<CurrentUser>,
    pub current_user_profile: Option<ProfileMetadata>,
    pub joined_communities: Vec<CommunitySummary>,
    pub joined_communities_total: u64,
    pub bookmarked_article_addresses: Vec<String>,
    pub bookmarked_article_address_count: u64,
    pub connection_state: HighlighterConnectionState,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterWhatsNewEntry {
    pub shipped_at: String,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterWhatsNewSnapshot {
    pub entries: Vec<HighlighterWhatsNewEntry>,
    pub entry_count: u64,
    pub last_seen_at: Option<String>,
}

impl HighlighterWhatsNewSnapshot {
    fn empty() -> Self {
        Self {
            entries: Vec::new(),
            entry_count: 0,
            last_seen_at: None,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterShareExtensionCommunity {
    pub id: String,
    pub name: String,
    pub picture: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterShareExtensionSnapshot {
    pub communities: Vec<HighlighterShareExtensionCommunity>,
    pub community_count: u64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterShareComposerSnapshot {
    pub is_publishing: bool,
    pub publishing_group_id: Option<String>,
    pub error_message: Option<String>,
    pub published_group_id: Option<String>,
}

impl HighlighterShareComposerSnapshot {
    fn empty() -> Self {
        Self {
            is_publishing: false,
            publishing_group_id: None,
            error_message: None,
            published_group_id: None,
        }
    }
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum HighlighterCaptureArtifact {
    Existing { record: ArtifactRecord },
    Pending { preview: ArtifactPreview },
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterCaptureSnapshot {
    pub upload: Option<BlossomUpload>,
    pub is_uploading: bool,
    pub upload_error_message: Option<String>,
    pub is_publishing: bool,
    pub published_event_id: Option<String>,
    pub error_message: Option<String>,
}

impl HighlighterCaptureSnapshot {
    fn empty() -> Self {
        Self {
            upload: None,
            is_uploading: false,
            upload_error_message: None,
            is_publishing: false,
            published_event_id: None,
            error_message: None,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterBookPickerSnapshot {
    pub recent_books: Vec<ArtifactRecord>,
    pub recent_book_count: u64,
    pub search_query: String,
    pub search_results: Vec<ArtifactRecord>,
    pub search_result_count: u64,
    pub is_loading_recents: bool,
    pub is_searching: bool,
    pub error_message: Option<String>,
}

impl HighlighterBookPickerSnapshot {
    fn empty() -> Self {
        Self {
            recent_books: Vec::new(),
            recent_book_count: 0,
            search_query: String::new(),
            search_results: Vec::new(),
            search_result_count: 0,
            is_loading_recents: false,
            is_searching: false,
            error_message: None,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterOnboardingInterest {
    pub id: String,
    pub emoji: String,
    pub label: String,
    pub selected: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterOnboardingSnapshot {
    pub is_complete: bool,
    pub interests: Vec<HighlighterOnboardingInterest>,
    pub selected_interest_ids: Vec<String>,
    pub minimum_selection_count: u32,
    pub remaining_selection_count: u32,
    pub can_finish: bool,
    pub is_finishing: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterIsbnPreview {
    pub isbn: String,
    pub preview: ArtifactPreview,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterWebMetadata {
    pub url: String,
    pub metadata: WebMetadata,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterProfile {
    pub pubkey_hex: String,
    pub metadata: ProfileMetadata,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterProfileViewSnapshot {
    pub pubkey_hex: String,
    pub viewer_pubkey_hex: Option<String>,
    pub profile: Option<ProfileMetadata>,
    pub articles: Vec<ArticleRecord>,
    pub article_count: u64,
    pub highlights: Vec<HighlightRecord>,
    pub highlight_count: u64,
    pub communities: Vec<CommunitySummary>,
    pub community_count: u64,
    pub is_following: bool,
    pub is_own_profile: bool,
    pub is_mutating_follow: bool,
    pub is_loading: bool,
    pub error_message: Option<String>,
}

impl HighlighterProfileViewSnapshot {
    fn empty() -> Self {
        Self {
            pubkey_hex: String::new(),
            viewer_pubkey_hex: None,
            profile: None,
            articles: Vec::new(),
            article_count: 0,
            highlights: Vec::new(),
            highlight_count: 0,
            communities: Vec::new(),
            community_count: 0,
            is_following: false,
            is_own_profile: false,
            is_mutating_follow: false,
            is_loading: false,
            error_message: None,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterArticleReaderSnapshot {
    pub pubkey_hex: String,
    pub d_tag: String,
    pub address: String,
    pub article: Option<ArticleRecord>,
    pub author_profile: Option<ProfileMetadata>,
    pub highlights: Vec<HighlightRecord>,
    pub highlight_count: u64,
    pub is_loading: bool,
    pub is_publishing_highlight: bool,
    pub last_published_highlight_id: Option<String>,
    pub error_message: Option<String>,
}

impl HighlighterArticleReaderSnapshot {
    fn empty() -> Self {
        Self {
            pubkey_hex: String::new(),
            d_tag: String::new(),
            address: String::new(),
            article: None,
            author_profile: None,
            highlights: Vec::new(),
            highlight_count: 0,
            is_loading: false,
            is_publishing_highlight: false,
            last_published_highlight_id: None,
            error_message: None,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterReferenceHighlightBucket {
    pub key: String,
    pub tag_name: String,
    pub tag_value: String,
    pub highlights: Vec<HighlightRecord>,
    pub highlight_count: u64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterReferenceCommentBucket {
    pub key: String,
    pub tag_name: String,
    pub tag_value: String,
    pub comments: Vec<CommentRecord>,
    pub comment_count: u64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterRoomDetailSnapshot {
    pub group_id: String,
    pub artifacts: Vec<ArtifactRecord>,
    pub artifact_count: u64,
    pub highlights: Vec<HydratedHighlight>,
    pub highlight_count: u64,
    pub discussions: Vec<DiscussionRecord>,
    pub discussion_count: u64,
    pub is_publishing_discussion: bool,
    pub discussion_error_message: Option<String>,
    pub last_published_discussion_id: Option<String>,
    pub chat_messages: Vec<ChatMessageRecord>,
    pub chat_message_count: u64,
    pub chat_has_more: bool,
    pub is_chat_loading_more: bool,
    pub is_sending_chat_message: bool,
    pub chat_error_message: Option<String>,
    pub highlights_by_reference: Vec<HighlighterReferenceHighlightBucket>,
    pub reference_highlight_count: u64,
    pub comments_by_reference: Vec<HighlighterReferenceCommentBucket>,
    pub reference_comment_count: u64,
    pub is_loading: bool,
    pub error_message: Option<String>,
}

impl HighlighterRoomDetailSnapshot {
    fn empty() -> Self {
        Self {
            group_id: String::new(),
            artifacts: Vec::new(),
            artifact_count: 0,
            highlights: Vec::new(),
            highlight_count: 0,
            discussions: Vec::new(),
            discussion_count: 0,
            is_publishing_discussion: false,
            discussion_error_message: None,
            last_published_discussion_id: None,
            chat_messages: Vec::new(),
            chat_message_count: 0,
            chat_has_more: false,
            is_chat_loading_more: false,
            is_sending_chat_message: false,
            chat_error_message: None,
            highlights_by_reference: Vec::new(),
            reference_highlight_count: 0,
            comments_by_reference: Vec::new(),
            reference_comment_count: 0,
            is_loading: false,
            error_message: None,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterSearchSnapshot {
    pub query: String,
    pub applied_query: String,
    pub recent_queries: Vec<String>,
    pub recent_query_count: u64,
    pub highlights: Vec<HighlightRecord>,
    pub highlight_count: u64,
    pub articles: Vec<ArticleRecord>,
    pub article_count: u64,
    pub communities: Vec<CommunitySummary>,
    pub community_count: u64,
    pub profiles: Vec<ProfileMetadata>,
    pub profile_count: u64,
    pub is_local_loading: bool,
    pub is_relay_loading: bool,
    pub search_relays: Vec<String>,
}

impl HighlighterSearchSnapshot {
    fn empty() -> Self {
        Self {
            query: String::new(),
            applied_query: String::new(),
            recent_queries: Vec::new(),
            recent_query_count: 0,
            highlights: Vec::new(),
            highlight_count: 0,
            articles: Vec::new(),
            article_count: 0,
            communities: Vec::new(),
            community_count: 0,
            profiles: Vec::new(),
            profile_count: 0,
            is_local_loading: false,
            is_relay_loading: false,
            search_relays: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum HighlighterHomeFeedItemKind {
    Highlights,
    Read,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterHomeReadItem {
    pub pubkey: String,
    pub identifier: String,
    pub title: String,
    pub summary: String,
    pub image: String,
    pub first_hashtag: Option<String>,
    pub published_at: Option<u64>,
    pub created_at: Option<u64>,
    pub author_followed: bool,
    pub interactor_pubkeys: Vec<String>,
    pub read_time_minutes: Option<u32>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterHomeFeedItem {
    pub kind: HighlighterHomeFeedItemKind,
    pub stable_id: String,
    pub sort_key: u64,
    pub highlights: Vec<HydratedHighlight>,
    pub highlight_count: u64,
    pub read: Option<HighlighterHomeReadItem>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterHomeFeedSnapshot {
    pub items: Vec<HighlighterHomeFeedItem>,
    pub item_count: u64,
    pub is_loading: bool,
    pub error_message: Option<String>,
}

impl HighlighterHomeFeedSnapshot {
    fn empty() -> Self {
        Self {
            items: Vec::new(),
            item_count: 0,
            is_loading: false,
            error_message: None,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterBookmarkCollectionDetailSnapshot {
    pub collection: Option<BookmarkSetRecord>,
    pub articles: Vec<ArticleRecord>,
    pub article_count: u64,
    pub has_note_items: bool,
    pub is_loading: bool,
    pub error_message: Option<String>,
}

impl HighlighterBookmarkCollectionDetailSnapshot {
    fn empty() -> Self {
        Self {
            collection: None,
            articles: Vec::new(),
            article_count: 0,
            has_note_items: false,
            is_loading: false,
            error_message: None,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterBookmarksSnapshot {
    pub articles: Vec<ArticleRecord>,
    pub article_count: u64,
    pub my_bookmark_sets: Vec<BookmarkSetRecord>,
    pub my_bookmark_set_count: u64,
    pub my_curation_sets: Vec<BookmarkSetRecord>,
    pub my_curation_set_count: u64,
    pub web_bookmarks: Vec<WebBookmarkRecord>,
    pub web_bookmark_count: u64,
    pub following_curation_sets: Vec<BookmarkSetRecord>,
    pub following_curation_set_count: u64,
    pub selected_collection: HighlighterBookmarkCollectionDetailSnapshot,
    pub is_loading: bool,
    pub error_message: Option<String>,
}

impl HighlighterBookmarksSnapshot {
    fn empty() -> Self {
        Self {
            articles: Vec::new(),
            article_count: 0,
            my_bookmark_sets: Vec::new(),
            my_bookmark_set_count: 0,
            my_curation_sets: Vec::new(),
            my_curation_set_count: 0,
            web_bookmarks: Vec::new(),
            web_bookmark_count: 0,
            following_curation_sets: Vec::new(),
            following_curation_set_count: 0,
            selected_collection: HighlighterBookmarkCollectionDetailSnapshot::empty(),
            is_loading: false,
            error_message: None,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterCurationMenuSnapshot {
    pub article_address: String,
    pub curation_sets: Vec<BookmarkSetRecord>,
    pub curation_set_count: u64,
    pub is_loading: bool,
    /// True while a membership write / set creation is in flight off-actor.
    pub is_saving: bool,
    pub error_message: Option<String>,
}

impl HighlighterCurationMenuSnapshot {
    fn empty() -> Self {
        Self {
            article_address: String::new(),
            curation_sets: Vec::new(),
            curation_set_count: 0,
            is_loading: false,
            is_saving: false,
            error_message: None,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterRoomExplorerSnapshot {
    pub featured: Vec<CommunitySummary>,
    pub featured_count: u64,
    pub new_noteworthy: Vec<CommunitySummary>,
    pub new_noteworthy_count: u64,
    pub friends_shelf: Vec<RoomRecommendation>,
    pub friends_shelf_count: u64,
    pub authors_shelf: Vec<RoomRecommendation>,
    pub authors_shelf_count: u64,
    pub all_rooms: Vec<CommunitySummary>,
    pub all_room_count: u64,
    pub curator_pubkey_hex: String,
    pub is_loading: bool,
    pub is_browse_loading: bool,
    /// Group id of an in-flight join request; empty when none. Set before the
    /// off-actor join op is submitted, cleared when its outcome resolves, so
    /// the UI can show per-room joining feedback for the up-to-30s wait.
    pub joining_group_id: String,
    pub error_message: Option<String>,
}

impl HighlighterRoomExplorerSnapshot {
    fn empty() -> Self {
        Self {
            featured: Vec::new(),
            featured_count: 0,
            new_noteworthy: Vec::new(),
            new_noteworthy_count: 0,
            friends_shelf: Vec::new(),
            friends_shelf_count: 0,
            authors_shelf: Vec::new(),
            authors_shelf_count: 0,
            all_rooms: Vec::new(),
            all_room_count: 0,
            curator_pubkey_hex: ROOM_EXPLORER_CURATOR_PUBKEY_HEX.into(),
            is_loading: false,
            is_browse_loading: false,
            joining_group_id: String::new(),
            error_message: None,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterRelayRemovalImpact {
    pub relay_url: String,
    pub room_names: Vec<String>,
    pub room_count: u64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterRelayNip11Snapshot {
    pub url: String,
    pub document: Option<Nip11Document>,
    pub is_loading: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterNetworkImportSnapshot {
    pub npub: String,
    pub candidates: Vec<RelayConfig>,
    pub candidate_count: u64,
    pub selected_urls: Vec<String>,
    pub is_fetching: bool,
    pub is_applying: bool,
    pub error_message: Option<String>,
}

impl HighlighterNetworkImportSnapshot {
    fn empty() -> Self {
        Self {
            npub: String::new(),
            candidates: Vec::new(),
            candidate_count: 0,
            selected_urls: Vec::new(),
            is_fetching: false,
            is_applying: false,
            error_message: None,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterNetworkSnapshot {
    pub wifi_only_enabled: bool,
    pub current_path_is_wifi: Option<bool>,
    pub relays: Vec<RelayConfig>,
    pub relay_count: u64,
    pub auto_connected_relays: Vec<RelayConfig>,
    pub auto_connected_relay_count: u64,
    pub diagnostics: Vec<RelayDiagnostic>,
    pub diagnostic_count: u64,
    pub nip11: Vec<HighlighterRelayNip11Snapshot>,
    pub cache_stats: Option<CacheStats>,
    pub connected_count: u64,
    pub visible_relay_count: u64,
    pub has_outbox: bool,
    pub is_loading: bool,
    pub is_saving: bool,
    pub error_message: Option<String>,
    pub action_error_message: Option<String>,
    pub import_relays: HighlighterNetworkImportSnapshot,
    pub relay_removal_impacts: Vec<HighlighterRelayRemovalImpact>,
}

impl HighlighterNetworkSnapshot {
    fn empty() -> Self {
        Self {
            wifi_only_enabled: false,
            current_path_is_wifi: None,
            relays: Vec::new(),
            relay_count: 0,
            auto_connected_relays: Vec::new(),
            auto_connected_relay_count: 0,
            diagnostics: Vec::new(),
            diagnostic_count: 0,
            nip11: Vec::new(),
            cache_stats: None,
            connected_count: 0,
            visible_relay_count: 0,
            has_outbox: false,
            is_loading: false,
            is_saving: false,
            error_message: None,
            action_error_message: None,
            import_relays: HighlighterNetworkImportSnapshot::empty(),
            relay_removal_impacts: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlighterAppState {
    pub rev: u64,
    pub is_bootstrapping: bool,
    pub create_account: HighlighterCreateAccountSnapshot,
    pub create_room: HighlighterCreateRoomSnapshot,
    pub room_invite: HighlighterRoomInviteSnapshot,
    pub comments: HighlighterCommentsSnapshot,
    pub feedback: HighlighterFeedbackSnapshot,
    pub media_settings: HighlighterMediaSettingsSnapshot,
    pub edit_profile: HighlighterEditProfileSnapshot,
    pub auth: HighlighterAuthSnapshot,
    pub chrome: HighlighterChromeSnapshot,
    pub whats_new: HighlighterWhatsNewSnapshot,
    pub share_extension: HighlighterShareExtensionSnapshot,
    pub share_composer: HighlighterShareComposerSnapshot,
    pub capture: HighlighterCaptureSnapshot,
    pub book_picker: HighlighterBookPickerSnapshot,
    pub onboarding: HighlighterOnboardingSnapshot,
    pub isbn_previews: Vec<HighlighterIsbnPreview>,
    pub isbn_preview_count: u64,
    pub web_metadata: Vec<HighlighterWebMetadata>,
    pub web_metadata_count: u64,
    pub reference_highlights: Vec<HighlighterReferenceHighlightBucket>,
    pub reference_highlight_count: u64,
    pub profiles: Vec<HighlighterProfile>,
    pub profile_count: u64,
    pub profile_view: HighlighterProfileViewSnapshot,
    pub article_reader: HighlighterArticleReaderSnapshot,
    pub room_detail: HighlighterRoomDetailSnapshot,
    pub home_feed: HighlighterHomeFeedSnapshot,
    pub bookmarks: HighlighterBookmarksSnapshot,
    pub curation_menu: HighlighterCurationMenuSnapshot,
    pub search: HighlighterSearchSnapshot,
    pub room_explorer: HighlighterRoomExplorerSnapshot,
    pub network: HighlighterNetworkSnapshot,
    pub toast: Option<HighlighterToast>,
}

impl HighlighterAppState {
    fn empty(onboarding_complete: bool) -> Self {
        Self {
            rev: 0,
            is_bootstrapping: false,
            create_account: HighlighterCreateAccountSnapshot::empty(),
            create_room: HighlighterCreateRoomSnapshot::empty(),
            room_invite: HighlighterRoomInviteSnapshot::empty(),
            comments: HighlighterCommentsSnapshot::empty(),
            feedback: HighlighterFeedbackSnapshot::empty(),
            media_settings: HighlighterMediaSettingsSnapshot::empty(),
            edit_profile: HighlighterEditProfileSnapshot::empty(),
            auth: HighlighterAuthSnapshot {
                is_signing_in: false,
            },
            chrome: HighlighterChromeSnapshot {
                current_user: None,
                current_user_profile: None,
                joined_communities: Vec::new(),
                joined_communities_total: 0,
                bookmarked_article_addresses: Vec::new(),
                bookmarked_article_address_count: 0,
                connection_state: HighlighterConnectionState::Unknown,
            },
            whats_new: HighlighterWhatsNewSnapshot::empty(),
            share_extension: HighlighterShareExtensionSnapshot {
                communities: Vec::new(),
                community_count: 0,
            },
            share_composer: HighlighterShareComposerSnapshot::empty(),
            capture: HighlighterCaptureSnapshot::empty(),
            book_picker: HighlighterBookPickerSnapshot::empty(),
            onboarding: onboarding_snapshot(onboarding_complete, BTreeSet::new(), false),
            isbn_previews: Vec::new(),
            isbn_preview_count: 0,
            web_metadata: Vec::new(),
            web_metadata_count: 0,
            reference_highlights: Vec::new(),
            reference_highlight_count: 0,
            profiles: Vec::new(),
            profile_count: 0,
            profile_view: HighlighterProfileViewSnapshot::empty(),
            article_reader: HighlighterArticleReaderSnapshot::empty(),
            room_detail: HighlighterRoomDetailSnapshot::empty(),
            home_feed: HighlighterHomeFeedSnapshot::empty(),
            bookmarks: HighlighterBookmarksSnapshot::empty(),
            curation_menu: HighlighterCurationMenuSnapshot::empty(),
            search: HighlighterSearchSnapshot::empty(),
            room_explorer: HighlighterRoomExplorerSnapshot::empty(),
            network: HighlighterNetworkSnapshot::empty(),
            toast: None,
        }
    }

    fn bump(&mut self) {
        self.rev = self.rev.saturating_add(1);
    }
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum HighlighterAppAction {
    Bootstrap,
    RefreshAppChrome,
    AppForegrounded,
    SignInNsec {
        nsec: String,
        persist: bool,
        clear_stored_on_failure: bool,
    },
    PairBunker {
        uri: String,
        persist: bool,
        clear_stored_on_failure: bool,
    },
    /// Begin a NIP-55 (Amber / external signer) sign-in (ADR-0048).
    ///
    /// `signer_package` is the Android package name of the signer app (e.g.
    /// `com.greenart7c3.nostrsigner` for Amber). `None` lets the OS resolver
    /// pick any installed NIP-55 signer. `persist` controls whether the
    /// `Nip55SignerPackage` credential is persisted after a successful
    /// sign-in; `clear_stored_on_failure` drops the stored credential when
    /// the pairing fails (restore path — a revoked signer must not retry
    /// forever). Same contract as [`Self::PairBunker`].
    SignInNip55 {
        signer_package: Option<String>,
        persist: bool,
        clear_stored_on_failure: bool,
    },
    /// Deliver a raw `ExternalSignerResponse` JSON from the Kotlin
    /// `ExternalSignerCapabilityBridge` back to the NIP-55 driver (D7).
    DeliverExternalSignerResponse {
        response_json: String,
    },
    SetCreateAccountDisplayName {
        display_name: String,
    },
    SetCreateAccountUsername {
        username: String,
    },
    SubmitCreateAccount,
    UploadCreateRoomCover {
        bytes: Vec<u8>,
        mime: String,
        width: u32,
        height: u32,
        alt: String,
    },
    CreateRoomCapabilityFailed {
        message: String,
    },
    ClearCreateRoomCover,
    SubmitCreateRoom {
        name: String,
        about: String,
        visibility: RoomVisibility,
        access: RoomAccess,
    },
    ClearCreateRoomResult,
    ClearCreateRoomError,
    OpenRoomInvite {
        group_id: String,
    },
    RefreshRoomInvite,
    SetRoomInviteQuery {
        query: String,
    },
    ToggleRoomInviteCandidate {
        pubkey_hex: String,
        source: HighlighterRoomInviteCandidateSource,
    },
    RemoveRoomInviteCandidate {
        pubkey_hex: String,
    },
    AcceptRoomInvitePastedCandidate,
    MintRoomInviteLink,
    SubmitRoomInviteMembers,
    ClearRoomInviteAddError,
    ClearRoomInviteInviteLinkError,
    ClearRoomInviteToast,
    CloseRoomInvite,
    OpenComments {
        root_tag_name: String,
        root_tag_value: String,
        root_kind: u16,
    },
    RefreshComments,
    SetCommentDraft {
        parent_event_id: Option<String>,
        body: String,
    },
    PublishComment {
        parent_event_id: Option<String>,
    },
    ClearCommentPublishError,
    ToggleCommentLike {
        event_id: String,
    },
    ToggleCommentBookmark {
        event_id: String,
    },
    ClearCommentInteractionError,
    CloseComments,
    OpenFeedback {
        coordinate: String,
    },
    RefreshFeedbackThreads,
    SetFeedbackNewThreadDraft {
        body: String,
    },
    PublishFeedbackNewThread,
    OpenFeedbackThread {
        root_event_id: String,
    },
    RefreshFeedbackThread,
    SetFeedbackReplyDraft {
        body: String,
    },
    PublishFeedbackReply,
    ClearFeedbackPublishError,
    CloseFeedbackThread,
    CloseFeedback,
    OpenMediaSettings,
    RefreshMediaSettings,
    AddBlossomServer {
        url: String,
    },
    RemoveBlossomServer {
        url: String,
    },
    MoveBlossomServers {
        from_indices: Vec<u32>,
        to_index: u32,
    },
    ClearMediaSettingsError,
    CloseMediaSettings,
    OpenEditProfile {
        seed: Option<ProfileMetadata>,
    },
    SetEditProfileDisplayName {
        value: String,
    },
    SetEditProfileName {
        value: String,
    },
    SetEditProfileAbout {
        value: String,
    },
    SetEditProfilePicture {
        value: String,
    },
    SetEditProfileBanner {
        value: String,
    },
    SetEditProfileNip05 {
        value: String,
    },
    SetEditProfileWebsite {
        value: String,
    },
    SetEditProfileLud16 {
        value: String,
    },
    UploadEditProfileImage {
        target: HighlighterEditProfileImageTarget,
        bytes: Vec<u8>,
        mime: String,
        width: u32,
        height: u32,
        alt: String,
    },
    EditProfileCapabilityFailed {
        message: String,
    },
    SubmitEditProfile,
    ClearEditProfileError,
    ClearEditProfileResult,
    CloseEditProfile,
    StartNostrConnect {
        callback_url: String,
    },
    ExternalUrlOpenFailed {
        url: String,
    },
    Logout,
    ToggleArticleBookmark {
        address: String,
    },
    OpenBookmarks,
    RefreshBookmarks,
    CloseBookmarks,
    OpenBookmarkCollection {
        pubkey_hex: String,
        d_tag: String,
        kind: u32,
    },
    RefreshBookmarkCollection,
    OpenCurationMenu {
        article_address: String,
    },
    CloseCurationMenu,
    SetAddressInCurationSet {
        d_tag: String,
        address: String,
        member: bool,
    },
    CreateCurationSetAndAdd {
        title: String,
        address: String,
    },
    OpenRoomExplorer,
    RefreshRoomExplorer,
    RefreshRoomBrowseAll,
    RequestJoinRoom {
        group_id: String,
        room_name: String,
    },
    RequestIsbnPreview {
        isbn: String,
    },
    RequestWebMetadata {
        url: String,
    },
    RequestReferenceHighlights {
        tag_name: String,
        tag_value: String,
        limit: u32,
    },
    RequestBookPickerRecents {
        limit: u32,
    },
    SearchBookPickerArtifacts {
        query: String,
        limit: u32,
    },
    ClearBookPickerSearch,
    UploadCapturePhoto {
        bytes: Vec<u8>,
        mime: String,
        width: u32,
        height: u32,
        alt: String,
    },
    ClearCaptureUpload,
    PublishCaptureHighlight {
        selection: HighlighterCaptureArtifact,
        target_group_id: Option<String>,
        draft: HighlightDraft,
    },
    PublishCapturePicture {
        selection: Option<HighlighterCaptureArtifact>,
        target_group_id: Option<String>,
        image: BlossomUpload,
        note: String,
    },
    PublishClipHighlight {
        artifact: ArtifactRecord,
        target_group_id: Option<String>,
        draft: HighlightDraft,
    },
    ClearCaptureResult,
    ClearCaptureError,
    RequestProfile {
        pubkey_hex: String,
    },
    OpenProfile {
        pubkey_hex: String,
    },
    RefreshProfile,
    CloseProfile,
    ToggleProfileFollow,
    OpenArticleReader {
        pubkey_hex: String,
        d_tag: String,
        seed: Option<ArticleRecord>,
    },
    RefreshArticleReader,
    CloseArticleReader,
    PublishArticleHighlight {
        quote: String,
        context: String,
        note: String,
    },
    PublishArtifactShare {
        preview: ArtifactPreview,
        group_id: String,
        note: Option<String>,
    },
    PublishUrlShare {
        url: String,
        group_id: String,
        note: Option<String>,
    },
    ShareHighlightRepost {
        event_id: String,
        author_pubkey_hex: String,
        relay_hint: String,
        target_group_id: String,
    },
    ClearShareComposerResult,
    ClearShareComposerError,
    OpenRoom {
        group_id: String,
    },
    RefreshRoom,
    PublishRoomDiscussion {
        title: String,
        body: String,
        attachment_url: Option<String>,
    },
    ClearRoomDiscussionError,
    LoadMoreRoomChat,
    PublishRoomChatMessage {
        content: String,
        reply_to_event_id: Option<String>,
    },
    ClearRoomChatError,
    CloseRoom,
    OpenHomeFeed,
    RefreshHomeFeed,
    CloseHomeFeed,
    SearchOpened,
    SearchClosed,
    SetSearchQuery {
        query: String,
    },
    SubmitSearch {
        query: String,
    },
    ClearSearch,
    RecordRecentSearch {
        query: String,
    },
    ClearRecentSearches,
    OpenNetworkSettings,
    RefreshNetworkSettings,
    UpsertNetworkRelay {
        config: RelayConfig,
    },
    RemoveNetworkRelay {
        url: String,
    },
    SetNetworkRelayRoles {
        url: String,
        read: bool,
        write: bool,
        rooms: bool,
        indexer: bool,
    },
    ProbeNetworkRelayNip11 {
        url: String,
    },
    SetNetworkImportNpub {
        npub: String,
    },
    FetchNetworkImportRelays,
    ToggleNetworkImportRelay {
        url: String,
    },
    ApplyNetworkImportRelays,
    ClearNetworkError,
    CloseNetworkSettings,
    SetNetworkWifiOnly {
        enabled: bool,
    },
    NetworkPathChanged {
        is_wifi: bool,
    },
    ReconnectNetwork,
    DismissWhatsNew,
    ToggleOnboardingInterest {
        interest_id: String,
    },
    CompleteOnboarding,
    ClearToast,
}

impl HighlighterAppAction {
    fn tag(&self) -> &'static str {
        match self {
            Self::Bootstrap => "bootstrap",
            Self::RefreshAppChrome => "refresh_app_chrome",
            Self::AppForegrounded => "app_foregrounded",
            Self::SignInNsec { .. } => "sign_in_nsec",
            Self::PairBunker { .. } => "pair_bunker",
            Self::SignInNip55 { .. } => "sign_in_nip55",
            Self::DeliverExternalSignerResponse { .. } => "deliver_external_signer_response",
            Self::SetCreateAccountDisplayName { .. } => "set_create_account_display_name",
            Self::SetCreateAccountUsername { .. } => "set_create_account_username",
            Self::SubmitCreateAccount => "submit_create_account",
            Self::UploadCreateRoomCover { .. } => "upload_create_room_cover",
            Self::CreateRoomCapabilityFailed { .. } => "create_room_capability_failed",
            Self::ClearCreateRoomCover => "clear_create_room_cover",
            Self::SubmitCreateRoom { .. } => "submit_create_room",
            Self::ClearCreateRoomResult => "clear_create_room_result",
            Self::ClearCreateRoomError => "clear_create_room_error",
            Self::OpenRoomInvite { .. } => "open_room_invite",
            Self::RefreshRoomInvite => "refresh_room_invite",
            Self::SetRoomInviteQuery { .. } => "set_room_invite_query",
            Self::ToggleRoomInviteCandidate { .. } => "toggle_room_invite_candidate",
            Self::RemoveRoomInviteCandidate { .. } => "remove_room_invite_candidate",
            Self::AcceptRoomInvitePastedCandidate => "accept_room_invite_pasted_candidate",
            Self::MintRoomInviteLink => "mint_room_invite_link",
            Self::SubmitRoomInviteMembers => "submit_room_invite_members",
            Self::ClearRoomInviteAddError => "clear_room_invite_add_error",
            Self::ClearRoomInviteInviteLinkError => "clear_room_invite_invite_link_error",
            Self::ClearRoomInviteToast => "clear_room_invite_toast",
            Self::CloseRoomInvite => "close_room_invite",
            Self::OpenComments { .. } => "open_comments",
            Self::RefreshComments => "refresh_comments",
            Self::SetCommentDraft { .. } => "set_comment_draft",
            Self::PublishComment { .. } => "publish_comment",
            Self::ClearCommentPublishError => "clear_comment_publish_error",
            Self::ToggleCommentLike { .. } => "toggle_comment_like",
            Self::ToggleCommentBookmark { .. } => "toggle_comment_bookmark",
            Self::ClearCommentInteractionError => "clear_comment_interaction_error",
            Self::CloseComments => "close_comments",
            Self::OpenFeedback { .. } => "open_feedback",
            Self::RefreshFeedbackThreads => "refresh_feedback_threads",
            Self::SetFeedbackNewThreadDraft { .. } => "set_feedback_new_thread_draft",
            Self::PublishFeedbackNewThread => "publish_feedback_new_thread",
            Self::OpenFeedbackThread { .. } => "open_feedback_thread",
            Self::RefreshFeedbackThread => "refresh_feedback_thread",
            Self::SetFeedbackReplyDraft { .. } => "set_feedback_reply_draft",
            Self::PublishFeedbackReply => "publish_feedback_reply",
            Self::ClearFeedbackPublishError => "clear_feedback_publish_error",
            Self::CloseFeedbackThread => "close_feedback_thread",
            Self::CloseFeedback => "close_feedback",
            Self::OpenMediaSettings => "open_media_settings",
            Self::RefreshMediaSettings => "refresh_media_settings",
            Self::AddBlossomServer { .. } => "add_blossom_server",
            Self::RemoveBlossomServer { .. } => "remove_blossom_server",
            Self::MoveBlossomServers { .. } => "move_blossom_servers",
            Self::ClearMediaSettingsError => "clear_media_settings_error",
            Self::CloseMediaSettings => "close_media_settings",
            Self::OpenEditProfile { .. } => "open_edit_profile",
            Self::SetEditProfileDisplayName { .. } => "set_edit_profile_display_name",
            Self::SetEditProfileName { .. } => "set_edit_profile_name",
            Self::SetEditProfileAbout { .. } => "set_edit_profile_about",
            Self::SetEditProfilePicture { .. } => "set_edit_profile_picture",
            Self::SetEditProfileBanner { .. } => "set_edit_profile_banner",
            Self::SetEditProfileNip05 { .. } => "set_edit_profile_nip05",
            Self::SetEditProfileWebsite { .. } => "set_edit_profile_website",
            Self::SetEditProfileLud16 { .. } => "set_edit_profile_lud16",
            Self::UploadEditProfileImage { .. } => "upload_edit_profile_image",
            Self::EditProfileCapabilityFailed { .. } => "edit_profile_capability_failed",
            Self::SubmitEditProfile => "submit_edit_profile",
            Self::ClearEditProfileError => "clear_edit_profile_error",
            Self::ClearEditProfileResult => "clear_edit_profile_result",
            Self::CloseEditProfile => "close_edit_profile",
            Self::StartNostrConnect { .. } => "start_nostr_connect",
            Self::ExternalUrlOpenFailed { .. } => "external_url_open_failed",
            Self::Logout => "logout",
            Self::ToggleArticleBookmark { .. } => "toggle_article_bookmark",
            Self::OpenBookmarks => "open_bookmarks",
            Self::RefreshBookmarks => "refresh_bookmarks",
            Self::CloseBookmarks => "close_bookmarks",
            Self::OpenBookmarkCollection { .. } => "open_bookmark_collection",
            Self::RefreshBookmarkCollection => "refresh_bookmark_collection",
            Self::OpenCurationMenu { .. } => "open_curation_menu",
            Self::CloseCurationMenu => "close_curation_menu",
            Self::SetAddressInCurationSet { .. } => "set_address_in_curation_set",
            Self::CreateCurationSetAndAdd { .. } => "create_curation_set_and_add",
            Self::OpenRoomExplorer => "open_room_explorer",
            Self::RefreshRoomExplorer => "refresh_room_explorer",
            Self::RefreshRoomBrowseAll => "refresh_room_browse_all",
            Self::RequestJoinRoom { .. } => "request_join_room",
            Self::RequestIsbnPreview { .. } => "request_isbn_preview",
            Self::RequestWebMetadata { .. } => "request_web_metadata",
            Self::RequestReferenceHighlights { .. } => "request_reference_highlights",
            Self::RequestBookPickerRecents { .. } => "request_book_picker_recents",
            Self::SearchBookPickerArtifacts { .. } => "search_book_picker_artifacts",
            Self::ClearBookPickerSearch => "clear_book_picker_search",
            Self::UploadCapturePhoto { .. } => "upload_capture_photo",
            Self::ClearCaptureUpload => "clear_capture_upload",
            Self::PublishCaptureHighlight { .. } => "publish_capture_highlight",
            Self::PublishCapturePicture { .. } => "publish_capture_picture",
            Self::PublishClipHighlight { .. } => "publish_clip_highlight",
            Self::ClearCaptureResult => "clear_capture_result",
            Self::ClearCaptureError => "clear_capture_error",
            Self::RequestProfile { .. } => "request_profile",
            Self::OpenProfile { .. } => "open_profile",
            Self::RefreshProfile => "refresh_profile",
            Self::CloseProfile => "close_profile",
            Self::ToggleProfileFollow => "toggle_profile_follow",
            Self::OpenArticleReader { .. } => "open_article_reader",
            Self::RefreshArticleReader => "refresh_article_reader",
            Self::CloseArticleReader => "close_article_reader",
            Self::PublishArticleHighlight { .. } => "publish_article_highlight",
            Self::PublishArtifactShare { .. } => "publish_artifact_share",
            Self::PublishUrlShare { .. } => "publish_url_share",
            Self::ShareHighlightRepost { .. } => "share_highlight_repost",
            Self::ClearShareComposerResult => "clear_share_composer_result",
            Self::ClearShareComposerError => "clear_share_composer_error",
            Self::OpenRoom { .. } => "open_room",
            Self::RefreshRoom => "refresh_room",
            Self::PublishRoomDiscussion { .. } => "publish_room_discussion",
            Self::ClearRoomDiscussionError => "clear_room_discussion_error",
            Self::LoadMoreRoomChat => "load_more_room_chat",
            Self::PublishRoomChatMessage { .. } => "publish_room_chat_message",
            Self::ClearRoomChatError => "clear_room_chat_error",
            Self::CloseRoom => "close_room",
            Self::OpenHomeFeed => "open_home_feed",
            Self::RefreshHomeFeed => "refresh_home_feed",
            Self::CloseHomeFeed => "close_home_feed",
            Self::SearchOpened => "search_opened",
            Self::SearchClosed => "search_closed",
            Self::SetSearchQuery { .. } => "set_search_query",
            Self::SubmitSearch { .. } => "submit_search",
            Self::ClearSearch => "clear_search",
            Self::RecordRecentSearch { .. } => "record_recent_search",
            Self::ClearRecentSearches => "clear_recent_searches",
            Self::OpenNetworkSettings => "open_network_settings",
            Self::RefreshNetworkSettings => "refresh_network_settings",
            Self::UpsertNetworkRelay { .. } => "upsert_network_relay",
            Self::RemoveNetworkRelay { .. } => "remove_network_relay",
            Self::SetNetworkRelayRoles { .. } => "set_network_relay_roles",
            Self::ProbeNetworkRelayNip11 { .. } => "probe_network_relay_nip11",
            Self::SetNetworkImportNpub { .. } => "set_network_import_npub",
            Self::FetchNetworkImportRelays => "fetch_network_import_relays",
            Self::ToggleNetworkImportRelay { .. } => "toggle_network_import_relay",
            Self::ApplyNetworkImportRelays => "apply_network_import_relays",
            Self::ClearNetworkError => "clear_network_error",
            Self::CloseNetworkSettings => "close_network_settings",
            Self::SetNetworkWifiOnly { .. } => "set_network_wifi_only",
            Self::NetworkPathChanged { .. } => "network_path_changed",
            Self::ReconnectNetwork => "reconnect_network",
            Self::DismissWhatsNew => "dismiss_whats_new",
            Self::ToggleOnboardingInterest { .. } => "toggle_onboarding_interest",
            Self::CompleteOnboarding => "complete_onboarding",
            Self::ClearToast => "clear_toast",
        }
    }
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum HighlighterSessionCredential {
    Nsec {
        nsec: String,
    },
    BunkerUri {
        uri: String,
    },
    /// NIP-55 external-signer (Amber). Persists only the signer app's
    /// package name; no key material ever enters the Rust process (ADR-0048
    /// D4). On next launch the host restores this credential and calls
    /// `signInNip55` so NMP reconstructs the `Nip55Signer` without a
    /// fresh user interaction.
    Nip55SignerPackage {
        signer_package: String,
    },
}

/// Result of one [`HighlighterNmpApp::next_signer_request`] drain tick
/// (ADR-0048 Stage 2). Mirrors `nmp-android-ffi`'s `NextSignerRequest`
/// contract:
/// - `Request` carries one `ExternalSignerRequest` JSON payload to hand to
///   `ExternalSignerCapabilityBridge.handleJson` (D7 — verbatim).
/// - `Idle` is a normal ≤250 ms timeout tick; the reader parked INSIDE the
///   channel wait (D8 — never a sleep+check poll) and uses the tick only to
///   re-check its shutdown flag.
/// - `Closed` means the channel sender is gone (session teardown); the
///   reader must exit its loop.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum HighlighterSignerRequestDrain {
    Request { request_json: String },
    Idle,
    Closed,
}

#[uniffi::export(with_foreign)]
pub trait HighlighterAppReconciler: Send + Sync {
    fn on_state(&self, state: HighlighterAppState);
    fn on_persist_session_credential(&self, credential: HighlighterSessionCredential);
    fn on_clear_session_credentials(&self);
    fn on_open_external_url(&self, url: String);
}

#[derive(uniffi::Object)]
pub struct HighlighterNmpApp {
    core: Arc<HighlighterCore>,
    tx: SyncSender<KernelMsg>,
    state: Arc<RwLock<HighlighterAppState>>,
    reconciler: Arc<RwLock<Option<Arc<dyn HighlighterAppReconciler>>>>,
    core_event_callback: Arc<RwLock<Option<Arc<dyn EventCallback>>>>,
    actor: Mutex<Option<JoinHandle<()>>>,
}

#[uniffi::export(async_runtime = "tokio")]
impl HighlighterNmpApp {
    #[uniffi::constructor]
    pub fn new(config: HighlighterAppConfig) -> Arc<Self> {
        if let Some(policy_json) = config
            .relay_policy_json
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            crate::relays::set_relay_policy_override(policy_json);
        }
        let visible_limit = config.normalized_visible_limit();
        let emit_hz = config.normalized_emit_hz();
        let core = match config
            .data_dir
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(path) => HighlighterCore::new_with_data_dir(PathBuf::from(path)),
            None => HighlighterCore::new(),
        };

        let local_state_path = core.runtime().data_dir().join("highlighter_app_state.json");
        let mut local_state = load_local_state(&local_state_path);
        let (tx, rx) = sync_channel(ACTION_QUEUE_CAPACITY);
        let mut initial_state = HighlighterAppState::empty(local_state.onboarding_complete);
        apply_recent_searches_to_snapshot(
            &mut initial_state.search,
            local_state.recent_searches.clone(),
        );
        initial_state.network.wifi_only_enabled = local_state.network_wifi_only;
        if apply_whats_new_to_snapshot(&mut initial_state.whats_new, &mut local_state) {
            save_local_state(&local_state_path, &local_state);
        }
        let state = Arc::new(RwLock::new(initial_state));
        let reconciler = Arc::new(RwLock::new(None));
        let core_event_callback = Arc::new(RwLock::new(None));

        core.set_event_callback(Arc::new(CoreDeltaMultiplexer {
            tx: tx.clone(),
            core_event_callback: core_event_callback.clone(),
        }));

        let actor_ctx = ActorContext {
            core: core.clone(),
            state: state.clone(),
            reconciler: reconciler.clone(),
            actor_tx: tx.clone(),
            visible_limit,
            local_state_path,
        };
        let actor = spawn_actor(rx, actor_ctx, emit_hz);

        Arc::new(Self {
            core,
            tx,
            state,
            reconciler,
            core_event_callback,
            actor: Mutex::new(Some(actor)),
        })
    }

    pub fn state(&self) -> HighlighterAppState {
        self.state.read().clone()
    }

    pub fn network_removal_impact(&self, url: String) -> Option<HighlighterRelayRemovalImpact> {
        let state = self.state.read();
        relay_removal_impact_for_url(&state.network.relay_removal_impacts, &url)
    }

    pub fn highlight_share_url(
        &self,
        event_id_hex: String,
        author_pubkey_hex: Option<String>,
    ) -> Option<String> {
        crate::share_links::highlight_share_url(event_id_hex, author_pubkey_hex).ok()
    }

    pub fn decode_nostr_entity(
        &self,
        input: String,
    ) -> Option<crate::nostr_entities::NostrEntityRef> {
        crate::nostr_entities::decode_nostr_entity(&input).ok()
    }

    pub async fn resolve_nostr_entity(
        &self,
        entity: crate::nostr_entities::NostrEntityRef,
    ) -> Option<crate::nostr_entities::NostrEntityEvent> {
        self.core.resolve_nostr_entity(entity).await.ok().flatten()
    }

    pub async fn article(&self, pubkey_hex: String, d_tag: String) -> Option<ArticleRecord> {
        self.core
            .get_article(pubkey_hex, d_tag)
            .await
            .ok()
            .flatten()
    }

    pub async fn publish_url_share(
        &self,
        url: String,
        group_id: String,
        note: Option<String>,
    ) -> bool {
        let note = note
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let Ok(preview) = self.core.build_preview_from_url(url).await else {
            return false;
        };
        self.core
            .publish_artifact(preview, group_id, note)
            .await
            .is_ok()
    }

    pub fn dispatch(&self, action: HighlighterAppAction) {
        if let Err(err) = self.tx.try_send(KernelMsg::Action(Box::new(action))) {
            log_send_failure(err);
        }
    }

    pub fn listen_for_updates(&self, reconciler: Arc<dyn HighlighterAppReconciler>) {
        *self.reconciler.write() = Some(reconciler.clone());
        reconciler.on_state(self.state());
    }

    pub fn set_core_event_callback(&self, callback: Arc<dyn EventCallback>) {
        *self.core_event_callback.write() = Some(callback.clone());
        if let Some(user) = self.core.current_user() {
            callback.on_data_changed(Delta {
                subscription_id: 0,
                change: DataChangeType::SignerConnected { user },
            });
        }
    }

    pub fn clear_core_event_callback(&self) {
        *self.core_event_callback.write() = None;
    }

    /// Blocking timed drain of the NIP-55 signer-request channel (ADR-0048
    /// Stage 2). Blocks the calling thread for up to 250 ms INSIDE the
    /// channel's `recv_timeout` (D8 — no polling; zero wake-ups while parked).
    ///
    /// Call from a dedicated Kotlin daemon thread:
    /// `Request` → route to `ExternalSignerCapabilityBridge.handleJson`;
    /// `Idle` → re-check the shutdown flag and loop; `Closed` → exit.
    pub fn next_signer_request(&self) -> HighlighterSignerRequestDrain {
        match self.core.nmp_next_signer_request() {
            SignerRequestDrain::Request(request_json) => {
                HighlighterSignerRequestDrain::Request { request_json }
            }
            SignerRequestDrain::Idle => HighlighterSignerRequestDrain::Idle,
            SignerRequestDrain::Closed => HighlighterSignerRequestDrain::Closed,
        }
    }
}

/// Test-only surface. Lives outside the `#[uniffi::export]` impl block:
/// the uniffi macro generates scaffolding for every method it sees,
/// including `#[cfg(test)]` ones, which breaks non-test builds.
#[cfg(test)]
impl HighlighterNmpApp {
    /// Inject a core delta straight into the actor queue so a test can
    /// deterministically exercise `handle_core_delta`'s cascade (which on
    /// device is push-delivered by the event callback). Used by the Phase-3
    /// non-blocking-delta acceptance test.
    fn inject_delta_for_test(&self, change: DataChangeType) {
        let _ = self.tx.try_send(KernelMsg::CoreDelta(Box::new(Delta {
            subscription_id: 0,
            change,
        })));
    }
}

impl Drop for HighlighterNmpApp {
    fn drop(&mut self) {
        let _ = self.tx.try_send(KernelMsg::Stop);
        if let Some(actor) = self.actor.lock().expect("actor mutex poisoned").take() {
            let _ = actor.join();
        }
    }
}

#[derive(Clone)]
struct CoreDeltaMultiplexer {
    tx: SyncSender<KernelMsg>,
    core_event_callback: Arc<RwLock<Option<Arc<dyn EventCallback>>>>,
}

impl EventCallback for CoreDeltaMultiplexer {
    fn on_data_changed(&self, delta: Delta) {
        if let Some(callback) = self.core_event_callback.read().clone() {
            callback.on_data_changed(delta.clone());
        }
        if let Err(err) = self.tx.try_send(KernelMsg::CoreDelta(Box::new(delta))) {
            log_send_failure(err);
        }
    }
}

enum KernelMsg {
    Action(Box<HighlighterAppAction>),
    CoreDelta(Box<Delta>),
    /// Off-actor operation completion (Phase 1 OpRunner). A single variant
    /// covers every migrated network-class operation; the actor applies it in
    /// [`apply_op_outcome`] after a staleness check, preserving the
    /// single-writer invariant.
    OpResolved {
        domain: OpDomain,
        generation: u64,
        outcome: Box<OpOutcome>,
    },
    Stop,
}

struct ActorContext {
    core: Arc<HighlighterCore>,
    state: Arc<RwLock<HighlighterAppState>>,
    reconciler: Arc<RwLock<Option<Arc<dyn HighlighterAppReconciler>>>>,
    actor_tx: SyncSender<KernelMsg>,
    visible_limit: usize,
    local_state_path: PathBuf,
}

struct ActorRuntimes {
    auth_generation: u64,
    pending_joins: Vec<PendingJoin>,
    pending_isbn_lookups: BTreeSet<String>,
    pending_web_metadata: BTreeSet<String>,
    profile_handles: BTreeMap<String, u64>,
    profile_pubkeys_by_handle: BTreeMap<u64, String>,
    app_scope_subscriptions: AppScopeSubscriptions,
    search_runtime: SearchRuntime,
    create_account_runtime: CreateAccountRuntime,
    home_feed_runtime: HomeFeedRuntime,
    bookmark_runtime: BookmarkRuntime,
    profile_view_runtime: ProfileViewRuntime,
    article_reader_runtime: ArticleReaderRuntime,
    room_detail_runtime: RoomDetailRuntime,
    room_invite_runtime: RoomInviteRuntime,
    feedback_runtime: FeedbackRuntime,
    room_explorer_runtime: RoomExplorerRuntime,
    network_runtime: NetworkRuntime,
    /// Off-actor operation runner (design §4.1). Lives here (the actor's
    /// `&mut` state) rather than `ActorContext` because `submit_op` mutates
    /// `in_flight`/`generations`.
    ops: OpRunner,
}

impl ActorRuntimes {
    fn new(actor_tx: SyncSender<KernelMsg>) -> Self {
        Self {
            auth_generation: 0,
            pending_joins: Vec::new(),
            pending_isbn_lookups: BTreeSet::new(),
            pending_web_metadata: BTreeSet::new(),
            profile_handles: BTreeMap::new(),
            profile_pubkeys_by_handle: BTreeMap::new(),
            app_scope_subscriptions: AppScopeSubscriptions::default(),
            search_runtime: SearchRuntime::default(),
            create_account_runtime: CreateAccountRuntime::default(),
            home_feed_runtime: HomeFeedRuntime::default(),
            bookmark_runtime: BookmarkRuntime::default(),
            profile_view_runtime: ProfileViewRuntime::default(),
            article_reader_runtime: ArticleReaderRuntime::default(),
            room_detail_runtime: RoomDetailRuntime::default(),
            room_invite_runtime: RoomInviteRuntime::default(),
            feedback_runtime: FeedbackRuntime::default(),
            room_explorer_runtime: RoomExplorerRuntime::default(),
            network_runtime: NetworkRuntime::default(),
            ops: OpRunner::new(actor_tx),
        }
    }
}

#[derive(Default)]
struct RoomInviteRuntime {
    group_id: Option<String>,
    follows: Vec<String>,
}

#[derive(Default)]
struct FeedbackRuntime {
    coordinate: Option<String>,
    threads_handle: Option<u64>,
    selected_root_event_id: Option<String>,
    thread_handle: Option<u64>,
}

#[derive(Default)]
struct NetworkRuntime {
    is_open: bool,
}

struct SearchLocalResolution {
    generation: u64,
    query: String,
    result: Result<SearchResults, String>,
}

// ===========================================================================
// Off-actor operation runner (actor-blocking-fix, design §4.1).
//
// The kernel actor is the single writer and the single emitter. Any await
// inside a message handler freezes every other message and every snapshot
// emission. `OpRunner` moves network-class work off the actor onto a small
// shared multi-thread runtime; completion re-enters the actor as a single
// `KernelMsg::OpResolved` message, applied in `apply_op_outcome` — the only
// state-mutation point for migrated operations. This mirrors the nine
// spawn-and-message-back workers the facade already shipped, and ADR-0024 /
// ADR-0040 in the embedded framework.
// ===========================================================================

/// Per-message handler-duration watchdog (design §4.6 loop-stall guard,
/// ADR-0028 liveness philosophy). Max observed dispatch duration in
/// milliseconds, readable from the in-file test module.
static ACTOR_MAX_HANDLER_MS: AtomicU64 = AtomicU64::new(0);

/// Any single KernelMsg dispatch slower than this is a liveness smell.
const ACTOR_HANDLER_WARN_MS: u128 = 250;

/// `block_on_local` is for Class-D (local-only) work that is expected to be
/// cheap. Anything slower than this is logged so the lint and traces can
/// catch a Class-A/B operation that slipped past migration.
const BLOCK_ON_LOCAL_WARN_MS: u128 = 50;

/// Record one handler's dispatch duration and warn if it blew the budget.
fn record_handler_duration(tag: &str, elapsed: Duration) {
    let ms = elapsed.as_millis();
    // Monotonic max gauge for the test-only watchdog.
    let ms_u64 = ms.min(u128::from(u64::MAX)) as u64;
    let mut prev = ACTOR_MAX_HANDLER_MS.load(Ordering::Relaxed);
    while ms_u64 > prev {
        match ACTOR_MAX_HANDLER_MS.compare_exchange_weak(
            prev,
            ms_u64,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(observed) => prev = observed,
        }
    }
    if ms > ACTOR_HANDLER_WARN_MS {
        tracing::warn!(handler = tag, elapsed_ms = ms, "slow actor handler");
    }
}

#[cfg(test)]
pub(crate) fn actor_max_handler_ms() -> u64 {
    ACTOR_MAX_HANDLER_MS.load(Ordering::Relaxed)
}

#[cfg(test)]
pub(crate) fn reset_actor_max_handler_ms() {
    ACTOR_MAX_HANDLER_MS.store(0, Ordering::Relaxed);
}

/// Thin wrapper around `runtime.block_on` reserved for Class-D local work.
/// It measures and warns when a "local" future actually blocks; the Phase-0
/// lint bans naked `block_on_local(&runtime, "actor_block", ` everywhere else in this file so a
/// future network await cannot silently land back on the actor thread.
fn block_on_local<F: Future>(runtime: &tokio::runtime::Runtime, tag: &str, fut: F) -> F::Output {
    let started = Instant::now();
    let out = runtime.block_on(fut);
    let elapsed = started.elapsed();
    if elapsed.as_millis() > BLOCK_ON_LOCAL_WARN_MS {
        // Intentionally only a warn (not debug_assert): slow CI must not turn a
        // transient stall into a suite-wide panic.
        tracing::warn!(
            site = tag,
            elapsed_ms = elapsed.as_millis(),
            "block_on_local exceeded local budget"
        );
    }
    out
}

/// Which blossom upload surface an upload op feeds back into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum UploadSlot {
    RoomCover,
    ProfilePicture,
    ProfileBanner,
    Capture,
}

/// Domain key — one in-flight slot per UI surface, so a second request
/// supersedes the first (generation bump + abort).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum OpDomain {
    JoinRoom,
    RoomChatPublish,
    BlossomUpload(UploadSlot),
    /// NIP-11 probe keyed by a stable hash of the relay URL. The relay list
    /// probes every row on appear; an unkeyed slot would let each row's probe
    /// abort the previous one, leaving all but the last row unresolved. Same
    /// URL still supersedes (re-probe wins); different URLs run independently.
    RelayProbe {
        target: u64,
    },
    RelayImport,
    RoomCreate,
    RoomInvite,
    RoomInviteMembers,
    // --- Phase 2: Class B publish helpers (design §4.4). ---
    CommentPublish,
    /// Per-comment interaction toggle, keyed by a stable hash of the target
    /// comment event id plus the interaction kind. Keying rationale: a toggle
    /// on comment A must NOT abort an in-flight toggle on comment B (different
    /// key → independent slots), while a rapid double-tap on the SAME comment
    /// of the SAME kind SHOULD supersede (same key → abort+regenerate → the
    /// last tap wins). The event id itself rides in the outcome payload; only a
    /// `u64` lives in the key so `OpDomain` stays `Copy`.
    CommentInteraction {
        target: u64,
        kind: CommentInteractionKind,
    },
    FeedbackPublish,
    MediaSettingsWrite,
    ProfileEditSubmit,
    /// Bookmark toggle keyed by a stable hash of the article address — two
    /// different articles toggle independently; the same article supersedes.
    ArticleBookmarkToggle {
        target: u64,
    },
    CurationWrite,
    /// Follow toggle for the open profile view. Only one profile view is open
    /// at a time, so a single unkeyed slot is correct: a rapid double-tap on
    /// the same profile supersedes and the last tap wins.
    FollowToggle,
    HighlightPublish,
    SharePublish,
    CapturePublish,
    RoomDiscussionPublish,
    /// Relay-list writes (upsert / remove / set-roles / import-apply). One slot
    /// because these all mutate the single relay list; serializing them via
    /// supersession is acceptable (last write wins, then a refresh reconciles).
    NetworkRelayWrite,
    OnboardingFollows,
    // --- Phase 4: the eight bespoke legacy workers, folded onto OpRunner. ---
    /// Local-search worker. Single-flight by design: the `SearchRuntime`
    /// `local_running_query` gate refuses a second submit while one is in
    /// flight, so OpRunner's abort-on-resubmit never actually fires here; the
    /// generation supersession is carried by `search_runtime.generation`
    /// (checked in the apply arm exactly as the old resolver did), and the
    /// resolver re-drives the worker if the query moved on.
    SearchLocal,
    /// NIP-05 username availability check. Keyed unkeyed (one create-account
    /// form open at a time); the apply arm re-validates against
    /// `create_account_runtime.username_generation` AND the live form text,
    /// byte-for-byte with the former resolver.
    UsernameCheck,
    /// nsec + bunker sign-in SHARE this single domain so that, exactly as
    /// today (both bumped the one `auth_generation`), a bunker attempt
    /// supersedes an in-flight nsec attempt and vice-versa — last sign-in
    /// wins regardless of method.
    Auth,
    /// Account creation. Carries its own 30s deadline (preserved) and is
    /// superseded by `create_generation` semantics via OpRunner generations.
    AccountCreate,
    /// Default-blossom-server init. The original keyed dedup on the target
    /// pubkey (`initializing_blossom_defaults_for_pubkey`); the pubkey rides
    /// in the outcome and the apply arm performs the same still-this-user
    /// check, so a single unkeyed slot is sufficient.
    DefaultBlossomInit,
    /// ISBN preview lookup, keyed by a stable hash of the ISBN so concurrent
    /// lookups for different ISBNs run independently (no supersession), exactly
    /// like the former `pending_isbn_lookups` set. The dedup set is retained as
    /// the submit-time guard.
    IsbnPreview {
        target: u64,
    },
    /// Web-metadata lookup, same per-target keying + retained `pending_web_metadata`
    /// coalescing set as ISBN above.
    WebMetadata {
        target: u64,
    },
}

/// The two per-comment interactions sharing the [`OpDomain::CommentInteraction`]
/// keying scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CommentInteractionKind {
    Like,
    Bookmark,
}

/// Stable hash of a string identifier for use inside a `Copy` `OpDomain` key.
fn op_target_hash(id: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    hasher.finish()
}

/// Per-domain typed completion payloads. Each mirrors the Result the
/// corresponding handler produced inline today.
enum OpOutcome {
    JoinRoom {
        group_id: String,
        room_name: String,
        result: Result<(), String>,
    },
    RoomChatPublish {
        result: Result<(), String>,
    },
    BlossomUpload {
        slot: UploadSlot,
        result: Result<BlossomUpload, String>,
    },
    RelayProbe {
        url: String,
        document: Option<Nip11Document>,
    },
    RelayImport {
        result: Result<Vec<RelayConfig>, String>,
    },
    RoomCreate {
        result: Result<String, String>,
    },
    RoomInvite {
        group_id: String,
        result: Result<Vec<String>, String>,
    },
    RoomInviteMembers {
        /// Pubkeys that failed to add (empty = all succeeded).
        result: Result<RoomInviteMembersOutcome, String>,
    },
    CommentPublish {
        parent_event_id: Option<String>,
        result: Result<CommentRecord, String>,
    },
    CommentInteraction {
        kind: CommentInteractionKind,
        result: Result<(), String>,
    },
    FeedbackPublish {
        parent_event_id: Option<String>,
        result: Result<FeedbackEventRecord, String>,
    },
    MediaSettingsWrite {
        result: Result<(), String>,
    },
    ProfileEditSubmit {
        result: Result<ProfileMetadata, String>,
    },
    ArticleBookmarkToggle {
        result: Result<(), String>,
    },
    CurationWrite {
        /// Ok(toast message) on success.
        result: Result<String, String>,
    },
    FollowToggle {
        desired_following: bool,
        previous_following: bool,
        result: Result<(), String>,
    },
    HighlightPublish {
        note_was_empty: bool,
        result: Result<HighlightRecord, String>,
    },
    SharePublish {
        group_id: String,
        /// Ok(toast message) on success.
        result: Result<String, String>,
    },
    CapturePublish {
        /// Ok((event_id, toast message)) on success.
        result: Result<(String, String), String>,
    },
    RoomDiscussionPublish {
        result: Result<String, String>,
    },
    NetworkRelayWrite {
        kind: NetworkRelayWriteKind,
        result: Result<(), String>,
    },
    OnboardingFollows {
        failures: usize,
    },
    // --- Phase 4 worker payloads (mirror the deleted *Resolved variants). ---
    SearchLocal {
        generation: u64,
        query: String,
        result: Result<SearchResults, String>,
    },
    UsernameCheck {
        generation: u64,
        username: String,
        result: Result<Nip05Availability, String>,
    },
    NsecSignIn {
        generation: u64,
        nsec: String,
        persist: bool,
        clear_stored_on_failure: bool,
        result: Result<CurrentUser, String>,
    },
    BunkerSignIn {
        generation: u64,
        uri: String,
        persist: bool,
        clear_stored_on_failure: bool,
        result: Result<CurrentUser, String>,
    },
    Nip55SignIn {
        generation: u64,
        signer_package: Option<String>,
        persist: bool,
        clear_stored_on_failure: bool,
        result: Result<CurrentUser, String>,
    },
    AccountCreate {
        generation: u64,
        result: Box<Result<CreateAccountOutcome, String>>,
    },
    DefaultBlossomInit {
        pubkey_hex: String,
        result: Result<(), String>,
    },
    IsbnPreview {
        requested: String,
        result: Box<Result<ArtifactPreview, String>>,
    },
    WebMetadata {
        requested: String,
        result: Box<Result<WebMetadata, String>>,
    },
}

/// Result detail for a [`OpDomain::RoomInviteMembers`] batch add.
struct RoomInviteMembersOutcome {
    total: usize,
    failures: Vec<String>,
}

/// Which relay-list write a [`OpDomain::NetworkRelayWrite`] performed, so the
/// D6 copy and the follow-up refresh match the original handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NetworkRelayWriteKind {
    Upsert,
    Remove,
    SetRoles,
    ImportApply,
}

/// Standard D6 timeout copy, mirroring the shipped account-create message.
fn op_timeout_message(domain: OpDomain) -> String {
    match domain {
        OpDomain::JoinRoom => "Joining timed out. Check your connection and try again.".into(),
        OpDomain::RoomChatPublish => {
            "Sending timed out. Check your connection and try again.".into()
        }
        OpDomain::BlossomUpload(_) => {
            "Upload timed out. Check your connection and try again.".into()
        }
        OpDomain::RelayProbe { .. } => "Couldn't reach the relay — you can still add it.".into(),
        OpDomain::RelayImport => "Import timed out. Check your connection and try again.".into(),
        OpDomain::RoomCreate => {
            "Creating the room timed out. Check your connection and try again.".into()
        }
        OpDomain::RoomInvite => {
            "Minting the invite link timed out. Check your connection and try again.".into()
        }
        OpDomain::RoomInviteMembers => {
            "Adding members timed out. Check your connection and try again.".into()
        }
        OpDomain::CommentPublish => {
            "Publishing timed out. Check your connection and try again.".into()
        }
        OpDomain::CommentInteraction { kind, .. } => match kind {
            CommentInteractionKind::Like => {
                "Couldn't update like — check your connection and try again.".into()
            }
            CommentInteractionKind::Bookmark => {
                "Couldn't update bookmark — check your connection and try again.".into()
            }
        },
        OpDomain::FeedbackPublish => {
            "Sending feedback timed out. Check your connection and try again.".into()
        }
        OpDomain::MediaSettingsWrite => {
            "Saving media servers timed out. Check your connection and try again.".into()
        }
        OpDomain::ProfileEditSubmit => {
            "Saving your profile timed out. Check your connection and try again.".into()
        }
        OpDomain::ArticleBookmarkToggle { .. } => {
            "Couldn't update bookmark — check your connection and try again.".into()
        }
        OpDomain::CurationWrite => {
            "Updating the collection timed out. Check your connection and try again.".into()
        }
        OpDomain::FollowToggle => {
            "Couldn't update follow — check your connection and try again.".into()
        }
        OpDomain::HighlightPublish => {
            "Publishing the highlight timed out. Check your connection and try again.".into()
        }
        OpDomain::SharePublish => "Sharing timed out. Check your connection and try again.".into(),
        OpDomain::CapturePublish => {
            "Publishing timed out. Check your connection and try again.".into()
        }
        OpDomain::RoomDiscussionPublish => {
            "Publishing the discussion timed out. Check your connection and try again.".into()
        }
        OpDomain::NetworkRelayWrite => {
            "Saving relays timed out. Check your connection and try again.".into()
        }
        OpDomain::OnboardingFollows => "Saved your interests".into(),
        // The next five workers enforce/own their own error handling inside the
        // submitted future; the generic timeout fallback copy mirrors the
        // shipped strings (account-create keeps its exact on-device message).
        OpDomain::SearchLocal => "Search timed out. Check your connection and try again.".into(),
        OpDomain::UsernameCheck => "Username check timed out.".into(),
        OpDomain::Auth => "Sign-in timed out. Check your connection and try again.".into(),
        OpDomain::AccountCreate => {
            "Account creation timed out. Check your connection and try again.".into()
        }
        OpDomain::DefaultBlossomInit => "Blossom setup timed out.".into(),
        OpDomain::IsbnPreview { .. } => "ISBN lookup timed out.".into(),
        OpDomain::WebMetadata { .. } => "Web metadata lookup timed out.".into(),
    }
}

/// Class A/B deadline (UI cannot wait the protocol's 360s / 65s worst case).
const OP_DEADLINE_NETWORK: Duration = Duration::from_secs(30);
/// Interactive signer-pairing deadline (NIP-55). The op's inner wait
/// (`NMP_SIGNER_PAIR_TIMEOUT`, 300s) is the real budget — the user is
/// reading and approving a dialog in the signer app, which routinely takes
/// longer than the 30s network deadline (observed: a deliberate Amber
/// approval truncated by `OP_DEADLINE_NETWORK` left the kernel signed in
/// with the hl state stuck signed-out). Slightly above the inner timeout so
/// the fallback only fires if the future itself wedges. The user can always
/// bail early by rejecting in the signer app (resolves as `Rejected`).
const OP_DEADLINE_SIGNER_PAIR: Duration = Duration::from_secs(310);
/// Class C probe deadlines (keep the existing bounds; the worker enforces).
const OP_DEADLINE_RELAY_PROBE: Duration = Duration::from_secs(6);
const OP_DEADLINE_RELAY_IMPORT: Duration = Duration::from_secs(5);

struct InFlightOp {
    abort: tokio::task::AbortHandle,
}

/// Off-actor operation runner. Owned by `ActorRuntimes` (actor-thread only).
struct OpRunner {
    /// 2-worker multi-thread runtime: enough parallelism that a stuck upload
    /// cannot starve a publish, bounded so we never spawn a thread-per-tap
    /// zoo. Workers never touch `HighlighterAppState` or `ActorRuntimes`.
    runtime: tokio::runtime::Runtime,
    actor_tx: SyncSender<KernelMsg>,
    in_flight: HashMap<OpDomain, InFlightOp>,
    /// Monotonic generation per domain; survives `cancel_all` so a late
    /// resolution from a previous session can never be mistaken for current.
    generations: HashMap<OpDomain, u64>,
}

impl OpRunner {
    fn new(actor_tx: SyncSender<KernelMsg>) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("highlighter-op-runner")
            .build()
            .expect("build highlighter OpRunner runtime");
        Self {
            runtime,
            actor_tx,
            in_flight: HashMap::new(),
            generations: HashMap::new(),
        }
    }

    fn bump_generation(&mut self, domain: OpDomain) -> u64 {
        let slot = self.generations.entry(domain).or_insert(0);
        *slot = slot.saturating_add(1);
        *slot
    }

    /// A resolution is stale if its domain was superseded by a newer
    /// submission, or the in-flight record is gone (e.g. after `cancel_all`).
    fn is_stale(&self, domain: OpDomain, generation: u64) -> bool {
        match self.generations.get(&domain) {
            Some(current) => *current != generation,
            None => true,
        }
    }

    /// Submit `fut` off the actor thread with a worker-side deadline. On the
    /// actor thread only: bumps the domain generation, aborts any prior task
    /// for the domain (supersession), and records the new in-flight handle.
    fn submit_op<F>(
        &mut self,
        domain: OpDomain,
        deadline: Duration,
        timeout_outcome: OpOutcome,
        fut: F,
    ) where
        F: Future<Output = OpOutcome> + Send + 'static,
    {
        let generation = self.bump_generation(domain);
        if let Some(prev) = self.in_flight.remove(&domain) {
            prev.abort.abort();
        }
        let tx = self.actor_tx.clone();
        let handle = self.runtime.spawn(async move {
            // On deadline we send the caller-supplied truthful timeout outcome
            // (carrying the live generation + real keys/flags), so the apply
            // arm applies it instead of silently dropping a zeroed fallback.
            let outcome = match tokio::time::timeout(deadline, fut).await {
                Ok(outcome) => outcome,
                Err(_) => timeout_outcome,
            };
            if tx
                .send(KernelMsg::OpResolved {
                    domain,
                    generation,
                    outcome: Box::new(outcome),
                })
                .is_err()
            {
                tracing::warn!("highlighter NMP actor is stopped");
            }
        });
        self.in_flight.insert(
            domain,
            InFlightOp {
                abort: handle.abort_handle(),
            },
        );
    }

    /// Logout: abort every in-flight op and bump every known generation so any
    /// resolution still racing in cannot write a logged-out user's data into a
    /// fresh session.
    fn cancel_all(&mut self) {
        for (_, op) in self.in_flight.drain() {
            op.abort.abort();
        }
        for slot in self.generations.values_mut() {
            *slot = slot.saturating_add(1);
        }
    }
}

fn spawn_actor(rx: Receiver<KernelMsg>, ctx: ActorContext, emit_hz: u32) -> JoinHandle<()> {
    thread::Builder::new()
        .name("highlighter-nmp-actor".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .thread_name("highlighter-nmp-actions")
                .build()
                .expect("build highlighter NMP actor runtime");
            let mut runtimes = ActorRuntimes::new(ctx.actor_tx.clone());

            // Rate-limit OpResolved emits to at most emit_hz per second.
            // Action/CoreDelta emits (user-initiated, infrequent) are always
            // delivered immediately by handle_action/handle_core_delta — only the
            // burst from concurrent op resolutions is coalesced here.
            let op_emit_interval =
                Duration::from_millis((1000u64 / emit_hz.clamp(1, 120) as u64).max(1));
            let mut last_op_emit = Instant::now() - op_emit_interval; // allow first emit immediately
            let mut op_emit_dirty = false;

            loop {
                // Block for at most one emit interval so a queued dirty emit is
                // never delayed more than op_emit_interval even when the channel
                // is quiet.
                let recv_timeout = if op_emit_dirty {
                    let elapsed = last_op_emit.elapsed();
                    if elapsed >= op_emit_interval {
                        Duration::ZERO
                    } else {
                        op_emit_interval - elapsed
                    }
                } else {
                    // Nothing dirty — wait indefinitely (use a long timeout to
                    // avoid busy-looping while preserving the recv_timeout API).
                    Duration::from_secs(3600)
                };

                let msg = match rx.recv_timeout(recv_timeout) {
                    Ok(msg) => msg,
                    Err(RecvTimeoutError::Timeout) => {
                        // Interval elapsed with no new message — flush the
                        // pending dirty emit (trailing-emit guarantee).
                        if op_emit_dirty {
                            emit(&ctx.state, &ctx.reconciler);
                            last_op_emit = Instant::now();
                            op_emit_dirty = false;
                        }
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                };

                // Per-message duration watchdog (design §4.6). Tag captured
                // before the match consumes `msg`.
                let msg_tag = kernel_msg_tag(&msg);
                let dispatch_started = Instant::now();
                match msg {
                    KernelMsg::Action(action) => {
                        tracing::debug!(action = action.tag(), "highlighter app action");
                        handle_action(*action, &runtime, &ctx, &mut runtimes);
                        // Action emits are handled inside handle_action; mark the
                        // op-level dirty state clean so the next interval timeout
                        // does not fire a redundant extra emit.
                        op_emit_dirty = false;
                        last_op_emit = Instant::now();
                    }
                    KernelMsg::CoreDelta(delta) => {
                        // Phase 3 (actor-blocking-fix §4.4): `handle_core_delta`
                        // is now a synchronous fn. Every refresh/hydrate it
                        // cascades into is local-only (ndb reads + synchronous
                        // subscription registration), so the delta path holds no
                        // network-bounded await on the actor thread. The only
                        // awaits left are the FFI-wide `async fn` core methods,
                        // wrapped at the helper boundary in `block_on_local`
                        // exactly as `handle_action` already does — proving the
                        // non-blocking invariant structurally rather than by
                        // convention.
                        handle_core_delta(*delta, &runtime, &ctx, &mut runtimes);
                        op_emit_dirty = false;
                        last_op_emit = Instant::now();
                    }
                    KernelMsg::OpResolved {
                        domain,
                        generation,
                        outcome,
                    } => {
                        if !runtimes.ops.is_stale(domain, generation) {
                            // Drop the in-flight record now that it has resolved.
                            runtimes.ops.in_flight.remove(&domain);
                            apply_op_outcome(&runtime, &ctx, &mut runtimes, domain, *outcome);
                            // Rate-limit: emit immediately if the interval has
                            // elapsed; otherwise mark dirty and let the timeout
                            // flush the trailing emit within op_emit_interval.
                            if last_op_emit.elapsed() >= op_emit_interval {
                                emit(&ctx.state, &ctx.reconciler);
                                last_op_emit = Instant::now();
                                op_emit_dirty = false;
                            } else {
                                op_emit_dirty = true;
                            }
                        }
                    }
                    KernelMsg::Stop => {
                        clear_app_scope_subscriptions(
                            &ctx.core,
                            &mut runtimes.app_scope_subscriptions,
                        );
                        clear_profile_subscriptions(
                            &ctx.core,
                            &mut runtimes.profile_handles,
                            &mut runtimes.profile_pubkeys_by_handle,
                        );
                        clear_search_runtime(&ctx.core, &mut runtimes.search_runtime);
                        clear_home_feed_runtime(&ctx.core, &mut runtimes.home_feed_runtime);
                        clear_bookmark_runtime(&ctx.core, &mut runtimes.bookmark_runtime);
                        clear_profile_view_runtime(&ctx.core, &mut runtimes.profile_view_runtime);
                        clear_article_reader_runtime(
                            &ctx.core,
                            &mut runtimes.article_reader_runtime,
                        );
                        clear_room_detail_runtime(&ctx.core, &mut runtimes.room_detail_runtime);
                        runtimes.ops.cancel_all();
                        break;
                    }
                }
                record_handler_duration(msg_tag, dispatch_started.elapsed());
            }
            tracing::debug!(emit_hz, "highlighter NMP actor stopped");
        })
        .expect("spawn highlighter NMP actor")
}

/// Short static tag for a `KernelMsg`, used by the per-message watchdog.
fn kernel_msg_tag(msg: &KernelMsg) -> &'static str {
    match msg {
        KernelMsg::Action(_) => "action",
        KernelMsg::CoreDelta(_) => "core_delta",
        KernelMsg::OpResolved { .. } => "op_resolved",
        KernelMsg::Stop => "stop",
    }
}

/// The single state-mutation point for migrated off-actor operations
/// (design §4.1). Runs on the actor thread; clears the domain's busy flag,
/// applies the result or a D6 toast, and may run Class-D local follow-ups
/// (ndb-only refreshes) via `block_on_local`. The caller emits afterwards.
fn apply_op_outcome(
    runtime: &tokio::runtime::Runtime,
    ctx: &ActorContext,
    runtimes: &mut ActorRuntimes,
    _domain: OpDomain,
    outcome: OpOutcome,
) {
    let core = &ctx.core;
    let state = &ctx.state;
    let visible_limit = ctx.visible_limit;
    match outcome {
        OpOutcome::JoinRoom {
            group_id,
            room_name,
            result,
        } => match result {
            Ok(()) => {
                runtimes
                    .pending_joins
                    .retain(|join| join.group_id != group_id);
                runtimes.pending_joins.push(PendingJoin {
                    group_id: group_id.clone(),
                    room_name,
                });
                let mut current = state.write();
                clear_joining_marker(&mut current.room_explorer, &group_id);
                current.room_explorer.error_message = None;
                current.toast = Some(HighlighterToast {
                    kind: HighlighterToastKind::Info,
                    message: "Join requested".into(),
                });
                current.bump();
            }
            Err(err) => {
                runtimes
                    .pending_joins
                    .retain(|join| join.group_id != group_id);
                {
                    let mut current = state.write();
                    clear_joining_marker(&mut current.room_explorer, &group_id);
                    current.bump();
                }
                set_room_explorer_join_error(state, err);
            }
        },
        OpOutcome::RoomChatPublish { result } => match result {
            Ok(()) => {
                {
                    let mut current = state.write();
                    current.room_detail.is_sending_chat_message = false;
                    current.room_detail.chat_error_message = None;
                    current.bump();
                }
                // Local-only refresh (ndb reads + sync subscription).
                block_on_local(
                    runtime,
                    "refresh_room_detail",
                    refresh_room_detail(
                        core,
                        state,
                        &mut runtimes.room_detail_runtime,
                        visible_limit,
                    ),
                );
            }
            Err(err) => set_room_chat_error(state, format!("Couldn't send message: {err}")),
        },
        OpOutcome::BlossomUpload { slot, result } => {
            apply_blossom_upload_outcome(state, slot, result)
        }
        OpOutcome::RelayProbe { url, document } => {
            let mut current = state.write();
            if document.is_some() {
                upsert_network_nip11_projection(
                    &mut current.network,
                    HighlighterRelayNip11Snapshot {
                        url,
                        document,
                        is_loading: false,
                        error_message: None,
                    },
                );
            } else {
                upsert_network_nip11_projection(
                    &mut current.network,
                    HighlighterRelayNip11Snapshot {
                        url,
                        document: None,
                        is_loading: false,
                        error_message: Some(
                            "Couldn't reach the relay — you can still add it.".into(),
                        ),
                    },
                );
            }
            current.bump();
        }
        OpOutcome::RelayImport { result } => match result {
            Ok(mut rows) => {
                if rows.len() > NETWORK_IMPORT_LIMIT {
                    rows.truncate(NETWORK_IMPORT_LIMIT);
                }
                let selected_urls = rows.iter().map(|row| row.url.clone()).collect::<Vec<_>>();
                let mut current = state.write();
                current.network.import_relays.candidate_count = rows.len() as u64;
                current.network.import_relays.candidates = rows;
                current.network.import_relays.selected_urls = selected_urls;
                current.network.import_relays.is_fetching = false;
                current.network.import_relays.error_message =
                    (current.network.import_relays.candidate_count == 0)
                        .then(|| "No relay list found for this user.".to_string());
                current.bump();
            }
            Err(err) => {
                let mut current = state.write();
                current.network.import_relays.is_fetching = false;
                current.network.import_relays.error_message = Some(err);
                current.bump();
            }
        },
        OpOutcome::RoomCreate { result } => match result {
            Ok(group_id) => {
                {
                    let mut current = state.write();
                    current.create_room.is_creating = false;
                    current.create_room.created_group_id = Some(group_id);
                    current.create_room.error_message = None;
                    current.bump();
                }
                block_on_local(
                    runtime,
                    "hydrate_joined_communities",
                    hydrate_joined_communities(
                        core,
                        state,
                        &mut runtimes.pending_joins,
                        visible_limit,
                    ),
                );
            }
            Err(err) => set_create_room_error(state, format!("Couldn't publish: {err}")),
        },
        OpOutcome::RoomInvite { group_id, result } => match result {
            Ok(codes) => {
                let url = codes
                    .first()
                    .filter(|code| !code.trim().is_empty())
                    .map(|code| format!("https://highlighter.com/r/{group_id}/join/{code}"));
                let mut current = state.write();
                current.room_invite.is_minting_invite_link = false;
                current.room_invite.invite_url = url;
                current.room_invite.invite_link_error_message =
                    if current.room_invite.invite_url.is_none() {
                        Some("No invite code returned.".into())
                    } else {
                        None
                    };
                current.bump();
            }
            Err(err) => {
                let mut current = state.write();
                current.room_invite.is_minting_invite_link = false;
                current.room_invite.invite_url = None;
                current.room_invite.invite_link_error_message =
                    Some(format!("Couldn't mint invite link: {err}"));
                current.bump();
            }
        },
        OpOutcome::RoomInviteMembers { result } => match result {
            Ok(outcome) => {
                let RoomInviteMembersOutcome { total, failures } = outcome;
                let mut current = state.write();
                current.room_invite.is_adding_members = false;
                if failures.is_empty() {
                    current.room_invite.selected.clear();
                    current.room_invite.add_error_message = None;
                    current.room_invite.toast_message = Some(if total == 1 {
                        "Added 1 person".into()
                    } else {
                        format!("Added {total} people")
                    });
                } else if failures.len() == total {
                    current.room_invite.add_error_message =
                        Some("Couldn't add anyone. Are you a moderator of this room?".into());
                } else {
                    let failure_set: BTreeSet<String> = failures.iter().cloned().collect();
                    current
                        .room_invite
                        .selected
                        .retain(|candidate| failure_set.contains(&candidate.pubkey_hex));
                    let failed_names = failures
                        .iter()
                        .map(|pubkey| short_pubkey(pubkey))
                        .collect::<Vec<_>>()
                        .join(", ");
                    current.room_invite.add_error_message =
                        Some(format!("Some failed: {failed_names}"));
                }
                current.bump();
            }
            Err(err) => set_room_invite_add_error(state, err),
        },
        OpOutcome::CommentPublish {
            parent_event_id,
            result,
        } => match result {
            Ok(record) => {
                set_comment_draft(state, parent_event_id, String::new());
                {
                    let mut current = state.write();
                    current.comments.is_publishing = false;
                    current.comments.last_published_event_id = Some(record.event_id.clone());
                    current.comments.publish_error_message = None;
                    if !current
                        .comments
                        .records
                        .iter()
                        .any(|existing| existing.event_id == record.event_id)
                    {
                        current.comments.records.push(record);
                        current.comments.record_count = current.comments.records.len() as u64;
                    }
                    current.bump();
                }
                block_on_local(runtime, "refresh_comments", refresh_comments(core, state));
            }
            Err(err) => set_comment_publish_error(state, format!("Couldn't publish: {err}")),
        },
        OpOutcome::CommentInteraction { kind, result } => match result {
            Ok(()) => block_on_local(runtime, "refresh_comments", refresh_comments(core, state)),
            Err(err) => {
                let what = match kind {
                    CommentInteractionKind::Like => "like",
                    CommentInteractionKind::Bookmark => "bookmark",
                };
                set_comment_interaction_error(state, format!("Couldn't update {what}: {err}"));
            }
        },
        OpOutcome::FeedbackPublish {
            parent_event_id,
            result,
        } => match result {
            Ok(record) => {
                {
                    let mut current = state.write();
                    current.feedback.is_publishing_new_thread = false;
                    current.feedback.is_publishing_reply = false;
                    current.feedback.publish_error_message = None;
                    current.feedback.last_published_root_event_id =
                        Some(record.root_event_id.clone());
                    if parent_event_id.is_some() {
                        current.feedback.reply_draft.clear();
                        if current
                            .feedback
                            .selected_root_event_id
                            .as_deref()
                            .is_some_and(|root| root == record.root_event_id)
                            && !current
                                .feedback
                                .selected_events
                                .iter()
                                .any(|event| event.event_id == record.event_id)
                        {
                            current.feedback.selected_events.push(record);
                            current.feedback.selected_event_count =
                                current.feedback.selected_events.len() as u64;
                        }
                    } else {
                        current.feedback.new_thread_draft.clear();
                    }
                    current.bump();
                }
                block_on_local(
                    runtime,
                    "refresh_feedback_threads",
                    refresh_feedback_threads(core, state, &runtimes.feedback_runtime),
                );
                if parent_event_id.is_some() {
                    block_on_local(
                        runtime,
                        "refresh_feedback_thread",
                        refresh_feedback_thread(core, state, &runtimes.feedback_runtime),
                    );
                }
            }
            Err(err) => set_feedback_publish_error(state, format!("Couldn't send feedback: {err}")),
        },
        OpOutcome::MediaSettingsWrite { result } => match result {
            Ok(()) => {
                let mut current = state.write();
                current.media_settings.is_saving = false;
                current.media_settings.error_message = None;
                current.bump();
            }
            Err(err) => {
                set_media_settings_error(state, format!("Couldn't save media servers: {err}"))
            }
        },
        OpOutcome::ProfileEditSubmit { result } => match result {
            Ok(profile) => {
                let pubkey = profile.pubkey.clone();
                let profile_for_cache = profile.clone();
                {
                    let mut current = state.write();
                    current.edit_profile.is_saving = false;
                    current.edit_profile.error_message = None;
                    current.edit_profile.saved_profile = Some(profile.clone());
                    current.chrome.current_user_profile = Some(profile);
                    current.bump();
                }
                insert_profile_metadata(state, pubkey, profile_for_cache, visible_limit);
                block_on_local(
                    runtime,
                    "hydrate_app_chrome",
                    hydrate_app_chrome(core, state, &mut runtimes.pending_joins, visible_limit),
                );
            }
            Err(err) => set_edit_profile_error(state, err),
        },
        OpOutcome::ArticleBookmarkToggle { result } => match result {
            Ok(()) => {
                set_toast(state, None);
                block_on_local(
                    runtime,
                    "hydrate_bookmarks",
                    hydrate_bookmarks(core, state, visible_limit),
                );
                if runtimes.bookmark_runtime.library_open {
                    block_on_local(
                        runtime,
                        "refresh_bookmarks_library",
                        refresh_bookmarks_library(
                            core,
                            state,
                            &mut runtimes.bookmark_runtime,
                            visible_limit,
                        ),
                    );
                }
            }
            Err(err) => set_toast(
                state,
                Some(HighlighterToast {
                    kind: HighlighterToastKind::Error,
                    message: err,
                }),
            ),
        },
        OpOutcome::CurationWrite { result } => match result {
            Ok(message) => {
                set_curation_menu_saving(state, false);
                if message.is_empty() {
                    set_toast(state, None);
                } else {
                    set_toast(
                        state,
                        Some(HighlighterToast {
                            kind: HighlighterToastKind::Success,
                            message,
                        }),
                    );
                }
                block_on_local(
                    runtime,
                    "refresh_bookmark_surfaces",
                    refresh_bookmark_surfaces(
                        core,
                        state,
                        &mut runtimes.bookmark_runtime,
                        visible_limit,
                    ),
                );
            }
            Err(err) => {
                set_curation_menu_saving(state, false);
                set_toast(
                    state,
                    Some(HighlighterToast {
                        kind: HighlighterToastKind::Error,
                        message: err.clone(),
                    }),
                );
                set_curation_menu_error(state, err);
            }
        },
        OpOutcome::FollowToggle {
            desired_following,
            previous_following,
            result,
        } => match result {
            Ok(()) => {
                let mut current = state.write();
                current.profile_view.is_following = desired_following;
                current.profile_view.is_mutating_follow = false;
                current.profile_view.error_message = None;
                current.bump();
            }
            Err(err) => {
                let mut current = state.write();
                current.profile_view.is_following = previous_following;
                current.profile_view.is_mutating_follow = false;
                current.profile_view.error_message = Some(err.clone());
                current.toast = Some(HighlighterToast {
                    kind: HighlighterToastKind::Error,
                    message: err,
                });
                current.bump();
            }
        },
        OpOutcome::HighlightPublish {
            note_was_empty,
            result,
        } => match result {
            Ok(record) => {
                let mut current = state.write();
                current
                    .article_reader
                    .highlights
                    .retain(|highlight| highlight.event_id != record.event_id);
                current.article_reader.highlights.insert(0, record.clone());
                sort_highlights_newest_first(&mut current.article_reader.highlights);
                let max_len = visible_limit.clamp(1, ARTICLE_READER_HIGHLIGHT_LIMIT);
                current.article_reader.highlights.truncate(max_len);
                current.article_reader.highlight_count =
                    current.article_reader.highlights.len() as u64;
                current.article_reader.last_published_highlight_id = Some(record.event_id);
                current.article_reader.is_publishing_highlight = false;
                current.article_reader.error_message = None;
                current.toast = Some(HighlighterToast {
                    kind: HighlighterToastKind::Success,
                    message: if note_was_empty {
                        "Highlighted".into()
                    } else {
                        "Highlighted with note".into()
                    },
                });
                current.bump();
            }
            Err(err) => set_article_reader_publish_error(state, err),
        },
        OpOutcome::SharePublish { group_id, result } => match result {
            Ok(message) => set_share_composer_success(state, group_id, message),
            Err(err) => set_share_composer_error(state, err),
        },
        OpOutcome::CapturePublish { result } => match result {
            Ok((event_id, message)) => set_capture_publish_success(state, event_id, message),
            Err(err) => set_capture_publish_error(state, err),
        },
        OpOutcome::RoomDiscussionPublish { result } => match result {
            Ok(event_id) => {
                {
                    let mut current = state.write();
                    current.room_detail.is_publishing_discussion = false;
                    current.room_detail.discussion_error_message = None;
                    current.room_detail.last_published_discussion_id = Some(event_id);
                    current.bump();
                }
                block_on_local(
                    runtime,
                    "refresh_room_detail",
                    refresh_room_detail(
                        core,
                        state,
                        &mut runtimes.room_detail_runtime,
                        visible_limit,
                    ),
                );
            }
            Err(err) => {
                set_room_discussion_error(state, format!("Failed to publish discussion: {err}"))
            }
        },
        OpOutcome::NetworkRelayWrite { kind, result } => {
            match result {
                Ok(()) => clear_network_action_error(state),
                Err(err) => {
                    let prefix = match kind {
                        NetworkRelayWriteKind::Upsert => "Couldn't save relay",
                        NetworkRelayWriteKind::Remove => "Couldn't remove relay",
                        NetworkRelayWriteKind::SetRoles => "Couldn't update relay roles",
                        NetworkRelayWriteKind::ImportApply => "Couldn't import",
                    };
                    if matches!(kind, NetworkRelayWriteKind::ImportApply) {
                        let mut current = state.write();
                        current.network.import_relays.is_applying = false;
                        current.network.import_relays.error_message =
                            Some(format!("{prefix}: {err}"));
                        current.bump();
                    } else {
                        set_network_action_error(state, format!("{prefix}: {err}"));
                    }
                }
            }
            // On success of an import-apply, clear the import staging area.
            if matches!(kind, NetworkRelayWriteKind::ImportApply) {
                let succeeded = {
                    let current = state.read();
                    current.network.import_relays.error_message.is_none()
                };
                if succeeded {
                    let mut current = state.write();
                    current.network.import_relays = HighlighterNetworkImportSnapshot::empty();
                    current.bump();
                }
            }
            // All relay writes refresh the network panel (Class-D, ndb + sync).
            block_on_local(
                runtime,
                "refresh_network_settings",
                refresh_network_settings(core, state),
            );
        }
        OpOutcome::OnboardingFollows { failures } => {
            if failures > 0 {
                let mut current = state.write();
                current.toast = Some(HighlighterToast {
                    kind: HighlighterToastKind::Info,
                    message: "Saved your interests".into(),
                });
                current.bump();
            }
        }
        // --- Phase 4: route the folded workers through their existing
        // resolvers, byte-for-byte. Each resolver still performs its own
        // domain-specific staleness re-check (search_runtime.generation,
        // username_generation, auth_generation, create_generation, the blossom
        // pubkey match, the pending_* coalescing sets), so behaviour is
        // identical to the deleted KernelMsg arms. The caller emits afterwards.
        OpOutcome::SearchLocal {
            generation,
            query,
            result,
        } => {
            handle_search_local_resolved(
                ctx,
                runtimes,
                SearchLocalResolution {
                    generation,
                    query,
                    result,
                },
            );
        }
        OpOutcome::UsernameCheck {
            generation,
            username,
            result,
        } => {
            handle_username_availability_resolved(
                state,
                &mut runtimes.create_account_runtime,
                generation,
                username,
                result,
            );
        }
        OpOutcome::NsecSignIn {
            generation,
            nsec,
            persist,
            clear_stored_on_failure,
            result,
        } => {
            handle_nsec_sign_in_resolved(
                ctx,
                runtimes,
                generation,
                nsec,
                persist,
                clear_stored_on_failure,
                result,
            );
        }
        OpOutcome::BunkerSignIn {
            generation,
            uri,
            persist,
            clear_stored_on_failure,
            result,
        } => {
            handle_bunker_sign_in_resolved(
                ctx,
                runtimes,
                generation,
                uri,
                persist,
                clear_stored_on_failure,
                result,
            );
        }
        OpOutcome::Nip55SignIn {
            generation,
            signer_package,
            persist,
            clear_stored_on_failure,
            result,
        } => {
            handle_nip55_sign_in_resolved(
                ctx,
                runtimes,
                generation,
                signer_package,
                persist,
                clear_stored_on_failure,
                result,
            );
        }
        OpOutcome::AccountCreate { generation, result } => {
            handle_account_create_resolved(
                ctx,
                &mut runtimes.create_account_runtime,
                generation,
                *result,
            );
        }
        OpOutcome::DefaultBlossomInit { pubkey_hex, result } => {
            handle_default_blossom_init_resolved(
                &ctx.state,
                &ctx.reconciler,
                &mut runtimes.app_scope_subscriptions,
                pubkey_hex,
                result,
            );
        }
        OpOutcome::IsbnPreview { requested, result } => {
            handle_isbn_preview_resolved(
                state,
                &mut runtimes.pending_isbn_lookups,
                requested,
                *result,
                visible_limit,
            );
        }
        OpOutcome::WebMetadata { requested, result } => {
            handle_web_metadata_resolved(
                state,
                &mut runtimes.pending_web_metadata,
                requested,
                *result,
                visible_limit,
            );
        }
    }
}

/// Apply a finished blossom upload into whichever surface requested it.
fn apply_blossom_upload_outcome(
    state: &Arc<RwLock<HighlighterAppState>>,
    slot: UploadSlot,
    result: Result<BlossomUpload, String>,
) {
    match slot {
        UploadSlot::RoomCover => match result {
            Ok(upload) => {
                let mut current = state.write();
                current.create_room.cover_upload = Some(upload);
                current.create_room.is_cover_uploading = false;
                current.create_room.error_message = None;
                current.bump();
            }
            Err(err) => set_create_room_error(state, format!("Couldn't upload cover: {err}")),
        },
        UploadSlot::ProfilePicture => match result {
            Ok(upload) => {
                let mut current = state.write();
                current.edit_profile.picture = upload.url;
                current.edit_profile.is_picture_uploading = false;
                current.edit_profile.error_message = None;
                current.bump();
            }
            Err(err) => {
                set_edit_profile_image_uploading(
                    state,
                    HighlighterEditProfileImageTarget::Picture,
                    false,
                );
                set_edit_profile_error(state, format!("Upload failed: {err}"));
            }
        },
        UploadSlot::ProfileBanner => match result {
            Ok(upload) => {
                let mut current = state.write();
                current.edit_profile.banner = upload.url;
                current.edit_profile.is_banner_uploading = false;
                current.edit_profile.error_message = None;
                current.bump();
            }
            Err(err) => {
                set_edit_profile_image_uploading(
                    state,
                    HighlighterEditProfileImageTarget::Banner,
                    false,
                );
                set_edit_profile_error(state, format!("Upload failed: {err}"));
            }
        },
        UploadSlot::Capture => match result {
            Ok(upload) => {
                let mut current = state.write();
                current.capture.upload = Some(upload);
                current.capture.is_uploading = false;
                current.capture.upload_error_message = None;
                current.bump();
            }
            Err(err) => set_capture_upload_error(state, format!("Upload failed: {err}")),
        },
    }
}

fn handle_action(
    action: HighlighterAppAction,
    runtime: &tokio::runtime::Runtime,
    ctx: &ActorContext,
    runtimes: &mut ActorRuntimes,
) {
    let core = &ctx.core;
    let state = &ctx.state;
    let reconciler = &ctx.reconciler;
    let visible_limit = ctx.visible_limit;
    let local_state_path = ctx.local_state_path.as_path();
    let pending_joins = &mut runtimes.pending_joins;
    let auth_generation = &mut runtimes.auth_generation;
    let pending_isbn_lookups = &mut runtimes.pending_isbn_lookups;
    let pending_web_metadata = &mut runtimes.pending_web_metadata;
    let profile_handles = &mut runtimes.profile_handles;
    let profile_pubkeys_by_handle = &mut runtimes.profile_pubkeys_by_handle;
    let app_scope_subscriptions = &mut runtimes.app_scope_subscriptions;
    let search_runtime = &mut runtimes.search_runtime;
    let create_account_runtime = &mut runtimes.create_account_runtime;
    let home_feed_runtime = &mut runtimes.home_feed_runtime;
    let bookmark_runtime = &mut runtimes.bookmark_runtime;
    let profile_view_runtime = &mut runtimes.profile_view_runtime;
    let article_reader_runtime = &mut runtimes.article_reader_runtime;
    let room_detail_runtime = &mut runtimes.room_detail_runtime;
    let room_invite_runtime = &mut runtimes.room_invite_runtime;
    let feedback_runtime = &mut runtimes.feedback_runtime;
    let room_explorer_runtime = &mut runtimes.room_explorer_runtime;
    let network_runtime = &mut runtimes.network_runtime;
    let ops = &mut runtimes.ops;

    match action {
        HighlighterAppAction::Bootstrap => {
            set_bootstrapping(state, true);
            emit(state, reconciler);
            block_on_local(
                runtime,
                "hydrate_app_chrome",
                hydrate_app_chrome(core, state, pending_joins, visible_limit),
            );
            block_on_local(
                runtime,
                "ensure_signed_in_app_scope",
                ensure_signed_in_app_scope(core, state, app_scope_subscriptions, ops),
            );
            set_bootstrapping(state, false);
            emit(state, reconciler);
        }
        HighlighterAppAction::RefreshAppChrome => {
            block_on_local(
                runtime,
                "hydrate_app_chrome",
                hydrate_app_chrome(core, state, pending_joins, visible_limit),
            );
            block_on_local(
                runtime,
                "ensure_signed_in_app_scope",
                ensure_signed_in_app_scope(core, state, app_scope_subscriptions, ops),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::AppForegrounded => {
            block_on_local(
                runtime,
                "handle_app_foregrounded",
                handle_app_foregrounded(core, state, pending_joins, visible_limit),
            );
            block_on_local(
                runtime,
                "ensure_signed_in_app_scope",
                ensure_signed_in_app_scope(core, state, app_scope_subscriptions, ops),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::SignInNsec {
            nsec,
            persist,
            clear_stored_on_failure,
        } => {
            set_signing_in(state, true);
            emit(state, reconciler);
            let nsec = nsec.trim().to_string();
            // nsec + bunker share `auth_generation` AND OpRunner's single
            // `OpDomain::Auth` slot, preserving today's cross-method
            // supersession: a later sign-in (of either kind) supersedes any
            // in-flight attempt — last sign-in wins (Phase 4 fold).
            *auth_generation = auth_generation.saturating_add(1);
            let generation = *auth_generation;
            // Establish the nsec identity synchronously on the actor thread
            // (network-free) BEFORE submitting the off-actor op, so a rapid
            // superseding sign-in (e.g. a bunker pairing dispatched right after)
            // observes this account as the prior active identity. Without this,
            // the bunker op could read an unset prior identity and latch onto the
            // nsec's later async identity-apply, masking a genuine pairing
            // timeout. The full `login_nsec` inside the op re-applies idempotently
            // and still performs the (network-bound) signer install off-actor.
            let _ = core.apply_nsec_identity(&nsec);
            let core_for_op = core.clone();
            let timeout_nsec = nsec.clone();
            ops.submit_op(
                OpDomain::Auth,
                OP_DEADLINE_NETWORK,
                // Truthful timeout fallback: live generation + the real
                // persist/clear flags so a wedged sign-in clears `is_signing_in`
                // (and honors clear-stored) instead of being dropped at gen 0.
                // Carries the secret exactly as the success outcome does; the
                // Err arm never logs it.
                OpOutcome::NsecSignIn {
                    generation,
                    nsec: timeout_nsec,
                    persist,
                    clear_stored_on_failure,
                    result: Err(op_timeout_message(OpDomain::Auth)),
                },
                async move {
                    // `login_nsec` is a SYNC core method that blocks internally on
                    // the NMP runtime (it installs the signer via `block_on`). The
                    // legacy worker therefore ran on a dedicated current-thread
                    // runtime; on OpRunner's multi-thread runtime a nested
                    // `block_on` panics ("runtime within a runtime"), so we move it
                    // to the blocking pool with `spawn_blocking`.
                    let nsec_for_call = nsec.clone();
                    let result = tokio::task::spawn_blocking(move || {
                        core_for_op
                            .login_nsec(nsec_for_call)
                            .map_err(|err| err.to_string())
                    })
                    .await
                    .unwrap_or_else(|join_err| Err(format!("sign-in task failed: {join_err}")));
                    OpOutcome::NsecSignIn {
                        generation,
                        nsec,
                        persist,
                        clear_stored_on_failure,
                        result,
                    }
                },
            );
        }
        HighlighterAppAction::PairBunker {
            uri,
            persist,
            clear_stored_on_failure,
        } => {
            set_signing_in(state, true);
            emit(state, reconciler);
            let uri = uri.trim().to_string();
            *auth_generation = auth_generation.saturating_add(1);
            let generation = *auth_generation;
            let core_for_op = core.clone();
            let timeout_uri = uri.clone();
            ops.submit_op(
                OpDomain::Auth,
                OP_DEADLINE_NETWORK,
                // Truthful bunker timeout fallback: live generation + real flags
                // so a black-holed pairing clears `is_signing_in` with an error
                // toast instead of wedging at gen 0.
                OpOutcome::BunkerSignIn {
                    generation,
                    uri: timeout_uri,
                    persist,
                    clear_stored_on_failure,
                    result: Err(op_timeout_message(OpDomain::Auth)),
                },
                async move {
                    let result = core_for_op
                        .pair_bunker(uri.clone())
                        .await
                        .map_err(|err| err.to_string());
                    OpOutcome::BunkerSignIn {
                        generation,
                        uri,
                        persist,
                        clear_stored_on_failure,
                        result,
                    }
                },
            );
        }
        HighlighterAppAction::SignInNip55 {
            signer_package,
            persist,
            clear_stored_on_failure,
        } => {
            // NIP-55 sign-in (ADR-0048 Stage 2) — the PairBunker shape, NIP-55
            // backend (V-78: completion semantics are signer-backend-neutral).
            // `pair_nip55` fires the `Nip55Driver` `get_public_key` handshake
            // (Amber Intent round-trip via the capability trampoline + the
            // Kotlin drain thread + `DeliverExternalSignerResponse`), waits
            // for the identity actor to expose the paired account, installs
            // the session user and login bootstrap, and resolves here so
            // `is_signing_in` always clears — success, rejection, or timeout.
            set_signing_in(state, true);
            emit(state, reconciler);
            *auth_generation = auth_generation.saturating_add(1);
            let generation = *auth_generation;
            let core_for_op = core.clone();
            let timeout_package = signer_package.clone();
            let op_package = signer_package.clone();
            ops.submit_op(
                OpDomain::Auth,
                OP_DEADLINE_SIGNER_PAIR,
                // Truthful timeout fallback: a user who abandons the Amber
                // dialog clears `is_signing_in` with an error toast instead
                // of wedging the login screen.
                OpOutcome::Nip55SignIn {
                    generation,
                    signer_package: timeout_package,
                    persist,
                    clear_stored_on_failure,
                    result: Err(op_timeout_message(OpDomain::Auth)),
                },
                async move {
                    let result = core_for_op
                        .pair_nip55(op_package.clone())
                        .await
                        .map_err(|err| err.to_string());
                    OpOutcome::Nip55SignIn {
                        generation,
                        signer_package: op_package,
                        persist,
                        clear_stored_on_failure,
                        result,
                    }
                },
            );
        }
        HighlighterAppAction::DeliverExternalSignerResponse { response_json } => {
            // Route the host's raw result back to the NIP-55 driver (D7 —
            // verbatim; the driver owns correlation routing and all policy).
            core.nmp_deliver_external_signer_response(&response_json);
        }
        HighlighterAppAction::SetCreateAccountDisplayName { display_name } => {
            let next_username =
                update_create_account_display_name(state, display_name, create_account_runtime);
            if let Some((generation, username)) = next_username {
                submit_username_availability_op(ops, generation, username);
            }
            emit(state, reconciler);
        }
        HighlighterAppAction::SetCreateAccountUsername { username } => {
            let next_username =
                update_create_account_username(state, username, create_account_runtime);
            if let Some((generation, username)) = next_username {
                submit_username_availability_op(ops, generation, username);
            }
            emit(state, reconciler);
        }
        HighlighterAppAction::SubmitCreateAccount => {
            if let Some(request) = prepare_create_account_request(state, create_account_runtime) {
                submit_create_account_op(ops, core, request);
            }
            emit(state, reconciler);
        }
        HighlighterAppAction::UploadCreateRoomCover {
            bytes,
            mime,
            width,
            height,
            alt,
        } => {
            if bytes.is_empty() {
                set_create_room_error(state, "That image couldn't be read.".into());
                emit(state, reconciler);
            } else {
                set_create_room_cover_uploading(state, true);
                emit(state, reconciler);
                let core = core.clone();
                ops.submit_op(
                    OpDomain::BlossomUpload(UploadSlot::RoomCover),
                    OP_DEADLINE_NETWORK,
                    OpOutcome::BlossomUpload {
                        slot: UploadSlot::RoomCover,
                        result: Err(op_timeout_message(OpDomain::BlossomUpload(
                            UploadSlot::RoomCover,
                        ))),
                    },
                    async move {
                        let result = core
                            .upload_photo(bytes, mime, width, height, alt)
                            .await
                            .map_err(|err| err.to_string());
                        OpOutcome::BlossomUpload {
                            slot: UploadSlot::RoomCover,
                            result,
                        }
                    },
                );
            }
        }
        HighlighterAppAction::CreateRoomCapabilityFailed { message } => {
            let message = message.trim();
            set_create_room_error(
                state,
                if message.is_empty() {
                    "Couldn't read that image.".into()
                } else {
                    message.to_string()
                },
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::ClearCreateRoomCover => {
            clear_create_room_cover(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::SubmitCreateRoom {
            name,
            about,
            visibility,
            access,
        } => {
            let name = name.trim().to_string();
            if name.chars().count() < 2 {
                set_create_room_error(state, "Name your room first.".into());
                emit(state, reconciler);
            } else {
                let about = about.trim().to_string();
                let picture = state
                    .read()
                    .create_room
                    .cover_upload
                    .as_ref()
                    .map(|upload| upload.url.clone())
                    .unwrap_or_default();
                set_create_room_creating(state, true);
                emit(state, reconciler);
                let core = core.clone();
                ops.submit_op(
                    OpDomain::RoomCreate,
                    OP_DEADLINE_NETWORK,
                    OpOutcome::RoomCreate {
                        result: Err(op_timeout_message(OpDomain::RoomCreate)),
                    },
                    async move {
                        let result = core
                            .create_room(name, about, picture, visibility, access)
                            .await
                            .map_err(|err| err.to_string());
                        OpOutcome::RoomCreate { result }
                    },
                );
            }
        }
        HighlighterAppAction::ClearCreateRoomResult => {
            clear_create_room_result(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::ClearCreateRoomError => {
            clear_create_room_error(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::OpenRoomInvite { group_id } => {
            prepare_open_room_invite(state, room_invite_runtime, group_id);
            let prefetch = block_on_local(
                runtime,
                "refresh_room_invite_follows",
                refresh_room_invite_follows(core, state, room_invite_runtime),
            );
            for pubkey in prefetch {
                request_profile(
                    runtime,
                    core,
                    state,
                    profile_handles,
                    profile_pubkeys_by_handle,
                    pubkey,
                    visible_limit,
                );
            }
            submit_mint_room_invite_link(ops, state, core);
            emit(state, reconciler);
        }
        HighlighterAppAction::RefreshRoomInvite => {
            let prefetch = block_on_local(
                runtime,
                "refresh_room_invite_follows",
                refresh_room_invite_follows(core, state, room_invite_runtime),
            );
            for pubkey in prefetch {
                request_profile(
                    runtime,
                    core,
                    state,
                    profile_handles,
                    profile_pubkeys_by_handle,
                    pubkey,
                    visible_limit,
                );
            }
            emit(state, reconciler);
        }
        HighlighterAppAction::SetRoomInviteQuery { query } => {
            let resolved = set_room_invite_query(core, state, room_invite_runtime, query);
            if let Some(pubkey) = resolved {
                request_profile(
                    runtime,
                    core,
                    state,
                    profile_handles,
                    profile_pubkeys_by_handle,
                    pubkey,
                    visible_limit,
                );
            }
            emit(state, reconciler);
        }
        HighlighterAppAction::ToggleRoomInviteCandidate { pubkey_hex, source } => {
            toggle_room_invite_candidate(state, pubkey_hex, source);
            emit(state, reconciler);
        }
        HighlighterAppAction::RemoveRoomInviteCandidate { pubkey_hex } => {
            remove_room_invite_candidate(state, &pubkey_hex);
            emit(state, reconciler);
        }
        HighlighterAppAction::AcceptRoomInvitePastedCandidate => {
            accept_room_invite_pasted_candidate(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::MintRoomInviteLink => {
            submit_mint_room_invite_link(ops, state, core);
            emit(state, reconciler);
        }
        HighlighterAppAction::SubmitRoomInviteMembers => {
            let (group_id, selected) = {
                let current = state.read();
                (
                    current.room_invite.group_id.clone(),
                    current.room_invite.selected.clone(),
                )
            };
            if group_id.trim().is_empty() || selected.is_empty() {
                emit(state, reconciler);
            } else {
                {
                    let mut current = state.write();
                    current.room_invite.is_adding_members = true;
                    current.room_invite.add_error_message = None;
                    current.room_invite.toast_message = None;
                    current.bump();
                }
                emit(state, reconciler);
                let core = core.clone();
                ops.submit_op(
                    OpDomain::RoomInviteMembers,
                    OP_DEADLINE_NETWORK,
                    OpOutcome::RoomInviteMembers {
                        result: Err(op_timeout_message(OpDomain::RoomInviteMembers)),
                    },
                    async move {
                        let total = selected.len();
                        let mut failures = Vec::new();
                        for candidate in &selected {
                            if core
                                .add_room_member(group_id.clone(), candidate.pubkey_hex.clone())
                                .await
                                .is_err()
                            {
                                failures.push(candidate.pubkey_hex.clone());
                            }
                        }
                        OpOutcome::RoomInviteMembers {
                            result: Ok(RoomInviteMembersOutcome { total, failures }),
                        }
                    },
                );
            }
        }
        HighlighterAppAction::ClearRoomInviteAddError => {
            clear_room_invite_add_error(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::ClearRoomInviteInviteLinkError => {
            clear_room_invite_link_error(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::ClearRoomInviteToast => {
            clear_room_invite_toast(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::CloseRoomInvite => {
            clear_room_invite_snapshot(state);
            room_invite_runtime.group_id = None;
            room_invite_runtime.follows.clear();
            emit(state, reconciler);
        }
        HighlighterAppAction::OpenComments {
            root_tag_name,
            root_tag_value,
            root_kind,
        } => {
            if prepare_open_comments(state, root_tag_name, root_tag_value, root_kind) {
                emit(state, reconciler);
                block_on_local(runtime, "refresh_comments", refresh_comments(core, state));
            }
            emit(state, reconciler);
        }
        HighlighterAppAction::RefreshComments => {
            block_on_local(runtime, "refresh_comments", refresh_comments(core, state));
            emit(state, reconciler);
        }
        HighlighterAppAction::SetCommentDraft {
            parent_event_id,
            body,
        } => {
            set_comment_draft(state, parent_event_id, body);
            emit(state, reconciler);
        }
        HighlighterAppAction::PublishComment { parent_event_id } => {
            let (root_tag_name, root_tag_value, root_kind, body) = {
                let current = state.read();
                (
                    current.comments.root_tag_name.clone(),
                    current.comments.root_tag_value.clone(),
                    current.comments.root_kind,
                    comment_draft_body(&current.comments, parent_event_id.as_deref())
                        .trim()
                        .to_string(),
                )
            };
            if body.is_empty() {
                set_comment_publish_error(state, "Write a comment first.".into());
                emit(state, reconciler);
            } else {
                set_comment_publishing(state, true);
                emit(state, reconciler);
                let core = core.clone();
                let timeout_parent = parent_event_id.clone();
                ops.submit_op(
                    OpDomain::CommentPublish,
                    OP_DEADLINE_NETWORK,
                    OpOutcome::CommentPublish {
                        parent_event_id: timeout_parent,
                        result: Err(op_timeout_message(OpDomain::CommentPublish)),
                    },
                    async move {
                        let result = core
                            .publish_comment(
                                root_tag_name,
                                root_tag_value,
                                root_kind,
                                parent_event_id.clone(),
                                body,
                            )
                            .await
                            .map_err(|err| err.to_string());
                        OpOutcome::CommentPublish {
                            parent_event_id,
                            result,
                        }
                    },
                );
            }
        }
        HighlighterAppAction::ClearCommentPublishError => {
            clear_comment_publish_error(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::ToggleCommentLike { event_id } => {
            let event_id = event_id.trim().to_string();
            let (author, existing_like) = {
                let current = state.read();
                let author = current
                    .comments
                    .records
                    .iter()
                    .find(|record| record.event_id == event_id)
                    .map(|record| record.pubkey.clone());
                let existing_like = current
                    .comments
                    .interactions
                    .iter()
                    .find(|interaction| interaction.event_id == event_id)
                    .and_then(|interaction| interaction.my_like_event_id.clone());
                (author, existing_like)
            };
            let Some(author) = author else {
                set_comment_interaction_error(state, "Comment not found.".into());
                emit(state, reconciler);
                return;
            };
            let domain = OpDomain::CommentInteraction {
                target: op_target_hash(&event_id),
                kind: CommentInteractionKind::Like,
            };
            let core = core.clone();
            ops.submit_op(
                domain,
                OP_DEADLINE_NETWORK,
                OpOutcome::CommentInteraction {
                    kind: CommentInteractionKind::Like,
                    result: Err(op_timeout_message(domain)),
                },
                async move {
                    let result = if let Some(reaction_id) = existing_like {
                        core.unpublish_reaction(reaction_id).await.map(|_| ())
                    } else {
                        core.publish_reaction(event_id, author, 1111, "+".into())
                            .await
                            .map(|_| ())
                    }
                    .map_err(|err| err.to_string());
                    OpOutcome::CommentInteraction {
                        kind: CommentInteractionKind::Like,
                        result,
                    }
                },
            );
        }
        HighlighterAppAction::ToggleCommentBookmark { event_id } => {
            let event_id = event_id.trim().to_string();
            if event_id.is_empty() {
                emit(state, reconciler);
            } else {
                let domain = OpDomain::CommentInteraction {
                    target: op_target_hash(&event_id),
                    kind: CommentInteractionKind::Bookmark,
                };
                let core = core.clone();
                ops.submit_op(
                    domain,
                    OP_DEADLINE_NETWORK,
                    OpOutcome::CommentInteraction {
                        kind: CommentInteractionKind::Bookmark,
                        result: Err(op_timeout_message(domain)),
                    },
                    async move {
                        let result = core
                            .toggle_event_bookmark(event_id)
                            .await
                            .map(|_| ())
                            .map_err(|err| err.to_string());
                        OpOutcome::CommentInteraction {
                            kind: CommentInteractionKind::Bookmark,
                            result,
                        }
                    },
                );
            }
        }
        HighlighterAppAction::ClearCommentInteractionError => {
            clear_comment_interaction_error(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::CloseComments => {
            clear_comments_snapshot(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::OpenFeedback { coordinate } => {
            prepare_open_feedback(core, state, feedback_runtime, coordinate);
            set_feedback_threads_loading(state, true);
            emit(state, reconciler);
            block_on_local(
                runtime,
                "refresh_feedback_threads",
                refresh_feedback_threads(core, state, feedback_runtime),
            );
            block_on_local(
                runtime,
                "ensure_feedback_threads_subscription",
                ensure_feedback_threads_subscription(core, state, feedback_runtime),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::RefreshFeedbackThreads => {
            set_feedback_threads_loading(state, true);
            emit(state, reconciler);
            block_on_local(
                runtime,
                "refresh_feedback_threads",
                refresh_feedback_threads(core, state, feedback_runtime),
            );
            block_on_local(
                runtime,
                "ensure_feedback_threads_subscription",
                ensure_feedback_threads_subscription(core, state, feedback_runtime),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::SetFeedbackNewThreadDraft { body } => {
            set_feedback_new_thread_draft(state, body);
            emit(state, reconciler);
        }
        HighlighterAppAction::PublishFeedbackNewThread => {
            let (coordinate, body) = {
                let current = state.read();
                (
                    current.feedback.coordinate.clone(),
                    current.feedback.new_thread_draft.trim().to_string(),
                )
            };
            if coordinate.trim().is_empty() {
                set_feedback_publish_error(state, "Feedback isn't ready yet.".into());
                emit(state, reconciler);
            } else if body.is_empty() {
                set_feedback_publish_error(state, "Write feedback first.".into());
                emit(state, reconciler);
            } else {
                set_feedback_new_thread_publishing(state, true);
                emit(state, reconciler);
                let core = core.clone();
                let parent: Option<String> = None;
                let timeout_parent = parent.clone();
                ops.submit_op(
                    OpDomain::FeedbackPublish,
                    OP_DEADLINE_NETWORK,
                    OpOutcome::FeedbackPublish {
                        parent_event_id: timeout_parent,
                        result: Err(op_timeout_message(OpDomain::FeedbackPublish)),
                    },
                    async move {
                        let result = core
                            .publish_feedback_note(coordinate, None, parent.clone(), body)
                            .await
                            .map_err(|err| err.to_string());
                        OpOutcome::FeedbackPublish {
                            parent_event_id: parent,
                            result,
                        }
                    },
                );
            }
        }
        HighlighterAppAction::OpenFeedbackThread { root_event_id } => {
            prepare_open_feedback_thread(core, state, feedback_runtime, root_event_id);
            set_feedback_thread_loading(state, true);
            emit(state, reconciler);
            block_on_local(
                runtime,
                "refresh_feedback_thread",
                refresh_feedback_thread(core, state, feedback_runtime),
            );
            block_on_local(
                runtime,
                "ensure_feedback_thread_subscription",
                ensure_feedback_thread_subscription(core, state, feedback_runtime),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::RefreshFeedbackThread => {
            set_feedback_thread_loading(state, true);
            emit(state, reconciler);
            block_on_local(
                runtime,
                "refresh_feedback_thread",
                refresh_feedback_thread(core, state, feedback_runtime),
            );
            block_on_local(
                runtime,
                "ensure_feedback_thread_subscription",
                ensure_feedback_thread_subscription(core, state, feedback_runtime),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::SetFeedbackReplyDraft { body } => {
            set_feedback_reply_draft(state, body);
            emit(state, reconciler);
        }
        HighlighterAppAction::PublishFeedbackReply => {
            let root = feedback_runtime.selected_root_event_id.clone();
            {
                let (coordinate, body) = {
                    let current = state.read();
                    let body = if root.is_some() {
                        current.feedback.reply_draft.trim().to_string()
                    } else {
                        current.feedback.new_thread_draft.trim().to_string()
                    };
                    (current.feedback.coordinate.clone(), body)
                };
                if coordinate.trim().is_empty() {
                    set_feedback_publish_error(state, "Feedback isn't ready yet.".into());
                    emit(state, reconciler);
                } else if body.is_empty() {
                    set_feedback_publish_error(state, "Write feedback first.".into());
                    emit(state, reconciler);
                } else {
                    set_feedback_reply_publishing(state, true);
                    emit(state, reconciler);
                    let core = core.clone();
                    let parent = root.clone();
                    let timeout_parent = parent.clone();
                    ops.submit_op(
                        OpDomain::FeedbackPublish,
                        OP_DEADLINE_NETWORK,
                        OpOutcome::FeedbackPublish {
                            parent_event_id: timeout_parent,
                            result: Err(op_timeout_message(OpDomain::FeedbackPublish)),
                        },
                        async move {
                            let result = core
                                .publish_feedback_note(coordinate, None, parent.clone(), body)
                                .await
                                .map_err(|err| err.to_string());
                            OpOutcome::FeedbackPublish {
                                parent_event_id: parent,
                                result,
                            }
                        },
                    );
                }
            }
        }
        HighlighterAppAction::ClearFeedbackPublishError => {
            clear_feedback_publish_error(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::CloseFeedbackThread => {
            clear_feedback_thread_runtime(core, feedback_runtime);
            clear_feedback_thread_snapshot(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::CloseFeedback => {
            clear_feedback_runtime(core, feedback_runtime);
            clear_feedback_snapshot(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::OpenMediaSettings | HighlighterAppAction::RefreshMediaSettings => {
            set_media_settings_loading(state, true);
            emit(state, reconciler);
            block_on_local(
                runtime,
                "refresh_media_settings",
                refresh_media_settings(core, state),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::AddBlossomServer { url } => {
            if add_blossom_server_to_snapshot(state, url) {
                set_media_settings_saving(state, true);
                emit(state, reconciler);
                let servers = state.read().media_settings.blossom_servers.clone();
                if servers.is_empty() {
                    set_media_settings_error(state, "Keep at least one Blossom server.".into());
                } else {
                    let core = core.clone();
                    ops.submit_op(
                        OpDomain::MediaSettingsWrite,
                        OP_DEADLINE_NETWORK,
                        OpOutcome::MediaSettingsWrite {
                            result: Err(op_timeout_message(OpDomain::MediaSettingsWrite)),
                        },
                        async move {
                            let result = core
                                .set_blossom_servers(servers)
                                .await
                                .map(|_| ())
                                .map_err(|err| err.to_string());
                            OpOutcome::MediaSettingsWrite { result }
                        },
                    );
                }
            }
            emit(state, reconciler);
        }
        HighlighterAppAction::RemoveBlossomServer { url } => {
            if remove_blossom_server_from_snapshot(state, &url) {
                set_media_settings_saving(state, true);
                emit(state, reconciler);
                let servers = state.read().media_settings.blossom_servers.clone();
                if servers.is_empty() {
                    set_media_settings_error(state, "Keep at least one Blossom server.".into());
                } else {
                    let core = core.clone();
                    ops.submit_op(
                        OpDomain::MediaSettingsWrite,
                        OP_DEADLINE_NETWORK,
                        OpOutcome::MediaSettingsWrite {
                            result: Err(op_timeout_message(OpDomain::MediaSettingsWrite)),
                        },
                        async move {
                            let result = core
                                .set_blossom_servers(servers)
                                .await
                                .map(|_| ())
                                .map_err(|err| err.to_string());
                            OpOutcome::MediaSettingsWrite { result }
                        },
                    );
                }
            }
            emit(state, reconciler);
        }
        HighlighterAppAction::MoveBlossomServers {
            from_indices,
            to_index,
        } => {
            if move_blossom_servers_in_snapshot(state, from_indices, to_index) {
                set_media_settings_saving(state, true);
                emit(state, reconciler);
                let servers = state.read().media_settings.blossom_servers.clone();
                if servers.is_empty() {
                    set_media_settings_error(state, "Keep at least one Blossom server.".into());
                } else {
                    let core = core.clone();
                    ops.submit_op(
                        OpDomain::MediaSettingsWrite,
                        OP_DEADLINE_NETWORK,
                        OpOutcome::MediaSettingsWrite {
                            result: Err(op_timeout_message(OpDomain::MediaSettingsWrite)),
                        },
                        async move {
                            let result = core
                                .set_blossom_servers(servers)
                                .await
                                .map(|_| ())
                                .map_err(|err| err.to_string());
                            OpOutcome::MediaSettingsWrite { result }
                        },
                    );
                }
            }
            emit(state, reconciler);
        }
        HighlighterAppAction::ClearMediaSettingsError => {
            clear_media_settings_error(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::CloseMediaSettings => {
            clear_media_settings_transient_state(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::OpenEditProfile { seed } => {
            open_edit_profile(state, seed);
            emit(state, reconciler);
        }
        HighlighterAppAction::SetEditProfileDisplayName { value } => {
            set_edit_profile_field(state, |snapshot| snapshot.display_name = value);
            emit(state, reconciler);
        }
        HighlighterAppAction::SetEditProfileName { value } => {
            set_edit_profile_field(state, |snapshot| snapshot.name = value);
            emit(state, reconciler);
        }
        HighlighterAppAction::SetEditProfileAbout { value } => {
            set_edit_profile_field(state, |snapshot| snapshot.about = value);
            emit(state, reconciler);
        }
        HighlighterAppAction::SetEditProfilePicture { value } => {
            set_edit_profile_field(state, |snapshot| snapshot.picture = value);
            emit(state, reconciler);
        }
        HighlighterAppAction::SetEditProfileBanner { value } => {
            set_edit_profile_field(state, |snapshot| snapshot.banner = value);
            emit(state, reconciler);
        }
        HighlighterAppAction::SetEditProfileNip05 { value } => {
            set_edit_profile_field(state, |snapshot| snapshot.nip05 = value);
            emit(state, reconciler);
        }
        HighlighterAppAction::SetEditProfileWebsite { value } => {
            set_edit_profile_field(state, |snapshot| snapshot.website = value);
            emit(state, reconciler);
        }
        HighlighterAppAction::SetEditProfileLud16 { value } => {
            set_edit_profile_field(state, |snapshot| snapshot.lud16 = value);
            emit(state, reconciler);
        }
        HighlighterAppAction::UploadEditProfileImage {
            target,
            bytes,
            mime,
            width,
            height,
            alt,
        } => {
            set_edit_profile_image_uploading(state, target, true);
            emit(state, reconciler);
            let slot = match target {
                HighlighterEditProfileImageTarget::Picture => UploadSlot::ProfilePicture,
                HighlighterEditProfileImageTarget::Banner => UploadSlot::ProfileBanner,
            };
            let core = core.clone();
            ops.submit_op(
                OpDomain::BlossomUpload(slot),
                OP_DEADLINE_NETWORK,
                OpOutcome::BlossomUpload {
                    slot,
                    result: Err(op_timeout_message(OpDomain::BlossomUpload(slot))),
                },
                async move {
                    let result = core
                        .upload_photo(bytes, mime, width, height, alt)
                        .await
                        .map_err(|err| err.to_string());
                    OpOutcome::BlossomUpload { slot, result }
                },
            );
        }
        HighlighterAppAction::EditProfileCapabilityFailed { message } => {
            set_edit_profile_error(
                state,
                if message.trim().is_empty() {
                    "Couldn't read that image.".into()
                } else {
                    message
                },
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::SubmitEditProfile => {
            let draft = {
                let current = state.read();
                ProfileUpdateDraft {
                    name: current.edit_profile.name.trim().to_string(),
                    display_name: current.edit_profile.display_name.trim().to_string(),
                    about: current.edit_profile.about.trim().to_string(),
                    picture: current.edit_profile.picture.trim().to_string(),
                    banner: current.edit_profile.banner.trim().to_string(),
                    nip05: current.edit_profile.nip05.trim().to_string(),
                    website: current.edit_profile.website.trim().to_string(),
                    lud16: current.edit_profile.lud16.trim().to_string(),
                }
            };
            set_edit_profile_saving(state, true);
            emit(state, reconciler);
            let core = core.clone();
            ops.submit_op(
                OpDomain::ProfileEditSubmit,
                OP_DEADLINE_NETWORK,
                OpOutcome::ProfileEditSubmit {
                    result: Err(op_timeout_message(OpDomain::ProfileEditSubmit)),
                },
                async move {
                    let result = core
                        .update_profile(draft)
                        .await
                        .map_err(|err| err.to_string());
                    OpOutcome::ProfileEditSubmit { result }
                },
            );
        }
        HighlighterAppAction::ClearEditProfileError => {
            clear_edit_profile_error(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::ClearEditProfileResult => {
            clear_edit_profile_result(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::CloseEditProfile => {
            clear_edit_profile_snapshot(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::StartNostrConnect { callback_url } => {
            set_signing_in(state, true);
            emit(state, reconciler);
            let options = NostrConnectOptions {
                name: NOSTR_CONNECT_NAME.into(),
                url: NOSTR_CONNECT_URL.into(),
                image: NOSTR_CONNECT_IMAGE.into(),
                perms: NOSTR_CONNECT_PERMS.into(),
            };
            match block_on_local(
                runtime,
                "start_nostr_connect",
                core.start_nostr_connect(options),
            ) {
                Ok(uri) => {
                    set_toast(state, None);
                    set_signing_in(state, false);
                    emit(state, reconciler);
                    emit_open_external_url(reconciler, append_callback_url(uri, &callback_url));
                }
                Err(err) => {
                    set_toast(
                        state,
                        Some(HighlighterToast {
                            kind: HighlighterToastKind::Error,
                            message: err.to_string(),
                        }),
                    );
                    set_signing_in(state, false);
                    emit(state, reconciler);
                }
            }
        }
        HighlighterAppAction::ExternalUrlOpenFailed { url } => {
            tracing::warn!(url = %url, "external URL open failed");
            set_toast(
                state,
                Some(HighlighterToast {
                    kind: HighlighterToastKind::Error,
                    message: "Couldn't open signer app".into(),
                }),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::Logout => {
            clear_app_scope_subscriptions(core, app_scope_subscriptions);
            clear_profile_subscriptions(core, profile_handles, profile_pubkeys_by_handle);
            clear_search_runtime(core, search_runtime);
            clear_home_feed_runtime(core, home_feed_runtime);
            clear_bookmark_runtime(core, bookmark_runtime);
            clear_profile_view_runtime(core, profile_view_runtime);
            clear_article_reader_runtime(core, article_reader_runtime);
            clear_room_detail_runtime(core, room_detail_runtime);
            clear_feedback_runtime(core, feedback_runtime);
            room_invite_runtime.group_id = None;
            room_invite_runtime.follows.clear();
            *room_explorer_runtime = RoomExplorerRuntime::default();
            core.logout();
            pending_joins.clear();
            ops.cancel_all();
            save_local_state(local_state_path, &local_state_for_snapshot(state, false));
            {
                let snapshot = state.read().clone();
                let recent_queries = snapshot.search.recent_queries;
                let mut network = HighlighterNetworkSnapshot::empty();
                network.wifi_only_enabled = snapshot.network.wifi_only_enabled;
                network.current_path_is_wifi = snapshot.network.current_path_is_wifi;
                let mut current = state.write();
                let rev = current.rev.saturating_add(1);
                *current = HighlighterAppState::empty(false);
                apply_recent_searches_to_snapshot(&mut current.search, recent_queries);
                current.network = network;
                current.rev = rev;
            }
            emit(state, reconciler);
        }
        HighlighterAppAction::ToggleArticleBookmark { address } => {
            let domain = OpDomain::ArticleBookmarkToggle {
                target: op_target_hash(&address),
            };
            let core = core.clone();
            ops.submit_op(
                domain,
                OP_DEADLINE_NETWORK,
                OpOutcome::ArticleBookmarkToggle {
                    result: Err(op_timeout_message(domain)),
                },
                async move {
                    let result = core
                        .toggle_article_bookmark(address)
                        .await
                        .map(|_| ())
                        .map_err(|err| err.to_string());
                    OpOutcome::ArticleBookmarkToggle { result }
                },
            );
        }
        HighlighterAppAction::OpenBookmarks | HighlighterAppAction::RefreshBookmarks => {
            bookmark_runtime.library_open = true;
            set_bookmarks_loading(state, true);
            emit(state, reconciler);
            block_on_local(
                runtime,
                "refresh_bookmarks_library",
                refresh_bookmarks_library(core, state, bookmark_runtime, visible_limit),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::CloseBookmarks => {
            bookmark_runtime.library_open = false;
            bookmark_runtime.selected_collection = None;
            trim_bookmark_subscriptions(core, bookmark_runtime);
            clear_bookmarks_snapshot(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::OpenBookmarkCollection {
            pubkey_hex,
            d_tag,
            kind,
        } => {
            bookmark_runtime.selected_collection = Some(BookmarkCollectionKey {
                pubkey_hex,
                d_tag,
                kind,
            });
            set_bookmark_collection_loading(state, true);
            emit(state, reconciler);
            block_on_local(
                runtime,
                "refresh_bookmark_collection_detail",
                refresh_bookmark_collection_detail(core, state, bookmark_runtime, visible_limit),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::RefreshBookmarkCollection => {
            set_bookmark_collection_loading(state, true);
            emit(state, reconciler);
            block_on_local(
                runtime,
                "refresh_bookmark_collection_detail",
                refresh_bookmark_collection_detail(core, state, bookmark_runtime, visible_limit),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::OpenCurationMenu { article_address } => {
            bookmark_runtime.curation_menu_article_address =
                normalize_article_address(&article_address);
            set_curation_menu_loading(
                state,
                bookmark_runtime
                    .curation_menu_article_address
                    .clone()
                    .unwrap_or_default(),
                true,
            );
            emit(state, reconciler);
            block_on_local(
                runtime,
                "refresh_curation_menu",
                refresh_curation_menu(core, state, bookmark_runtime),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::CloseCurationMenu => {
            bookmark_runtime.curation_menu_article_address = None;
            trim_bookmark_subscriptions(core, bookmark_runtime);
            clear_curation_menu_snapshot(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::SetAddressInCurationSet {
            d_tag,
            address,
            member,
        } => {
            let d_tag = d_tag.trim().to_string();
            let Some(address) = normalize_article_address(&address) else {
                set_toast(
                    state,
                    Some(HighlighterToast {
                        kind: HighlighterToastKind::Error,
                        message: "Choose an article to add to a collection".into(),
                    }),
                );
                set_curation_menu_error(state, "Choose an article to add to a collection".into());
                emit(state, reconciler);
                return;
            };
            if d_tag.is_empty() {
                set_toast(
                    state,
                    Some(HighlighterToast {
                        kind: HighlighterToastKind::Error,
                        message: "Choose a collection".into(),
                    }),
                );
                set_curation_menu_error(state, "Choose a collection".into());
                emit(state, reconciler);
            } else {
                set_curation_menu_saving(state, true);
                emit(state, reconciler);
                let core = core.clone();
                ops.submit_op(
                    OpDomain::CurationWrite,
                    OP_DEADLINE_NETWORK,
                    OpOutcome::CurationWrite {
                        result: Err(op_timeout_message(OpDomain::CurationWrite)),
                    },
                    async move {
                        let result = core
                            .set_address_in_curation_set(d_tag, address, member)
                            .await
                            .map(|_| String::new())
                            .map_err(|err| err.to_string());
                        OpOutcome::CurationWrite { result }
                    },
                );
            }
        }
        HighlighterAppAction::CreateCurationSetAndAdd { title, address } => {
            let title = title.trim().to_string();
            let Some(address) = normalize_article_address(&address) else {
                set_toast(
                    state,
                    Some(HighlighterToast {
                        kind: HighlighterToastKind::Error,
                        message: "Choose an article to add to a collection".into(),
                    }),
                );
                set_curation_menu_error(state, "Choose an article to add to a collection".into());
                emit(state, reconciler);
                return;
            };
            if title.is_empty() {
                set_toast(
                    state,
                    Some(HighlighterToast {
                        kind: HighlighterToastKind::Error,
                        message: "Enter a collection name".into(),
                    }),
                );
                set_curation_menu_error(state, "Enter a collection name".into());
                emit(state, reconciler);
            } else {
                set_curation_menu_saving(state, true);
                emit(state, reconciler);
                let core = core.clone();
                ops.submit_op(
                    OpDomain::CurationWrite,
                    OP_DEADLINE_NETWORK,
                    OpOutcome::CurationWrite {
                        result: Err(op_timeout_message(OpDomain::CurationWrite)),
                    },
                    async move {
                        let result = match core.create_curation_set(title).await {
                            Ok(record) => core
                                .set_address_in_curation_set(record.id, address, true)
                                .await
                                .map(|_| "Added to collection".to_string())
                                .map_err(|err| err.to_string()),
                            Err(err) => Err(err.to_string()),
                        };
                        OpOutcome::CurationWrite { result }
                    },
                );
            }
        }
        HighlighterAppAction::OpenRoomExplorer | HighlighterAppAction::RefreshRoomExplorer => {
            room_explorer_runtime.is_open = true;
            set_room_explorer_loading(state, true);
            emit(state, reconciler);
            block_on_local(
                runtime,
                "refresh_room_explorer_home",
                refresh_room_explorer_home(core, state, room_explorer_runtime),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::RefreshRoomBrowseAll => {
            room_explorer_runtime.is_open = true;
            room_explorer_runtime.is_browse_open = true;
            set_room_explorer_browse_loading(state, true);
            emit(state, reconciler);
            block_on_local(
                runtime,
                "refresh_room_explorer_browse",
                refresh_room_explorer_browse(core, state),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::RequestJoinRoom {
            group_id,
            room_name,
        } => {
            // Actor-side validation; network join runs off-actor (Class A).
            let group_id = group_id.trim().to_string();
            if group_id.is_empty() {
                set_room_explorer_join_error(state, "Choose a room to join".into());
                emit(state, reconciler);
            } else {
                let room_name = if room_name.trim().is_empty() {
                    "this room".to_string()
                } else {
                    room_name.trim().to_string()
                };
                {
                    let mut current = state.write();
                    current.room_explorer.joining_group_id = group_id.clone();
                    current.room_explorer.error_message = None;
                    current.bump();
                }
                emit(state, reconciler);
                let core = core.clone();
                let op_group_id = group_id.clone();
                let timeout_group_id = group_id.clone();
                let timeout_room_name = room_name.clone();
                ops.submit_op(
                    OpDomain::JoinRoom,
                    OP_DEADLINE_NETWORK,
                    OpOutcome::JoinRoom {
                        group_id: timeout_group_id,
                        room_name: timeout_room_name,
                        result: Err(op_timeout_message(OpDomain::JoinRoom)),
                    },
                    async move {
                        let result = core
                            .request_join_room(op_group_id.clone())
                            .await
                            .map(|_| ())
                            .map_err(|err| err.to_string());
                        OpOutcome::JoinRoom {
                            group_id: op_group_id,
                            room_name,
                            result,
                        }
                    },
                );
            }
        }
        HighlighterAppAction::RequestIsbnPreview { isbn } => {
            if start_isbn_preview_request(core, state, pending_isbn_lookups, ops, isbn) {
                emit(state, reconciler);
            }
        }
        HighlighterAppAction::RequestWebMetadata { url } => {
            if start_web_metadata_request(core, state, pending_web_metadata, ops, url) {
                emit(state, reconciler);
            }
        }
        HighlighterAppAction::RequestReferenceHighlights {
            tag_name,
            tag_value,
            limit,
        } => {
            block_on_local(
                runtime,
                "load_reference_highlights",
                load_reference_highlights(core, state, tag_name, tag_value, limit, visible_limit),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::RequestBookPickerRecents { limit } => {
            set_book_picker_recents_loading(state, true);
            emit(state, reconciler);
            block_on_local(
                runtime,
                "load_book_picker_recents",
                load_book_picker_recents(core, state, limit, visible_limit),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::SearchBookPickerArtifacts { query, limit } => {
            set_book_picker_searching(state, query.clone(), true);
            emit(state, reconciler);
            block_on_local(
                runtime,
                "search_book_picker_artifacts",
                search_book_picker_artifacts(core, state, query, limit, visible_limit),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::ClearBookPickerSearch => {
            clear_book_picker_search(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::UploadCapturePhoto {
            bytes,
            mime,
            width,
            height,
            alt,
        } => {
            if bytes.is_empty() {
                set_capture_upload_error(state, "That image couldn't be read.".into());
                emit(state, reconciler);
            } else {
                set_capture_uploading(state, true);
                emit(state, reconciler);
                let core = core.clone();
                ops.submit_op(
                    OpDomain::BlossomUpload(UploadSlot::Capture),
                    OP_DEADLINE_NETWORK,
                    OpOutcome::BlossomUpload {
                        slot: UploadSlot::Capture,
                        result: Err(op_timeout_message(OpDomain::BlossomUpload(
                            UploadSlot::Capture,
                        ))),
                    },
                    async move {
                        let result = core
                            .upload_photo(bytes, mime, width, height, alt)
                            .await
                            .map_err(|err| err.to_string());
                        OpOutcome::BlossomUpload {
                            slot: UploadSlot::Capture,
                            result,
                        }
                    },
                );
            }
        }
        HighlighterAppAction::ClearCaptureUpload => {
            clear_capture_upload(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::PublishCaptureHighlight {
            selection,
            target_group_id,
            draft,
        } => {
            set_capture_publishing(state, true);
            emit(state, reconciler);
            let core = core.clone();
            ops.submit_op(
                OpDomain::CapturePublish,
                OP_DEADLINE_NETWORK,
                OpOutcome::CapturePublish {
                    result: Err(op_timeout_message(OpDomain::CapturePublish)),
                },
                async move {
                    let result =
                        capture_highlight_network(&core, selection, target_group_id, draft).await;
                    OpOutcome::CapturePublish { result }
                },
            );
        }
        HighlighterAppAction::PublishCapturePicture {
            selection,
            target_group_id,
            image,
            note,
        } => {
            set_capture_publishing(state, true);
            emit(state, reconciler);
            let core = core.clone();
            ops.submit_op(
                OpDomain::CapturePublish,
                OP_DEADLINE_NETWORK,
                OpOutcome::CapturePublish {
                    result: Err(op_timeout_message(OpDomain::CapturePublish)),
                },
                async move {
                    let result =
                        capture_picture_network(&core, selection, target_group_id, image, note)
                            .await;
                    OpOutcome::CapturePublish { result }
                },
            );
        }
        HighlighterAppAction::PublishClipHighlight {
            artifact,
            target_group_id,
            draft,
        } => {
            set_capture_publishing(state, true);
            emit(state, reconciler);
            let core = core.clone();
            ops.submit_op(
                OpDomain::CapturePublish,
                OP_DEADLINE_NETWORK,
                OpOutcome::CapturePublish {
                    result: Err(op_timeout_message(OpDomain::CapturePublish)),
                },
                async move {
                    let result = capture_highlight_with_optional_share_network(
                        &core,
                        artifact,
                        target_group_id,
                        draft,
                    )
                    .await;
                    OpOutcome::CapturePublish { result }
                },
            );
        }
        HighlighterAppAction::ClearCaptureResult => {
            clear_capture_result(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::ClearCaptureError => {
            clear_capture_error(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::RequestProfile { pubkey_hex } => {
            if request_profile(
                runtime,
                core,
                state,
                profile_handles,
                profile_pubkeys_by_handle,
                pubkey_hex,
                visible_limit,
            ) {
                emit(state, reconciler);
            }
        }
        HighlighterAppAction::OpenProfile { pubkey_hex } => {
            let should_refresh =
                prepare_open_profile_view(core, state, profile_view_runtime, pubkey_hex);
            emit(state, reconciler);
            if should_refresh {
                block_on_local(
                    runtime,
                    "refresh_profile_view",
                    refresh_profile_view(core, state, profile_view_runtime, visible_limit),
                );
                emit(state, reconciler);
            }
        }
        HighlighterAppAction::RefreshProfile => {
            set_profile_view_loading(state, true);
            emit(state, reconciler);
            block_on_local(
                runtime,
                "refresh_profile_view",
                refresh_profile_view(core, state, profile_view_runtime, visible_limit),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::CloseProfile => {
            clear_profile_view_runtime(core, profile_view_runtime);
            clear_profile_view_snapshot(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::ToggleProfileFollow => {
            let prepared = {
                let mut current = state.write();
                if current.profile_view.pubkey_hex.is_empty()
                    || current.profile_view.viewer_pubkey_hex.is_none()
                    || current.profile_view.is_own_profile
                    || current.profile_view.is_mutating_follow
                {
                    None
                } else {
                    let previous = current.profile_view.is_following;
                    current.profile_view.is_following = !previous;
                    current.profile_view.is_mutating_follow = true;
                    current.profile_view.error_message = None;
                    current.bump();
                    Some((current.profile_view.pubkey_hex.clone(), !previous, previous))
                }
            };
            if let Some((target_pubkey_hex, desired_following, previous_following)) = prepared {
                emit(state, reconciler);
                let core = core.clone();
                ops.submit_op(
                    OpDomain::FollowToggle,
                    OP_DEADLINE_NETWORK,
                    // The OpRunner deadline now carries the truthful revert
                    // (real desired/previous), so the redundant inner
                    // tokio::time::timeout — whose only purpose was to dodge the
                    // old gen-0 fallback — is gone; the outer deadline is the
                    // single bound.
                    OpOutcome::FollowToggle {
                        desired_following,
                        previous_following,
                        result: Err(op_timeout_message(OpDomain::FollowToggle)),
                    },
                    async move {
                        let result = core
                            .set_follow(target_pubkey_hex, desired_following)
                            .await
                            .map(|_| ())
                            .map_err(|err| err.to_string());
                        OpOutcome::FollowToggle {
                            desired_following,
                            previous_following,
                            result,
                        }
                    },
                );
            } else {
                emit(state, reconciler);
            }
        }
        HighlighterAppAction::OpenArticleReader {
            pubkey_hex,
            d_tag,
            seed,
        } => {
            let should_refresh = prepare_open_article_reader(
                core,
                state,
                article_reader_runtime,
                pubkey_hex,
                d_tag,
                seed,
            );
            emit(state, reconciler);
            if should_refresh {
                block_on_local(
                    runtime,
                    "refresh_article_reader",
                    refresh_article_reader(core, state, article_reader_runtime, visible_limit),
                );
                emit(state, reconciler);
            }
        }
        HighlighterAppAction::RefreshArticleReader => {
            set_article_reader_loading(state, true);
            emit(state, reconciler);
            block_on_local(
                runtime,
                "refresh_article_reader",
                refresh_article_reader(core, state, article_reader_runtime, visible_limit),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::CloseArticleReader => {
            clear_article_reader_runtime(core, article_reader_runtime);
            clear_article_reader_snapshot(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::PublishArticleHighlight {
            quote,
            context,
            note,
        } => {
            let quote = quote.trim().to_string();
            let article = state.read().article_reader.article.clone();
            let target = article_reader_runtime.target.clone();
            if quote.is_empty() {
                set_article_reader_publish_error(state, "Choose text to highlight".into());
                emit(state, reconciler);
            } else if article.is_none() || target.is_none() {
                set_article_reader_publish_error(state, "Article not yet loaded".into());
                emit(state, reconciler);
            } else {
                let article = article.unwrap();
                let target = target.unwrap();
                let note_was_empty = note.trim().is_empty();
                let draft = HighlightDraft {
                    quote,
                    context: context.trim().to_string(),
                    note: note.trim().to_string(),
                    clip_start_seconds: None,
                    clip_end_seconds: None,
                    clip_speaker: String::new(),
                    clip_transcript_segment_ids: Vec::new(),
                    image: None,
                };
                let artifact = article_as_artifact(&article, &target.address);
                set_article_reader_publishing(state, true);
                emit(state, reconciler);
                let core = core.clone();
                ops.submit_op(
                    OpDomain::HighlightPublish,
                    OP_DEADLINE_NETWORK,
                    OpOutcome::HighlightPublish {
                        note_was_empty,
                        result: Err(op_timeout_message(OpDomain::HighlightPublish)),
                    },
                    async move {
                        let result = core
                            .publish_highlight(draft, artifact)
                            .await
                            .map_err(|err| err.to_string());
                        OpOutcome::HighlightPublish {
                            note_was_empty,
                            result,
                        }
                    },
                );
            }
        }
        HighlighterAppAction::PublishArtifactShare {
            preview,
            group_id,
            note,
        } => {
            set_share_composer_publishing(state, Some(group_id.clone()));
            emit(state, reconciler);
            let note = note
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            let core = core.clone();
            let op_group_id = group_id.clone();
            let timeout_group_id = group_id.clone();
            ops.submit_op(
                OpDomain::SharePublish,
                OP_DEADLINE_NETWORK,
                OpOutcome::SharePublish {
                    group_id: timeout_group_id,
                    result: Err(op_timeout_message(OpDomain::SharePublish)),
                },
                async move {
                    let result = core
                        .publish_artifact(preview, op_group_id, note)
                        .await
                        .map(|_| "Shared to community".to_string())
                        .map_err(|err| err.to_string());
                    OpOutcome::SharePublish { group_id, result }
                },
            );
        }
        HighlighterAppAction::PublishUrlShare {
            url,
            group_id,
            note,
        } => {
            let url = url.trim().to_string();
            if url.is_empty() {
                set_share_composer_error(state, "Missing URL".into());
                emit(state, reconciler);
            } else {
                set_share_composer_publishing(state, Some(group_id.clone()));
                emit(state, reconciler);
                let note = note
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty());
                let core = core.clone();
                let op_group_id = group_id.clone();
                let timeout_group_id = group_id.clone();
                ops.submit_op(
                    OpDomain::SharePublish,
                    OP_DEADLINE_NETWORK,
                    OpOutcome::SharePublish {
                        group_id: timeout_group_id,
                        result: Err(op_timeout_message(OpDomain::SharePublish)),
                    },
                    async move {
                        let result = match core.build_preview_from_url(url).await {
                            Ok(preview) => core
                                .publish_artifact(preview, op_group_id, note)
                                .await
                                .map(|_| "Shared to community".to_string())
                                .map_err(|err| err.to_string()),
                            Err(err) => Err(err.to_string()),
                        };
                        OpOutcome::SharePublish { group_id, result }
                    },
                );
            }
        }
        HighlighterAppAction::ShareHighlightRepost {
            event_id,
            author_pubkey_hex,
            relay_hint,
            target_group_id,
        } => {
            set_share_composer_publishing(state, Some(target_group_id.clone()));
            emit(state, reconciler);
            let core = core.clone();
            let group_id = target_group_id.clone();
            let timeout_group_id = group_id.clone();
            ops.submit_op(
                OpDomain::SharePublish,
                OP_DEADLINE_NETWORK,
                OpOutcome::SharePublish {
                    group_id: timeout_group_id,
                    result: Err(op_timeout_message(OpDomain::SharePublish)),
                },
                async move {
                    let result = core
                        .share_highlight_to_room(
                            event_id,
                            author_pubkey_hex,
                            relay_hint,
                            target_group_id,
                        )
                        .await
                        .map(|_| "Highlight shared".to_string())
                        .map_err(|err| err.to_string());
                    OpOutcome::SharePublish { group_id, result }
                },
            );
        }
        HighlighterAppAction::ClearShareComposerResult => {
            clear_share_composer_result(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::ClearShareComposerError => {
            clear_share_composer_error(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::OpenRoom { group_id } => {
            let should_refresh =
                prepare_open_room_detail(core, state, room_detail_runtime, group_id);
            emit(state, reconciler);
            if should_refresh {
                block_on_local(
                    runtime,
                    "refresh_room_detail",
                    refresh_room_detail(core, state, room_detail_runtime, visible_limit),
                );
                emit(state, reconciler);
            }
        }
        HighlighterAppAction::RefreshRoom => {
            set_room_detail_loading(state, true);
            emit(state, reconciler);
            block_on_local(
                runtime,
                "refresh_room_detail",
                refresh_room_detail(core, state, room_detail_runtime, visible_limit),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::PublishRoomDiscussion {
            title,
            body,
            attachment_url,
        } => {
            let Some(group_id) = room_detail_runtime.group_id.clone() else {
                set_room_discussion_error(state, "Open a room before posting a discussion".into());
                emit(state, reconciler);
                return;
            };
            let title = title.trim().to_string();
            if title.is_empty() {
                set_room_discussion_error(state, "Discussion title required".into());
                emit(state, reconciler);
            } else {
                let attachment_url = attachment_url
                    .map(|url| url.trim().to_string())
                    .filter(|url| !url.is_empty());
                set_room_discussion_publishing(state, true);
                emit(state, reconciler);
                let core = core.clone();
                ops.submit_op(
                    OpDomain::RoomDiscussionPublish,
                    OP_DEADLINE_NETWORK,
                    OpOutcome::RoomDiscussionPublish {
                        result: Err(op_timeout_message(OpDomain::RoomDiscussionPublish)),
                    },
                    async move {
                        let attachment = match attachment_url {
                            Some(url) => match core.build_preview_from_url(url).await {
                                Ok(preview) => Some(preview),
                                Err(_) => {
                                    return OpOutcome::RoomDiscussionPublish {
                                        result: Err("Enter a valid attachment URL.".to_string()),
                                    };
                                }
                            },
                            None => None,
                        };
                        let result = core
                            .publish_discussion(group_id, title, body, attachment)
                            .await
                            .map(|record| record.event_id)
                            .map_err(|err| err.to_string());
                        OpOutcome::RoomDiscussionPublish { result }
                    },
                );
            }
        }
        HighlighterAppAction::ClearRoomDiscussionError => {
            clear_room_discussion_error(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::LoadMoreRoomChat => {
            if prepare_load_more_room_chat(state, room_detail_runtime) {
                emit(state, reconciler);
                block_on_local(
                    runtime,
                    "refresh_room_detail",
                    refresh_room_detail(core, state, room_detail_runtime, visible_limit),
                );
                emit(state, reconciler);
            }
        }
        HighlighterAppAction::PublishRoomChatMessage {
            content,
            reply_to_event_id,
        } => {
            let Some(group_id) = room_detail_runtime.group_id.clone() else {
                set_room_chat_error(state, "Open a room before sending a message".into());
                emit(state, reconciler);
                return;
            };
            let content = content.trim().to_string();
            if content.is_empty() {
                set_room_chat_sending(state, false);
                emit(state, reconciler);
            } else {
                let reply_to_event_id = reply_to_event_id
                    .map(|id| id.trim().to_string())
                    .filter(|id| !id.is_empty());
                set_room_chat_sending(state, true);
                emit(state, reconciler);
                let core = core.clone();
                ops.submit_op(
                    OpDomain::RoomChatPublish,
                    OP_DEADLINE_NETWORK,
                    OpOutcome::RoomChatPublish {
                        result: Err(op_timeout_message(OpDomain::RoomChatPublish)),
                    },
                    async move {
                        let result = core
                            .publish_chat_message(group_id, content, reply_to_event_id)
                            .await
                            .map(|_| ())
                            .map_err(|err| err.to_string());
                        OpOutcome::RoomChatPublish { result }
                    },
                );
            }
        }
        HighlighterAppAction::ClearRoomChatError => {
            clear_room_chat_error(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::CloseRoom => {
            clear_room_detail_runtime(core, room_detail_runtime);
            clear_room_detail_snapshot(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::OpenHomeFeed | HighlighterAppAction::RefreshHomeFeed => {
            home_feed_runtime.is_open = true;
            set_home_feed_loading(state, true);
            emit(state, reconciler);
            block_on_local(
                runtime,
                "refresh_home_feed",
                refresh_home_feed(core, state, home_feed_runtime),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::CloseHomeFeed => {
            home_feed_runtime.is_open = false;
            clear_home_feed_runtime(core, home_feed_runtime);
            mark_home_feed_inactive(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::SearchOpened => {
            search_runtime.is_open = true;
            block_on_local(
                runtime,
                "hydrate_search_relays",
                hydrate_search_relays(core, state),
            );
            let _ = start_search_worker_if_idle(core, state, search_runtime, ops);
            emit(state, reconciler);
        }
        HighlighterAppAction::SearchClosed => {
            search_runtime.is_open = false;
            clear_search_runtime(core, search_runtime);
            mark_search_inactive(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::SetSearchQuery { query } => {
            apply_search_query(core, state, search_runtime, ops, query);
            emit(state, reconciler);
        }
        HighlighterAppAction::SubmitSearch { query } => {
            record_recent_search(state, local_state_path, query.clone());
            apply_search_query(core, state, search_runtime, ops, query);
            emit(state, reconciler);
        }
        HighlighterAppAction::ClearSearch => {
            clear_search_runtime(core, search_runtime);
            clear_search_snapshot(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::RecordRecentSearch { query } => {
            record_recent_search(state, local_state_path, query);
            emit(state, reconciler);
        }
        HighlighterAppAction::ClearRecentSearches => {
            clear_recent_searches(state, local_state_path);
            emit(state, reconciler);
        }
        HighlighterAppAction::OpenNetworkSettings
        | HighlighterAppAction::RefreshNetworkSettings => {
            network_runtime.is_open = true;
            set_network_settings_loading(state, true);
            emit(state, reconciler);
            block_on_local(
                runtime,
                "refresh_network_settings",
                refresh_network_settings(core, state),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::UpsertNetworkRelay { config } => {
            set_network_saving(state, true);
            emit(state, reconciler);
            let core = core.clone();
            ops.submit_op(
                OpDomain::NetworkRelayWrite,
                OP_DEADLINE_NETWORK,
                OpOutcome::NetworkRelayWrite {
                    kind: NetworkRelayWriteKind::Upsert,
                    result: Err(op_timeout_message(OpDomain::NetworkRelayWrite)),
                },
                async move {
                    let result = core
                        .upsert_relay(config)
                        .await
                        .map_err(|err| err.to_string());
                    OpOutcome::NetworkRelayWrite {
                        kind: NetworkRelayWriteKind::Upsert,
                        result,
                    }
                },
            );
        }
        HighlighterAppAction::RemoveNetworkRelay { url } => {
            set_network_saving(state, true);
            emit(state, reconciler);
            let core = core.clone();
            ops.submit_op(
                OpDomain::NetworkRelayWrite,
                OP_DEADLINE_NETWORK,
                OpOutcome::NetworkRelayWrite {
                    kind: NetworkRelayWriteKind::Remove,
                    result: Err(op_timeout_message(OpDomain::NetworkRelayWrite)),
                },
                async move {
                    let result = core.remove_relay(url).await.map_err(|err| err.to_string());
                    OpOutcome::NetworkRelayWrite {
                        kind: NetworkRelayWriteKind::Remove,
                        result,
                    }
                },
            );
        }
        HighlighterAppAction::SetNetworkRelayRoles {
            url,
            read,
            write,
            rooms,
            indexer,
        } => {
            set_network_saving(state, true);
            emit(state, reconciler);
            let core = core.clone();
            ops.submit_op(
                OpDomain::NetworkRelayWrite,
                OP_DEADLINE_NETWORK,
                OpOutcome::NetworkRelayWrite {
                    kind: NetworkRelayWriteKind::SetRoles,
                    result: Err(op_timeout_message(OpDomain::NetworkRelayWrite)),
                },
                async move {
                    let result = core
                        .set_relay_roles(url, read, write, rooms, indexer)
                        .await
                        .map_err(|err| err.to_string());
                    OpOutcome::NetworkRelayWrite {
                        kind: NetworkRelayWriteKind::SetRoles,
                        result,
                    }
                },
            );
        }
        HighlighterAppAction::ProbeNetworkRelayNip11 { url } => {
            let url = url.trim().to_string();
            if url.is_empty() {
                emit(state, reconciler);
            } else {
                set_network_nip11_loading(state, url.clone());
                emit(state, reconciler);
                let core = core.clone();
                let op_url = url.clone();
                let timeout_url = url.clone();
                ops.submit_op(
                    OpDomain::RelayProbe {
                        target: op_target_hash(&url),
                    },
                    OP_DEADLINE_RELAY_PROBE,
                    // A failed probe is recorded as `document: None` for the real
                    // url; the apply arm surfaces the unreachable-relay copy.
                    OpOutcome::RelayProbe {
                        url: timeout_url,
                        document: None,
                    },
                    async move {
                        let document = core.probe_relay_nip11(op_url.clone()).await.ok();
                        OpOutcome::RelayProbe {
                            url: op_url,
                            document,
                        }
                    },
                );
            }
        }
        HighlighterAppAction::SetNetworkImportNpub { npub } => {
            set_network_import_npub(state, npub);
            emit(state, reconciler);
        }
        HighlighterAppAction::FetchNetworkImportRelays => {
            let npub = state.read().network.import_relays.npub.trim().to_string();
            if npub.is_empty() {
                let mut current = state.write();
                current.network.import_relays.is_fetching = false;
                current.network.import_relays.error_message = Some("Enter an npub first.".into());
                current.bump();
                emit(state, reconciler);
            } else {
                set_network_import_fetching(state, true);
                emit(state, reconciler);
                let core = core.clone();
                ops.submit_op(
                    OpDomain::RelayImport,
                    OP_DEADLINE_RELAY_IMPORT,
                    OpOutcome::RelayImport {
                        result: Err(op_timeout_message(OpDomain::RelayImport)),
                    },
                    async move {
                        let result = core
                            .import_relays_from_npub(npub)
                            .await
                            .map_err(|err| err.to_string());
                        OpOutcome::RelayImport { result }
                    },
                );
            }
        }
        HighlighterAppAction::ToggleNetworkImportRelay { url } => {
            toggle_network_import_relay(state, url);
            emit(state, reconciler);
        }
        HighlighterAppAction::ApplyNetworkImportRelays => {
            let rows = {
                let current = state.read();
                let selected = current
                    .network
                    .import_relays
                    .selected_urls
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>();
                current
                    .network
                    .import_relays
                    .candidates
                    .iter()
                    .filter(|row| selected.contains(&row.url))
                    .cloned()
                    .collect::<Vec<_>>()
            };
            if rows.is_empty() {
                let mut current = state.write();
                current.network.import_relays.is_applying = false;
                current.network.import_relays.error_message =
                    Some("Select at least one relay.".into());
                current.bump();
                emit(state, reconciler);
            } else {
                set_network_import_applying(state, true);
                emit(state, reconciler);
                let core = core.clone();
                ops.submit_op(
                    OpDomain::NetworkRelayWrite,
                    OP_DEADLINE_NETWORK,
                    OpOutcome::NetworkRelayWrite {
                        kind: NetworkRelayWriteKind::ImportApply,
                        result: Err(op_timeout_message(OpDomain::NetworkRelayWrite)),
                    },
                    async move {
                        let mut result = Ok(());
                        for row in rows {
                            if let Err(err) = core.upsert_relay(row.clone()).await {
                                result = Err(format!("{}: {err}", row.url));
                                break;
                            }
                        }
                        OpOutcome::NetworkRelayWrite {
                            kind: NetworkRelayWriteKind::ImportApply,
                            result,
                        }
                    },
                );
            }
        }
        HighlighterAppAction::ClearNetworkError => {
            clear_network_error(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::CloseNetworkSettings => {
            network_runtime.is_open = false;
            clear_network_transient_state(state);
            emit(state, reconciler);
        }
        HighlighterAppAction::SetNetworkWifiOnly { enabled } => {
            set_network_wifi_only(state, local_state_path, enabled);
            block_on_local(
                runtime,
                "apply_network_connectivity_policy",
                apply_network_connectivity_policy(core, state),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::NetworkPathChanged { is_wifi } => {
            set_network_path(state, is_wifi);
            block_on_local(
                runtime,
                "apply_network_connectivity_policy",
                apply_network_connectivity_policy(core, state),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::ReconnectNetwork => {
            block_on_local(
                runtime,
                "apply_network_connectivity_policy",
                apply_network_connectivity_policy(core, state),
            );
            emit(state, reconciler);
        }
        HighlighterAppAction::DismissWhatsNew => {
            dismiss_whats_new(state, local_state_path);
            emit(state, reconciler);
        }
        HighlighterAppAction::ToggleOnboardingInterest { interest_id } => {
            toggle_onboarding_interest(state, interest_id);
            emit(state, reconciler);
        }
        HighlighterAppAction::CompleteOnboarding => {
            complete_onboarding(core, state, local_state_path, ops);
            emit(state, reconciler);
        }
        HighlighterAppAction::ClearToast => {
            set_toast(state, None);
            emit(state, reconciler);
        }
    }
}

async fn handle_app_foregrounded(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    pending_joins: &mut Vec<PendingJoin>,
    visible_limit: usize,
) {
    if let Err(err) = core.disconnect_all().await {
        set_toast(
            state,
            Some(HighlighterToast {
                kind: HighlighterToastKind::Error,
                message: err.to_string(),
            }),
        );
        return;
    }

    apply_network_connectivity_policy(core, state).await;

    hydrate_app_chrome(core, state, pending_joins, visible_limit).await;
}

/// Handle one push-delivered core delta on the actor thread.
///
/// Phase 3 (actor-blocking-fix §4.4): this is a **synchronous** fn. The audit
/// in that phase proved every refresh/hydrate it cascades into is local-only
/// (nostrdb reads + synchronous subscription/interest registration +
/// in-memory relay snapshots); no network-bounded await is reachable from the
/// delta path. The leaf `get_*`/`subscribe_*`/`start_*` core methods remain
/// `async fn` because they are part of the FFI-wide `HighlighterCore` surface
/// (de-asyncing them would ripple through hundreds of call sites and the
/// iOS/Android bindings — out of scope). We bridge that single genuine async
/// dependency at the helper boundary with `block_on_local`, identical to how
/// `handle_action` already calls these same helpers. Keeping the function body
/// sync makes the "the delta path never blocks the actor on the network"
/// invariant structural: there is no `.await` here to accidentally hold a
/// `parking_lot` guard across, which is also why the former `await_holding_lock`
/// warning in the `RelayStatusChanged` arm is gone.
fn handle_core_delta(
    delta: Delta,
    runtime: &tokio::runtime::Runtime,
    ctx: &ActorContext,
    runtimes: &mut ActorRuntimes,
) {
    let core = &ctx.core;
    let state = &ctx.state;
    let reconciler = &ctx.reconciler;
    let actor_tx = &ctx.actor_tx;
    let visible_limit = ctx.visible_limit;
    let pending_joins = &mut runtimes.pending_joins;
    let profile_pubkeys_by_handle = &runtimes.profile_pubkeys_by_handle;
    let search_runtime = &mut runtimes.search_runtime;
    let home_feed_runtime = &mut runtimes.home_feed_runtime;
    let bookmark_runtime = &mut runtimes.bookmark_runtime;
    let profile_view_runtime = &mut runtimes.profile_view_runtime;
    let article_reader_runtime = &mut runtimes.article_reader_runtime;
    let room_detail_runtime = &mut runtimes.room_detail_runtime;
    let room_invite_runtime = &mut runtimes.room_invite_runtime;
    let feedback_runtime = &mut runtimes.feedback_runtime;
    let room_explorer_runtime = &mut runtimes.room_explorer_runtime;
    let network_runtime = &mut runtimes.network_runtime;
    let ops = &mut runtimes.ops;

    let subscription_id = delta.subscription_id;
    match delta.change {
        DataChangeType::SignerConnected { user } => {
            set_signed_in_user(state, user);
            emit(state, reconciler);
            if let Err(err) = actor_tx.try_send(KernelMsg::Action(Box::new(
                HighlighterAppAction::RefreshAppChrome,
            ))) {
                log_send_failure(err);
            }
        }
        DataChangeType::CommunityUpserted { .. } | DataChangeType::MembershipChanged { .. } => {
            block_on_local(
                runtime,
                "delta.hydrate_joined_communities",
                hydrate_joined_communities(core, state, pending_joins, visible_limit),
            );
            if home_feed_runtime.is_open {
                block_on_local(
                    runtime,
                    "delta.refresh_home_feed",
                    refresh_home_feed(core, state, home_feed_runtime),
                );
            }
            if room_explorer_runtime.is_open {
                block_on_local(
                    runtime,
                    "delta.refresh_room_explorer_home",
                    refresh_room_explorer_home(core, state, room_explorer_runtime),
                );
            }
            if room_explorer_runtime.is_browse_open {
                block_on_local(
                    runtime,
                    "delta.refresh_room_explorer_browse",
                    refresh_room_explorer_browse(core, state),
                );
            }
            if profile_view_runtime.pubkey_hex.is_some() {
                block_on_local(
                    runtime,
                    "delta.refresh_profile_view",
                    refresh_profile_view(core, state, profile_view_runtime, visible_limit),
                );
            }
            if article_reader_runtime.target.is_some() {
                block_on_local(
                    runtime,
                    "delta.refresh_article_reader",
                    refresh_article_reader(core, state, article_reader_runtime, visible_limit),
                );
            }
            emit(state, reconciler);
        }
        DataChangeType::BookmarksUpdated => {
            block_on_local(
                runtime,
                "delta.hydrate_bookmarks",
                hydrate_bookmarks(core, state, visible_limit),
            );
            if bookmark_runtime.library_open {
                block_on_local(
                    runtime,
                    "delta.refresh_bookmarks_library",
                    refresh_bookmarks_library(core, state, bookmark_runtime, visible_limit),
                );
            }
            if bookmark_runtime.curation_menu_article_address.is_some() {
                block_on_local(
                    runtime,
                    "delta.refresh_curation_menu",
                    refresh_curation_menu(core, state, bookmark_runtime),
                );
            }
            emit(state, reconciler);
        }
        DataChangeType::BookmarkSetsUpdated
        | DataChangeType::FollowingCurationSetsUpdated
        | DataChangeType::WebBookmarksUpdated => {
            if bookmark_runtime.library_open {
                block_on_local(
                    runtime,
                    "delta.refresh_bookmarks_library",
                    refresh_bookmarks_library(core, state, bookmark_runtime, visible_limit),
                );
            }
            if bookmark_runtime.selected_collection.is_some() {
                block_on_local(
                    runtime,
                    "delta.refresh_bookmark_collection_detail",
                    refresh_bookmark_collection_detail(
                        core,
                        state,
                        bookmark_runtime,
                        visible_limit,
                    ),
                );
            }
            if bookmark_runtime.curation_menu_article_address.is_some() {
                block_on_local(
                    runtime,
                    "delta.refresh_curation_menu",
                    refresh_curation_menu(core, state, bookmark_runtime),
                );
            }
            emit(state, reconciler);
        }
        DataChangeType::RelayStatusChanged { state: status, .. } => {
            // Mutate-then-drop the parking_lot guard in its own scope BEFORE any
            // `block_on_local`, so no guard is ever held across the bridge call
            // (former `await_holding_lock` lint, now structurally impossible).
            {
                let mut current = state.write();
                current.chrome.connection_state = map_connection_state(status);
                current.bump();
            }
            if network_runtime.is_open {
                block_on_local(
                    runtime,
                    "delta.refresh_network_settings",
                    refresh_network_settings(core, state),
                );
            }
            emit(state, reconciler);
        }
        DataChangeType::UserProfileUpdated { pubkey, kind } => {
            if profile_view_delta_affects_snapshot(
                profile_view_runtime,
                subscription_id,
                &pubkey,
                kind,
            ) {
                block_on_local(
                    runtime,
                    "delta.refresh_profile_view",
                    refresh_profile_view(core, state, profile_view_runtime, visible_limit),
                );
                emit(state, reconciler);
            }
            if article_reader_profile_delta_affects_snapshot(
                article_reader_runtime,
                subscription_id,
                &pubkey,
                kind,
            ) {
                block_on_local(
                    runtime,
                    "delta.refresh_article_reader",
                    refresh_article_reader(core, state, article_reader_runtime, visible_limit),
                );
                emit(state, reconciler);
            }
            if kind == 3
                && room_invite_runtime.group_id.is_some()
                && state
                    .read()
                    .chrome
                    .current_user
                    .as_ref()
                    .is_some_and(|user| user.pubkey.eq_ignore_ascii_case(&pubkey))
            {
                block_on_local(
                    runtime,
                    "delta.refresh_room_invite_follows",
                    refresh_room_invite_follows(core, state, room_invite_runtime),
                );
                emit(state, reconciler);
            }
            if kind == 0 {
                let pubkey_hex = profile_pubkeys_by_handle
                    .get(&subscription_id)
                    .cloned()
                    .unwrap_or(pubkey);
                if block_on_local(
                    runtime,
                    "delta.hydrate_profile",
                    hydrate_profile(core, state, pubkey_hex, visible_limit),
                ) {
                    recompute_room_invite_visible_follows(state, &room_invite_runtime.follows);
                    emit(state, reconciler);
                }
            }
        }
        DataChangeType::ArticleUpdated { address, .. } => {
            if article_reader_delta_affects_snapshot(
                article_reader_runtime,
                subscription_id,
                &address,
            ) {
                block_on_local(
                    runtime,
                    "delta.refresh_article_reader",
                    refresh_article_reader(core, state, article_reader_runtime, visible_limit),
                );
                emit(state, reconciler);
            }
        }
        DataChangeType::SearchArticlesUpdated { query } => {
            if search_runtime.active_relay_query.as_deref() == Some(query.as_str()) {
                mark_relay_search_settled(state, &query);
                if start_search_worker_if_idle(core, state, search_runtime, ops) {
                    emit(state, reconciler);
                }
            }
        }
        DataChangeType::FollowingReadsUpdated | DataChangeType::FollowingHighlightsUpdated => {
            if home_feed_runtime.is_open {
                block_on_local(
                    runtime,
                    "delta.refresh_home_feed",
                    refresh_home_feed(core, state, home_feed_runtime),
                );
                emit(state, reconciler);
            }
        }
        DataChangeType::FeedbackThreadsUpdated => {
            if feedback_threads_delta_affects_snapshot(feedback_runtime, subscription_id) {
                block_on_local(
                    runtime,
                    "delta.refresh_feedback_threads",
                    refresh_feedback_threads(core, state, feedback_runtime),
                );
                emit(state, reconciler);
            }
        }
        DataChangeType::FeedbackThreadEventUpserted { .. } => {
            if feedback_thread_delta_affects_snapshot(feedback_runtime, subscription_id) {
                block_on_local(
                    runtime,
                    "delta.refresh_feedback_thread",
                    refresh_feedback_thread(core, state, feedback_runtime),
                );
                if feedback_runtime.coordinate.is_some() {
                    block_on_local(
                        runtime,
                        "delta.refresh_feedback_threads",
                        refresh_feedback_threads(core, state, feedback_runtime),
                    );
                }
                emit(state, reconciler);
            }
        }
        DataChangeType::ArtifactUpserted { group_id }
        | DataChangeType::HighlightUpserted { group_id }
        | DataChangeType::HighlightShared { group_id, .. }
        | DataChangeType::DiscussionUpserted { group_id }
        | DataChangeType::ChatMessageUpserted { group_id } => {
            if room_detail_delta_affects_snapshot(room_detail_runtime, subscription_id, &group_id) {
                block_on_local(
                    runtime,
                    "delta.refresh_room_detail",
                    refresh_room_detail(core, state, room_detail_runtime, visible_limit),
                );
                emit(state, reconciler);
            }
        }
        _ => {}
    }
}

fn apply_search_query(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    search_runtime: &mut SearchRuntime,
    ops: &mut OpRunner,
    query: String,
) {
    let query = query.trim().to_string();
    if query.is_empty() {
        clear_search_runtime(core, search_runtime);
        clear_search_snapshot(state);
        return;
    }

    {
        let mut current = state.write();
        if current.search.query != query {
            search_runtime.generation = search_runtime.generation.saturating_add(1);
        }
        current.search.query = query;
        current.search.is_local_loading = search_runtime.is_open;
        current.search.is_relay_loading = false;
        current.bump();
    }

    start_search_worker_if_idle(core, state, search_runtime, ops);
}

fn start_search_worker_if_idle(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    search_runtime: &mut SearchRuntime,
    ops: &mut OpRunner,
) -> bool {
    if !search_runtime.is_open || search_runtime.local_running_query.is_some() {
        return false;
    }

    let query = current_search_query(state);
    if query.is_empty() {
        return false;
    }

    // Single-flight gate (unchanged): `local_running_query` is set here and
    // refuses a second submit until the resolver clears it. Because of that
    // gate, OpRunner's abort-on-resubmit for `OpDomain::SearchLocal` is inert
    // in practice; supersession is still carried by `search_runtime.generation`
    // and re-checked in the apply arm, exactly as the old resolver did.
    search_runtime.local_running_query = Some(query.clone());
    let mut changed = false;
    {
        let mut current = state.write();
        if !current.search.is_local_loading {
            current.search.is_local_loading = true;
            current.bump();
            changed = true;
        }
    }

    let generation = search_runtime.generation;
    let core = core.clone();
    let worker_query = query;
    let timeout_query = worker_query.clone();
    ops.submit_op(
        OpDomain::SearchLocal,
        OP_DEADLINE_NETWORK,
        OpOutcome::SearchLocal {
            generation,
            query: timeout_query,
            result: Err(op_timeout_message(OpDomain::SearchLocal)),
        },
        async move {
            let result = run_local_search(core, worker_query.clone()).await;
            OpOutcome::SearchLocal {
                generation,
                query: worker_query,
                result,
            }
        },
    );
    changed
}

async fn run_local_search(
    core: Arc<HighlighterCore>,
    query: String,
) -> Result<SearchResults, String> {
    let highlights = core
        .search_highlights(query.clone(), SEARCH_HIGHLIGHT_LIMIT)
        .await
        .map_err(|err| err.to_string())?;
    let articles = core
        .search_articles(query.clone(), SEARCH_ARTICLE_LIMIT)
        .await
        .map_err(|err| err.to_string())?;
    let communities = core
        .search_communities(
            query.clone(),
            public_room_candidate_limit(SEARCH_COMMUNITY_LIMIT),
        )
        .await
        .map_err(|err| err.to_string())?
        .into_iter()
        .filter(is_public_open_room)
        .take(SEARCH_COMMUNITY_LIMIT as usize)
        .collect();
    let profiles = core
        .search_profiles(query, SEARCH_PROFILE_LIMIT)
        .await
        .map_err(|err| err.to_string())?;

    Ok(SearchResults {
        highlights,
        articles,
        communities,
        profiles,
    })
}

fn handle_search_local_resolved(
    ctx: &ActorContext,
    runtimes: &mut ActorRuntimes,
    resolution: SearchLocalResolution,
) {
    let core = &ctx.core;
    let state = &ctx.state;
    let reconciler = &ctx.reconciler;
    let visible_limit = ctx.visible_limit;
    let search_runtime = &mut runtimes.search_runtime;
    let ops = &mut runtimes.ops;
    let SearchLocalResolution {
        generation,
        query,
        result,
    } = resolution;

    if search_runtime.local_running_query.as_deref() == Some(query.as_str()) {
        search_runtime.local_running_query = None;
    }

    if generation != search_runtime.generation {
        start_search_worker_if_idle(core, state, search_runtime, ops);
        emit(state, reconciler);
        return;
    }

    let current_query = current_search_query(state);
    if current_query != query || !search_runtime.is_open {
        start_search_worker_if_idle(core, state, search_runtime, ops);
        emit(state, reconciler);
        return;
    }

    match result {
        Ok(results) => apply_search_results(state, query.clone(), results, visible_limit),
        Err(message) => {
            set_toast(
                state,
                Some(HighlighterToast {
                    kind: HighlighterToastKind::Error,
                    message,
                }),
            );
            mark_search_local_idle(state);
        }
    }

    ensure_search_relay_subscription(core, state, search_runtime, &query);
    start_search_worker_if_idle(core, state, search_runtime, ops);
    emit(state, reconciler);
}

fn ensure_search_relay_subscription(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    search_runtime: &mut SearchRuntime,
    query: &str,
) {
    if !search_runtime.is_open || query.trim().is_empty() {
        return;
    }
    if search_runtime.active_relay_query.as_deref() == Some(query)
        && search_runtime.relay_handle.is_some()
    {
        return;
    }

    if let Some(handle) = search_runtime.relay_handle.take() {
        core.unsubscribe(handle);
    }
    search_runtime.active_relay_query = None;

    {
        let mut current = state.write();
        current.search.is_relay_loading = true;
        current.bump();
    }

    match block_on_search_subscription(core, query.to_string()) {
        Ok(handle) => {
            search_runtime.active_relay_query = Some(query.to_string());
            search_runtime.relay_handle = Some(handle);
            mark_relay_search_settled(state, query);
        }
        Err(message) => {
            set_toast(
                state,
                Some(HighlighterToast {
                    kind: HighlighterToastKind::Error,
                    message,
                }),
            );
            mark_relay_search_settled(state, query);
        }
    }
}

fn block_on_search_subscription(core: &Arc<HighlighterCore>, query: String) -> Result<u64, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .thread_name("highlighter-search-subscribe")
        .build()
        .map_err(|err| err.to_string())?;
    runtime
        .block_on(core.subscribe_article_search(query)) // lint-allow: block_on (worker runtime)
        .map_err(|err| err.to_string())
}

async fn hydrate_search_relays(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
) {
    match core.get_search_relays().await {
        Ok(relays) => {
            let mut current = state.write();
            current.search.search_relays = relays;
            current.bump();
        }
        Err(err) => set_toast(
            state,
            Some(HighlighterToast {
                kind: HighlighterToastKind::Error,
                message: err.to_string(),
            }),
        ),
    }
}

fn apply_search_results(
    state: &Arc<RwLock<HighlighterAppState>>,
    query: String,
    results: SearchResults,
    visible_limit: usize,
) {
    let mut current = state.write();
    let max_highlights = visible_limit.max(SEARCH_HIGHLIGHT_LIMIT as usize);
    let max_articles = visible_limit.max(SEARCH_ARTICLE_LIMIT as usize);
    let max_communities = visible_limit.max(SEARCH_COMMUNITY_LIMIT as usize);
    let max_profiles = visible_limit.max(SEARCH_PROFILE_LIMIT as usize);
    let highlight_count = results.highlights.len() as u64;
    let article_count = results.articles.len() as u64;
    let community_count = results.communities.len() as u64;
    let profile_count = results.profiles.len() as u64;

    current.search.applied_query = query;
    current.search.highlights = results
        .highlights
        .into_iter()
        .take(max_highlights)
        .collect();
    current.search.highlight_count = highlight_count;
    current.search.articles = results.articles.into_iter().take(max_articles).collect();
    current.search.article_count = article_count;
    current.search.communities = results
        .communities
        .into_iter()
        .take(max_communities)
        .collect();
    current.search.community_count = community_count;
    current.search.profiles = results.profiles.into_iter().take(max_profiles).collect();
    current.search.profile_count = profile_count;
    current.search.is_local_loading = false;
    current.bump();
}

fn current_search_query(state: &Arc<RwLock<HighlighterAppState>>) -> String {
    state.read().search.query.trim().to_string()
}

fn clear_search_runtime(core: &Arc<HighlighterCore>, search_runtime: &mut SearchRuntime) {
    search_runtime.generation = search_runtime.generation.saturating_add(1);
    if let Some(handle) = search_runtime.relay_handle.take() {
        core.unsubscribe(handle);
    }
    search_runtime.active_relay_query = None;
}

fn clear_search_snapshot(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    let relays = current.search.search_relays.clone();
    let recent_queries = current.search.recent_queries.clone();
    current.search = HighlighterSearchSnapshot::empty();
    current.search.search_relays = relays;
    apply_recent_searches_to_snapshot(&mut current.search, recent_queries);
    current.bump();
}

fn record_recent_search(
    state: &Arc<RwLock<HighlighterAppState>>,
    local_state_path: &Path,
    query: String,
) {
    let mut current = state.write();
    let mut recent = current.search.recent_queries.clone();
    recent.insert(0, query);
    apply_recent_searches_to_snapshot(&mut current.search, recent);
    let local = local_state_from_current(&current);
    current.bump();
    drop(current);
    save_local_state(local_state_path, &local);
}

fn clear_recent_searches(state: &Arc<RwLock<HighlighterAppState>>, local_state_path: &Path) {
    let mut current = state.write();
    current.search.recent_queries.clear();
    current.search.recent_query_count = 0;
    let local = local_state_from_current(&current);
    current.bump();
    drop(current);
    save_local_state(local_state_path, &local);
}

fn set_network_wifi_only(
    state: &Arc<RwLock<HighlighterAppState>>,
    local_state_path: &Path,
    enabled: bool,
) {
    let mut current = state.write();
    if current.network.wifi_only_enabled == enabled {
        return;
    }
    current.network.wifi_only_enabled = enabled;
    let local = local_state_from_current(&current);
    current.bump();
    drop(current);
    save_local_state(local_state_path, &local);
}

fn set_network_path(state: &Arc<RwLock<HighlighterAppState>>, is_wifi: bool) {
    let mut current = state.write();
    if current.network.current_path_is_wifi == Some(is_wifi) {
        return;
    }
    current.network.current_path_is_wifi = Some(is_wifi);
    current.bump();
}

async fn apply_network_connectivity_policy(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
) {
    let network = state.read().network.clone();
    let result = if network.wifi_only_enabled {
        match network.current_path_is_wifi {
            Some(true) => core.reconnect_all().await,
            Some(false) => core.disconnect_all().await,
            None => Ok(()),
        }
    } else {
        core.reconnect_all().await
    };

    if let Err(err) = result {
        set_toast(
            state,
            Some(HighlighterToast {
                kind: HighlighterToastKind::Error,
                message: err.to_string(),
            }),
        );
    }
}

fn set_network_settings_loading(state: &Arc<RwLock<HighlighterAppState>>, is_loading: bool) {
    let mut current = state.write();
    current.network.is_loading = is_loading;
    if is_loading {
        current.network.error_message = None;
    }
    current.bump();
}

fn set_network_saving(state: &Arc<RwLock<HighlighterAppState>>, is_saving: bool) {
    let mut current = state.write();
    current.network.is_saving = is_saving;
    if is_saving {
        current.network.action_error_message = None;
    }
    current.bump();
}

async fn refresh_network_settings(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
) {
    let relays_result = core.get_relays().await;
    let diagnostics_result = core.get_relay_diagnostics().await;
    let auto_result = core.get_auto_connected_relays().await;
    let cache_stats = core.get_cache_stats().await.ok();

    let mut error_message = None;
    let relays = match relays_result {
        Ok(rows) => rows,
        Err(err) => {
            error_message = Some(format!("Couldn't load relays: {err}"));
            Vec::new()
        }
    };
    let diagnostics = match diagnostics_result {
        Ok(rows) => rows,
        Err(err) => {
            if error_message.is_none() {
                error_message = Some(format!("Couldn't load diagnostics: {err}"));
            }
            Vec::new()
        }
    };
    let auto_connected_relays = match auto_result {
        Ok(rows) => rows,
        Err(err) => {
            if error_message.is_none() {
                error_message = Some(format!("Couldn't load auto-connected relays: {err}"));
            }
            Vec::new()
        }
    };

    let connected_count = diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.state, RelayStatus::Connected))
        .count() as u64;
    let visible_relay_count = relays.len().saturating_add(auto_connected_relays.len()) as u64;
    let has_outbox = relays.iter().any(|relay| relay.write);

    // NIP-11 documents NMP fetched for pool relays (ADR-0051). Seeded into
    // the nip11 projection so rows resolve without any per-row probe; the
    // change-guard keeps the projection order stable across refreshes.
    let info_documents = core.relay_info_documents();

    let mut current = state.write();
    current.network.relay_count = relays.len() as u64;
    current.network.relays = relays;
    current.network.auto_connected_relay_count = auto_connected_relays.len() as u64;
    current.network.auto_connected_relays = auto_connected_relays;
    current.network.diagnostic_count = diagnostics.len() as u64;
    current.network.diagnostics = diagnostics;
    for doc in info_documents {
        let unchanged = current.network.nip11.iter().any(|row| {
            row.url == doc.url && !row.is_loading && row.document.as_ref() == Some(&doc)
        });
        if unchanged {
            continue;
        }
        upsert_network_nip11_projection(
            &mut current.network,
            HighlighterRelayNip11Snapshot {
                url: doc.url.clone(),
                document: Some(doc),
                is_loading: false,
                error_message: None,
            },
        );
    }
    current.network.cache_stats = cache_stats;
    current.network.connected_count = connected_count;
    current.network.visible_relay_count = visible_relay_count;
    current.network.has_outbox = has_outbox;
    current.network.is_loading = false;
    current.network.is_saving = false;
    current.network.error_message = error_message;
    current.bump();
}

fn set_network_nip11_loading(state: &Arc<RwLock<HighlighterAppState>>, url: String) {
    let url = url.trim().to_string();
    if url.is_empty() {
        return;
    }
    let mut current = state.write();
    upsert_network_nip11_projection(
        &mut current.network,
        HighlighterRelayNip11Snapshot {
            url,
            document: None,
            is_loading: true,
            error_message: None,
        },
    );
    current.bump();
}

fn upsert_network_nip11_projection(
    snapshot: &mut HighlighterNetworkSnapshot,
    entry: HighlighterRelayNip11Snapshot,
) {
    snapshot.nip11.retain(|existing| existing.url != entry.url);
    snapshot.nip11.insert(0, entry);
    if snapshot.nip11.len() > NETWORK_NIP11_LIMIT {
        snapshot.nip11.truncate(NETWORK_NIP11_LIMIT);
    }
}

fn set_network_import_npub(state: &Arc<RwLock<HighlighterAppState>>, npub: String) {
    let mut current = state.write();
    current.network.import_relays.npub = npub;
    current.network.import_relays.error_message = None;
    current.bump();
}

fn set_network_import_fetching(state: &Arc<RwLock<HighlighterAppState>>, is_fetching: bool) {
    let mut current = state.write();
    current.network.import_relays.is_fetching = is_fetching;
    if is_fetching {
        current.network.import_relays.error_message = None;
        current.network.import_relays.candidates.clear();
        current.network.import_relays.candidate_count = 0;
        current.network.import_relays.selected_urls.clear();
    }
    current.bump();
}

fn toggle_network_import_relay(state: &Arc<RwLock<HighlighterAppState>>, url: String) {
    let url = url.trim().to_string();
    if url.is_empty() {
        return;
    }
    let mut current = state.write();
    if current.network.import_relays.selected_urls.contains(&url) {
        current
            .network
            .import_relays
            .selected_urls
            .retain(|selected| selected != &url);
    } else if current
        .network
        .import_relays
        .candidates
        .iter()
        .any(|candidate| candidate.url == url)
    {
        current.network.import_relays.selected_urls.push(url);
    }
    current.network.import_relays.error_message = None;
    current.bump();
}

fn set_network_import_applying(state: &Arc<RwLock<HighlighterAppState>>, is_applying: bool) {
    let mut current = state.write();
    current.network.import_relays.is_applying = is_applying;
    if is_applying {
        current.network.import_relays.error_message = None;
    }
    current.bump();
}

fn set_network_action_error(state: &Arc<RwLock<HighlighterAppState>>, message: String) {
    let mut current = state.write();
    current.network.is_saving = false;
    current.network.action_error_message = Some(message);
    current.bump();
}

fn clear_network_action_error(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.network.action_error_message = None;
    // Clear the busy flag here too: the follow-up refresh_network_settings
    // also resets it, but the success path must not depend on that refresh
    // staying unconditional.
    current.network.is_saving = false;
    current.bump();
}

fn clear_network_error(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.network.error_message = None;
    current.network.action_error_message = None;
    current.network.import_relays.error_message = None;
    current.bump();
}

fn clear_network_transient_state(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.network.is_loading = false;
    current.network.is_saving = false;
    current.network.error_message = None;
    current.network.action_error_message = None;
    current.network.import_relays = HighlighterNetworkImportSnapshot::empty();
    current.bump();
}

fn apply_recent_searches_to_snapshot(
    snapshot: &mut HighlighterSearchSnapshot,
    recent_queries: Vec<String>,
) {
    snapshot.recent_queries = normalize_recent_searches(recent_queries);
    snapshot.recent_query_count = snapshot.recent_queries.len() as u64;
}

fn normalize_recent_searches(recent_queries: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for query in recent_queries {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed.to_ascii_lowercase();
        if seen.insert(key) {
            out.push(trimmed.to_string());
        }
        if out.len() >= RECENT_SEARCH_LIMIT {
            break;
        }
    }
    out
}

fn mark_search_inactive(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.search.is_local_loading = false;
    current.search.is_relay_loading = false;
    current.bump();
}

fn mark_search_local_idle(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.search.is_local_loading = false;
    current.bump();
}

fn mark_relay_search_settled(state: &Arc<RwLock<HighlighterAppState>>, query: &str) {
    let mut current = state.write();
    if current.search.query == query {
        current.search.is_relay_loading = false;
        current.bump();
    }
}

fn set_home_feed_loading(state: &Arc<RwLock<HighlighterAppState>>, is_loading: bool) {
    let mut current = state.write();
    current.home_feed.is_loading = is_loading;
    if is_loading {
        current.home_feed.error_message = None;
    }
    current.bump();
}

fn mark_home_feed_inactive(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.home_feed.is_loading = false;
    current.bump();
}

async fn refresh_home_feed(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &mut HomeFeedRuntime,
) {
    if core.current_user().is_none() {
        let mut current = state.write();
        current.home_feed = HighlighterHomeFeedSnapshot::empty();
        current.bump();
        return;
    }

    let mut first_error = None;
    ensure_home_feed_subscriptions(core, state, runtime, &mut first_error).await;

    let highlights = core
        .get_following_highlights(HOME_FEED_HIGHLIGHT_QUERY_LIMIT)
        .await
        .map_err(|err| format!("Couldn't load highlights: {err}"));
    let reads = core
        .get_following_reads(HOME_FEED_READ_QUERY_LIMIT)
        .await
        .map_err(|err| format!("Couldn't load reads: {err}"));

    match (highlights, reads) {
        (Ok(highlights), Ok(reads)) => {
            let items = build_home_feed_items(highlights, reads);
            let mut current = state.write();
            current.home_feed.item_count = items.len() as u64;
            current.home_feed.items = items.into_iter().take(HOME_FEED_ITEM_LIMIT).collect();
            current.home_feed.is_loading = false;
            current.home_feed.error_message = first_error;
            current.bump();
        }
        (highlights, reads) => {
            if let Err(message) = highlights {
                record_first_error(&mut first_error, message);
            }
            if let Err(message) = reads {
                record_first_error(&mut first_error, message);
            }
            let mut current = state.write();
            current.home_feed.is_loading = false;
            current.home_feed.error_message = first_error;
            current.bump();
        }
    }
}

async fn ensure_home_feed_subscriptions(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &mut HomeFeedRuntime,
    first_error: &mut Option<String>,
) {
    if core.current_user().is_none() {
        return;
    }
    if runtime.reads_handle.is_none() {
        match core.subscribe_following_reads().await {
            Ok(handle) => runtime.reads_handle = Some(handle),
            Err(err) => {
                let message = format!("Couldn't subscribe to reads: {err}");
                record_first_error(first_error, message.clone());
                set_home_feed_error(state, message);
            }
        }
    }
    if runtime.highlights_handle.is_none() {
        match core.subscribe_following_highlights().await {
            Ok(handle) => runtime.highlights_handle = Some(handle),
            Err(err) => {
                let message = format!("Couldn't subscribe to highlights: {err}");
                record_first_error(first_error, message.clone());
                set_home_feed_error(state, message);
            }
        }
    }
}

fn set_home_feed_error(state: &Arc<RwLock<HighlighterAppState>>, message: String) {
    let mut current = state.write();
    current.home_feed.error_message = Some(message);
    current.bump();
}

fn clear_home_feed_runtime(core: &Arc<HighlighterCore>, runtime: &mut HomeFeedRuntime) {
    if let Some(handle) = runtime.reads_handle.take() {
        core.unsubscribe(handle);
    }
    if let Some(handle) = runtime.highlights_handle.take() {
        core.unsubscribe(handle);
    }
    *runtime = HomeFeedRuntime::default();
}

fn build_home_feed_items(
    highlights: Vec<HydratedHighlight>,
    reads: Vec<ReadingFeedItem>,
) -> Vec<HighlighterHomeFeedItem> {
    let highlighted_addresses: BTreeSet<String> = highlights
        .iter()
        .filter_map(|highlight| {
            let address = highlight.highlight.artifact_address.trim();
            (!address.is_empty()).then(|| address.to_string())
        })
        .collect();

    let mut groups: BTreeMap<String, Vec<HydratedHighlight>> = BTreeMap::new();
    let mut group_order = Vec::<String>::new();
    for highlight in highlights {
        let key = home_feed_group_key(&highlight)
            .unwrap_or_else(|| format!("solo:{}", highlight.highlight.event_id));
        if !groups.contains_key(&key) {
            group_order.push(key.clone());
        }
        groups.entry(key).or_default().push(highlight);
    }

    let mut items = Vec::with_capacity(group_order.len().saturating_add(reads.len()));

    for key in group_order {
        let Some(mut group) = groups.remove(&key) else {
            continue;
        };
        group.sort_by_key(|highlight| highlight.highlight.created_at.unwrap_or(0));
        let sort_key = group
            .iter()
            .filter_map(|highlight| highlight.highlight.created_at)
            .max()
            .unwrap_or(0);
        let highlight_count = group.len() as u64;
        items.push(HighlighterHomeFeedItem {
            kind: HighlighterHomeFeedItemKind::Highlights,
            stable_id: home_feed_highlight_stable_id(&group[0]),
            sort_key,
            highlights: group
                .into_iter()
                .take(HOME_FEED_GROUP_HIGHLIGHT_LIMIT)
                .collect(),
            highlight_count,
            read: None,
        });
    }

    for read in reads {
        let address = format!("30023:{}:{}", read.article.pubkey, read.article.identifier);
        if highlighted_addresses.contains(&address) {
            continue;
        }
        items.push(HighlighterHomeFeedItem {
            kind: HighlighterHomeFeedItemKind::Read,
            stable_id: format!("r:{address}"),
            sort_key: read.latest_activity_at,
            highlights: Vec::new(),
            highlight_count: 0,
            read: Some(home_feed_read_item(read)),
        });
    }

    items.sort_by_key(|item| std::cmp::Reverse(home_feed_sort_key(item)));
    items
}

fn home_feed_group_key(highlight: &HydratedHighlight) -> Option<String> {
    let key = highlight.highlight.source_reference_key.trim();
    (!key.is_empty()).then(|| key.to_string())
}

fn home_feed_highlight_stable_id(highlight: &HydratedHighlight) -> String {
    let address = highlight.highlight.artifact_address.trim();
    if !address.is_empty() {
        return format!("h:src:{address}");
    }
    let source_url = highlight.highlight.source_url.trim();
    if !source_url.is_empty() {
        return format!("h:src:{source_url}");
    }
    format!("h:evt:{}", highlight.highlight.event_id)
}

fn home_feed_sort_key(item: &HighlighterHomeFeedItem) -> u64 {
    item.sort_key
}

fn home_feed_read_item(read: ReadingFeedItem) -> HighlighterHomeReadItem {
    let word_count = read.article.content.split_whitespace().count();
    let read_time_minutes = (word_count > 60).then(|| std::cmp::max(1, word_count / 240) as u32);
    HighlighterHomeReadItem {
        pubkey: read.article.pubkey,
        identifier: read.article.identifier,
        title: read.article.title,
        summary: read.article.summary,
        image: read.article.image,
        first_hashtag: read
            .article
            .hashtags
            .into_iter()
            .find(|tag| !tag.trim().is_empty()),
        published_at: read.article.published_at,
        created_at: read.article.created_at,
        author_followed: read.author_followed,
        interactor_pubkeys: read.interactor_pubkeys,
        read_time_minutes,
    }
}

fn public_room_candidate_limit(limit: u32) -> u32 {
    let expanded = limit.saturating_mul(4);
    expanded.clamp(limit, 512)
}

fn is_public_open_room(community: &CommunitySummary) -> bool {
    community.visibility == "public" && community.access == "open"
}

fn set_room_explorer_loading(state: &Arc<RwLock<HighlighterAppState>>, is_loading: bool) {
    let mut current = state.write();
    current.room_explorer.is_loading = is_loading;
    if is_loading {
        current.room_explorer.error_message = None;
    }
    current.bump();
}

fn set_room_explorer_browse_loading(state: &Arc<RwLock<HighlighterAppState>>, is_loading: bool) {
    let mut current = state.write();
    current.room_explorer.is_browse_loading = is_loading;
    if is_loading {
        current.room_explorer.error_message = None;
    }
    current.bump();
}

async fn refresh_room_explorer_home(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    _runtime: &mut RoomExplorerRuntime,
) {
    let mut first_error = None;
    core.start_room_discovery().await;
    if let Err(err) = core.start_friends_rooms_discovery().await {
        record_first_error(
            &mut first_error,
            format!("Couldn't refresh social room recommendations: {err}"),
        );
    }
    if let Err(err) = core
        .start_featured_rooms(ROOM_EXPLORER_CURATOR_PUBKEY_HEX.to_string())
        .await
    {
        record_first_error(
            &mut first_error,
            format!("Couldn't refresh featured rooms: {err}"),
        );
    }

    let featured = core
        .get_featured_rooms(ROOM_EXPLORER_CURATOR_PUBKEY_HEX.to_string())
        .await
        .map(|rooms| bounded_public_rooms(rooms, ROOM_EXPLORER_FEATURED_LIMIT as usize))
        .map_err(|err| format!("Couldn't load featured rooms: {err}"));
    let new_noteworthy = core
        .get_new_rooms(public_room_candidate_limit(ROOM_EXPLORER_NEW_LIMIT))
        .await
        .map(|rooms| {
            let joined_ids = joined_room_ids(state);
            bounded_public_rooms_excluding(rooms, ROOM_EXPLORER_NEW_LIMIT as usize, &joined_ids)
        })
        .map_err(|err| format!("Couldn't load new rooms: {err}"));
    let friends = core
        .get_rooms_with_friends(public_room_candidate_limit(
            ROOM_EXPLORER_RECOMMENDATION_LIMIT,
        ))
        .await
        .map(|rooms| bounded_public_recommendations(rooms, ROOM_EXPLORER_RECOMMENDATION_LIMIT))
        .map_err(|err| format!("Couldn't load friend room recommendations: {err}"));
    let authors = core
        .get_rooms_from_read_authors(public_room_candidate_limit(
            ROOM_EXPLORER_RECOMMENDATION_LIMIT,
        ))
        .await
        .map(|rooms| bounded_public_recommendations(rooms, ROOM_EXPLORER_RECOMMENDATION_LIMIT))
        .map_err(|err| format!("Couldn't load author room recommendations: {err}"));

    let mut current = state.write();
    current.room_explorer.curator_pubkey_hex = ROOM_EXPLORER_CURATOR_PUBKEY_HEX.into();
    match featured {
        Ok(rooms) => {
            current.room_explorer.featured_count = rooms.len() as u64;
            current.room_explorer.featured = rooms;
        }
        Err(message) => record_first_error(&mut first_error, message),
    }
    match new_noteworthy {
        Ok(rooms) => {
            current.room_explorer.new_noteworthy_count = rooms.len() as u64;
            current.room_explorer.new_noteworthy = rooms;
        }
        Err(message) => record_first_error(&mut first_error, message),
    }
    match friends {
        Ok(rooms) => {
            current.room_explorer.friends_shelf_count = rooms.len() as u64;
            current.room_explorer.friends_shelf = rooms;
        }
        Err(message) => record_first_error(&mut first_error, message),
    }
    match authors {
        Ok(rooms) => {
            current.room_explorer.authors_shelf_count = rooms.len() as u64;
            current.room_explorer.authors_shelf = rooms;
        }
        Err(message) => record_first_error(&mut first_error, message),
    }
    current.room_explorer.is_loading = false;
    current.room_explorer.error_message = first_error;
    current.bump();
}

async fn refresh_room_explorer_browse(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
) {
    let rooms = core
        .get_all_rooms(public_room_candidate_limit(ROOM_EXPLORER_BROWSE_LIMIT))
        .await
        .map(|rooms| bounded_public_rooms(rooms, ROOM_EXPLORER_BROWSE_LIMIT as usize))
        .map_err(|err| format!("Couldn't load rooms: {err}"));

    let mut current = state.write();
    match rooms {
        Ok(rooms) => {
            current.room_explorer.all_room_count = rooms.len() as u64;
            current.room_explorer.all_rooms = rooms;
            current.room_explorer.error_message = None;
        }
        Err(message) => {
            current.room_explorer.error_message = Some(message);
        }
    }
    current.room_explorer.is_browse_loading = false;
    current.bump();
}

/// Clear the explorer's joining marker, but only if it still refers to
/// `group_id` — a newer join request may have replaced it (supersession).
fn clear_joining_marker(explorer: &mut HighlighterRoomExplorerSnapshot, group_id: &str) {
    if explorer.joining_group_id == group_id {
        explorer.joining_group_id.clear();
    }
}

fn set_room_explorer_join_error(state: &Arc<RwLock<HighlighterAppState>>, message: String) {
    let mut current = state.write();
    current.room_explorer.error_message = Some(message.clone());
    current.toast = Some(HighlighterToast {
        kind: HighlighterToastKind::Error,
        message,
    });
    current.bump();
}

fn record_first_error(first_error: &mut Option<String>, message: String) {
    if first_error.is_none() {
        *first_error = Some(message);
    }
}

fn bounded_public_rooms(rooms: Vec<CommunitySummary>, limit: usize) -> Vec<CommunitySummary> {
    rooms
        .into_iter()
        .filter(is_public_open_room)
        .take(limit)
        .collect()
}

fn bounded_public_rooms_excluding(
    rooms: Vec<CommunitySummary>,
    limit: usize,
    excluded_ids: &BTreeSet<String>,
) -> Vec<CommunitySummary> {
    rooms
        .into_iter()
        .filter(is_public_open_room)
        .filter(|room| !excluded_ids.contains(&room.id))
        .take(limit)
        .collect()
}

fn bounded_public_recommendations(
    recommendations: Vec<RoomRecommendation>,
    limit: u32,
) -> Vec<RoomRecommendation> {
    recommendations
        .into_iter()
        .filter(|recommendation| is_public_open_room(&recommendation.summary))
        .take(limit as usize)
        .collect()
}

fn joined_room_ids(state: &Arc<RwLock<HighlighterAppState>>) -> BTreeSet<String> {
    state
        .read()
        .chrome
        .joined_communities
        .iter()
        .map(|community| community.id.clone())
        .collect()
}

async fn hydrate_app_chrome(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    pending_joins: &mut Vec<PendingJoin>,
    visible_limit: usize,
) {
    let user = core.current_user();
    let profile = match &user {
        Some(user) => core
            .get_user_profile(user.pubkey.clone())
            .await
            .ok()
            .flatten(),
        None => None,
    };

    {
        let mut current = state.write();
        current.chrome.current_user = user;
        current.chrome.current_user_profile = profile;
        current.bump();
    }

    hydrate_joined_communities(core, state, pending_joins, visible_limit).await;
    hydrate_bookmarks(core, state, visible_limit).await;
}

async fn ensure_signed_in_app_scope(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    subscriptions: &mut AppScopeSubscriptions,
    ops: &mut OpRunner,
) {
    let Some(user) = core.current_user() else {
        return;
    };
    let user_pubkey = user.pubkey;

    if subscriptions.joined_communities.is_none() {
        match core.subscribe_joined_communities().await {
            Ok(handle) => subscriptions.joined_communities = Some(handle),
            Err(err) => set_toast(
                state,
                Some(HighlighterToast {
                    kind: HighlighterToastKind::Error,
                    message: err.to_string(),
                }),
            ),
        }
    }

    if subscriptions.bookmarks.is_none() {
        match core.subscribe_bookmarks().await {
            Ok(handle) => subscriptions.bookmarks = Some(handle),
            Err(err) => set_toast(
                state,
                Some(HighlighterToast {
                    kind: HighlighterToastKind::Error,
                    message: err.to_string(),
                }),
            ),
        }
    }

    let already_initialized = subscriptions
        .initialized_blossom_defaults_for_pubkey
        .as_deref()
        == Some(user_pubkey.as_str());
    let already_initializing = subscriptions
        .initializing_blossom_defaults_for_pubkey
        .as_deref()
        == Some(user_pubkey.as_str());
    if !already_initialized && !already_initializing {
        subscriptions.initializing_blossom_defaults_for_pubkey = Some(user_pubkey.clone());
        submit_default_blossom_init_op(ops, core, user_pubkey);
    }
}

/// Submit default-blossom-server init off-actor (Phase 4 fold of the former
/// `highlighter-blossom-init` worker). The target pubkey rides in the outcome;
/// the worker future still no-ops if the signed-in user changed mid-flight,
/// and `handle_default_blossom_init_resolved` performs the same
/// `initializing_blossom_defaults_for_pubkey` / still-this-user check as before.
fn submit_default_blossom_init_op(
    ops: &mut OpRunner,
    core: &Arc<HighlighterCore>,
    pubkey_hex: String,
) {
    let core = core.clone();
    let timeout_pubkey_hex = pubkey_hex.clone();
    ops.submit_op(
        OpDomain::DefaultBlossomInit,
        OP_DEADLINE_NETWORK,
        OpOutcome::DefaultBlossomInit {
            pubkey_hex: timeout_pubkey_hex,
            result: Err(op_timeout_message(OpDomain::DefaultBlossomInit)),
        },
        async move {
            let result = if core
                .current_user()
                .is_none_or(|user| user.pubkey != pubkey_hex)
            {
                Ok(())
            } else {
                core.init_default_blossom_servers()
                    .await
                    .map_err(|err| err.to_string())
            };
            OpOutcome::DefaultBlossomInit { pubkey_hex, result }
        },
    );
}

fn handle_default_blossom_init_resolved(
    state: &Arc<RwLock<HighlighterAppState>>,
    reconciler: &Arc<RwLock<Option<Arc<dyn HighlighterAppReconciler>>>>,
    subscriptions: &mut AppScopeSubscriptions,
    pubkey_hex: String,
    result: Result<(), String>,
) {
    if subscriptions
        .initializing_blossom_defaults_for_pubkey
        .as_deref()
        == Some(pubkey_hex.as_str())
    {
        subscriptions.initializing_blossom_defaults_for_pubkey = None;
    }

    match result {
        Ok(()) => subscriptions.initialized_blossom_defaults_for_pubkey = Some(pubkey_hex),
        Err(message) => {
            let mut current = state.write();
            if current
                .chrome
                .current_user
                .as_ref()
                .is_some_and(|user| user.pubkey == pubkey_hex)
            {
                current.toast = Some(HighlighterToast {
                    kind: HighlighterToastKind::Error,
                    message,
                });
                current.bump();
                drop(current);
                emit(state, reconciler);
            }
        }
    }
}

fn clear_app_scope_subscriptions(
    core: &Arc<HighlighterCore>,
    subscriptions: &mut AppScopeSubscriptions,
) {
    if let Some(handle) = subscriptions.joined_communities.take() {
        core.unsubscribe(handle);
    }
    if let Some(handle) = subscriptions.bookmarks.take() {
        core.unsubscribe(handle);
    }
    subscriptions.initialized_blossom_defaults_for_pubkey = None;
    subscriptions.initializing_blossom_defaults_for_pubkey = None;
}

fn clear_profile_subscriptions(
    core: &Arc<HighlighterCore>,
    profile_handles: &mut BTreeMap<String, u64>,
    profile_pubkeys_by_handle: &mut BTreeMap<u64, String>,
) {
    for handle in profile_handles.values() {
        core.unsubscribe(*handle);
    }
    profile_handles.clear();
    profile_pubkeys_by_handle.clear();
}

fn append_callback_url(uri: String, callback_url: &str) -> String {
    let callback_url = callback_url.trim();
    if callback_url.is_empty() {
        return uri;
    }

    let separator = if uri.contains('?') { '&' } else { '?' };
    let encoded = url::form_urlencoded::byte_serialize(callback_url.as_bytes()).collect::<String>();
    format!("{uri}{separator}callback={encoded}")
}

fn update_create_account_display_name(
    state: &Arc<RwLock<HighlighterAppState>>,
    display_name: String,
    runtime: &mut CreateAccountRuntime,
) -> Option<(u64, String)> {
    let mut current = state.write();
    let trimmed = display_name.trim().to_string();
    let should_suggest_username = current.create_account.username.is_empty();
    current.create_account.display_name = display_name;
    current.create_account.error_message = None;
    current.create_account.created_user = None;

    let next_username = if should_suggest_username {
        let suggested = slugify_username(&trimmed);
        apply_create_account_username(&mut current.create_account, suggested)
    } else {
        None
    };

    recompute_create_account_submit(&mut current.create_account);
    current.bump();

    next_username.map(|username| {
        runtime.username_generation = runtime.username_generation.saturating_add(1);
        (runtime.username_generation, username)
    })
}

fn update_create_account_username(
    state: &Arc<RwLock<HighlighterAppState>>,
    username: String,
    runtime: &mut CreateAccountRuntime,
) -> Option<(u64, String)> {
    let mut current = state.write();
    current.create_account.error_message = None;
    current.create_account.created_user = None;
    let next_username = apply_create_account_username(&mut current.create_account, username);
    recompute_create_account_submit(&mut current.create_account);
    current.bump();

    next_username.map(|username| {
        runtime.username_generation = runtime.username_generation.saturating_add(1);
        (runtime.username_generation, username)
    })
}

fn apply_create_account_username(
    snapshot: &mut HighlighterCreateAccountSnapshot,
    username: String,
) -> Option<String> {
    let normalized = normalize_username(&username);
    snapshot.username = normalized.clone();
    snapshot.username_identifier.clear();
    snapshot.username_domain.clear();

    if normalized.is_empty() {
        snapshot.username_status = HighlighterUsernameStatus::Idle;
        return None;
    }

    if !is_valid_username(&normalized) {
        snapshot.username_status = HighlighterUsernameStatus::Invalid;
        return None;
    }

    snapshot.username_status = HighlighterUsernameStatus::Checking;
    Some(normalized)
}

fn recompute_create_account_submit(snapshot: &mut HighlighterCreateAccountSnapshot) {
    let has_name = !snapshot.display_name.trim().is_empty();
    // `Error` means the availability check itself failed (API/network), not
    // that the name is unavailable. The username is an optional nicety, so a
    // broken check must not brick signup — submit proceeds without the claim
    // (see `prepare_create_account_request`).
    let username_ok = snapshot.username.is_empty()
        || matches!(
            snapshot.username_status,
            HighlighterUsernameStatus::Available | HighlighterUsernameStatus::Error
        );
    snapshot.can_submit = has_name && username_ok && !snapshot.is_creating;
}

fn normalize_username(username: &str) -> String {
    username.trim().to_ascii_lowercase()
}

fn slugify_username(display_name: &str) -> String {
    display_name
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else if ch.is_whitespace() {
                '_'
            } else {
                '\0'
            }
        })
        .filter(|ch| *ch != '\0')
        .take(64)
        .collect()
}

fn is_valid_username(username: &str) -> bool {
    let len = username.len();
    (1..=64).contains(&len)
        && username
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
}

/// Submit a NIP-05 username availability check off-actor (Phase 4 fold of the
/// former `highlighter-nip05-check` worker). The `username_generation` carried
/// here is re-checked against the live form in `handle_username_availability_resolved`,
/// exactly as before; OpRunner's `UsernameCheck` slot supersedes an in-flight
/// check on each keystroke.
fn submit_username_availability_op(ops: &mut OpRunner, generation: u64, username: String) {
    let timeout_username = username.clone();
    ops.submit_op(
        OpDomain::UsernameCheck,
        OP_DEADLINE_NETWORK,
        OpOutcome::UsernameCheck {
            generation,
            username: timeout_username,
            result: Err(op_timeout_message(OpDomain::UsernameCheck)),
        },
        async move {
            let result = check_nip05_availability(&username).await;
            OpOutcome::UsernameCheck {
                generation,
                username,
                result,
            }
        },
    );
}

fn handle_username_availability_resolved(
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &mut CreateAccountRuntime,
    generation: u64,
    username: String,
    result: Result<Nip05Availability, String>,
) {
    if generation != runtime.username_generation {
        return;
    }

    let mut current = state.write();
    if current.create_account.username != username {
        return;
    }

    match result {
        Ok(availability) if availability.available => {
            current.create_account.username_status = HighlighterUsernameStatus::Available;
            current.create_account.username_identifier = availability.identifier;
            current.create_account.username_domain = availability.domain;
            current.create_account.error_message = None;
        }
        Ok(_) => {
            current.create_account.username_status = HighlighterUsernameStatus::Taken;
            current.create_account.username_identifier.clear();
            current.create_account.username_domain.clear();
            current.create_account.error_message = None;
        }
        Err(message) => {
            current.create_account.username_status = HighlighterUsernameStatus::Error;
            current.create_account.username_identifier.clear();
            current.create_account.username_domain.clear();
            current.create_account.error_message = Some(message);
        }
    }
    recompute_create_account_submit(&mut current.create_account);
    current.bump();
}

fn handle_nsec_sign_in_resolved(
    ctx: &ActorContext,
    runtimes: &mut ActorRuntimes,
    generation: u64,
    nsec: String,
    persist: bool,
    clear_stored_on_failure: bool,
    result: Result<CurrentUser, String>,
) {
    if generation != runtimes.auth_generation {
        return;
    }

    let state = &ctx.state;
    let reconciler = &ctx.reconciler;
    match result {
        Ok(user) => {
            set_toast(state, None);
            set_signed_in_user(state, user);
            emit(state, reconciler);
            if persist {
                emit_session_credential(reconciler, HighlighterSessionCredential::Nsec { nsec });
            }
        }
        Err(message) => {
            set_toast(
                state,
                Some(HighlighterToast {
                    kind: HighlighterToastKind::Error,
                    message,
                }),
            );
            set_signing_in(state, false);
            emit(state, reconciler);
            if clear_stored_on_failure {
                emit_clear_session_credentials(reconciler);
            }
        }
    }
}

fn handle_bunker_sign_in_resolved(
    ctx: &ActorContext,
    runtimes: &mut ActorRuntimes,
    generation: u64,
    uri: String,
    persist: bool,
    clear_stored_on_failure: bool,
    result: Result<CurrentUser, String>,
) {
    if generation != runtimes.auth_generation {
        return;
    }

    let state = &ctx.state;
    let reconciler = &ctx.reconciler;
    match result {
        Ok(user) => {
            set_toast(state, None);
            set_signed_in_user(state, user);
            emit(state, reconciler);
            if persist {
                emit_session_credential(
                    reconciler,
                    HighlighterSessionCredential::BunkerUri { uri },
                );
            }
        }
        Err(message) => {
            set_toast(
                state,
                Some(HighlighterToast {
                    kind: HighlighterToastKind::Error,
                    message,
                }),
            );
            set_signing_in(state, false);
            emit(state, reconciler);
            if clear_stored_on_failure {
                emit_clear_session_credentials(reconciler);
            }
        }
    }
}

/// Resolve a finished NIP-55 sign-in op — the [`handle_bunker_sign_in_resolved`]
/// shape for the external-signer backend (V-78). Persists the
/// `Nip55SignerPackage` credential only AFTER a successful pairing (matching
/// the bunker rule: never store a credential the signer hasn't honoured).
fn handle_nip55_sign_in_resolved(
    ctx: &ActorContext,
    runtimes: &mut ActorRuntimes,
    generation: u64,
    signer_package: Option<String>,
    persist: bool,
    clear_stored_on_failure: bool,
    result: Result<CurrentUser, String>,
) {
    if generation != runtimes.auth_generation {
        return;
    }

    let state = &ctx.state;
    let reconciler = &ctx.reconciler;
    match result {
        Ok(user) => {
            set_toast(state, None);
            set_signed_in_user(state, user);
            emit(state, reconciler);
            if persist {
                if let Some(pkg) = signer_package.filter(|s| !s.trim().is_empty()) {
                    emit_session_credential(
                        reconciler,
                        HighlighterSessionCredential::Nip55SignerPackage {
                            signer_package: pkg,
                        },
                    );
                }
            }
        }
        Err(message) => {
            set_toast(
                state,
                Some(HighlighterToast {
                    kind: HighlighterToastKind::Error,
                    message,
                }),
            );
            set_signing_in(state, false);
            emit(state, reconciler);
            if clear_stored_on_failure {
                emit_clear_session_credentials(reconciler);
            }
        }
    }
}

fn prepare_create_account_request(
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &mut CreateAccountRuntime,
) -> Option<CreateAccountRequest> {
    let mut current = state.write();
    if current.create_account.is_creating {
        return None;
    }

    let display_name = current.create_account.display_name.trim().to_string();
    if display_name.is_empty() {
        current.create_account.error_message = Some("Enter a display name".into());
        recompute_create_account_submit(&mut current.create_account);
        current.bump();
        return None;
    }

    let mut username = current.create_account.username.trim().to_string();
    if !username.is_empty() {
        match current.create_account.username_status {
            HighlighterUsernameStatus::Available => {}
            // The check itself failed (API/network): create the account
            // without claiming the username rather than blocking signup.
            HighlighterUsernameStatus::Error => username = String::new(),
            _ => {
                current.create_account.error_message =
                    Some("Choose an available username or leave it blank".into());
                recompute_create_account_submit(&mut current.create_account);
                current.bump();
                return None;
            }
        }
    }

    runtime.create_generation = runtime.create_generation.saturating_add(1);
    current.create_account.is_creating = true;
    current.create_account.can_submit = false;
    current.create_account.error_message = None;
    current.create_account.created_user = None;
    current.bump();

    Some(CreateAccountRequest {
        generation: runtime.create_generation,
        display_name,
        username,
        identifier: current.create_account.username_identifier.clone(),
        domain: current.create_account.username_domain.clone(),
    })
}

/// Submit account creation off-actor (Phase 4 fold of the former
/// `highlighter-create-account` worker). The hard 30s deadline that the old
/// worker enforced inline is now OpRunner's `OP_DEADLINE_NETWORK` (also 30s),
/// and the exact on-device timeout copy is preserved in `op_timeout_message`
/// — signing goes through the NMP actor and NIP-05 registration hits the
/// network, so without the bound the UI would sit on "Creating…" forever.
/// `create_generation` supersession is carried in the outcome and re-checked
/// in `handle_account_create_resolved`, byte-for-byte.
fn submit_create_account_op(
    ops: &mut OpRunner,
    core: &Arc<HighlighterCore>,
    request: CreateAccountRequest,
) {
    let core = core.clone();
    let generation = request.generation;
    ops.submit_op(
        OpDomain::AccountCreate,
        OP_DEADLINE_NETWORK,
        OpOutcome::AccountCreate {
            generation,
            result: Box::new(Err(op_timeout_message(OpDomain::AccountCreate))),
        },
        async move {
            let result = create_account(&core, request).await;
            OpOutcome::AccountCreate {
                generation,
                result: Box::new(result),
            }
        },
    );
}

fn set_create_room_cover_uploading(state: &Arc<RwLock<HighlighterAppState>>, is_uploading: bool) {
    let mut current = state.write();
    current.create_room.is_cover_uploading = is_uploading;
    if is_uploading {
        current.create_room.error_message = None;
        current.create_room.created_group_id = None;
    }
    current.bump();
}

fn set_create_room_creating(state: &Arc<RwLock<HighlighterAppState>>, is_creating: bool) {
    let mut current = state.write();
    current.create_room.is_creating = is_creating;
    if is_creating {
        current.create_room.error_message = None;
        current.create_room.created_group_id = None;
    }
    current.bump();
}

fn set_create_room_error(state: &Arc<RwLock<HighlighterAppState>>, message: String) {
    let mut current = state.write();
    current.create_room.is_cover_uploading = false;
    current.create_room.is_creating = false;
    current.create_room.error_message = Some(message);
    current.bump();
}

fn clear_create_room_cover(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.create_room.cover_upload = None;
    current.create_room.is_cover_uploading = false;
    current.create_room.error_message = None;
    current.bump();
}

fn clear_create_room_result(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.create_room = HighlighterCreateRoomSnapshot::empty();
    current.bump();
}

fn clear_create_room_error(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.create_room.error_message = None;
    current.bump();
}

fn prepare_open_room_invite(
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &mut RoomInviteRuntime,
    group_id: String,
) {
    let group_id = group_id.trim().to_string();
    if group_id.is_empty() {
        set_room_invite_add_error(state, "Choose a room first.".into());
        return;
    }

    if runtime.group_id.as_deref() != Some(group_id.as_str()) {
        runtime.group_id = Some(group_id.clone());
        runtime.follows.clear();
        let mut current = state.write();
        current.room_invite = HighlighterRoomInviteSnapshot::empty();
        current.room_invite.group_id = group_id;
        current.room_invite.is_loading_follows = true;
        current.room_invite.is_minting_invite_link = true;
        current.bump();
    } else {
        let mut current = state.write();
        current.room_invite.group_id = group_id;
        current.room_invite.is_loading_follows = true;
        current.room_invite.add_error_message = None;
        current.bump();
    }
}

async fn refresh_room_invite_follows(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &mut RoomInviteRuntime,
) -> Vec<String> {
    let group_id = state.read().room_invite.group_id.clone();
    if group_id.trim().is_empty() {
        clear_room_invite_snapshot(state);
        runtime.group_id = None;
        runtime.follows.clear();
        return Vec::new();
    }

    match core.get_follows().await {
        Ok(follows) => {
            runtime.follows = normalized_pubkeys(follows);
            let prefetch = runtime
                .follows
                .iter()
                .take(ROOM_INVITE_PROFILE_PREFETCH_LIMIT)
                .cloned()
                .collect();
            recompute_room_invite_visible_follows(state, &runtime.follows);
            let mut current = state.write();
            current.room_invite.is_loading_follows = false;
            current.room_invite.add_error_message = None;
            current.bump();
            prefetch
        }
        Err(err) => {
            runtime.follows.clear();
            let mut current = state.write();
            current.room_invite.visible_follows.clear();
            current.room_invite.follow_count = 0;
            current.room_invite.is_loading_follows = false;
            current.room_invite.add_error_message = Some(format!("Couldn't load follows: {err}"));
            current.bump();
            Vec::new()
        }
    }
}

fn set_room_invite_query(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &RoomInviteRuntime,
    query: String,
) -> Option<String> {
    let query = query.trim().to_string();
    let pasted_candidate = resolve_room_invite_paste(core, &query)
        .map(|(pubkey_hex, kind)| HighlighterRoomInviteResolvedCandidate { pubkey_hex, kind });
    let resolved_pubkey = pasted_candidate
        .as_ref()
        .map(|candidate| candidate.pubkey_hex.clone());
    {
        let mut current = state.write();
        current.room_invite.query = query;
        current.room_invite.pasted_candidate = pasted_candidate;
        current.room_invite.add_error_message = None;
        current.bump();
    }
    recompute_room_invite_visible_follows(state, &runtime.follows);
    resolved_pubkey
}

fn resolve_room_invite_paste(
    core: &Arc<HighlighterCore>,
    input: &str,
) -> Option<(String, HighlighterRoomInvitePastedKind)> {
    let trimmed = input
        .trim()
        .strip_prefix("nostr:")
        .unwrap_or(input.trim())
        .trim();
    if !looks_like_room_invite_reference(trimmed) {
        return None;
    }
    let pubkey_hex = core.decode_npub(trimmed.to_string()).ok()?;
    let lower = trimmed.to_ascii_lowercase();
    let kind = if lower.starts_with("npub1") {
        HighlighterRoomInvitePastedKind::Npub
    } else if lower.starts_with("nprofile1") {
        HighlighterRoomInvitePastedKind::Nprofile
    } else {
        HighlighterRoomInvitePastedKind::Hex
    };
    Some((pubkey_hex, kind))
}

fn looks_like_room_invite_reference(input: &str) -> bool {
    let lower = input.to_ascii_lowercase();
    (lower.starts_with("npub1") && lower.len() >= 60)
        || (lower.starts_with("nprofile1") && lower.len() >= 60)
        || (input.len() == 64 && input.chars().all(|c| c.is_ascii_hexdigit()))
}

fn toggle_room_invite_candidate(
    state: &Arc<RwLock<HighlighterAppState>>,
    pubkey_hex: String,
    source: HighlighterRoomInviteCandidateSource,
) {
    let pubkey_hex = pubkey_hex.trim().to_ascii_lowercase();
    if pubkey_hex.is_empty() {
        return;
    }

    let mut current = state.write();
    if current
        .room_invite
        .selected
        .iter()
        .any(|candidate| candidate.pubkey_hex == pubkey_hex)
    {
        current
            .room_invite
            .selected
            .retain(|candidate| candidate.pubkey_hex != pubkey_hex);
    } else if current
        .chrome
        .current_user
        .as_ref()
        .is_some_and(|user| user.pubkey.eq_ignore_ascii_case(&pubkey_hex))
    {
        current.room_invite.add_error_message = Some("You're already in this room.".into());
    } else {
        current
            .room_invite
            .selected
            .push(HighlighterRoomInviteCandidate { pubkey_hex, source });
        current.room_invite.add_error_message = None;
    }
    current.bump();
}

fn remove_room_invite_candidate(state: &Arc<RwLock<HighlighterAppState>>, pubkey_hex: &str) {
    let pubkey_hex = pubkey_hex.trim().to_ascii_lowercase();
    let mut current = state.write();
    current
        .room_invite
        .selected
        .retain(|candidate| candidate.pubkey_hex != pubkey_hex);
    current.bump();
}

fn accept_room_invite_pasted_candidate(state: &Arc<RwLock<HighlighterAppState>>) {
    let candidate = state
        .read()
        .room_invite
        .pasted_candidate
        .as_ref()
        .map(|candidate| candidate.pubkey_hex.clone());
    if let Some(pubkey_hex) = candidate {
        toggle_room_invite_candidate(
            state,
            pubkey_hex,
            HighlighterRoomInviteCandidateSource::Paste,
        );
        let mut current = state.write();
        current.room_invite.query.clear();
        current.room_invite.pasted_candidate = None;
        current.bump();
    }
}

/// Actor-side: read the room id, set the minting busy flag, and submit the
/// invite-code mint off-actor (Class B sign + publish). Resolution lands in
/// `apply_op_outcome` for `OpDomain::RoomInvite`.
fn submit_mint_room_invite_link(
    ops: &mut OpRunner,
    state: &Arc<RwLock<HighlighterAppState>>,
    core: &Arc<HighlighterCore>,
) {
    let group_id = state.read().room_invite.group_id.clone();
    if group_id.trim().is_empty() {
        return;
    }
    {
        let mut current = state.write();
        current.room_invite.is_minting_invite_link = true;
        current.room_invite.invite_link_error_message = None;
        current.bump();
    }
    let core = core.clone();
    let op_group_id = group_id.clone();
    ops.submit_op(
        OpDomain::RoomInvite,
        OP_DEADLINE_NETWORK,
        OpOutcome::RoomInvite {
            group_id,
            result: Err(op_timeout_message(OpDomain::RoomInvite)),
        },
        async move {
            let result = core
                .create_room_invite_codes(op_group_id.clone(), 1)
                .await
                .map_err(|err| err.to_string());
            OpOutcome::RoomInvite {
                group_id: op_group_id,
                result,
            }
        },
    );
}

fn clear_room_invite_add_error(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.room_invite.add_error_message = None;
    current.bump();
}

fn clear_room_invite_link_error(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.room_invite.invite_link_error_message = None;
    current.bump();
}

fn clear_room_invite_toast(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.room_invite.toast_message = None;
    current.bump();
}

fn clear_room_invite_snapshot(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.room_invite = HighlighterRoomInviteSnapshot::empty();
    current.bump();
}

fn set_room_invite_add_error(state: &Arc<RwLock<HighlighterAppState>>, message: String) {
    let mut current = state.write();
    current.room_invite.is_adding_members = false;
    current.room_invite.add_error_message = Some(message);
    current.bump();
}

fn recompute_room_invite_visible_follows(
    state: &Arc<RwLock<HighlighterAppState>>,
    follows: &[String],
) {
    let (query, profiles) = {
        let current = state.read();
        (
            current.room_invite.query.trim().to_ascii_lowercase(),
            current
                .profiles
                .iter()
                .map(|profile| (profile.pubkey_hex.clone(), profile.metadata.clone()))
                .collect::<BTreeMap<_, _>>(),
        )
    };
    let visible = if query.is_empty() {
        follows
            .iter()
            .take(ROOM_INVITE_VISIBLE_FOLLOW_LIMIT)
            .cloned()
            .collect()
    } else {
        follows
            .iter()
            .filter(|pubkey| room_invite_follow_matches(&query, pubkey, &profiles))
            .take(ROOM_INVITE_VISIBLE_FOLLOW_LIMIT)
            .cloned()
            .collect()
    };

    let mut current = state.write();
    current.room_invite.visible_follows = visible;
    current.room_invite.follow_count = follows.len() as u64;
    current.bump();
}

fn room_invite_follow_matches(
    query: &str,
    pubkey: &str,
    profiles: &BTreeMap<String, ProfileMetadata>,
) -> bool {
    if pubkey.to_ascii_lowercase().contains(query) {
        return true;
    }
    let Some(profile) = profiles.get(pubkey) else {
        return false;
    };
    profile.name.to_ascii_lowercase().contains(query)
        || profile.display_name.to_ascii_lowercase().contains(query)
        || profile.nip05.to_ascii_lowercase().contains(query)
}

fn normalized_pubkeys(pubkeys: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for pubkey in pubkeys {
        let pubkey = pubkey.trim().to_ascii_lowercase();
        if pubkey.len() == 64
            && pubkey.chars().all(|c| c.is_ascii_hexdigit())
            && seen.insert(pubkey.clone())
        {
            out.push(pubkey);
        }
    }
    out
}

fn short_pubkey(pubkey: &str) -> String {
    let trimmed = pubkey.trim();
    if trimmed.len() <= 12 {
        return trimmed.to_string();
    }
    format!("{}…{}", &trimmed[..6], &trimmed[trimmed.len() - 4..])
}

fn prepare_open_comments(
    state: &Arc<RwLock<HighlighterAppState>>,
    root_tag_name: String,
    root_tag_value: String,
    root_kind: u16,
) -> bool {
    let root_tag_name = root_tag_name.trim().to_ascii_uppercase();
    let root_tag_value = root_tag_value.trim().to_string();
    if !matches!(root_tag_name.as_str(), "A" | "E" | "I") || root_tag_value.is_empty() {
        let mut current = state.write();
        current.comments = HighlighterCommentsSnapshot::empty();
        current.comments.error_message = Some("Couldn't open comments for this item.".into());
        current.bump();
        return false;
    }

    let mut current = state.write();
    let same_root = current.comments.root_tag_name == root_tag_name
        && current.comments.root_tag_value == root_tag_value
        && current.comments.root_kind == root_kind;
    if !same_root {
        current.comments = HighlighterCommentsSnapshot::empty();
        current.comments.root_tag_name = root_tag_name;
        current.comments.root_tag_value = root_tag_value;
        current.comments.root_kind = root_kind;
    }
    current.comments.is_loading = true;
    current.comments.error_message = None;
    current.comments.publish_error_message = None;
    current.comments.interaction_error_message = None;
    current.bump();
    true
}

async fn refresh_comments(core: &Arc<HighlighterCore>, state: &Arc<RwLock<HighlighterAppState>>) {
    let (root_tag_name, root_tag_value) = {
        let current = state.read();
        (
            current.comments.root_tag_name.clone(),
            current.comments.root_tag_value.clone(),
        )
    };
    if root_tag_name.is_empty() || root_tag_value.is_empty() {
        clear_comments_snapshot(state);
        return;
    }

    match core
        .get_comments_for_reference(root_tag_name, root_tag_value, COMMENTS_LIMIT)
        .await
    {
        Ok(records) => {
            let interactions = hydrate_comment_interactions(core, state, &records).await;
            let (top_level_event_ids, child_links) =
                comment_thread_projection(&records, &state.read().comments.root_tag_value);
            let mut current = state.write();
            current.comments.record_count = records.len() as u64;
            current.comments.records = records;
            current.comments.top_level_event_ids = top_level_event_ids;
            current.comments.child_links = child_links;
            current.comments.interactions = interactions;
            current.comments.is_loading = false;
            current.comments.error_message = None;
            current.bump();
        }
        Err(err) => {
            let mut current = state.write();
            current.comments.is_loading = false;
            current.comments.error_message = Some(format!("Couldn't load comments: {err}"));
            current.bump();
        }
    }
}

async fn hydrate_comment_interactions(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    records: &[CommentRecord],
) -> Vec<HighlighterCommentInteraction> {
    let current_user = state
        .read()
        .chrome
        .current_user
        .as_ref()
        .map(|user| user.pubkey.clone());
    let mut interactions = Vec::with_capacity(records.len());
    for record in records {
        let reactions = core
            .get_reactions_for_event(record.event_id.clone(), COMMENT_REACTION_LIMIT)
            .await
            .unwrap_or_default();
        let likes = reactions
            .iter()
            .filter(|reaction| reaction.content == "+")
            .collect::<Vec<_>>();
        let my_like_event_id = current_user.as_ref().and_then(|me| {
            likes
                .iter()
                .find(|reaction| reaction.pubkey.eq_ignore_ascii_case(me))
                .map(|reaction| reaction.event_id.clone())
        });
        let is_bookmarked = core
            .is_event_bookmarked(record.event_id.clone())
            .await
            .unwrap_or(false);
        interactions.push(HighlighterCommentInteraction {
            event_id: record.event_id.clone(),
            like_count: likes.len() as u64,
            my_like_event_id,
            is_bookmarked,
        });
    }
    interactions
}

fn comment_thread_projection(
    records: &[CommentRecord],
    root_tag_value: &str,
) -> (Vec<String>, Vec<HighlighterCommentChildLinks>) {
    let mut sorted = records.to_vec();
    sorted.sort_by(|lhs, rhs| {
        lhs.created_at
            .unwrap_or(0)
            .cmp(&rhs.created_at.unwrap_or(0))
    });

    let seen_ids = sorted
        .iter()
        .map(|record| record.event_id.clone())
        .collect::<BTreeSet<_>>();
    let mut by_parent = BTreeMap::<String, Vec<String>>::new();
    for record in &sorted {
        by_parent
            .entry(record.parent_tag_value.clone())
            .or_default()
            .push(record.event_id.clone());
    }

    let mut top_level_event_ids = by_parent.get(root_tag_value).cloned().unwrap_or_default();
    for record in &sorted {
        let parent = record.parent_tag_value.as_str();
        if parent == root_tag_value || seen_ids.contains(parent) {
            continue;
        }
        if !top_level_event_ids.contains(&record.event_id) {
            top_level_event_ids.push(record.event_id.clone());
        }
    }

    let child_links = sorted
        .into_iter()
        .map(|record| HighlighterCommentChildLinks {
            event_id: record.event_id.clone(),
            child_event_ids: by_parent.remove(&record.event_id).unwrap_or_default(),
        })
        .collect();
    (top_level_event_ids, child_links)
}

fn comment_draft_key(parent_event_id: Option<&str>) -> Option<String> {
    parent_event_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn comment_draft_body(
    snapshot: &HighlighterCommentsSnapshot,
    parent_event_id: Option<&str>,
) -> String {
    let key = comment_draft_key(parent_event_id);
    snapshot
        .drafts
        .iter()
        .find(|draft| draft.parent_event_id == key)
        .map(|draft| draft.body.clone())
        .unwrap_or_default()
}

fn set_comment_draft(
    state: &Arc<RwLock<HighlighterAppState>>,
    parent_event_id: Option<String>,
    body: String,
) {
    let key = comment_draft_key(parent_event_id.as_deref());
    let mut current = state.write();
    current
        .comments
        .drafts
        .retain(|draft| draft.parent_event_id != key);
    if !body.is_empty() {
        current.comments.drafts.push(HighlighterCommentDraft {
            parent_event_id: key,
            body,
        });
    }
    current.comments.publish_error_message = None;
    current.bump();
}

fn set_comment_publishing(state: &Arc<RwLock<HighlighterAppState>>, is_publishing: bool) {
    let mut current = state.write();
    current.comments.is_publishing = is_publishing;
    if is_publishing {
        current.comments.publish_error_message = None;
        current.comments.last_published_event_id = None;
    }
    current.bump();
}

fn set_comment_publish_error(state: &Arc<RwLock<HighlighterAppState>>, message: String) {
    let mut current = state.write();
    current.comments.is_publishing = false;
    current.comments.publish_error_message = Some(message);
    current.bump();
}

fn clear_comment_publish_error(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.comments.publish_error_message = None;
    current.bump();
}

fn set_comment_interaction_error(state: &Arc<RwLock<HighlighterAppState>>, message: String) {
    let mut current = state.write();
    current.comments.interaction_error_message = Some(message);
    current.bump();
}

fn clear_comment_interaction_error(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.comments.interaction_error_message = None;
    current.bump();
}

fn clear_comments_snapshot(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.comments = HighlighterCommentsSnapshot::empty();
    current.bump();
}

fn prepare_open_feedback(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &mut FeedbackRuntime,
    coordinate: String,
) {
    let coordinate = coordinate.trim().to_string();
    if coordinate.is_empty() {
        clear_feedback_runtime(core, runtime);
        let mut current = state.write();
        current.feedback = HighlighterFeedbackSnapshot::empty();
        current.feedback.threads_error_message = Some("Couldn't open feedback.".into());
        current.bump();
        return;
    }

    if runtime.coordinate.as_deref() != Some(coordinate.as_str()) {
        clear_feedback_runtime(core, runtime);
        runtime.coordinate = Some(coordinate.clone());
        let mut current = state.write();
        current.feedback = HighlighterFeedbackSnapshot::empty();
        current.feedback.coordinate = coordinate;
        current.feedback.is_loading_threads = true;
        current.bump();
    } else {
        let mut current = state.write();
        current.feedback.coordinate = coordinate;
        current.feedback.threads_error_message = None;
        current.bump();
    }
}

fn set_feedback_threads_loading(state: &Arc<RwLock<HighlighterAppState>>, is_loading: bool) {
    let mut current = state.write();
    current.feedback.is_loading_threads = is_loading;
    if is_loading {
        current.feedback.threads_error_message = None;
    }
    current.bump();
}

async fn refresh_feedback_threads(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &FeedbackRuntime,
) {
    let coordinate = runtime
        .coordinate
        .clone()
        .or_else(|| {
            let value = state.read().feedback.coordinate.clone();
            (!value.is_empty()).then_some(value)
        })
        .unwrap_or_default();
    if coordinate.is_empty() {
        return;
    }

    match core.get_feedback_threads(coordinate.clone()).await {
        Ok(mut threads) => {
            if threads.len() > FEEDBACK_THREAD_LIMIT as usize {
                threads.truncate(FEEDBACK_THREAD_LIMIT as usize);
            }
            let mut current = state.write();
            current.feedback.coordinate = coordinate;
            current.feedback.thread_count = threads.len() as u64;
            current.feedback.threads = threads;
            current.feedback.is_loading_threads = false;
            current.feedback.threads_error_message = None;
            current.bump();
        }
        Err(err) => {
            let mut current = state.write();
            current.feedback.is_loading_threads = false;
            current.feedback.threads_error_message = Some(format!("Couldn't load feedback: {err}"));
            current.bump();
        }
    }
}

async fn ensure_feedback_threads_subscription(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &mut FeedbackRuntime,
) {
    if runtime.threads_handle.is_some() {
        return;
    }
    let Some(coordinate) = runtime.coordinate.clone() else {
        return;
    };
    match core.subscribe_feedback_threads(coordinate).await {
        Ok(handle) => runtime.threads_handle = Some(handle),
        Err(err) => {
            let mut current = state.write();
            current.feedback.threads_error_message =
                Some(format!("Couldn't subscribe to feedback: {err}"));
            current.feedback.is_loading_threads = false;
            current.bump();
        }
    }
}

fn prepare_open_feedback_thread(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &mut FeedbackRuntime,
    root_event_id: String,
) {
    let root_event_id = root_event_id.trim().to_ascii_lowercase();
    if root_event_id.is_empty() {
        clear_feedback_thread_runtime(core, runtime);
        let mut current = state.write();
        current.feedback.selected_root_event_id = None;
        current.feedback.selected_events.clear();
        current.feedback.selected_event_count = 0;
        current.feedback.thread_error_message = Some("Couldn't open feedback thread.".into());
        current.feedback.is_loading_thread = false;
        current.bump();
        return;
    }

    if runtime.selected_root_event_id.as_deref() != Some(root_event_id.as_str()) {
        clear_feedback_thread_runtime(core, runtime);
        runtime.selected_root_event_id = Some(root_event_id.clone());
        let mut current = state.write();
        current.feedback.selected_root_event_id = Some(root_event_id);
        current.feedback.selected_events.clear();
        current.feedback.selected_event_count = 0;
        current.feedback.thread_error_message = None;
        current.feedback.reply_draft.clear();
        current.feedback.is_loading_thread = true;
        current.bump();
    } else {
        let mut current = state.write();
        current.feedback.thread_error_message = None;
        current.bump();
    }
}

fn set_feedback_thread_loading(state: &Arc<RwLock<HighlighterAppState>>, is_loading: bool) {
    let mut current = state.write();
    current.feedback.is_loading_thread = is_loading;
    if is_loading {
        current.feedback.thread_error_message = None;
    }
    current.bump();
}

async fn refresh_feedback_thread(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &FeedbackRuntime,
) {
    let root_event_id = runtime
        .selected_root_event_id
        .clone()
        .or_else(|| state.read().feedback.selected_root_event_id.clone())
        .unwrap_or_default();
    if root_event_id.is_empty() {
        return;
    }

    match core.get_feedback_thread_events(root_event_id.clone()).await {
        Ok(mut events) => {
            if events.len() > FEEDBACK_EVENT_LIMIT as usize {
                let start = events.len() - FEEDBACK_EVENT_LIMIT as usize;
                events = events.split_off(start);
            }
            let mut current = state.write();
            current.feedback.selected_root_event_id = Some(root_event_id);
            current.feedback.selected_event_count = events.len() as u64;
            current.feedback.selected_events = events;
            current.feedback.is_loading_thread = false;
            current.feedback.thread_error_message = None;
            current.bump();
        }
        Err(err) => {
            let mut current = state.write();
            current.feedback.is_loading_thread = false;
            current.feedback.thread_error_message =
                Some(format!("Couldn't load feedback thread: {err}"));
            current.bump();
        }
    }
}

async fn ensure_feedback_thread_subscription(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &mut FeedbackRuntime,
) {
    if runtime.thread_handle.is_some() {
        return;
    }
    let Some(root_event_id) = runtime.selected_root_event_id.clone() else {
        return;
    };
    match core.subscribe_feedback_thread(root_event_id).await {
        Ok(handle) => runtime.thread_handle = Some(handle),
        Err(err) => {
            let mut current = state.write();
            current.feedback.thread_error_message =
                Some(format!("Couldn't subscribe to feedback thread: {err}"));
            current.feedback.is_loading_thread = false;
            current.bump();
        }
    }
}

fn set_feedback_new_thread_draft(state: &Arc<RwLock<HighlighterAppState>>, body: String) {
    let mut current = state.write();
    current.feedback.new_thread_draft = body;
    current.feedback.publish_error_message = None;
    current.bump();
}

fn set_feedback_reply_draft(state: &Arc<RwLock<HighlighterAppState>>, body: String) {
    let mut current = state.write();
    current.feedback.reply_draft = body;
    current.feedback.publish_error_message = None;
    current.bump();
}

fn set_feedback_new_thread_publishing(
    state: &Arc<RwLock<HighlighterAppState>>,
    is_publishing: bool,
) {
    let mut current = state.write();
    current.feedback.is_publishing_new_thread = is_publishing;
    if is_publishing {
        current.feedback.publish_error_message = None;
        current.feedback.last_published_root_event_id = None;
    }
    current.bump();
}

fn set_feedback_reply_publishing(state: &Arc<RwLock<HighlighterAppState>>, is_publishing: bool) {
    let mut current = state.write();
    current.feedback.is_publishing_reply = is_publishing;
    if is_publishing {
        current.feedback.publish_error_message = None;
    }
    current.bump();
}

fn set_feedback_publish_error(state: &Arc<RwLock<HighlighterAppState>>, message: String) {
    let mut current = state.write();
    current.feedback.is_publishing_new_thread = false;
    current.feedback.is_publishing_reply = false;
    current.feedback.publish_error_message = Some(message);
    current.bump();
}

fn clear_feedback_publish_error(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.feedback.publish_error_message = None;
    current.bump();
}

fn clear_feedback_thread_runtime(core: &Arc<HighlighterCore>, runtime: &mut FeedbackRuntime) {
    if let Some(handle) = runtime.thread_handle.take() {
        core.unsubscribe(handle);
    }
    runtime.selected_root_event_id = None;
}

fn clear_feedback_runtime(core: &Arc<HighlighterCore>, runtime: &mut FeedbackRuntime) {
    clear_feedback_thread_runtime(core, runtime);
    if let Some(handle) = runtime.threads_handle.take() {
        core.unsubscribe(handle);
    }
    runtime.coordinate = None;
}

fn clear_feedback_thread_snapshot(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.feedback.selected_root_event_id = None;
    current.feedback.selected_events.clear();
    current.feedback.selected_event_count = 0;
    current.feedback.is_loading_thread = false;
    current.feedback.thread_error_message = None;
    current.feedback.reply_draft.clear();
    current.feedback.is_publishing_reply = false;
    current.bump();
}

fn clear_feedback_snapshot(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.feedback = HighlighterFeedbackSnapshot::empty();
    current.bump();
}

fn feedback_threads_delta_affects_snapshot(
    runtime: &FeedbackRuntime,
    subscription_id: u64,
) -> bool {
    runtime.threads_handle == Some(subscription_id)
}

fn feedback_thread_delta_affects_snapshot(runtime: &FeedbackRuntime, subscription_id: u64) -> bool {
    runtime.thread_handle == Some(subscription_id)
}

fn set_media_settings_loading(state: &Arc<RwLock<HighlighterAppState>>, is_loading: bool) {
    let mut current = state.write();
    current.media_settings.is_loading = is_loading;
    if is_loading {
        current.media_settings.error_message = None;
    }
    current.bump();
}

fn set_media_settings_saving(state: &Arc<RwLock<HighlighterAppState>>, is_saving: bool) {
    let mut current = state.write();
    current.media_settings.is_saving = is_saving;
    if is_saving {
        current.media_settings.error_message = None;
    }
    current.bump();
}

async fn refresh_media_settings(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
) {
    match core.get_blossom_servers().await {
        Ok(servers) => {
            let servers = normalized_blossom_servers(servers);
            let mut current = state.write();
            current.media_settings.blossom_server_count = servers.len() as u64;
            current.media_settings.blossom_servers = servers;
            current.media_settings.is_loading = false;
            current.media_settings.is_saving = false;
            current.media_settings.error_message = None;
            current.bump();
        }
        Err(err) => {
            let mut current = state.write();
            current.media_settings.is_loading = false;
            current.media_settings.is_saving = false;
            current.media_settings.error_message =
                Some(format!("Couldn't load media servers: {err}"));
            current.bump();
        }
    }
}

fn add_blossom_server_to_snapshot(state: &Arc<RwLock<HighlighterAppState>>, url: String) -> bool {
    let Some(url) = normalize_blossom_server(&url) else {
        set_media_settings_error(state, "Enter a valid HTTP(S) Blossom server.".into());
        return false;
    };
    let mut current = state.write();
    if current
        .media_settings
        .blossom_servers
        .iter()
        .any(|existing| existing == &url)
    {
        return false;
    }
    current.media_settings.blossom_servers.push(url);
    current.media_settings.blossom_server_count =
        current.media_settings.blossom_servers.len() as u64;
    current.media_settings.error_message = None;
    current.bump();
    true
}

fn remove_blossom_server_from_snapshot(
    state: &Arc<RwLock<HighlighterAppState>>,
    url: &str,
) -> bool {
    let key = url.trim();
    let mut current = state.write();
    let before = current.media_settings.blossom_servers.len();
    current
        .media_settings
        .blossom_servers
        .retain(|server| server != key);
    if current.media_settings.blossom_servers.is_empty() {
        current.media_settings.blossom_servers = normalized_blossom_servers(vec![key.into()]);
        current.media_settings.error_message = Some("Keep at least one Blossom server.".into());
        current.media_settings.is_saving = false;
        current.bump();
        return false;
    }
    if current.media_settings.blossom_servers.len() == before {
        return false;
    }
    current.media_settings.blossom_server_count =
        current.media_settings.blossom_servers.len() as u64;
    current.media_settings.error_message = None;
    current.bump();
    true
}

fn move_blossom_servers_in_snapshot(
    state: &Arc<RwLock<HighlighterAppState>>,
    from_indices: Vec<u32>,
    to_index: u32,
) -> bool {
    if from_indices.is_empty() {
        return false;
    }
    let mut current = state.write();
    let mut servers = current.media_settings.blossom_servers.clone();
    let mut indices = from_indices
        .into_iter()
        .map(|idx| idx as usize)
        .filter(|idx| *idx < servers.len())
        .collect::<Vec<_>>();
    indices.sort_unstable();
    indices.dedup();
    if indices.is_empty() {
        return false;
    }

    let moving = indices
        .iter()
        .map(|idx| servers[*idx].clone())
        .collect::<Vec<_>>();
    for idx in indices.iter().rev() {
        servers.remove(*idx);
    }
    let removed_before_target = indices
        .iter()
        .filter(|idx| **idx < to_index as usize)
        .count();
    let insertion = (to_index as usize)
        .saturating_sub(removed_before_target)
        .min(servers.len());
    for (offset, server) in moving.into_iter().enumerate() {
        servers.insert(insertion + offset, server);
    }

    if servers == current.media_settings.blossom_servers {
        return false;
    }
    current.media_settings.blossom_servers = servers;
    current.media_settings.blossom_server_count =
        current.media_settings.blossom_servers.len() as u64;
    current.media_settings.error_message = None;
    current.bump();
    true
}

fn set_media_settings_error(state: &Arc<RwLock<HighlighterAppState>>, message: String) {
    let mut current = state.write();
    current.media_settings.is_loading = false;
    current.media_settings.is_saving = false;
    current.media_settings.error_message = Some(message);
    current.bump();
}

fn clear_media_settings_error(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.media_settings.error_message = None;
    current.bump();
}

fn clear_media_settings_transient_state(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.media_settings.is_loading = false;
    current.media_settings.is_saving = false;
    current.media_settings.error_message = None;
    current.bump();
}

fn normalized_blossom_servers(servers: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for server in servers {
        if let Some(server) = normalize_blossom_server(&server) {
            if seen.insert(server.clone()) {
                out.push(server);
            }
        }
    }
    out
}

fn normalize_blossom_server(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches('/').to_string();
    if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        Some(trimmed)
    } else {
        None
    }
}

fn open_edit_profile(state: &Arc<RwLock<HighlighterAppState>>, seed: Option<ProfileMetadata>) {
    let seed = seed.or_else(|| state.read().chrome.current_user_profile.clone());
    let mut snapshot = HighlighterEditProfileSnapshot::empty();
    if let Some(profile) = seed {
        snapshot.display_name = profile.display_name;
        snapshot.name = profile.name;
        snapshot.about = profile.about;
        snapshot.picture = profile.picture;
        snapshot.banner = profile.banner;
        snapshot.nip05 = profile.nip05;
        snapshot.website = profile.website;
        snapshot.lud16 = profile.lud16;
    }
    let mut current = state.write();
    current.edit_profile = snapshot;
    current.bump();
}

fn set_edit_profile_field<F>(state: &Arc<RwLock<HighlighterAppState>>, update: F)
where
    F: FnOnce(&mut HighlighterEditProfileSnapshot),
{
    let mut current = state.write();
    update(&mut current.edit_profile);
    current.edit_profile.error_message = None;
    current.edit_profile.saved_profile = None;
    current.bump();
}

fn set_edit_profile_image_uploading(
    state: &Arc<RwLock<HighlighterAppState>>,
    target: HighlighterEditProfileImageTarget,
    is_uploading: bool,
) {
    let mut current = state.write();
    match target {
        HighlighterEditProfileImageTarget::Picture => {
            current.edit_profile.is_picture_uploading = is_uploading;
        }
        HighlighterEditProfileImageTarget::Banner => {
            current.edit_profile.is_banner_uploading = is_uploading;
        }
    }
    if is_uploading {
        current.edit_profile.error_message = None;
    }
    current.bump();
}

fn set_edit_profile_saving(state: &Arc<RwLock<HighlighterAppState>>, is_saving: bool) {
    let mut current = state.write();
    current.edit_profile.is_saving = is_saving;
    if is_saving {
        current.edit_profile.error_message = None;
        current.edit_profile.saved_profile = None;
    }
    current.bump();
}

fn set_edit_profile_error(state: &Arc<RwLock<HighlighterAppState>>, message: String) {
    let mut current = state.write();
    current.edit_profile.is_picture_uploading = false;
    current.edit_profile.is_banner_uploading = false;
    current.edit_profile.is_saving = false;
    current.edit_profile.error_message = Some(message);
    current.bump();
}

fn clear_edit_profile_error(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.edit_profile.error_message = None;
    current.bump();
}

fn clear_edit_profile_result(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.edit_profile.saved_profile = None;
    current.bump();
}

fn clear_edit_profile_snapshot(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.edit_profile = HighlighterEditProfileSnapshot::empty();
    current.bump();
}

fn handle_account_create_resolved(
    ctx: &ActorContext,
    runtime: &mut CreateAccountRuntime,
    generation: u64,
    result: Result<CreateAccountOutcome, String>,
) {
    let state = &ctx.state;
    let reconciler = &ctx.reconciler;
    let actor_tx = &ctx.actor_tx;

    if generation != runtime.create_generation {
        return;
    }

    match result {
        Ok(outcome) => {
            {
                let mut current = state.write();
                current.create_account.is_creating = false;
                current.create_account.can_submit = false;
                current.create_account.error_message = None;
                current.create_account.created_user = Some(outcome.user.clone());
                current.chrome.current_user = Some(outcome.user.clone());
                current.auth.is_signing_in = false;
                current.toast = Some(HighlighterToast {
                    kind: outcome
                        .warning
                        .as_ref()
                        .map(|_| HighlighterToastKind::Info)
                        .unwrap_or(HighlighterToastKind::Success),
                    message: outcome
                        .warning
                        .unwrap_or_else(|| "Account created".to_string()),
                });
                current.bump();
            }
            emit(state, reconciler);
            emit_session_credential(
                reconciler,
                HighlighterSessionCredential::Nsec { nsec: outcome.nsec },
            );
            if let Err(err) = actor_tx.try_send(KernelMsg::Action(Box::new(
                HighlighterAppAction::RefreshAppChrome,
            ))) {
                log_send_failure(err);
            }
        }
        Err(message) => {
            let mut current = state.write();
            current.create_account.is_creating = false;
            current.create_account.error_message = Some(message.clone());
            current.toast = Some(HighlighterToast {
                kind: HighlighterToastKind::Error,
                message,
            });
            recompute_create_account_submit(&mut current.create_account);
            current.bump();
            emit(state, reconciler);
        }
    }
}

fn nip05_http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|err| format!("HTTP client init failed: {err}"))
}

async fn check_nip05_availability(username: &str) -> Result<Nip05Availability, String> {
    let url = reqwest::Url::parse_with_params(NIP05_API_URL, [("name", username)])
        .map_err(|err| format!("Invalid username check URL: {err}"))?;
    let response = nip05_http_client()?
        .get(url)
        .send()
        .await
        .map_err(|err| format!("Username check failed: {err}"))?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("Username check failed: {err}"))?;
    if !status.is_success() {
        return Err(format!("Username check failed ({})", status.as_u16()));
    }
    let decoded: Nip05AvailabilityResponse =
        serde_json::from_slice(&bytes).map_err(|err| format!("Username check failed: {err}"))?;
    let domain = decoded
        .identifier
        .split('@')
        .next_back()
        .unwrap_or("highlighter.com")
        .to_string();
    Ok(Nip05Availability {
        available: decoded.available,
        identifier: decoded.identifier,
        domain,
    })
}

async fn create_account(
    core: &Arc<HighlighterCore>,
    request: CreateAccountRequest,
) -> Result<CreateAccountOutcome, String> {
    let account = core
        .generate_account()
        .map_err(|err| format!("Account creation failed: {err}"))?;
    let mut nip05 = String::new();
    let mut warning = None;

    if !request.username.is_empty() {
        let registration = match core
            .sign_nip05_registration_auth(request.username.clone(), request.domain.clone())
            .await
            .map_err(|err| format!("Username registration failed: {err}"))
        {
            Ok(auth) => async_register_nip05(&request.username, &auth).await,
            Err(message) => Err(message),
        };
        match registration {
            Ok(()) => nip05 = request.identifier.clone(),
            Err(message) => warning = Some(format!("Account created; {message}")),
        }
    }

    if let Err(err) = core
        .update_profile(ProfileUpdateDraft {
            display_name: request.display_name,
            nip05,
            ..ProfileUpdateDraft::default()
        })
        .await
    {
        if warning.is_none() {
            warning = Some(format!("Account created; profile update failed: {err}"));
        }
    }

    Ok(CreateAccountOutcome {
        user: account.user,
        nsec: account.nsec,
        warning,
    })
}

async fn async_register_nip05(username: &str, auth_json: &str) -> Result<(), String> {
    let auth: serde_json::Value =
        serde_json::from_str(auth_json).map_err(|err| format!("Username auth failed: {err}"))?;
    let body = serde_json::json!({
        "name": username,
        "auth": auth,
    });
    let response = nip05_http_client()?
        .post(NIP05_API_URL)
        .json(&body)
        .send()
        .await
        .map_err(|err| format!("Username registration failed: {err}"))?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("Username registration failed: {err}"))?;
    if status.is_success() {
        return Ok(());
    }
    if let Ok(error) = serde_json::from_slice::<Nip05ErrorResponse>(&bytes) {
        if !error.error.trim().is_empty() {
            return Err(error.error);
        }
    }
    Err(format!(
        "Username registration failed ({})",
        status.as_u16()
    ))
}

/// Submit an ISBN preview lookup off-actor (Phase 4 fold of the former
/// `highlighter-isbn-preview` worker). Request coalescing is unchanged: the
/// `pending_isbn_lookups` set still dedups concurrent lookups for the same
/// ISBN, and the per-ISBN `OpDomain::IsbnPreview { target }` key (a stable hash
/// of the ISBN) gives each distinct ISBN an independent in-flight slot, so
/// concurrent lookups for different ISBNs do not supersede one another — exactly
/// the old multi-worker behaviour. Returns `false` (no immediate snapshot
/// change); the resolved handler emits.
fn start_isbn_preview_request(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    pending_isbn_lookups: &mut BTreeSet<String>,
    ops: &mut OpRunner,
    isbn: String,
) -> bool {
    let requested = isbn.trim().to_string();
    if requested.is_empty() {
        return false;
    }

    if state_has_isbn_preview(state, &requested) || !pending_isbn_lookups.insert(requested.clone())
    {
        return false;
    }

    let core = core.clone();
    let worker_requested = requested.clone();
    let domain = OpDomain::IsbnPreview {
        target: op_target_hash(&requested),
    };
    ops.submit_op(
        domain,
        OP_DEADLINE_NETWORK,
        OpOutcome::IsbnPreview {
            requested,
            result: Box::new(Err(op_timeout_message(domain))),
        },
        async move {
            let result = core
                .lookup_isbn(worker_requested.clone())
                .await
                .map_err(|err| err.to_string());
            OpOutcome::IsbnPreview {
                requested: worker_requested,
                result: Box::new(result),
            }
        },
    );
    false
}

fn handle_isbn_preview_resolved(
    state: &Arc<RwLock<HighlighterAppState>>,
    pending_isbn_lookups: &mut BTreeSet<String>,
    requested: String,
    result: Result<ArtifactPreview, String>,
    visible_limit: usize,
) {
    pending_isbn_lookups.remove(&requested);
    match result {
        Ok(preview) => {
            let isbn = isbn_key_for_preview(&requested, &preview);
            insert_isbn_preview(state, isbn, preview, visible_limit);
        }
        Err(message) => {
            // Benign background lookup failure (dead/missing ISBN) — do not
            // surface as a global error toast; just log and move on.
            tracing::debug!(isbn = %requested, error = %message, "isbn preview not found");
        }
    }
}

/// Submit a web-metadata lookup off-actor (Phase 4 fold of the former
/// `highlighter-web-metadata` worker). Identical coalescing/keying scheme to
/// [`start_isbn_preview_request`]: `pending_web_metadata` dedups, and the
/// per-URL `OpDomain::WebMetadata { target }` key keeps distinct URLs
/// independent. Returns `false`; the resolved handler emits.
fn start_web_metadata_request(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    pending_web_metadata: &mut BTreeSet<String>,
    ops: &mut OpRunner,
    url: String,
) -> bool {
    let requested = url.trim().to_string();
    if requested.is_empty() {
        return false;
    }

    if state_has_web_metadata(state, &requested) || !pending_web_metadata.insert(requested.clone())
    {
        return false;
    }

    let core = core.clone();
    let worker_requested = requested.clone();
    let domain = OpDomain::WebMetadata {
        target: op_target_hash(&requested),
    };
    ops.submit_op(
        domain,
        OP_DEADLINE_NETWORK,
        OpOutcome::WebMetadata {
            requested,
            result: Box::new(Err(op_timeout_message(domain))),
        },
        async move {
            let result = core
                .get_web_metadata(worker_requested.clone())
                .await
                .map_err(|err| err.to_string());
            OpOutcome::WebMetadata {
                requested: worker_requested,
                result: Box::new(result),
            }
        },
    );
    false
}

fn handle_web_metadata_resolved(
    state: &Arc<RwLock<HighlighterAppState>>,
    pending_web_metadata: &mut BTreeSet<String>,
    requested: String,
    result: Result<WebMetadata, String>,
    visible_limit: usize,
) {
    pending_web_metadata.remove(&requested);
    match result {
        Ok(metadata) => insert_web_metadata(state, requested, metadata, visible_limit),
        Err(message) => {
            // Benign background lookup failure (dead link / non-HTML page) — do
            // not surface as a global error toast; just log and move on.
            tracing::debug!(url = %requested, error = %message, "web metadata not found");
        }
    }
}

async fn hydrate_joined_communities(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    pending_joins: &mut Vec<PendingJoin>,
    visible_limit: usize,
) {
    let communities = core.get_joined_communities().await.unwrap_or_default();
    let share_extension = share_extension_snapshot_for(&communities, visible_limit);
    let relay_removal_impacts = relay_removal_impacts_for(&communities);
    let joined_ids: std::collections::HashSet<&str> = communities
        .iter()
        .map(|community| community.id.as_str())
        .collect();
    let mut confirmed = Vec::new();
    pending_joins.retain(|join| {
        if joined_ids.contains(join.group_id.as_str()) {
            confirmed.push(join.room_name.clone());
            false
        } else {
            true
        }
    });

    let total = communities.len() as u64;
    let visible = communities.into_iter().take(visible_limit).collect();
    let mut current = state.write();
    current.chrome.joined_communities = visible;
    current.chrome.joined_communities_total = total;
    current.share_extension = share_extension;
    current.network.relay_removal_impacts = relay_removal_impacts;
    if let Some(room_name) = confirmed.last() {
        current.toast = Some(HighlighterToast {
            kind: HighlighterToastKind::Success,
            message: format!("You're in {room_name}"),
        });
    }
    current.bump();
}

fn relay_removal_impacts_for(
    communities: &[CommunitySummary],
) -> Vec<HighlighterRelayRemovalImpact> {
    let mut by_relay = BTreeMap::<String, Vec<String>>::new();
    for community in communities {
        let relay_url = community.relay_url.trim();
        if relay_url.is_empty() {
            continue;
        }
        let room_name = if community.name.trim().is_empty() {
            community.id.clone()
        } else {
            community.name.clone()
        };
        by_relay
            .entry(relay_url.to_string())
            .or_default()
            .push(room_name);
    }

    by_relay
        .into_iter()
        .map(|(relay_url, room_names)| {
            let room_count = room_names.len() as u64;
            HighlighterRelayRemovalImpact {
                relay_url,
                room_names: room_names
                    .into_iter()
                    .take(RELAY_REMOVAL_ROOM_NAME_LIMIT)
                    .collect(),
                room_count,
            }
        })
        .collect()
}

fn relay_removal_impact_for_url(
    impacts: &[HighlighterRelayRemovalImpact],
    url: &str,
) -> Option<HighlighterRelayRemovalImpact> {
    let key = url.trim();
    if key.is_empty() {
        return None;
    }
    impacts
        .iter()
        .find(|impact| impact.relay_url == key)
        .cloned()
}

fn share_extension_snapshot_for(
    communities: &[CommunitySummary],
    visible_limit: usize,
) -> HighlighterShareExtensionSnapshot {
    HighlighterShareExtensionSnapshot {
        communities: communities
            .iter()
            .take(visible_limit)
            .map(|community| HighlighterShareExtensionCommunity {
                id: community.id.clone(),
                name: community.name.clone(),
                picture: community.picture.clone(),
            })
            .collect(),
        community_count: communities.len() as u64,
    }
}

fn state_has_isbn_preview(state: &Arc<RwLock<HighlighterAppState>>, isbn: &str) -> bool {
    let key = isbn.trim();
    state
        .read()
        .isbn_previews
        .iter()
        .any(|entry| entry.isbn == key)
}

fn insert_isbn_preview(
    state: &Arc<RwLock<HighlighterAppState>>,
    isbn: String,
    preview: ArtifactPreview,
    visible_limit: usize,
) {
    let mut current = state.write();
    current.isbn_previews.retain(|entry| entry.isbn != isbn);
    current
        .isbn_previews
        .push(HighlighterIsbnPreview { isbn, preview });
    current.isbn_preview_count = current.isbn_previews.len() as u64;
    let max_len = visible_limit.max(1);
    if current.isbn_previews.len() > max_len {
        let drain_count = current.isbn_previews.len() - max_len;
        current.isbn_previews.drain(0..drain_count);
    }
    current.isbn_preview_count = current.isbn_previews.len() as u64;
    current.bump();
}

fn isbn_key_for_preview(requested: &str, preview: &ArtifactPreview) -> String {
    preview
        .catalog_id
        .strip_prefix("isbn:")
        .filter(|isbn| !isbn.trim().is_empty())
        .unwrap_or(requested.trim())
        .to_string()
}

fn state_has_web_metadata(state: &Arc<RwLock<HighlighterAppState>>, url: &str) -> bool {
    let key = url.trim();
    state.read().web_metadata.iter().any(|entry| {
        entry.url == key || (!entry.metadata.url.is_empty() && entry.metadata.url == key)
    })
}

fn insert_web_metadata(
    state: &Arc<RwLock<HighlighterAppState>>,
    url: String,
    metadata: WebMetadata,
    visible_limit: usize,
) {
    let mut current = state.write();
    let canonical_url = metadata.url.clone();
    current.web_metadata.retain(|entry| {
        entry.url != url && (canonical_url.is_empty() || entry.metadata.url != canonical_url)
    });
    current
        .web_metadata
        .push(HighlighterWebMetadata { url, metadata });
    current.web_metadata_count = current.web_metadata.len() as u64;
    let max_len = visible_limit.max(1);
    if current.web_metadata.len() > max_len {
        let drain_count = current.web_metadata.len() - max_len;
        current.web_metadata.drain(0..drain_count);
    }
    current.web_metadata_count = current.web_metadata.len() as u64;
    current.bump();
}

async fn load_reference_highlights(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    tag_name: String,
    tag_value: String,
    limit: u32,
    visible_limit: usize,
) {
    let tag_name = tag_name.trim().to_ascii_lowercase();
    let tag_value = tag_value.trim().to_string();
    if tag_name.is_empty() || tag_value.is_empty() {
        return;
    }
    let limit = limit.clamp(1, ROOM_DETAIL_REFERENCE_LIMIT);
    match core
        .get_highlights_for_reference(tag_name.clone(), tag_value.clone(), limit)
        .await
    {
        Ok(highlights) => {
            let bucket = HighlighterReferenceHighlightBucket {
                key: reference_bucket_key(&tag_name, &tag_value),
                tag_name,
                tag_value,
                highlight_count: highlights.len() as u64,
                highlights,
            };
            insert_reference_highlight_bucket(state, bucket, visible_limit);
        }
        Err(err) => set_toast(
            state,
            Some(HighlighterToast {
                kind: HighlighterToastKind::Error,
                message: format!("Couldn't load highlights: {err}"),
            }),
        ),
    }
}

fn insert_reference_highlight_bucket(
    state: &Arc<RwLock<HighlighterAppState>>,
    bucket: HighlighterReferenceHighlightBucket,
    visible_limit: usize,
) {
    let mut current = state.write();
    current
        .reference_highlights
        .retain(|entry| entry.key != bucket.key);
    current.reference_highlights.push(bucket);
    let max_len = visible_limit.max(1);
    if current.reference_highlights.len() > max_len {
        let drain_count = current.reference_highlights.len() - max_len;
        current.reference_highlights.drain(0..drain_count);
    }
    current.reference_highlight_count = current.reference_highlights.len() as u64;
    current.bump();
}

fn reference_bucket_key(tag_name: &str, tag_value: &str) -> String {
    format!(
        "{}:{}",
        tag_name.trim().to_ascii_lowercase(),
        tag_value.trim()
    )
}

fn set_book_picker_recents_loading(state: &Arc<RwLock<HighlighterAppState>>, loading: bool) {
    let mut current = state.write();
    current.book_picker.is_loading_recents = loading;
    if loading {
        current.book_picker.error_message = None;
    }
    current.bump();
}

async fn load_book_picker_recents(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    limit: u32,
    visible_limit: usize,
) {
    let limit = limit.clamp(1, visible_limit.max(1) as u32);
    match core.get_recent_books(limit).await {
        Ok(mut recent_books) => {
            recent_books.truncate(visible_limit.max(1));
            let mut current = state.write();
            current.book_picker.recent_book_count = recent_books.len() as u64;
            current.book_picker.recent_books = recent_books;
            current.book_picker.is_loading_recents = false;
            current.book_picker.error_message = None;
            current.bump();
        }
        Err(err) => {
            let mut current = state.write();
            current.book_picker.recent_books.clear();
            current.book_picker.recent_book_count = 0;
            current.book_picker.is_loading_recents = false;
            current.book_picker.error_message = Some(format!("Couldn't load recent books: {err}"));
            current.bump();
        }
    }
}

fn set_book_picker_searching(
    state: &Arc<RwLock<HighlighterAppState>>,
    query: String,
    searching: bool,
) {
    let mut current = state.write();
    current.book_picker.search_query = query.trim().to_string();
    current.book_picker.is_searching = searching;
    if searching {
        current.book_picker.error_message = None;
    }
    current.bump();
}

async fn search_book_picker_artifacts(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    query: String,
    limit: u32,
    visible_limit: usize,
) {
    let query = query.trim().to_string();
    if query.is_empty() {
        clear_book_picker_search(state);
        return;
    }
    let limit = limit.clamp(1, visible_limit.max(1) as u32);
    match core.search_artifacts(query.clone(), limit).await {
        Ok(mut results) => {
            results.truncate(visible_limit.max(1));
            let mut current = state.write();
            if current.book_picker.search_query != query {
                current.book_picker.is_searching = false;
                current.bump();
                return;
            }
            current.book_picker.search_result_count = results.len() as u64;
            current.book_picker.search_results = results;
            current.book_picker.is_searching = false;
            current.book_picker.error_message = None;
            current.bump();
        }
        Err(err) => {
            let mut current = state.write();
            if current.book_picker.search_query != query {
                current.book_picker.is_searching = false;
                current.bump();
                return;
            }
            current.book_picker.search_results.clear();
            current.book_picker.search_result_count = 0;
            current.book_picker.is_searching = false;
            current.book_picker.error_message = Some(format!("Couldn't search books: {err}"));
            current.bump();
        }
    }
}

fn clear_book_picker_search(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.book_picker.search_query.clear();
    current.book_picker.search_results.clear();
    current.book_picker.search_result_count = 0;
    current.book_picker.is_searching = false;
    current.book_picker.error_message = None;
    current.bump();
}

fn set_capture_uploading(state: &Arc<RwLock<HighlighterAppState>>, uploading: bool) {
    let mut current = state.write();
    current.capture.is_uploading = uploading;
    if uploading {
        current.capture.upload = None;
        current.capture.upload_error_message = None;
    }
    current.bump();
}

fn set_capture_upload_error(state: &Arc<RwLock<HighlighterAppState>>, message: String) {
    let mut current = state.write();
    current.capture.upload = None;
    current.capture.is_uploading = false;
    current.capture.upload_error_message = Some(message);
    current.bump();
}

fn clear_capture_upload(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.capture.upload = None;
    current.capture.is_uploading = false;
    current.capture.upload_error_message = None;
    current.bump();
}

fn set_capture_publishing(state: &Arc<RwLock<HighlighterAppState>>, publishing: bool) {
    let mut current = state.write();
    current.capture.is_publishing = publishing;
    if publishing {
        current.capture.published_event_id = None;
        current.capture.error_message = None;
    }
    current.bump();
}

/// Network-only capture-highlight publish (no state/runtimes touched). Runs in
/// an `OpRunner` worker; the result is applied on the actor in `apply_op_outcome`.
async fn capture_highlight_network(
    core: &Arc<HighlighterCore>,
    selection: HighlighterCaptureArtifact,
    target_group_id: Option<String>,
    draft: HighlightDraft,
) -> Result<(String, String), String> {
    let artifact = resolve_capture_artifact(core, selection, target_group_id.as_deref()).await?;
    capture_highlight_with_optional_share_network(core, artifact, target_group_id, draft).await
}

/// Shared network publish path for capture / clip highlights.
async fn capture_highlight_with_optional_share_network(
    core: &Arc<HighlighterCore>,
    artifact: ArtifactRecord,
    target_group_id: Option<String>,
    draft: HighlightDraft,
) -> Result<(String, String), String> {
    let target_group_id = target_group_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let event_id = if let Some(group_id) = target_group_id {
        match core
            .publish_highlights_and_share(artifact, vec![draft], group_id)
            .await
        {
            Ok(records) => records
                .into_iter()
                .next()
                .map(|record| record.event_id)
                .ok_or_else(|| "Publish did not return a highlight event".to_string())?,
            Err(err) => return Err(err.to_string()),
        }
    } else {
        core.publish_highlight(draft, artifact)
            .await
            .map(|record| record.event_id)
            .map_err(|err| err.to_string())?
    };
    Ok((event_id, "Highlight published".to_string()))
}

/// Network-only capture-picture publish (no state/runtimes touched).
async fn capture_picture_network(
    core: &Arc<HighlighterCore>,
    selection: Option<HighlighterCaptureArtifact>,
    target_group_id: Option<String>,
    image: BlossomUpload,
    note: String,
) -> Result<(String, String), String> {
    let artifact = match selection {
        Some(selection) => {
            Some(resolve_capture_artifact(core, selection, target_group_id.as_deref()).await?)
        }
        None => None,
    };
    let target_group_id = target_group_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let draft = PictureDraft {
        image,
        note: note.trim().to_string(),
        artifact,
        target_group_id,
    };
    core.publish_picture(draft)
        .await
        .map(|record| (record.event_id, "Photo shared".to_string()))
        .map_err(|err| err.to_string())
}

async fn resolve_capture_artifact(
    core: &Arc<HighlighterCore>,
    selection: HighlighterCaptureArtifact,
    target_group_id: Option<&str>,
) -> Result<ArtifactRecord, String> {
    match selection {
        HighlighterCaptureArtifact::Existing { record } => Ok(record),
        HighlighterCaptureArtifact::Pending { preview } => {
            let group_id = target_group_id
                .map(str::trim)
                .filter(|value| !value.is_empty());
            if let Some(group_id) = group_id {
                core.publish_artifact(preview, group_id.to_string(), None)
                    .await
                    .map_err(|err| err.to_string())
            } else {
                Ok(synthesized_artifact_record(preview))
            }
        }
    }
}

fn synthesized_artifact_record(preview: ArtifactPreview) -> ArtifactRecord {
    ArtifactRecord {
        preview,
        group_id: String::new(),
        share_event_id: String::new(),
        pubkey: String::new(),
        created_at: None,
        note: String::new(),
    }
}

fn set_capture_publish_success(
    state: &Arc<RwLock<HighlighterAppState>>,
    event_id: String,
    toast_message: String,
) {
    let mut current = state.write();
    current.capture.is_publishing = false;
    current.capture.error_message = None;
    current.capture.published_event_id = Some(event_id);
    current.toast = Some(HighlighterToast {
        kind: HighlighterToastKind::Success,
        message: toast_message,
    });
    current.bump();
}

fn set_capture_publish_error(state: &Arc<RwLock<HighlighterAppState>>, message: String) {
    let mut current = state.write();
    current.capture.is_publishing = false;
    current.capture.error_message = Some(message);
    current.capture.published_event_id = None;
    current.bump();
}

fn clear_capture_result(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.capture.published_event_id = None;
    current.bump();
}

fn clear_capture_error(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.capture.error_message = None;
    current.capture.upload_error_message = None;
    current.bump();
}

fn request_profile(
    runtime: &tokio::runtime::Runtime,
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    profile_handles: &mut BTreeMap<String, u64>,
    profile_pubkeys_by_handle: &mut BTreeMap<u64, String>,
    pubkey_hex: String,
    visible_limit: usize,
) -> bool {
    let pubkey_hex = pubkey_hex.trim().to_ascii_lowercase();
    if pubkey_hex.is_empty() {
        return false;
    }

    let mut changed = block_on_local(
        runtime,
        "hydrate_profile",
        hydrate_profile(core, state, pubkey_hex.clone(), visible_limit),
    );

    if !profile_handles.contains_key(&pubkey_hex) {
        match block_on_local(
            runtime,
            "subscribe_user_profile",
            core.subscribe_user_profile(pubkey_hex.clone()),
        ) {
            Ok(handle) => {
                profile_handles.insert(pubkey_hex.clone(), handle);
                profile_pubkeys_by_handle.insert(handle, pubkey_hex);
            }
            Err(err) => {
                // Benign background profile subscription failure — not user-actionable;
                // log instead of raising a global error toast.
                tracing::debug!(pubkey = %pubkey_hex, error = %err, "profile subscription failed");
                changed = true;
            }
        }
    }

    changed
}

async fn hydrate_profile(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    pubkey_hex: String,
    visible_limit: usize,
) -> bool {
    match core.get_user_profile(pubkey_hex.clone()).await {
        Ok(Some(metadata)) => insert_profile_metadata(state, pubkey_hex, metadata, visible_limit),
        Ok(None) => false,
        Err(err) => {
            set_toast(
                state,
                Some(HighlighterToast {
                    kind: HighlighterToastKind::Error,
                    message: err.to_string(),
                }),
            );
            true
        }
    }
}

fn insert_profile_metadata(
    state: &Arc<RwLock<HighlighterAppState>>,
    pubkey_hex: String,
    metadata: ProfileMetadata,
    visible_limit: usize,
) -> bool {
    let pubkey_hex = if metadata.pubkey.trim().is_empty() {
        pubkey_hex
    } else {
        metadata.pubkey.trim().to_ascii_lowercase()
    };
    if pubkey_hex.is_empty() {
        return false;
    }

    let mut current = state.write();
    current
        .profiles
        .retain(|entry| entry.pubkey_hex != pubkey_hex);
    current.profiles.push(HighlighterProfile {
        pubkey_hex,
        metadata,
    });
    current.profile_count = current.profiles.len() as u64;
    let max_len = visible_limit.max(1);
    if current.profiles.len() > max_len {
        let drain_count = current.profiles.len() - max_len;
        current.profiles.drain(0..drain_count);
    }
    current.profile_count = current.profiles.len() as u64;
    current.bump();
    true
}

fn prepare_open_profile_view(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    profile_runtime: &mut ProfileViewRuntime,
    pubkey_hex: String,
) -> bool {
    let pubkey_hex = pubkey_hex.trim().to_ascii_lowercase();
    if pubkey_hex.is_empty() {
        set_profile_view_error(state, "Choose a profile to open".into());
        return false;
    }

    if profile_runtime.pubkey_hex.as_deref() != Some(pubkey_hex.as_str()) {
        clear_profile_view_runtime(core, profile_runtime);
        profile_runtime.pubkey_hex = Some(pubkey_hex.clone());
    }
    set_profile_view_loading_for(state, pubkey_hex, current_viewer_pubkey(core, state), true);
    true
}

async fn refresh_profile_view(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    profile_runtime: &mut ProfileViewRuntime,
    visible_limit: usize,
) {
    let Some(pubkey_hex) = profile_runtime.pubkey_hex.clone() else {
        clear_profile_view_snapshot(state);
        return;
    };

    let viewer_pubkey_hex = current_viewer_pubkey(core, state);
    let mut first_error: Option<String> = None;
    ensure_profile_view_subscriptions(
        core,
        profile_runtime,
        &pubkey_hex,
        viewer_pubkey_hex.as_deref(),
        &mut first_error,
    )
    .await;

    let profile = match core.get_user_profile(pubkey_hex.clone()).await {
        Ok(profile) => profile,
        Err(err) => {
            record_first_error(&mut first_error, err.to_string());
            None
        }
    };

    let article_limit = visible_limit.clamp(1, PROFILE_ARTICLE_LIMIT) as u32;
    let mut articles = match core
        .get_user_articles(pubkey_hex.clone(), article_limit)
        .await
    {
        Ok(articles) => articles,
        Err(err) => {
            record_first_error(&mut first_error, err.to_string());
            Vec::new()
        }
    };
    sort_articles_newest_first(&mut articles);

    let highlight_limit = visible_limit.clamp(1, PROFILE_HIGHLIGHT_LIMIT) as u32;
    let mut highlights = match core
        .get_user_highlights(pubkey_hex.clone(), highlight_limit)
        .await
    {
        Ok(highlights) => highlights,
        Err(err) => {
            record_first_error(&mut first_error, err.to_string());
            Vec::new()
        }
    };
    highlights.truncate(highlight_limit as usize);

    let mut communities = match core.get_user_communities(pubkey_hex.clone()).await {
        Ok(communities) => communities,
        Err(err) => {
            record_first_error(&mut first_error, err.to_string());
            Vec::new()
        }
    };
    communities.truncate(visible_limit.clamp(1, PROFILE_COMMUNITY_LIMIT));

    let is_own_profile = viewer_pubkey_hex
        .as_deref()
        .is_some_and(|viewer| viewer.eq_ignore_ascii_case(&pubkey_hex));
    let is_following = if viewer_pubkey_hex.is_some() && !is_own_profile {
        match core.is_following(pubkey_hex.clone()).await {
            Ok(is_following) => is_following,
            Err(err) => {
                record_first_error(&mut first_error, err.to_string());
                false
            }
        }
    } else {
        false
    };

    let snapshot = HighlighterProfileViewSnapshot {
        pubkey_hex,
        viewer_pubkey_hex,
        profile,
        article_count: articles.len() as u64,
        articles,
        highlight_count: highlights.len() as u64,
        highlights,
        community_count: communities.len() as u64,
        communities,
        is_following,
        is_own_profile,
        is_mutating_follow: false,
        is_loading: false,
        error_message: first_error,
    };

    let mut current = state.write();
    current.profile_view = snapshot;
    current.bump();
}

async fn ensure_profile_view_subscriptions(
    core: &Arc<HighlighterCore>,
    profile_runtime: &mut ProfileViewRuntime,
    pubkey_hex: &str,
    viewer_pubkey_hex: Option<&str>,
    first_error: &mut Option<String>,
) {
    if profile_runtime.target_handle.is_none() {
        match core.subscribe_user_profile(pubkey_hex.to_string()).await {
            Ok(handle) => profile_runtime.target_handle = Some(handle),
            Err(err) => record_first_error(first_error, err.to_string()),
        }
    }

    let follow_viewer = viewer_pubkey_hex
        .map(str::trim)
        .filter(|viewer| !viewer.is_empty() && !viewer.eq_ignore_ascii_case(pubkey_hex));
    if profile_runtime.viewer_follow_pubkey_hex.as_deref() != follow_viewer {
        if let Some(handle) = profile_runtime.viewer_follow_handle.take() {
            core.unsubscribe(handle);
        }
        profile_runtime.viewer_follow_pubkey_hex = follow_viewer.map(str::to_string);
        if let Some(viewer) = follow_viewer {
            match core.subscribe_user_profile(viewer.to_string()).await {
                Ok(handle) => profile_runtime.viewer_follow_handle = Some(handle),
                Err(err) => record_first_error(first_error, err.to_string()),
            }
        }
    }
}

fn profile_view_delta_affects_snapshot(
    profile_runtime: &ProfileViewRuntime,
    subscription_id: u64,
    pubkey_hex: &str,
    kind: u32,
) -> bool {
    if profile_runtime.target_handle == Some(subscription_id) {
        return profile_runtime
            .pubkey_hex
            .as_deref()
            .is_some_and(|target| target.eq_ignore_ascii_case(pubkey_hex));
    }
    profile_runtime.viewer_follow_handle == Some(subscription_id)
        && kind == 3
        && profile_runtime
            .viewer_follow_pubkey_hex
            .as_deref()
            .is_some_and(|viewer| viewer.eq_ignore_ascii_case(pubkey_hex))
}

fn current_viewer_pubkey(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
) -> Option<String> {
    state
        .read()
        .chrome
        .current_user
        .as_ref()
        .map(|user| user.pubkey.clone())
        .or_else(|| core.current_user().map(|user| user.pubkey))
}

fn clear_profile_view_runtime(core: &Arc<HighlighterCore>, runtime: &mut ProfileViewRuntime) {
    if let Some(handle) = runtime.target_handle.take() {
        core.unsubscribe(handle);
    }
    if let Some(handle) = runtime.viewer_follow_handle.take() {
        core.unsubscribe(handle);
    }
    *runtime = ProfileViewRuntime::default();
}

fn set_profile_view_loading(state: &Arc<RwLock<HighlighterAppState>>, is_loading: bool) {
    let mut current = state.write();
    current.profile_view.is_loading = is_loading;
    current.profile_view.error_message = None;
    current.bump();
}

fn set_profile_view_loading_for(
    state: &Arc<RwLock<HighlighterAppState>>,
    pubkey_hex: String,
    viewer_pubkey_hex: Option<String>,
    is_loading: bool,
) {
    let is_own_profile = viewer_pubkey_hex
        .as_deref()
        .is_some_and(|viewer| viewer.eq_ignore_ascii_case(&pubkey_hex));
    let mut current = state.write();
    current.profile_view = HighlighterProfileViewSnapshot {
        pubkey_hex,
        viewer_pubkey_hex,
        is_own_profile,
        is_loading,
        ..HighlighterProfileViewSnapshot::empty()
    };
    current.bump();
}

fn clear_profile_view_snapshot(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.profile_view = HighlighterProfileViewSnapshot::empty();
    current.bump();
}

fn set_profile_view_error(state: &Arc<RwLock<HighlighterAppState>>, message: String) {
    let mut current = state.write();
    current.profile_view.is_loading = false;
    current.profile_view.error_message = Some(message);
    current.bump();
}

fn prepare_open_article_reader(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &mut ArticleReaderRuntime,
    pubkey_hex: String,
    d_tag: String,
    seed: Option<ArticleRecord>,
) -> bool {
    let pubkey_hex = pubkey_hex.trim().to_ascii_lowercase();
    let d_tag = d_tag.trim().to_string();
    if pubkey_hex.is_empty() || d_tag.is_empty() {
        set_article_reader_error(state, "Choose an article to open".into());
        return false;
    }
    let address = format!("30023:{pubkey_hex}:{d_tag}");
    if runtime
        .target
        .as_ref()
        .is_none_or(|target| target.address != address)
    {
        clear_article_reader_runtime(core, runtime);
        runtime.target = Some(ArticleReaderKey {
            pubkey_hex: pubkey_hex.clone(),
            d_tag: d_tag.clone(),
            address: address.clone(),
        });
    }

    let mut current = state.write();
    current.article_reader = HighlighterArticleReaderSnapshot {
        pubkey_hex,
        d_tag,
        address,
        article: seed,
        is_loading: true,
        ..HighlighterArticleReaderSnapshot::empty()
    };
    current.bump();
    true
}

async fn refresh_article_reader(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &mut ArticleReaderRuntime,
    visible_limit: usize,
) {
    let Some(target) = runtime.target.clone() else {
        clear_article_reader_snapshot(state);
        return;
    };

    let mut first_error: Option<String> = None;
    ensure_article_reader_subscriptions(core, runtime, &target, &mut first_error).await;

    let previous_article = state.read().article_reader.article.clone();
    let article = match core
        .get_article(target.pubkey_hex.clone(), target.d_tag.clone())
        .await
    {
        Ok(article) => article.or(previous_article),
        Err(err) => {
            record_first_error(&mut first_error, err.to_string());
            previous_article
        }
    };

    let author_profile = match core.get_user_profile(target.pubkey_hex.clone()).await {
        Ok(profile) => profile,
        Err(err) => {
            record_first_error(&mut first_error, err.to_string());
            None
        }
    };

    let highlight_limit = visible_limit.clamp(1, ARTICLE_READER_HIGHLIGHT_LIMIT) as u32;
    let mut highlights = match core
        .get_highlights_for_article(target.address.clone(), highlight_limit)
        .await
    {
        Ok(highlights) => highlights,
        Err(err) => {
            record_first_error(&mut first_error, err.to_string());
            Vec::new()
        }
    };
    sort_highlights_newest_first(&mut highlights);
    highlights.truncate(highlight_limit as usize);

    let last_published_highlight_id = state
        .read()
        .article_reader
        .last_published_highlight_id
        .clone();
    let snapshot = HighlighterArticleReaderSnapshot {
        pubkey_hex: target.pubkey_hex,
        d_tag: target.d_tag,
        address: target.address,
        article,
        author_profile,
        highlight_count: highlights.len() as u64,
        highlights,
        is_loading: false,
        is_publishing_highlight: false,
        last_published_highlight_id,
        error_message: first_error,
    };

    let mut current = state.write();
    current.article_reader = snapshot;
    current.bump();
}

async fn ensure_article_reader_subscriptions(
    core: &Arc<HighlighterCore>,
    runtime: &mut ArticleReaderRuntime,
    target: &ArticleReaderKey,
    first_error: &mut Option<String>,
) {
    if runtime.article_handle.is_none() {
        match core
            .subscribe_article(target.pubkey_hex.clone(), target.d_tag.clone())
            .await
        {
            Ok(handle) => runtime.article_handle = Some(handle),
            Err(err) => record_first_error(first_error, err.to_string()),
        }
    }
    if runtime.author_profile_handle.is_none() {
        match core.subscribe_user_profile(target.pubkey_hex.clone()).await {
            Ok(handle) => runtime.author_profile_handle = Some(handle),
            Err(err) => record_first_error(first_error, err.to_string()),
        }
    }
}

fn set_share_composer_publishing(
    state: &Arc<RwLock<HighlighterAppState>>,
    group_id: Option<String>,
) {
    let mut current = state.write();
    current.share_composer.is_publishing = true;
    current.share_composer.publishing_group_id = group_id;
    current.share_composer.error_message = None;
    current.share_composer.published_group_id = None;
    current.bump();
}

fn set_share_composer_success(
    state: &Arc<RwLock<HighlighterAppState>>,
    group_id: String,
    toast_message: String,
) {
    let mut current = state.write();
    current.share_composer.is_publishing = false;
    current.share_composer.publishing_group_id = None;
    current.share_composer.error_message = None;
    current.share_composer.published_group_id = Some(group_id);
    current.toast = Some(HighlighterToast {
        kind: HighlighterToastKind::Success,
        message: toast_message,
    });
    current.bump();
}

fn set_share_composer_error(state: &Arc<RwLock<HighlighterAppState>>, message: String) {
    let mut current = state.write();
    current.share_composer.is_publishing = false;
    current.share_composer.publishing_group_id = None;
    current.share_composer.error_message = Some(message);
    current.share_composer.published_group_id = None;
    current.bump();
}

fn clear_share_composer_result(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.share_composer.published_group_id = None;
    current.bump();
}

fn clear_share_composer_error(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.share_composer.error_message = None;
    current.bump();
}

fn article_reader_delta_affects_snapshot(
    runtime: &ArticleReaderRuntime,
    subscription_id: u64,
    address: &str,
) -> bool {
    runtime.article_handle == Some(subscription_id)
        && runtime
            .target
            .as_ref()
            .is_some_and(|target| target.address == address)
}

fn article_reader_profile_delta_affects_snapshot(
    runtime: &ArticleReaderRuntime,
    subscription_id: u64,
    pubkey_hex: &str,
    kind: u32,
) -> bool {
    kind == 0
        && runtime.author_profile_handle == Some(subscription_id)
        && runtime
            .target
            .as_ref()
            .is_some_and(|target| target.pubkey_hex.eq_ignore_ascii_case(pubkey_hex))
}

fn clear_article_reader_runtime(core: &Arc<HighlighterCore>, runtime: &mut ArticleReaderRuntime) {
    if let Some(handle) = runtime.article_handle.take() {
        core.unsubscribe(handle);
    }
    if let Some(handle) = runtime.author_profile_handle.take() {
        core.unsubscribe(handle);
    }
    *runtime = ArticleReaderRuntime::default();
}

fn set_article_reader_loading(state: &Arc<RwLock<HighlighterAppState>>, is_loading: bool) {
    let mut current = state.write();
    current.article_reader.is_loading = is_loading;
    current.article_reader.error_message = None;
    current.bump();
}

fn set_article_reader_publishing(state: &Arc<RwLock<HighlighterAppState>>, is_publishing: bool) {
    let mut current = state.write();
    current.article_reader.is_publishing_highlight = is_publishing;
    current.article_reader.error_message = None;
    current.bump();
}

fn clear_article_reader_snapshot(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.article_reader = HighlighterArticleReaderSnapshot::empty();
    current.bump();
}

fn set_article_reader_error(state: &Arc<RwLock<HighlighterAppState>>, message: String) {
    let mut current = state.write();
    current.article_reader.is_loading = false;
    current.article_reader.is_publishing_highlight = false;
    current.article_reader.error_message = Some(message);
    current.bump();
}

fn set_article_reader_publish_error(state: &Arc<RwLock<HighlighterAppState>>, message: String) {
    let mut current = state.write();
    current.article_reader.is_publishing_highlight = false;
    current.article_reader.error_message = Some(message.clone());
    current.toast = Some(HighlighterToast {
        kind: HighlighterToastKind::Error,
        message,
    });
    current.bump();
}

fn prepare_open_room_detail(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &mut RoomDetailRuntime,
    group_id: String,
) -> bool {
    let group_id = group_id.trim().to_string();
    if group_id.is_empty() {
        clear_room_detail_snapshot(state);
        clear_room_detail_runtime(core, runtime);
        return false;
    }

    let changed = runtime.group_id.as_deref() != Some(group_id.as_str());
    if changed {
        if let Some(handle) = runtime.handle.take() {
            core.unsubscribe(handle);
        }
        if let Some(handle) = runtime.discussions_handle.take() {
            core.unsubscribe(handle);
        }
        if let Some(handle) = runtime.chat_handle.take() {
            core.unsubscribe(handle);
        }
        runtime.chat_loaded_limit = ROOM_DETAIL_CHAT_PAGE_SIZE;
        runtime.group_id = Some(group_id.clone());
    }

    let mut current = state.write();
    current.room_detail.group_id = group_id;
    current.room_detail.is_loading = true;
    current.room_detail.error_message = None;
    current.room_detail.discussion_error_message = None;
    current.room_detail.chat_error_message = None;
    current.bump();
    true
}

async fn refresh_room_detail(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &mut RoomDetailRuntime,
    visible_limit: usize,
) {
    let Some(group_id) = runtime.group_id.clone() else {
        clear_room_detail_snapshot(state);
        return;
    };

    let mut first_error = None;
    ensure_room_detail_subscriptions(core, runtime, &mut first_error).await;

    let artifact_limit = visible_limit
        .clamp(1, ROOM_DETAIL_ARTIFACT_LIMIT as usize)
        .try_into()
        .unwrap_or(ROOM_DETAIL_ARTIFACT_LIMIT);
    let highlight_limit = visible_limit
        .max(ROOM_DETAIL_HIGHLIGHT_LIMIT as usize)
        .try_into()
        .unwrap_or(ROOM_DETAIL_HIGHLIGHT_LIMIT);
    let discussion_limit = visible_limit
        .max(ROOM_DETAIL_DISCUSSION_LIMIT as usize)
        .try_into()
        .unwrap_or(ROOM_DETAIL_DISCUSSION_LIMIT);
    let chat_limit = runtime
        .chat_loaded_limit
        .clamp(ROOM_DETAIL_CHAT_PAGE_SIZE, ROOM_DETAIL_CHAT_MAX_LIMIT);

    let artifacts = core
        .get_artifacts(group_id.clone(), artifact_limit)
        .await
        .map_err(|err| format!("Couldn't load room artifacts: {err}"));
    let highlights = core
        .get_highlights(group_id.clone(), highlight_limit)
        .await
        .map_err(|err| format!("Couldn't load room highlights: {err}"));
    let discussions = core
        .get_discussions(group_id.clone(), discussion_limit)
        .await
        .map_err(|err| format!("Couldn't load room discussions: {err}"));
    let chat_messages = core
        .get_chat_messages(group_id.clone(), chat_limit)
        .await
        .map_err(|err| format!("Couldn't load room chat: {err}"));

    match (artifacts, highlights, discussions, chat_messages) {
        (Ok(artifacts), Ok(highlights), Ok(discussions), Ok(chat_messages)) => {
            let (highlight_buckets, comment_buckets) =
                build_room_reference_buckets(core, &artifacts, visible_limit).await;
            let mut current = state.write();
            current.room_detail.group_id = group_id;
            current.room_detail.artifact_count = artifacts.len() as u64;
            current.room_detail.artifacts = artifacts;
            current.room_detail.highlight_count = highlights.len() as u64;
            current.room_detail.highlights = highlights;
            current.room_detail.discussion_count = discussions.len() as u64;
            current.room_detail.discussions = discussions;
            current.room_detail.chat_message_count = chat_messages.len() as u64;
            current.room_detail.chat_has_more = chat_messages.len() >= chat_limit as usize
                && chat_limit < ROOM_DETAIL_CHAT_MAX_LIMIT;
            current.room_detail.chat_messages = chat_messages;
            current.room_detail.is_chat_loading_more = false;
            current.room_detail.reference_highlight_count = highlight_buckets.len() as u64;
            current.room_detail.highlights_by_reference = highlight_buckets;
            current.room_detail.reference_comment_count = comment_buckets.len() as u64;
            current.room_detail.comments_by_reference = comment_buckets;
            current.room_detail.is_loading = false;
            current.room_detail.error_message = first_error;
            current.bump();
        }
        (artifacts, highlights, discussions, chat_messages) => {
            if let Err(message) = artifacts {
                record_first_error(&mut first_error, message);
            }
            if let Err(message) = highlights {
                record_first_error(&mut first_error, message);
            }
            if let Err(message) = discussions {
                record_first_error(&mut first_error, message);
            }
            if let Err(message) = chat_messages {
                record_first_error(&mut first_error, message);
            }
            set_room_detail_error(
                state,
                first_error.unwrap_or_else(|| "Couldn't load room".into()),
            );
        }
    }
}

async fn ensure_room_detail_subscriptions(
    core: &Arc<HighlighterCore>,
    runtime: &mut RoomDetailRuntime,
    first_error: &mut Option<String>,
) {
    let Some(group_id) = runtime.group_id.clone() else {
        return;
    };
    if runtime.handle.is_none() {
        match core.subscribe_room(group_id.clone()).await {
            Ok(handle) => runtime.handle = Some(handle),
            Err(err) => {
                record_first_error(first_error, format!("Couldn't subscribe to room: {err}"))
            }
        }
    }
    if runtime.discussions_handle.is_none() {
        match core.subscribe_room_discussions(group_id.clone()).await {
            Ok(handle) => runtime.discussions_handle = Some(handle),
            Err(err) => record_first_error(
                first_error,
                format!("Couldn't subscribe to discussions: {err}"),
            ),
        }
    }
    if runtime.chat_handle.is_none() {
        match core.subscribe_room_chat(group_id).await {
            Ok(handle) => runtime.chat_handle = Some(handle),
            Err(err) => {
                record_first_error(first_error, format!("Couldn't subscribe to chat: {err}"))
            }
        }
    }
}

async fn build_room_reference_buckets(
    core: &Arc<HighlighterCore>,
    artifacts: &[ArtifactRecord],
    visible_limit: usize,
) -> (
    Vec<HighlighterReferenceHighlightBucket>,
    Vec<HighlighterReferenceCommentBucket>,
) {
    let max_targets = visible_limit.clamp(1, ROOM_DETAIL_ARTIFACT_LIMIT as usize);
    let per_reference_limit = ROOM_DETAIL_REFERENCE_LIMIT;
    let mut highlight_buckets = Vec::new();
    let mut comment_buckets = Vec::new();

    for target in artifacts
        .iter()
        .filter_map(room_reference_target)
        .take(max_targets)
    {
        if let Ok(highlights) = core
            .get_highlights_for_reference(
                target.lowercase_tag.clone(),
                target.value.clone(),
                per_reference_limit,
            )
            .await
        {
            if !highlights.is_empty() {
                highlight_buckets.push(HighlighterReferenceHighlightBucket {
                    key: target.lowercase_key(),
                    tag_name: target.lowercase_tag.clone(),
                    tag_value: target.value.clone(),
                    highlight_count: highlights.len() as u64,
                    highlights,
                });
            }
        }

        if let Ok(comments) = core
            .get_comments_for_reference(
                target.uppercase_tag.clone(),
                target.value.clone(),
                per_reference_limit,
            )
            .await
        {
            if !comments.is_empty() {
                comment_buckets.push(HighlighterReferenceCommentBucket {
                    key: target.uppercase_key(),
                    tag_name: target.uppercase_tag,
                    tag_value: target.value,
                    comment_count: comments.len() as u64,
                    comments,
                });
            }
        }
    }

    (highlight_buckets, comment_buckets)
}

#[derive(Clone)]
struct RoomReferenceTarget {
    lowercase_tag: String,
    uppercase_tag: String,
    value: String,
}

impl RoomReferenceTarget {
    fn lowercase_key(&self) -> String {
        format!("{}:{}", self.lowercase_tag, self.value)
    }

    fn uppercase_key(&self) -> String {
        format!("{}:{}", self.uppercase_tag, self.value)
    }
}

fn room_reference_target(artifact: &ArtifactRecord) -> Option<RoomReferenceTarget> {
    let preview = &artifact.preview;
    if !preview.reference_tag_name.trim().is_empty()
        && !preview.reference_tag_value.trim().is_empty()
    {
        let tag = preview.reference_tag_name.trim();
        return Some(RoomReferenceTarget {
            lowercase_tag: tag.to_ascii_lowercase(),
            uppercase_tag: tag.to_ascii_uppercase(),
            value: preview.reference_tag_value.trim().to_string(),
        });
    }
    if !preview.highlight_tag_name.trim().is_empty()
        && !preview.highlight_tag_value.trim().is_empty()
    {
        let tag = preview.highlight_tag_name.trim();
        return Some(RoomReferenceTarget {
            lowercase_tag: tag.to_ascii_lowercase(),
            uppercase_tag: tag.to_ascii_uppercase(),
            value: preview.highlight_tag_value.trim().to_string(),
        });
    }
    None
}

fn room_detail_delta_affects_snapshot(
    runtime: &RoomDetailRuntime,
    subscription_id: u64,
    group_id: &str,
) -> bool {
    (runtime.handle == Some(subscription_id)
        || runtime.discussions_handle == Some(subscription_id)
        || runtime.chat_handle == Some(subscription_id))
        && runtime
            .group_id
            .as_ref()
            .is_some_and(|current| current == group_id)
}

fn clear_room_detail_runtime(core: &Arc<HighlighterCore>, runtime: &mut RoomDetailRuntime) {
    if let Some(handle) = runtime.handle.take() {
        core.unsubscribe(handle);
    }
    if let Some(handle) = runtime.discussions_handle.take() {
        core.unsubscribe(handle);
    }
    if let Some(handle) = runtime.chat_handle.take() {
        core.unsubscribe(handle);
    }
    *runtime = RoomDetailRuntime::default();
}

fn set_room_detail_loading(state: &Arc<RwLock<HighlighterAppState>>, is_loading: bool) {
    let mut current = state.write();
    current.room_detail.is_loading = is_loading;
    if is_loading {
        current.room_detail.error_message = None;
    }
    current.bump();
}

fn set_room_detail_error(state: &Arc<RwLock<HighlighterAppState>>, message: String) {
    let mut current = state.write();
    current.room_detail.is_loading = false;
    current.room_detail.is_publishing_discussion = false;
    current.room_detail.is_chat_loading_more = false;
    current.room_detail.error_message = Some(message);
    current.bump();
}

fn set_room_discussion_publishing(state: &Arc<RwLock<HighlighterAppState>>, is_publishing: bool) {
    let mut current = state.write();
    current.room_detail.is_publishing_discussion = is_publishing;
    if is_publishing {
        current.room_detail.discussion_error_message = None;
        current.room_detail.last_published_discussion_id = None;
    }
    current.bump();
}

fn set_room_discussion_error(state: &Arc<RwLock<HighlighterAppState>>, message: String) {
    let mut current = state.write();
    current.room_detail.is_publishing_discussion = false;
    current.room_detail.discussion_error_message = Some(message);
    current.bump();
}

fn clear_room_discussion_error(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.room_detail.discussion_error_message = None;
    current.bump();
}

fn prepare_load_more_room_chat(
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &mut RoomDetailRuntime,
) -> bool {
    if runtime.group_id.is_none() {
        return false;
    }

    let mut current = state.write();
    if current.room_detail.is_chat_loading_more
        || !current.room_detail.chat_has_more
        || runtime.chat_loaded_limit >= ROOM_DETAIL_CHAT_MAX_LIMIT
    {
        return false;
    }

    runtime.chat_loaded_limit = runtime
        .chat_loaded_limit
        .saturating_add(ROOM_DETAIL_CHAT_PAGE_SIZE)
        .min(ROOM_DETAIL_CHAT_MAX_LIMIT);
    current.room_detail.is_chat_loading_more = true;
    current.room_detail.chat_error_message = None;
    current.bump();
    true
}

fn set_room_chat_sending(state: &Arc<RwLock<HighlighterAppState>>, is_sending: bool) {
    let mut current = state.write();
    current.room_detail.is_sending_chat_message = is_sending;
    if is_sending {
        current.room_detail.chat_error_message = None;
    }
    current.bump();
}

fn set_room_chat_error(state: &Arc<RwLock<HighlighterAppState>>, message: String) {
    let mut current = state.write();
    current.room_detail.is_sending_chat_message = false;
    current.room_detail.chat_error_message = Some(message);
    current.bump();
}

fn clear_room_chat_error(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.room_detail.chat_error_message = None;
    current.bump();
}

fn clear_room_detail_snapshot(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.room_detail = HighlighterRoomDetailSnapshot::empty();
    current.bump();
}

fn article_as_artifact(article: &ArticleRecord, address: &str) -> crate::models::ArtifactRecord {
    let preview = ArtifactPreview {
        id: article.identifier.clone(),
        url: String::new(),
        title: article.title.clone(),
        author: String::new(),
        image: article.image.clone(),
        description: article.summary.clone(),
        source: "article".into(),
        domain: String::new(),
        catalog_id: String::new(),
        catalog_kind: String::new(),
        podcast_guid: String::new(),
        podcast_item_guid: String::new(),
        podcast_show_title: String::new(),
        audio_url: String::new(),
        audio_preview_url: String::new(),
        transcript_url: String::new(),
        feed_url: String::new(),
        published_at: article
            .published_at
            .map(|ts| ts.to_string())
            .unwrap_or_default(),
        duration_seconds: None,
        reference_tag_name: "a".into(),
        reference_tag_value: address.into(),
        reference_kind: "30023".into(),
        highlight_tag_name: "a".into(),
        highlight_tag_value: address.into(),
        highlight_reference_key: format!("a:{address}"),
        chapters: Vec::new(),
    };
    crate::models::ArtifactRecord {
        preview,
        group_id: String::new(),
        share_event_id: String::new(),
        pubkey: article.pubkey.clone(),
        created_at: article.created_at,
        note: String::new(),
    }
}

fn sort_highlights_newest_first(highlights: &mut [HighlightRecord]) {
    highlights.sort_by(|a, b| {
        b.created_at
            .unwrap_or(0)
            .cmp(&a.created_at.unwrap_or(0))
            .then_with(|| a.event_id.cmp(&b.event_id))
    });
}

async fn hydrate_bookmarks(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    visible_limit: usize,
) {
    let mut addresses = core
        .get_bookmarked_article_addresses()
        .await
        .unwrap_or_default();
    addresses.sort();
    let total = addresses.len() as u64;
    let visible = addresses.into_iter().take(visible_limit).collect();
    let mut current = state.write();
    current.chrome.bookmarked_article_addresses = visible;
    current.chrome.bookmarked_article_address_count = total;
    current.bump();
}

async fn refresh_bookmarks_library(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &mut BookmarkRuntime,
    visible_limit: usize,
) {
    if !runtime.library_open {
        return;
    }

    let mut first_error: Option<String> = None;
    ensure_bookmark_subscriptions(core, state, runtime, &mut first_error).await;

    let article_limit = visible_limit.clamp(1, BOOKMARK_ARTICLE_LIMIT);
    let collection_limit = visible_limit.clamp(1, BOOKMARK_COLLECTION_LIMIT);
    let web_limit = visible_limit.clamp(1, BOOKMARK_WEB_LIMIT);

    let addresses = match core.get_bookmarked_article_addresses().await {
        Ok(addresses) => addresses,
        Err(err) => {
            record_first_error(&mut first_error, err.to_string());
            Vec::new()
        }
    };
    let articles = resolve_articles_for_addresses(core, addresses, article_limit).await;

    let my_bookmark_sets = match core.get_my_bookmark_sets().await {
        Ok(records) => records,
        Err(err) => {
            record_first_error(&mut first_error, err.to_string());
            Vec::new()
        }
    };
    let my_curation_sets = match core.get_my_curation_sets().await {
        Ok(records) => records,
        Err(err) => {
            record_first_error(&mut first_error, err.to_string());
            Vec::new()
        }
    };
    let web_bookmarks = match core.get_my_web_bookmarks().await {
        Ok(records) => records,
        Err(err) => {
            record_first_error(&mut first_error, err.to_string());
            Vec::new()
        }
    };
    let following_curation_sets = match core.get_following_curation_sets().await {
        Ok(records) => filter_resolvable_curation_sets(core, records).await,
        Err(err) => {
            record_first_error(&mut first_error, err.to_string());
            Vec::new()
        }
    };

    let article_count = articles.len() as u64;
    let my_bookmark_set_count = my_bookmark_sets.len() as u64;
    let my_curation_set_count = my_curation_sets.len() as u64;
    let web_bookmark_count = web_bookmarks.len() as u64;
    let following_curation_set_count = following_curation_sets.len() as u64;

    {
        let mut current = state.write();
        current.bookmarks.articles = articles.into_iter().take(article_limit).collect();
        current.bookmarks.article_count = article_count;
        current.bookmarks.my_bookmark_sets = my_bookmark_sets
            .into_iter()
            .take(collection_limit)
            .collect();
        current.bookmarks.my_bookmark_set_count = my_bookmark_set_count;
        current.bookmarks.my_curation_sets = my_curation_sets
            .into_iter()
            .take(collection_limit)
            .collect();
        current.bookmarks.my_curation_set_count = my_curation_set_count;
        current.bookmarks.web_bookmarks = web_bookmarks.into_iter().take(web_limit).collect();
        current.bookmarks.web_bookmark_count = web_bookmark_count;
        current.bookmarks.following_curation_sets = following_curation_sets
            .into_iter()
            .take(collection_limit)
            .collect();
        current.bookmarks.following_curation_set_count = following_curation_set_count;
        current.bookmarks.is_loading = false;
        current.bookmarks.error_message = first_error;
        current.bump();
    }

    if runtime.selected_collection.is_some() {
        refresh_bookmark_collection_detail(core, state, runtime, visible_limit).await;
    }
}

async fn refresh_bookmark_collection_detail(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &mut BookmarkRuntime,
    visible_limit: usize,
) {
    let Some(key) = runtime.selected_collection.clone() else {
        let mut current = state.write();
        current.bookmarks.selected_collection =
            HighlighterBookmarkCollectionDetailSnapshot::empty();
        current.bump();
        return;
    };

    let Some(collection) = find_bookmark_collection(state, &key) else {
        let mut current = state.write();
        current.bookmarks.selected_collection = HighlighterBookmarkCollectionDetailSnapshot {
            collection: None,
            articles: Vec::new(),
            article_count: 0,
            has_note_items: false,
            is_loading: false,
            error_message: Some("Collection is no longer available".into()),
        };
        current.bump();
        return;
    };

    let article_limit = visible_limit.clamp(1, BOOKMARK_DETAIL_ARTICLE_LIMIT);
    let articles =
        resolve_articles_for_addresses(core, collection.article_addresses.clone(), article_limit)
            .await;
    let article_count = articles.len() as u64;
    let has_note_items = !collection.note_ids.is_empty();

    let mut current = state.write();
    current.bookmarks.selected_collection = HighlighterBookmarkCollectionDetailSnapshot {
        collection: Some(collection),
        articles,
        article_count,
        has_note_items,
        is_loading: false,
        error_message: None,
    };
    current.bump();
}

async fn refresh_curation_menu(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &mut BookmarkRuntime,
) {
    let Some(article_address) = runtime.curation_menu_article_address.clone() else {
        clear_curation_menu_snapshot(state);
        return;
    };

    let mut first_error = None;
    ensure_bookmark_subscriptions(core, state, runtime, &mut first_error).await;

    let curation_sets = match core.get_my_curation_sets().await {
        Ok(records) => records,
        Err(err) => {
            record_first_error(&mut first_error, err.to_string());
            Vec::new()
        }
    };
    let curation_set_count = curation_sets.len() as u64;

    let mut current = state.write();
    current.curation_menu.article_address = article_address;
    current.curation_menu.curation_sets = curation_sets
        .into_iter()
        .take(CURATION_MENU_SET_LIMIT)
        .collect();
    current.curation_menu.curation_set_count = curation_set_count;
    current.curation_menu.is_loading = false;
    current.curation_menu.error_message = first_error;
    current.bump();
}

async fn refresh_bookmark_surfaces(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &mut BookmarkRuntime,
    visible_limit: usize,
) {
    if runtime.library_open {
        refresh_bookmarks_library(core, state, runtime, visible_limit).await;
    }
    if runtime.curation_menu_article_address.is_some() {
        refresh_curation_menu(core, state, runtime).await;
    }
}

async fn ensure_bookmark_subscriptions(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    runtime: &mut BookmarkRuntime,
    first_error: &mut Option<String>,
) {
    if core.current_user().is_none() {
        return;
    }

    let needs_sets = runtime.library_open || runtime.curation_menu_article_address.is_some();
    if needs_sets && runtime.sets_handle.is_none() {
        match core.subscribe_bookmark_sets().await {
            Ok(handle) => runtime.sets_handle = Some(handle),
            Err(err) => record_first_error(first_error, err.to_string()),
        }
    }

    if runtime.library_open && runtime.following_handle.is_none() {
        match core.subscribe_following_curation_sets().await {
            Ok(handle) => runtime.following_handle = Some(handle),
            Err(err) => record_first_error(first_error, err.to_string()),
        }
    }

    if runtime.library_open && runtime.web_handle.is_none() {
        match core.subscribe_web_bookmarks().await {
            Ok(handle) => runtime.web_handle = Some(handle),
            Err(err) => record_first_error(first_error, err.to_string()),
        }
    }

    if first_error.is_some() {
        let mut current = state.write();
        if runtime.library_open {
            current.bookmarks.error_message = first_error.clone();
        }
        if runtime.curation_menu_article_address.is_some() {
            current.curation_menu.error_message = first_error.clone();
        }
        current.bump();
    }
}

fn trim_bookmark_subscriptions(core: &Arc<HighlighterCore>, runtime: &mut BookmarkRuntime) {
    if !runtime.library_open {
        if let Some(handle) = runtime.following_handle.take() {
            core.unsubscribe(handle);
        }
        if let Some(handle) = runtime.web_handle.take() {
            core.unsubscribe(handle);
        }
    }
    if !runtime.library_open && runtime.curation_menu_article_address.is_none() {
        if let Some(handle) = runtime.sets_handle.take() {
            core.unsubscribe(handle);
        }
    }
}

fn clear_bookmark_runtime(core: &Arc<HighlighterCore>, runtime: &mut BookmarkRuntime) {
    if let Some(handle) = runtime.sets_handle.take() {
        core.unsubscribe(handle);
    }
    if let Some(handle) = runtime.following_handle.take() {
        core.unsubscribe(handle);
    }
    if let Some(handle) = runtime.web_handle.take() {
        core.unsubscribe(handle);
    }
    *runtime = BookmarkRuntime::default();
}

fn set_bookmarks_loading(state: &Arc<RwLock<HighlighterAppState>>, is_loading: bool) {
    let mut current = state.write();
    current.bookmarks.is_loading = is_loading;
    current.bookmarks.error_message = None;
    current.bump();
}

fn clear_bookmarks_snapshot(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.bookmarks = HighlighterBookmarksSnapshot::empty();
    current.bump();
}

fn set_bookmark_collection_loading(state: &Arc<RwLock<HighlighterAppState>>, is_loading: bool) {
    let mut current = state.write();
    current.bookmarks.selected_collection.is_loading = is_loading;
    current.bookmarks.selected_collection.error_message = None;
    current.bump();
}

fn set_curation_menu_loading(
    state: &Arc<RwLock<HighlighterAppState>>,
    article_address: String,
    is_loading: bool,
) {
    let mut current = state.write();
    current.curation_menu.article_address = article_address;
    current.curation_menu.is_loading = is_loading;
    current.curation_menu.error_message = None;
    current.bump();
}

fn clear_curation_menu_snapshot(state: &Arc<RwLock<HighlighterAppState>>) {
    let mut current = state.write();
    current.curation_menu = HighlighterCurationMenuSnapshot::empty();
    current.bump();
}

fn set_curation_menu_saving(state: &Arc<RwLock<HighlighterAppState>>, saving: bool) {
    let mut current = state.write();
    current.curation_menu.is_saving = saving;
    current.bump();
}

fn set_curation_menu_error(state: &Arc<RwLock<HighlighterAppState>>, message: String) {
    let mut current = state.write();
    current.curation_menu.is_loading = false;
    current.curation_menu.error_message = Some(message);
    current.bump();
}

async fn filter_resolvable_curation_sets(
    core: &Arc<HighlighterCore>,
    sets: Vec<BookmarkSetRecord>,
) -> Vec<BookmarkSetRecord> {
    let mut out = Vec::with_capacity(sets.len());
    for set in sets {
        if curation_set_has_resolvable_item(core, &set).await {
            out.push(set);
        }
    }
    out
}

async fn curation_set_has_resolvable_item(
    core: &Arc<HighlighterCore>,
    set: &BookmarkSetRecord,
) -> bool {
    if !set.note_ids.is_empty() {
        return true;
    }
    for address in &set.article_addresses {
        if let Some((pubkey_hex, d_tag)) = parse_article_address(address) {
            if matches!(
                core.get_article(pubkey_hex.to_string(), d_tag.to_string())
                    .await,
                Ok(Some(_))
            ) {
                return true;
            }
        }
    }
    false
}

async fn resolve_articles_for_addresses(
    core: &Arc<HighlighterCore>,
    addresses: Vec<String>,
    limit: usize,
) -> Vec<ArticleRecord> {
    let mut articles = Vec::new();
    for address in addresses {
        if articles.len() >= limit {
            break;
        }
        let Some((pubkey_hex, d_tag)) = parse_article_address(&address) else {
            continue;
        };
        if let Ok(Some(article)) = core
            .get_article(pubkey_hex.to_string(), d_tag.to_string())
            .await
        {
            articles.push(article);
        }
    }
    sort_articles_newest_first(&mut articles);
    articles
}

fn sort_articles_newest_first(articles: &mut [ArticleRecord]) {
    articles.sort_by_key(|article| std::cmp::Reverse(article_sort_key(article)));
}

fn article_sort_key(article: &ArticleRecord) -> u64 {
    article.published_at.or(article.created_at).unwrap_or(0)
}

fn normalize_article_address(address: &str) -> Option<String> {
    parse_article_address(address).map(|(pubkey, d_tag)| format!("30023:{pubkey}:{d_tag}"))
}

fn parse_article_address(address: &str) -> Option<(&str, &str)> {
    let mut parts = address.trim().splitn(3, ':');
    let kind = parts.next()?;
    let pubkey = parts.next()?.trim();
    let d_tag = parts.next()?.trim();
    if kind != "30023" || pubkey.is_empty() || d_tag.is_empty() {
        return None;
    }
    Some((pubkey, d_tag))
}

fn find_bookmark_collection(
    state: &Arc<RwLock<HighlighterAppState>>,
    key: &BookmarkCollectionKey,
) -> Option<BookmarkSetRecord> {
    let current = state.read();
    current
        .bookmarks
        .my_bookmark_sets
        .iter()
        .chain(current.bookmarks.my_curation_sets.iter())
        .chain(current.bookmarks.following_curation_sets.iter())
        .find(|record| {
            record.pubkey == key.pubkey_hex && record.id == key.d_tag && record.kind == key.kind
        })
        .cloned()
}

fn toggle_onboarding_interest(state: &Arc<RwLock<HighlighterAppState>>, interest_id: String) {
    let interest_id = interest_id.trim().to_ascii_lowercase();
    if !ONBOARDING_INTERESTS
        .iter()
        .any(|interest| interest.id == interest_id)
    {
        return;
    }

    let current_selection = {
        let current = state.read();
        selected_onboarding_ids(&current.onboarding)
    };
    let mut next_selection = current_selection;
    if !next_selection.remove(&interest_id) {
        next_selection.insert(interest_id);
    }

    let mut current = state.write();
    let is_complete = current.onboarding.is_complete;
    let is_finishing = current.onboarding.is_finishing;
    current.onboarding = onboarding_snapshot(is_complete, next_selection, is_finishing);
    current.bump();
}

fn complete_onboarding(
    core: &Arc<HighlighterCore>,
    state: &Arc<RwLock<HighlighterAppState>>,
    local_state_path: &Path,
    ops: &mut OpRunner,
) {
    let selected = {
        let current = state.read();
        selected_onboarding_ids(&current.onboarding)
    };

    if selected.len() < MIN_ONBOARDING_INTERESTS as usize {
        let mut current = state.write();
        current.toast = Some(HighlighterToast {
            kind: HighlighterToastKind::Error,
            message: format!("Choose {} interests", MIN_ONBOARDING_INTERESTS),
        });
        current.bump();
        return;
    }

    if core.current_user().is_none() {
        let mut current = state.write();
        current.toast = Some(HighlighterToast {
            kind: HighlighterToastKind::Error,
            message: "Sign in before finishing onboarding".into(),
        });
        current.bump();
        return;
    }

    let target_pubkeys = onboarding_pubkeys_for(&selected);

    save_local_state(local_state_path, &local_state_for_snapshot(state, true));

    {
        let mut current = state.write();
        current.onboarding = onboarding_snapshot(true, selected, false);
        current.toast = Some(HighlighterToast {
            kind: HighlighterToastKind::Success,
            message: "Welcome to Highlighter".into(),
        });
        current.bump();
    }

    if target_pubkeys.is_empty() {
        return;
    }

    // Follow publish runs off-actor via OpRunner (folds the bespoke worker).
    let core = core.clone();
    ops.submit_op(
        OpDomain::OnboardingFollows,
        OP_DEADLINE_NETWORK,
        // Mirror the prior fallback: a timed-out batch reports a failure so the
        // resolver surfaces "save your interests" rather than success.
        OpOutcome::OnboardingFollows { failures: 1 },
        async move {
            let mut failures = 0usize;
            for pubkey in target_pubkeys {
                if let Err(err) = core.set_follow(pubkey, true).await {
                    failures = failures.saturating_add(1);
                    tracing::warn!(error = %err, "onboarding follow failed");
                }
            }
            OpOutcome::OnboardingFollows { failures }
        },
    );
}

fn set_bootstrapping(state: &Arc<RwLock<HighlighterAppState>>, is_bootstrapping: bool) {
    let mut current = state.write();
    current.is_bootstrapping = is_bootstrapping;
    current.bump();
}

fn set_signing_in(state: &Arc<RwLock<HighlighterAppState>>, is_signing_in: bool) {
    let mut current = state.write();
    current.auth.is_signing_in = is_signing_in;
    current.bump();
}

fn set_signed_in_user(state: &Arc<RwLock<HighlighterAppState>>, user: CurrentUser) {
    let mut current = state.write();
    current.chrome.current_user = Some(user);
    current.auth.is_signing_in = false;
    current.bump();
}

fn set_toast(state: &Arc<RwLock<HighlighterAppState>>, toast: Option<HighlighterToast>) {
    let mut current = state.write();
    current.toast = toast;
    current.bump();
}

fn emit(
    state: &Arc<RwLock<HighlighterAppState>>,
    reconciler: &Arc<RwLock<Option<Arc<dyn HighlighterAppReconciler>>>>,
) {
    if let Some(reconciler) = reconciler.read().clone() {
        reconciler.on_state(state.read().clone());
    }
}

fn emit_session_credential(
    reconciler: &Arc<RwLock<Option<Arc<dyn HighlighterAppReconciler>>>>,
    credential: HighlighterSessionCredential,
) {
    if let Some(reconciler) = reconciler.read().clone() {
        reconciler.on_persist_session_credential(credential);
    }
}

fn emit_clear_session_credentials(
    reconciler: &Arc<RwLock<Option<Arc<dyn HighlighterAppReconciler>>>>,
) {
    if let Some(reconciler) = reconciler.read().clone() {
        reconciler.on_clear_session_credentials();
    }
}

fn emit_open_external_url(
    reconciler: &Arc<RwLock<Option<Arc<dyn HighlighterAppReconciler>>>>,
    url: String,
) {
    if let Some(reconciler) = reconciler.read().clone() {
        reconciler.on_open_external_url(url);
    }
}

fn map_connection_state(status: RelayStatus) -> HighlighterConnectionState {
    match status {
        RelayStatus::Connecting => HighlighterConnectionState::Connecting,
        RelayStatus::Connected => HighlighterConnectionState::Online,
        RelayStatus::Disconnected | RelayStatus::Terminated | RelayStatus::Banned => {
            HighlighterConnectionState::Offline
        }
    }
}

fn log_send_failure<T>(err: TrySendError<T>) {
    match err {
        TrySendError::Full(_) => tracing::warn!("highlighter NMP action queue is full"),
        TrySendError::Disconnected(_) => tracing::warn!("highlighter NMP actor is stopped"),
    }
}

struct PendingJoin {
    group_id: String,
    room_name: String,
}

#[derive(Default)]
struct AppScopeSubscriptions {
    joined_communities: Option<u64>,
    bookmarks: Option<u64>,
    initialized_blossom_defaults_for_pubkey: Option<String>,
    initializing_blossom_defaults_for_pubkey: Option<String>,
}

#[derive(Default)]
struct SearchRuntime {
    is_open: bool,
    generation: u64,
    local_running_query: Option<String>,
    active_relay_query: Option<String>,
    relay_handle: Option<u64>,
}

struct SearchResults {
    highlights: Vec<HighlightRecord>,
    articles: Vec<ArticleRecord>,
    communities: Vec<CommunitySummary>,
    profiles: Vec<ProfileMetadata>,
}

#[derive(Default)]
struct HomeFeedRuntime {
    is_open: bool,
    reads_handle: Option<u64>,
    highlights_handle: Option<u64>,
}

#[derive(Clone)]
struct BookmarkCollectionKey {
    pubkey_hex: String,
    d_tag: String,
    kind: u32,
}

#[derive(Default)]
struct BookmarkRuntime {
    library_open: bool,
    curation_menu_article_address: Option<String>,
    selected_collection: Option<BookmarkCollectionKey>,
    sets_handle: Option<u64>,
    following_handle: Option<u64>,
    web_handle: Option<u64>,
}

#[derive(Default)]
struct ProfileViewRuntime {
    pubkey_hex: Option<String>,
    target_handle: Option<u64>,
    viewer_follow_handle: Option<u64>,
    viewer_follow_pubkey_hex: Option<String>,
}

#[derive(Clone)]
struct ArticleReaderKey {
    pubkey_hex: String,
    d_tag: String,
    address: String,
}

#[derive(Default)]
struct ArticleReaderRuntime {
    target: Option<ArticleReaderKey>,
    article_handle: Option<u64>,
    author_profile_handle: Option<u64>,
}

struct RoomDetailRuntime {
    group_id: Option<String>,
    handle: Option<u64>,
    discussions_handle: Option<u64>,
    chat_handle: Option<u64>,
    chat_loaded_limit: u32,
}

impl Default for RoomDetailRuntime {
    fn default() -> Self {
        Self {
            group_id: None,
            handle: None,
            discussions_handle: None,
            chat_handle: None,
            chat_loaded_limit: ROOM_DETAIL_CHAT_PAGE_SIZE,
        }
    }
}

#[derive(Default)]
struct RoomExplorerRuntime {
    is_open: bool,
    is_browse_open: bool,
}

#[derive(Default)]
struct CreateAccountRuntime {
    username_generation: u64,
    create_generation: u64,
}

struct CreateAccountRequest {
    generation: u64,
    display_name: String,
    username: String,
    identifier: String,
    domain: String,
}

struct CreateAccountOutcome {
    user: CurrentUser,
    nsec: String,
    warning: Option<String>,
}

struct Nip05Availability {
    available: bool,
    identifier: String,
    domain: String,
}

#[derive(serde::Deserialize)]
struct Nip05AvailabilityResponse {
    available: bool,
    identifier: String,
}

#[derive(serde::Deserialize)]
struct Nip05ErrorResponse {
    error: String,
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
struct LocalAppState {
    onboarding_complete: bool,
    #[serde(default)]
    recent_searches: Vec<String>,
    #[serde(default)]
    network_wifi_only: bool,
    #[serde(default)]
    whats_new_last_seen_at: Option<String>,
}

#[derive(serde::Deserialize)]
struct WhatsNewPayload {
    schema_version: u32,
    entries: Vec<WhatsNewEntryJson>,
}

#[derive(serde::Deserialize)]
struct WhatsNewEntryJson {
    shipped_at: String,
    lines: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct OnboardingInterestDef {
    id: &'static str,
    emoji: &'static str,
    label: &'static str,
    pubkeys: &'static [&'static str],
}

const MIN_ONBOARDING_INTERESTS: u32 = 3;

const JACK_PUBKEY: &str = "82341f882b6eabcd2ba7f1ef90aad961cf074af15b9ef44a09f9d2a8fbfbe6a2";
const FIATJAF_PUBKEY: &str = "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d";

const ONBOARDING_INTERESTS: &[OnboardingInterestDef] = &[
    OnboardingInterestDef {
        id: "philosophy",
        emoji: "\u{1f9e0}",
        label: "Philosophy",
        pubkeys: &[JACK_PUBKEY],
    },
    OnboardingInterestDef {
        id: "science_fiction",
        emoji: "\u{1f680}",
        label: "Science Fiction",
        pubkeys: &[JACK_PUBKEY],
    },
    OnboardingInterestDef {
        id: "technology",
        emoji: "\u{1f4bb}",
        label: "Technology",
        pubkeys: &[FIATJAF_PUBKEY, JACK_PUBKEY],
    },
    OnboardingInterestDef {
        id: "history",
        emoji: "\u{1f4dc}",
        label: "History",
        pubkeys: &[JACK_PUBKEY],
    },
    OnboardingInterestDef {
        id: "economics",
        emoji: "\u{1f4c8}",
        label: "Economics",
        pubkeys: &[FIATJAF_PUBKEY],
    },
    OnboardingInterestDef {
        id: "psychology",
        emoji: "\u{1f52c}",
        label: "Psychology",
        pubkeys: &[JACK_PUBKEY],
    },
    OnboardingInterestDef {
        id: "literature",
        emoji: "\u{1f4da}",
        label: "Literature",
        pubkeys: &[JACK_PUBKEY],
    },
    OnboardingInterestDef {
        id: "politics",
        emoji: "\u{1f5f3}\u{fe0f}",
        label: "Politics",
        pubkeys: &[],
    },
    OnboardingInterestDef {
        id: "bitcoin",
        emoji: "\u{20bf}",
        label: "Bitcoin",
        pubkeys: &[JACK_PUBKEY, FIATJAF_PUBKEY],
    },
    OnboardingInterestDef {
        id: "self_improvement",
        emoji: "\u{1f331}",
        label: "Self-improvement",
        pubkeys: &[JACK_PUBKEY],
    },
    OnboardingInterestDef {
        id: "science",
        emoji: "\u{1f52d}",
        label: "Science",
        pubkeys: &[],
    },
    OnboardingInterestDef {
        id: "art",
        emoji: "\u{1f3a8}",
        label: "Art",
        pubkeys: &[],
    },
    OnboardingInterestDef {
        id: "music",
        emoji: "\u{1f3b5}",
        label: "Music",
        pubkeys: &[],
    },
    OnboardingInterestDef {
        id: "design",
        emoji: "\u{270f}\u{fe0f}",
        label: "Design",
        pubkeys: &[],
    },
    OnboardingInterestDef {
        id: "writing",
        emoji: "\u{270d}\u{fe0f}",
        label: "Writing",
        pubkeys: &[JACK_PUBKEY],
    },
    OnboardingInterestDef {
        id: "startups",
        emoji: "\u{26a1}\u{fe0f}",
        label: "Startups",
        pubkeys: &[JACK_PUBKEY],
    },
    OnboardingInterestDef {
        id: "nostr",
        emoji: "\u{1f7e3}",
        label: "Nostr",
        pubkeys: &[FIATJAF_PUBKEY],
    },
    OnboardingInterestDef {
        id: "food",
        emoji: "\u{1f373}",
        label: "Food",
        pubkeys: &[],
    },
    OnboardingInterestDef {
        id: "travel",
        emoji: "\u{1f5fa}\u{fe0f}",
        label: "Travel",
        pubkeys: &[],
    },
    OnboardingInterestDef {
        id: "health",
        emoji: "\u{1f3c3}",
        label: "Health",
        pubkeys: &[],
    },
];

fn onboarding_snapshot(
    is_complete: bool,
    selected: BTreeSet<String>,
    is_finishing: bool,
) -> HighlighterOnboardingSnapshot {
    let selected_interest_ids: Vec<String> = selected.iter().cloned().collect();
    let selected_count = selected_interest_ids.len() as u32;
    let remaining_selection_count = MIN_ONBOARDING_INTERESTS.saturating_sub(selected_count);
    let interests = ONBOARDING_INTERESTS
        .iter()
        .map(|interest| HighlighterOnboardingInterest {
            id: interest.id.into(),
            emoji: interest.emoji.into(),
            label: interest.label.into(),
            selected: selected.contains(interest.id),
        })
        .collect();

    HighlighterOnboardingSnapshot {
        is_complete,
        interests,
        selected_interest_ids,
        minimum_selection_count: MIN_ONBOARDING_INTERESTS,
        remaining_selection_count,
        can_finish: selected_count >= MIN_ONBOARDING_INTERESTS && !is_finishing,
        is_finishing,
    }
}

fn selected_onboarding_ids(snapshot: &HighlighterOnboardingSnapshot) -> BTreeSet<String> {
    snapshot
        .selected_interest_ids
        .iter()
        .map(|id| id.trim().to_ascii_lowercase())
        .filter(|id| {
            ONBOARDING_INTERESTS
                .iter()
                .any(|interest| interest.id == id)
        })
        .collect()
}

fn onboarding_pubkeys_for(selected: &BTreeSet<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for interest_id in selected {
        if let Some(interest) = ONBOARDING_INTERESTS
            .iter()
            .find(|interest| interest.id == interest_id)
        {
            for pubkey in interest.pubkeys {
                if seen.insert(*pubkey) {
                    out.push((*pubkey).to_string());
                }
            }
        }
    }
    out
}

fn apply_whats_new_to_snapshot(
    snapshot: &mut HighlighterWhatsNewSnapshot,
    local_state: &mut LocalAppState,
) -> bool {
    let entries = bundled_whats_new_entries();
    let mut mutated_local_state = false;
    if local_state.whats_new_last_seen_at.is_none() {
        local_state.whats_new_last_seen_at = entries.first().map(|entry| entry.shipped_at.clone());
        mutated_local_state = local_state.whats_new_last_seen_at.is_some();
    }

    let last_seen = local_state.whats_new_last_seen_at.clone();
    let visible_entries = last_seen
        .as_deref()
        .map(|marker| {
            entries
                .into_iter()
                .filter(|entry| entry.shipped_at.as_str() > marker)
                .take(WHATS_NEW_VISIBLE_LIMIT)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    snapshot.entries = visible_entries;
    snapshot.entry_count = snapshot.entries.len() as u64;
    snapshot.last_seen_at = last_seen;
    mutated_local_state
}

fn dismiss_whats_new(state: &Arc<RwLock<HighlighterAppState>>, local_state_path: &Path) {
    let mut current = state.write();
    let newest = current
        .whats_new
        .entries
        .first()
        .map(|entry| entry.shipped_at.clone());
    if let Some(newest) = newest {
        current.whats_new.last_seen_at = Some(newest);
    }
    current.whats_new.entries.clear();
    current.whats_new.entry_count = 0;
    current.bump();
    let local = local_state_from_current(&current);
    drop(current);
    save_local_state(local_state_path, &local);
}

fn bundled_whats_new_entries() -> Vec<HighlighterWhatsNewEntry> {
    let Ok(payload) = serde_json::from_str::<WhatsNewPayload>(WHATS_NEW_JSON) else {
        return Vec::new();
    };
    if payload.schema_version != 1 {
        return Vec::new();
    }
    let mut entries = payload
        .entries
        .into_iter()
        .filter(|entry| !entry.shipped_at.trim().is_empty() && !entry.lines.is_empty())
        .map(|entry| HighlighterWhatsNewEntry {
            shipped_at: entry.shipped_at,
            lines: entry.lines,
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| right.shipped_at.cmp(&left.shipped_at));
    entries
}

fn load_local_state(path: &Path) -> LocalAppState {
    let Ok(bytes) = std::fs::read(path) else {
        return LocalAppState::default();
    };
    match serde_json::from_slice(&bytes) {
        Ok(state) => state,
        Err(err) => {
            tracing::warn!(path = %path.display(), error = %err, "load highlighter app state");
            LocalAppState::default()
        }
    }
}

fn local_state_for_snapshot(
    state: &Arc<RwLock<HighlighterAppState>>,
    onboarding_complete: bool,
) -> LocalAppState {
    let current = state.read();
    LocalAppState {
        onboarding_complete,
        recent_searches: current.search.recent_queries.clone(),
        network_wifi_only: current.network.wifi_only_enabled,
        whats_new_last_seen_at: current.whats_new.last_seen_at.clone(),
    }
}

fn local_state_from_current(current: &HighlighterAppState) -> LocalAppState {
    LocalAppState {
        onboarding_complete: current.onboarding.is_complete,
        recent_searches: current.search.recent_queries.clone(),
        network_wifi_only: current.network.wifi_only_enabled,
        whats_new_last_seen_at: current.whats_new.last_seen_at.clone(),
    }
}

fn save_local_state(path: &Path, state: &LocalAppState) {
    if let Some(parent) = path.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            tracing::warn!(path = %parent.display(), error = %err, "create app state dir");
            return;
        }
    }
    let tmp_path = path.with_extension("json.tmp");
    let bytes = match serde_json::to_vec_pretty(state) {
        Ok(bytes) => bytes,
        Err(err) => {
            tracing::warn!(error = %err, "encode highlighter app state");
            return;
        }
    };
    if let Err(err) = std::fs::write(&tmp_path, bytes) {
        tracing::warn!(path = %tmp_path.display(), error = %err, "write app state");
        return;
    }
    if let Err(err) = std::fs::rename(&tmp_path, path) {
        tracing::warn!(path = %path.display(), error = %err, "replace app state");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr_sdk::prelude::*;
    use std::sync::mpsc::{channel, RecvTimeoutError};
    use std::time::{Duration, Instant};
    use tempfile::tempdir;

    #[derive(Debug, Clone)]
    enum TestUpdate {
        State(HighlighterAppState),
        PersistSessionCredential(HighlighterSessionCredential),
        ClearSessionCredentials,
        OpenExternalUrl,
    }

    struct TestReconciler {
        tx: std::sync::mpsc::Sender<TestUpdate>,
    }

    impl HighlighterAppReconciler for TestReconciler {
        fn on_state(&self, state: HighlighterAppState) {
            let _ = self.tx.send(TestUpdate::State(state));
        }

        fn on_persist_session_credential(&self, credential: HighlighterSessionCredential) {
            let _ = self
                .tx
                .send(TestUpdate::PersistSessionCredential(credential));
        }

        fn on_clear_session_credentials(&self) {
            let _ = self.tx.send(TestUpdate::ClearSessionCredentials);
        }

        fn on_open_external_url(&self, _url: String) {
            let _ = self.tx.send(TestUpdate::OpenExternalUrl);
        }
    }

    fn test_app() -> Arc<HighlighterNmpApp> {
        let tmp = tempdir().expect("tempdir");
        let data_dir = tmp.keep().join("ndb");
        test_app_with_data_dir(data_dir)
    }

    fn test_app_with_data_dir(data_dir: PathBuf) -> Arc<HighlighterNmpApp> {
        HighlighterNmpApp::new(HighlighterAppConfig {
            data_dir: Some(data_dir.to_string_lossy().into_owned()),
            visible_limit: 8,
            emit_hz: 30,
            relay_policy_json: None,
        })
    }

    #[test]
    fn whats_new_entries_are_rust_owned_and_sorted_newest_first() {
        let entries = bundled_whats_new_entries();
        assert!(!entries.is_empty());
        assert!(entries.iter().all(|entry| !entry.shipped_at.is_empty()));
        assert!(entries.iter().all(|entry| !entry.lines.is_empty()));
        let dates = entries
            .iter()
            .map(|entry| entry.shipped_at.clone())
            .collect::<Vec<_>>();
        let mut sorted = dates.clone();
        sorted.sort_by(|left, right| right.cmp(left));
        assert_eq!(dates, sorted);
    }

    #[test]
    fn whats_new_first_install_seeds_marker_without_presenting_history() {
        let mut local = LocalAppState::default();
        let mut snapshot = HighlighterWhatsNewSnapshot::empty();
        let mutated = apply_whats_new_to_snapshot(&mut snapshot, &mut local);
        assert!(mutated);
        assert!(local.whats_new_last_seen_at.is_some());
        assert!(snapshot.entries.is_empty());
        assert_eq!(snapshot.entry_count, 0);
        assert_eq!(snapshot.last_seen_at, local.whats_new_last_seen_at);
    }

    #[test]
    fn whats_new_presents_entries_newer_than_marker() {
        let entries = bundled_whats_new_entries();
        assert!(entries.len() > 1);
        let mut local = LocalAppState {
            whats_new_last_seen_at: Some(entries[1].shipped_at.clone()),
            ..LocalAppState::default()
        };
        let mut snapshot = HighlighterWhatsNewSnapshot::empty();
        let mutated = apply_whats_new_to_snapshot(&mut snapshot, &mut local);
        assert!(!mutated);
        assert_eq!(snapshot.entries.len(), 1);
        assert_eq!(snapshot.entry_count, 1);
        assert_eq!(snapshot.entries[0].shipped_at, entries[0].shipped_at);
    }

    #[test]
    fn dismiss_whats_new_persists_newest_seen_marker() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("highlighter_app_state.json");
        let entries = bundled_whats_new_entries();
        let state = Arc::new(RwLock::new(HighlighterAppState::empty(false)));
        {
            let mut current = state.write();
            current.whats_new.entries = entries[..2].to_vec();
            current.whats_new.entry_count = 2;
            current.whats_new.last_seen_at = Some(entries[2].shipped_at.clone());
        }

        dismiss_whats_new(&state, &path);

        let current = state.read();
        assert!(current.whats_new.entries.is_empty());
        assert_eq!(current.whats_new.entry_count, 0);
        assert_eq!(
            current.whats_new.last_seen_at,
            Some(entries[0].shipped_at.clone())
        );
        drop(current);
        let local = load_local_state(&path);
        assert_eq!(
            local.whats_new_last_seen_at,
            Some(entries[0].shipped_at.clone())
        );
    }

    fn next_update(rx: &std::sync::mpsc::Receiver<TestUpdate>) -> TestUpdate {
        rx.recv_timeout(Duration::from_secs(5))
            .expect("app update within timeout")
    }

    fn next_state(rx: &std::sync::mpsc::Receiver<TestUpdate>) -> HighlighterAppState {
        for _ in 0..16 {
            if let TestUpdate::State(state) = next_update(rx) {
                return state;
            }
        }
        panic!("full state update within timeout")
    }

    fn wait_for_state(
        rx: &std::sync::mpsc::Receiver<TestUpdate>,
        description: &str,
        predicate: impl FnMut(&HighlighterAppState) -> bool,
    ) -> HighlighterAppState {
        wait_for_state_with_timeout(rx, description, Duration::from_secs(5), predicate)
    }

    fn wait_for_state_with_timeout(
        rx: &std::sync::mpsc::Receiver<TestUpdate>,
        description: &str,
        timeout: Duration,
        mut predicate: impl FnMut(&HighlighterAppState) -> bool,
    ) -> HighlighterAppState {
        let deadline = Instant::now() + timeout;
        loop {
            let now = Instant::now();
            if now >= deadline {
                panic!("state matching {description} within timeout");
            }
            match rx.recv_timeout((deadline - now).min(Duration::from_millis(250))) {
                Ok(TestUpdate::State(state)) => {
                    if predicate(&state) {
                        return state;
                    }
                }
                Ok(
                    TestUpdate::PersistSessionCredential(_)
                    | TestUpdate::ClearSessionCredentials
                    | TestUpdate::OpenExternalUrl,
                ) => {}
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => panic!("app update channel disconnected"),
            }
        }
    }

    #[test]
    fn listen_for_updates_immediately_emits_snapshot() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));

        let state = next_state(&rx);

        assert_eq!(state.rev, 0);
        assert!(state.chrome.current_user.is_none());
        assert!(state.share_extension.communities.is_empty());
        assert_eq!(state.share_extension.community_count, 0);
    }

    #[test]
    fn share_extension_snapshot_is_rust_owned_and_bounded() {
        let communities = vec![
            community_summary("alpha", "Alpha", "https://example.com/a.png"),
            community_summary("beta", "Beta", "https://example.com/b.png"),
            community_summary("gamma", "Gamma", "https://example.com/c.png"),
        ];

        let snapshot = share_extension_snapshot_for(&communities, 2);

        assert_eq!(snapshot.community_count, 3);
        assert_eq!(snapshot.communities.len(), 2);
        assert_eq!(snapshot.communities[0].id, "alpha");
        assert_eq!(snapshot.communities[0].name, "Alpha");
        assert_eq!(snapshot.communities[0].picture, "https://example.com/a.png");
        assert_eq!(snapshot.communities[1].id, "beta");
    }

    #[test]
    fn relay_removal_impact_is_rust_owned_grouped_and_bounded() {
        let mut communities = vec![
            community_summary("alpha", "Alpha", ""),
            community_summary("beta", "Beta", ""),
            community_summary("gamma", "", ""),
            community_summary("delta", "Delta", ""),
            community_summary("epsilon", "Epsilon", ""),
            community_summary("zeta", "Zeta", ""),
            community_summary("other", "Other", ""),
        ];
        communities[0].relay_url = " wss://relay.example ".into();
        communities[1].relay_url = "wss://relay.example".into();
        communities[2].relay_url = "wss://relay.example".into();
        communities[3].relay_url = "wss://relay.example".into();
        communities[4].relay_url = "wss://relay.example".into();
        communities[5].relay_url = "wss://relay.example".into();
        communities[6].relay_url = "wss://relay.other".into();

        let impacts = relay_removal_impacts_for(&communities);

        assert_eq!(impacts.len(), 2);
        let relay = impacts
            .iter()
            .find(|impact| impact.relay_url == "wss://relay.example")
            .expect("impact for relay.example");
        assert_eq!(relay.room_count, 6);
        assert_eq!(relay.room_names.len(), RELAY_REMOVAL_ROOM_NAME_LIMIT);
        assert_eq!(
            relay.room_names,
            vec!["Alpha", "Beta", "gamma", "Delta", "Epsilon"]
        );
    }

    #[test]
    fn relay_removal_impact_selector_normalizes_url_in_rust() {
        let impacts = vec![HighlighterRelayRemovalImpact {
            relay_url: "wss://relay.example".into(),
            room_names: vec!["Alpha".into()],
            room_count: 1,
        }];

        let impact = relay_removal_impact_for_url(&impacts, " wss://relay.example ")
            .expect("normalized relay URL matches");

        assert_eq!(impact.room_count, 1);
        assert_eq!(impact.room_names, vec!["Alpha"]);
        assert!(relay_removal_impact_for_url(&impacts, " ").is_none());
    }

    #[test]
    fn foreground_action_is_rust_owned_and_emits_state() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let initial = next_state(&rx);

        app.dispatch(HighlighterAppAction::AppForegrounded);

        let state = next_state(&rx);
        assert!(state.rev > initial.rev);
    }

    #[test]
    fn search_query_state_is_rust_owned() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::SetSearchQuery {
            query: "  nostr architecture  ".into(),
        });

        let state = next_state(&rx);
        assert_eq!(state.search.query, "nostr architecture");
        assert_eq!(state.search.applied_query, "");
        assert!(!state.search.is_local_loading);
    }

    #[test]
    fn recent_searches_are_rust_owned_deduped_bounded_and_persisted() {
        let state = Arc::new(RwLock::new(HighlighterAppState::empty(false)));
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("state.json");

        for query in [
            "Rust", "nostr", "  rust  ", "", "books", "articles", "people", "rooms", "quotes",
            "relays", "podcasts",
        ] {
            record_recent_search(&state, &path, query.into());
        }

        let snapshot = state.read().search.clone();
        assert_eq!(snapshot.recent_query_count, RECENT_SEARCH_LIMIT as u64);
        assert_eq!(
            snapshot.recent_queries,
            vec!["podcasts", "relays", "quotes", "rooms", "people", "articles", "books", "rust"]
        );

        let local = load_local_state(&path);
        assert_eq!(local.recent_searches, snapshot.recent_queries);

        clear_recent_searches(&state, &path);
        assert!(state.read().search.recent_queries.is_empty());
        assert!(load_local_state(&path).recent_searches.is_empty());
    }

    #[test]
    fn network_wifi_only_is_rust_owned_bounded_and_persisted() {
        let state = Arc::new(RwLock::new(HighlighterAppState::empty(false)));
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("state.json");

        set_network_wifi_only(&state, &path, true);
        set_network_path(&state, false);

        let snapshot = state.read().network.clone();
        assert!(snapshot.wifi_only_enabled);
        assert_eq!(snapshot.current_path_is_wifi, Some(false));
        assert!(load_local_state(&path).network_wifi_only);

        set_network_wifi_only(&state, &path, false);
        assert!(!state.read().network.wifi_only_enabled);
        assert!(!load_local_state(&path).network_wifi_only);
    }

    #[test]
    fn clear_search_resets_rust_owned_snapshot() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::SetSearchQuery {
            query: "nostr".into(),
        });
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::ClearSearch);

        let state = next_state(&rx);
        assert!(state.search.query.is_empty());
        assert!(state.search.highlights.is_empty());
        assert!(state.search.articles.is_empty());
        assert!(state.search.communities.is_empty());
        assert!(state.search.profiles.is_empty());
        assert!(!state.search.is_local_loading);
        assert!(!state.search.is_relay_loading);
    }

    #[test]
    fn create_account_username_validation_is_rust_owned() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::SetCreateAccountUsername {
            username: "Bad Name!".into(),
        });
        let state = next_state(&rx);

        assert_eq!(
            state.create_account.username_status,
            HighlighterUsernameStatus::Invalid
        );
        assert_eq!(state.create_account.username, "bad name!");
        assert!(!state.create_account.can_submit);

        app.dispatch(HighlighterAppAction::SetCreateAccountDisplayName {
            display_name: "Alice Reader".into(),
        });
        let state = next_state(&rx);

        assert_eq!(
            state.create_account.username_status,
            HighlighterUsernameStatus::Invalid
        );
        assert!(!state.create_account.can_submit);
    }

    #[test]
    fn create_account_submit_error_surfaces_in_state() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::SubmitCreateAccount);
        let state = next_state(&rx);

        assert_eq!(
            state.create_account.error_message.as_deref(),
            Some("Enter a display name")
        );
        assert!(!state.create_account.is_creating);
        assert!(!state.create_account.can_submit);
    }

    #[test]
    fn sign_in_nsec_updates_rust_owned_session_snapshot() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        let keys = Keys::generate();
        let pubkey = keys.public_key().to_hex();
        let nsec = keys.secret_key().to_bech32().expect("nsec");
        app.dispatch(HighlighterAppAction::SignInNsec {
            nsec,
            persist: false,
            clear_stored_on_failure: false,
        });

        let mut last = None;
        for _ in 0..8 {
            let state = next_state(&rx);
            if state
                .chrome
                .current_user
                .as_ref()
                .is_some_and(|user| user.pubkey == pubkey)
            {
                last = Some(state);
                break;
            }
        }
        let state = last.expect("signed-in state");
        assert!(state.toast.is_none());
        assert!(!state.auth.is_signing_in);
    }

    #[test]
    fn valid_sign_in_emits_secret_storage_side_effect_after_validation() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        let keys = Keys::generate();
        let nsec = keys.secret_key().to_bech32().expect("nsec");
        let expected_nsec = nsec.clone();
        app.dispatch(HighlighterAppAction::SignInNsec {
            nsec: format!(" {nsec} "),
            persist: true,
            clear_stored_on_failure: false,
        });

        let mut saw_signed_in = false;
        let mut persisted = None;
        for _ in 0..16 {
            match next_update(&rx) {
                TestUpdate::State(state) => {
                    saw_signed_in |= state.chrome.current_user.is_some();
                }
                TestUpdate::PersistSessionCredential(credential) => {
                    if let HighlighterSessionCredential::Nsec { nsec } = credential {
                        persisted = Some(nsec);
                        break;
                    }
                }
                TestUpdate::ClearSessionCredentials => {}
                TestUpdate::OpenExternalUrl => {}
            }
        }

        assert!(saw_signed_in, "state must show validated session first");
        assert_eq!(persisted.as_deref(), Some(expected_nsec.as_str()));
    }

    #[test]
    fn invalid_sign_in_surfaces_error_as_state() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::SignInNsec {
            nsec: "not a key".into(),
            persist: true,
            clear_stored_on_failure: true,
        });

        let mut saw_error = false;
        let mut saw_clear = false;
        // Sticky flags + a generous update budget: resolutions are async
        // (OpRunner), so busy/tick emissions interleave with the error
        // snapshot in nondeterministic order — a one-shot reassignment would
        // flake when a later non-error snapshot overwrote the flag.
        for _ in 0..32 {
            match rx.recv_timeout(Duration::from_secs(5)) {
                Ok(TestUpdate::State(state)) => {
                    saw_error |= state
                        .toast
                        .as_ref()
                        .is_some_and(|toast| toast.kind == HighlighterToastKind::Error);
                }
                Ok(TestUpdate::ClearSessionCredentials) => saw_clear = true,
                Ok(TestUpdate::PersistSessionCredential(_)) => {}
                Ok(TestUpdate::OpenExternalUrl) => {}
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => break,
            }
            if saw_error && saw_clear {
                break;
            }
        }

        assert!(saw_error, "invalid sign-in must surface through state");
        assert!(
            saw_clear,
            "stored invalid credentials must clear through a side-effect update"
        );
    }

    #[test]
    fn invalid_bunker_pair_surfaces_error_and_clear_side_effect() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::PairBunker {
            uri: "   ".into(),
            persist: true,
            clear_stored_on_failure: true,
        });

        let mut saw_error = false;
        let mut saw_clear = false;
        // Sticky flags + a generous update budget: resolutions are async
        // (OpRunner), so busy/tick emissions interleave with the error
        // snapshot in nondeterministic order — a one-shot reassignment would
        // flake when a later non-error snapshot overwrote the flag.
        for _ in 0..32 {
            match rx.recv_timeout(Duration::from_secs(5)) {
                Ok(TestUpdate::State(state)) => {
                    saw_error |= state
                        .toast
                        .as_ref()
                        .is_some_and(|toast| toast.kind == HighlighterToastKind::Error);
                }
                Ok(TestUpdate::ClearSessionCredentials) => saw_clear = true,
                Ok(TestUpdate::PersistSessionCredential(_)) => {}
                Ok(TestUpdate::OpenExternalUrl) => {}
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => break,
            }
            if saw_error && saw_clear {
                break;
            }
        }

        assert!(saw_error, "invalid bunker pair must surface through state");
        assert!(
            saw_clear,
            "stored invalid bunker credentials must clear through a side-effect update"
        );
    }

    #[test]
    fn nostr_connect_callback_url_is_rust_owned_and_encoded() {
        assert_eq!(
            append_callback_url(
                "nostrconnect://abc?relay=wss%3A%2F%2Frelay.example".into(),
                "highlighter://nip46"
            ),
            "nostrconnect://abc?relay=wss%3A%2F%2Frelay.example&callback=highlighter%3A%2F%2Fnip46"
        );
        assert_eq!(
            append_callback_url("nostrconnect://abc".into(), "highlighter://nip46"),
            "nostrconnect://abc?callback=highlighter%3A%2F%2Fnip46"
        );
        assert_eq!(
            append_callback_url("nostrconnect://abc".into(), "  "),
            "nostrconnect://abc"
        );
    }

    #[test]
    fn external_url_open_failure_surfaces_error_as_state() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::ExternalUrlOpenFailed {
            url: "nostrconnect://abc".into(),
        });

        let state = wait_for_state(&rx, "invalid web metadata error toast", |state| {
            state
                .toast
                .as_ref()
                .is_some_and(|toast| toast.kind == HighlighterToastKind::Error)
        });
        assert_eq!(
            state.toast.as_ref().map(|toast| toast.kind),
            Some(HighlighterToastKind::Error)
        );
        assert_eq!(
            state.toast.as_ref().map(|toast| toast.message.as_str()),
            Some("Couldn't open signer app")
        );
    }

    #[test]
    fn room_explorer_snapshot_is_rust_owned_and_bounded() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::OpenRoomExplorer);

        // Poll rather than asserting on fixed snapshot positions: emissions
        // from app construction / ticks can interleave with the loading and
        // loaded snapshots (the long-standing one-shot `next_state` flake).
        let _ = wait_for_state(&rx, "room explorer loading snapshot", |state| {
            state.room_explorer.is_loading
        });
        let state = wait_for_state(&rx, "room explorer loaded snapshot", |state| {
            !state.room_explorer.is_loading
        });
        assert_eq!(
            state.room_explorer.curator_pubkey_hex,
            ROOM_EXPLORER_CURATOR_PUBKEY_HEX
        );
        assert!(state.room_explorer.featured.len() <= ROOM_EXPLORER_FEATURED_LIMIT as usize);
        assert!(state.room_explorer.new_noteworthy.len() <= ROOM_EXPLORER_NEW_LIMIT as usize);
        assert!(
            state.room_explorer.friends_shelf.len() <= ROOM_EXPLORER_RECOMMENDATION_LIMIT as usize
        );
        assert!(
            state.room_explorer.authors_shelf.len() <= ROOM_EXPLORER_RECOMMENDATION_LIMIT as usize
        );
    }

    #[test]
    fn room_explorer_browse_snapshot_is_rust_owned_and_bounded() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::RefreshRoomBrowseAll);

        let loading = next_state(&rx);
        assert!(loading.room_explorer.is_browse_loading);
        let state = next_state(&rx);
        assert!(!state.room_explorer.is_browse_loading);
        assert!(state.room_explorer.all_rooms.len() <= ROOM_EXPLORER_BROWSE_LIMIT as usize);
        assert_eq!(
            state.room_explorer.all_room_count,
            state.room_explorer.all_rooms.len() as u64
        );
    }

    #[test]
    fn join_request_error_surfaces_in_room_explorer_state() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::RequestJoinRoom {
            group_id: "books".into(),
            room_name: "Books".into(),
        });

        let joining = wait_for_state(&rx, "join busy state", |state| {
            state.room_explorer.joining_group_id == "books"
        });
        assert!(joining.room_explorer.error_message.is_none());

        let state = wait_for_state(&rx, "join error state", |state| {
            state.toast.as_ref().is_some_and(|toast| {
                toast.kind == HighlighterToastKind::Error
                    && state.room_explorer.error_message.is_some()
            })
        });
        let toast = state.toast.expect("toast");
        assert_eq!(toast.kind, HighlighterToastKind::Error);
        assert!(state.room_explorer.error_message.is_some());
    }

    #[test]
    fn home_feed_open_without_user_resets_to_empty_snapshot() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::OpenHomeFeed);

        let loading = next_state(&rx);
        assert!(loading.home_feed.is_loading);
        let state = next_state(&rx);
        assert!(!state.home_feed.is_loading);
        assert!(state.home_feed.items.is_empty());
        assert_eq!(state.home_feed.item_count, 0);
        assert!(state.home_feed.error_message.is_none());
    }

    #[test]
    fn home_feed_merge_groups_dedupes_sorts_and_bounds_in_rust() {
        let address = "30023:author:article-a";
        let mut highlights = Vec::new();
        for index in 0..8 {
            highlights.push(hydrated_highlight(
                &format!("highlight-{index}"),
                "a:30023:author:article-a",
                address,
                index + 1,
            ));
        }
        highlights.push(hydrated_highlight("solo", "", "", 20));

        let reads = vec![
            reading_feed_item("article-a", 30),
            reading_feed_item("article-b", 10),
        ];

        let items = build_home_feed_items(highlights, reads);

        assert_eq!(items.len(), 3);
        assert_eq!(items[0].kind, HighlighterHomeFeedItemKind::Highlights);
        assert_eq!(items[0].stable_id, "h:evt:solo");
        assert_eq!(items[0].sort_key, 20);

        assert_eq!(items[1].kind, HighlighterHomeFeedItemKind::Read);
        assert_eq!(items[1].stable_id, "r:30023:author:article-b");
        assert_eq!(items[1].sort_key, 10);
        assert_eq!(
            items[1].read.as_ref().map(|read| read.identifier.as_str()),
            Some("article-b")
        );

        assert_eq!(items[2].kind, HighlighterHomeFeedItemKind::Highlights);
        assert_eq!(items[2].stable_id, "h:src:30023:author:article-a");
        assert_eq!(items[2].sort_key, 8);
        assert_eq!(items[2].highlight_count, 8);
        assert_eq!(items[2].highlights.len(), HOME_FEED_GROUP_HIGHLIGHT_LIMIT);
        assert_eq!(items[2].highlights[0].highlight.event_id, "highlight-0");
    }

    #[test]
    fn bookmarks_open_without_user_resets_to_empty_snapshot() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::OpenBookmarks);

        let loading = next_state(&rx);
        assert!(loading.bookmarks.is_loading);
        let state = next_state(&rx);
        assert!(!state.bookmarks.is_loading);
        assert!(state.bookmarks.articles.is_empty());
        assert_eq!(state.bookmarks.article_count, 0);
        assert!(state.bookmarks.my_bookmark_sets.is_empty());
        assert!(state.bookmarks.my_curation_sets.is_empty());
        assert!(state.bookmarks.web_bookmarks.is_empty());
        assert!(state.bookmarks.following_curation_sets.is_empty());
        assert!(state.bookmarks.error_message.is_none());
    }

    #[test]
    fn curation_menu_open_without_user_is_empty_and_address_scoped() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::OpenCurationMenu {
            article_address: "30023:author:article".into(),
        });

        let loading = next_state(&rx);
        assert!(loading.curation_menu.is_loading);
        assert_eq!(
            loading.curation_menu.article_address,
            "30023:author:article"
        );
        let state = next_state(&rx);
        assert!(!state.curation_menu.is_loading);
        assert_eq!(state.curation_menu.article_address, "30023:author:article");
        assert!(state.curation_menu.curation_sets.is_empty());
        assert_eq!(state.curation_menu.curation_set_count, 0);
        assert!(state.curation_menu.error_message.is_none());
    }

    #[test]
    fn bookmark_article_address_parsing_is_rust_owned() {
        assert_eq!(
            normalize_article_address(" 30023:abc:essay ").as_deref(),
            Some("30023:abc:essay")
        );
        assert!(normalize_article_address("1:abc:essay").is_none());
        assert!(normalize_article_address("30023::essay").is_none());
        assert!(normalize_article_address("30023:abc:").is_none());
    }

    #[test]
    fn bookmark_articles_sort_newest_first_in_rust() {
        let mut articles = vec![
            article_record("old", 10, Some(10)),
            article_record("published", 5, Some(40)),
            article_record("created", 30, None),
        ];

        sort_articles_newest_first(&mut articles);

        let order: Vec<_> = articles
            .iter()
            .map(|article| article.identifier.as_str())
            .collect();
        assert_eq!(order, vec!["published", "created", "old"]);
    }

    #[test]
    fn invalid_isbn_preview_request_surfaces_error_as_state() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::RequestIsbnPreview {
            isbn: "not an isbn".into(),
        });

        let state = wait_for_state(&rx, "invalid ISBN error toast", |state| {
            state
                .toast
                .as_ref()
                .is_some_and(|toast| toast.kind == HighlighterToastKind::Error)
        });
        assert_eq!(
            state.toast.as_ref().map(|toast| toast.kind),
            Some(HighlighterToastKind::Error)
        );
        assert!(state.isbn_previews.is_empty());
    }

    #[test]
    fn invalid_web_metadata_request_surfaces_error_as_state() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::RequestWebMetadata {
            url: "not a url".into(),
        });

        let state = next_state(&rx);
        assert_eq!(
            state.toast.as_ref().map(|toast| toast.kind),
            Some(HighlighterToastKind::Error)
        );
        assert!(state.web_metadata.is_empty());
    }

    #[test]
    fn invalid_profile_request_surfaces_error_as_state() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::RequestProfile {
            pubkey_hex: "not a pubkey".into(),
        });

        let state = next_state(&rx);
        assert_eq!(
            state.toast.as_ref().map(|toast| toast.kind),
            Some(HighlighterToastKind::Error)
        );
        assert!(state.profiles.is_empty());
    }

    #[test]
    fn isbn_preview_projection_is_bounded_and_rust_owned() {
        let state = Arc::new(RwLock::new(HighlighterAppState::empty(false)));

        insert_isbn_preview(
            &state,
            "9780000000001".into(),
            preview_for_isbn("9780000000001", "One"),
            2,
        );
        insert_isbn_preview(
            &state,
            "9780000000002".into(),
            preview_for_isbn("9780000000002", "Two"),
            2,
        );
        insert_isbn_preview(
            &state,
            "9780000000003".into(),
            preview_for_isbn("9780000000003", "Three"),
            2,
        );
        insert_isbn_preview(
            &state,
            "9780000000002".into(),
            preview_for_isbn("9780000000002", "Two Updated"),
            2,
        );

        let state = state.read();
        let keys = state
            .isbn_previews
            .iter()
            .map(|entry| entry.isbn.as_str())
            .collect::<Vec<_>>();
        assert_eq!(keys, vec!["9780000000003", "9780000000002"]);
        assert_eq!(state.isbn_preview_count, 2);
        assert_eq!(state.isbn_previews[1].preview.title, "Two Updated");
    }

    #[test]
    fn web_metadata_projection_is_bounded_and_rust_owned() {
        let state = Arc::new(RwLock::new(HighlighterAppState::empty(false)));

        insert_web_metadata(
            &state,
            "https://example.com/one".into(),
            metadata_for_url("https://example.com/one", "One"),
            2,
        );
        insert_web_metadata(
            &state,
            "https://example.com/two".into(),
            metadata_for_url("https://example.com/two", "Two"),
            2,
        );
        insert_web_metadata(
            &state,
            "https://example.com/three".into(),
            metadata_for_url("https://example.com/three", "Three"),
            2,
        );
        insert_web_metadata(
            &state,
            "https://example.com/two".into(),
            metadata_for_url("https://example.com/two", "Two Updated"),
            2,
        );

        let state = state.read();
        let keys = state
            .web_metadata
            .iter()
            .map(|entry| entry.url.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            vec!["https://example.com/three", "https://example.com/two"]
        );
        assert_eq!(state.web_metadata_count, 2);
        assert_eq!(state.web_metadata[1].metadata.title, "Two Updated");
    }

    #[test]
    fn profile_projection_is_bounded_and_rust_owned() {
        let state = Arc::new(RwLock::new(HighlighterAppState::empty(false)));
        let pubkey_one = "0000000000000000000000000000000000000000000000000000000000000001";
        let pubkey_two = "0000000000000000000000000000000000000000000000000000000000000002";
        let pubkey_three = "0000000000000000000000000000000000000000000000000000000000000003";

        insert_profile_metadata(
            &state,
            pubkey_one.into(),
            profile_metadata(pubkey_one, "One"),
            2,
        );
        insert_profile_metadata(
            &state,
            pubkey_two.into(),
            profile_metadata(pubkey_two, "Two"),
            2,
        );
        insert_profile_metadata(
            &state,
            pubkey_three.into(),
            profile_metadata(pubkey_three, "Three"),
            2,
        );
        insert_profile_metadata(
            &state,
            pubkey_two.into(),
            profile_metadata(pubkey_two, "Two Updated"),
            2,
        );

        let state = state.read();
        let keys = state
            .profiles
            .iter()
            .map(|entry| entry.pubkey_hex.as_str())
            .collect::<Vec<_>>();
        assert_eq!(keys, vec![pubkey_three, pubkey_two]);
        assert!(!keys.contains(&pubkey_one));
        assert_eq!(state.profile_count, 2);
        assert_eq!(state.profiles[1].metadata.name, "Two Updated");
    }

    #[test]
    fn profile_view_open_without_user_is_rust_owned_and_bounded() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);
        let pubkey = Keys::generate().public_key().to_hex();

        app.dispatch(HighlighterAppAction::OpenProfile {
            pubkey_hex: format!(" {pubkey} "),
        });

        let loading = next_state(&rx);
        assert_eq!(loading.profile_view.pubkey_hex, pubkey);
        assert!(loading.profile_view.is_loading);
        let state = next_state(&rx);
        assert_eq!(state.profile_view.pubkey_hex, pubkey);
        assert!(state.profile_view.viewer_pubkey_hex.is_none());
        assert!(!state.profile_view.is_own_profile);
        assert!(!state.profile_view.is_following);
        assert!(!state.profile_view.is_mutating_follow);
        assert!(!state.profile_view.is_loading);
        assert!(state.profile_view.profile.is_none());
        assert!(state.profile_view.articles.len() <= PROFILE_ARTICLE_LIMIT);
        assert!(state.profile_view.highlights.len() <= PROFILE_HIGHLIGHT_LIMIT);
        assert!(state.profile_view.communities.len() <= PROFILE_COMMUNITY_LIMIT);
    }

    #[test]
    fn profile_view_invalid_open_surfaces_error_as_state() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::OpenProfile {
            pubkey_hex: "".into(),
        });

        let state = wait_for_state(&rx, "invalid profile error", |state| {
            state.profile_view.error_message.as_deref() == Some("Choose a profile to open")
        });
        assert_eq!(
            state.profile_view.error_message.as_deref(),
            Some("Choose a profile to open")
        );
        assert!(state.profile_view.pubkey_hex.is_empty());
    }

    #[test]
    fn profile_view_close_clears_screen_snapshot() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);
        let pubkey = Keys::generate().public_key().to_hex();

        app.dispatch(HighlighterAppAction::OpenProfile { pubkey_hex: pubkey });
        let _ = next_state(&rx);
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::CloseProfile);

        let state = wait_for_state(&rx, "closed profile snapshot", |state| {
            state.profile_view.pubkey_hex.is_empty()
        });
        assert!(state.profile_view.pubkey_hex.is_empty());
        assert!(state.profile_view.articles.is_empty());
        assert!(state.profile_view.highlights.is_empty());
        assert!(state.profile_view.communities.is_empty());
        assert!(!state.profile_view.is_loading);
    }

    #[test]
    fn article_reader_open_uses_rust_owned_seed_and_bounded_snapshot() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);
        let pubkey = Keys::generate().public_key().to_hex();
        let seed = article_record_for(&pubkey, "essay", 42, Some(50));

        app.dispatch(HighlighterAppAction::OpenArticleReader {
            pubkey_hex: format!(" {pubkey} "),
            d_tag: " essay ".into(),
            seed: Some(seed.clone()),
        });

        let expected_address = format!("30023:{pubkey}:essay");
        let loading = wait_for_state(&rx, "article reader loading snapshot", |state| {
            state.article_reader.address == expected_address && state.article_reader.is_loading
        });
        assert_eq!(loading.article_reader.address, expected_address);
        assert_eq!(
            loading
                .article_reader
                .article
                .as_ref()
                .map(|article| article.event_id.as_str()),
            Some(seed.event_id.as_str())
        );
        assert!(loading.article_reader.is_loading);

        let state = wait_for_state(&rx, "article reader loaded snapshot", |state| {
            state.article_reader.address == expected_address && !state.article_reader.is_loading
        });
        assert_eq!(state.article_reader.pubkey_hex, pubkey);
        assert_eq!(state.article_reader.d_tag, "essay");
        assert_eq!(
            state
                .article_reader
                .article
                .as_ref()
                .map(|article| article.event_id.as_str()),
            Some(seed.event_id.as_str())
        );
        assert!(state.article_reader.highlights.len() <= ARTICLE_READER_HIGHLIGHT_LIMIT);
        assert!(!state.article_reader.is_loading);
        assert!(!state.article_reader.is_publishing_highlight);
    }

    #[test]
    fn article_reader_invalid_open_surfaces_error_as_state() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::OpenArticleReader {
            pubkey_hex: "".into(),
            d_tag: "".into(),
            seed: None,
        });

        let state = next_state(&rx);
        assert_eq!(
            state.article_reader.error_message.as_deref(),
            Some("Choose an article to open")
        );
        assert!(state.article_reader.address.is_empty());
    }

    #[test]
    fn article_reader_publish_without_article_surfaces_error_as_state() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::PublishArticleHighlight {
            quote: "important".into(),
            context: "context".into(),
            note: "".into(),
        });

        // Phase 2: the publish helper now validates inputs on the actor BEFORE
        // submitting the off-actor op, so a no-article failure surfaces as a
        // single error snapshot with no spurious `is_publishing` flash.
        let state = next_state(&rx);
        assert!(!state.article_reader.is_publishing_highlight);
        assert_eq!(
            state.article_reader.error_message.as_deref(),
            Some("Article not yet loaded")
        );
        assert_eq!(
            state.toast.as_ref().map(|toast| toast.kind),
            Some(HighlighterToastKind::Error)
        );
    }

    #[test]
    fn article_reader_artifact_mapping_is_rust_owned() {
        let pubkey = Keys::generate().public_key().to_hex();
        let article = article_record_for(&pubkey, "essay", 10, Some(20));

        let artifact = article_as_artifact(&article, "30023:author:essay");

        assert_eq!(artifact.preview.source, "article");
        assert_eq!(artifact.preview.reference_tag_name, "a");
        assert_eq!(artifact.preview.reference_kind, "30023");
        assert_eq!(
            artifact.preview.highlight_reference_key,
            "a:30023:author:essay"
        );
        assert_eq!(artifact.pubkey, pubkey);
    }

    #[test]
    fn onboarding_toggle_updates_rust_owned_selection() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::ToggleOnboardingInterest {
            interest_id: "science".into(),
        });

        let state = next_state(&rx);
        assert_eq!(state.onboarding.selected_interest_ids, vec!["science"]);
        assert!(state
            .onboarding
            .interests
            .iter()
            .any(|interest| interest.id == "science" && interest.selected));
        assert_eq!(state.onboarding.remaining_selection_count, 2);
        assert!(!state.onboarding.can_finish);
    }

    #[test]
    fn complete_onboarding_requires_minimum_selection() {
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::CompleteOnboarding);

        let state = next_state(&rx);
        assert!(!state.onboarding.is_complete);
        assert_eq!(
            state.toast.as_ref().map(|toast| toast.kind),
            Some(HighlighterToastKind::Error)
        );
    }

    #[test]
    fn complete_onboarding_persists_after_finish() {
        let tmp = tempdir().expect("tempdir");
        let data_dir = tmp.path().join("ndb");
        let app = test_app_with_data_dir(data_dir.clone());
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        let keys = Keys::generate();
        let nsec = keys.secret_key().to_bech32().expect("nsec");
        app.dispatch(HighlighterAppAction::SignInNsec {
            nsec,
            persist: false,
            clear_stored_on_failure: false,
        });
        for _ in 0..8 {
            if next_state(&rx).chrome.current_user.is_some() {
                break;
            }
        }
        for interest_id in ["science", "art", "music"] {
            app.dispatch(HighlighterAppAction::ToggleOnboardingInterest {
                interest_id: interest_id.into(),
            });
            let _ = next_state(&rx);
        }

        app.dispatch(HighlighterAppAction::CompleteOnboarding);

        let mut completed = false;
        for _ in 0..8 {
            let state = next_state(&rx);
            if state.onboarding.is_complete {
                completed = true;
                break;
            }
        }
        assert!(completed, "onboarding should complete through state");
        drop(app);

        let next_app = test_app_with_data_dir(data_dir);
        assert!(next_app.state().onboarding.is_complete);
    }

    fn preview_for_isbn(isbn: &str, title: &str) -> ArtifactPreview {
        let catalog_id = format!("isbn:{isbn}");
        ArtifactPreview {
            id: format!("preview-{isbn}"),
            url: String::new(),
            title: title.into(),
            author: String::new(),
            image: String::new(),
            description: String::new(),
            source: "book".into(),
            domain: String::new(),
            catalog_id: catalog_id.clone(),
            catalog_kind: "isbn".into(),
            podcast_guid: String::new(),
            podcast_item_guid: String::new(),
            podcast_show_title: String::new(),
            audio_url: String::new(),
            audio_preview_url: String::new(),
            transcript_url: String::new(),
            feed_url: String::new(),
            published_at: String::new(),
            duration_seconds: None,
            reference_tag_name: "i".into(),
            reference_tag_value: catalog_id.clone(),
            reference_kind: "isbn".into(),
            highlight_tag_name: "i".into(),
            highlight_tag_value: catalog_id.clone(),
            highlight_reference_key: format!("i:{catalog_id}"),
            chapters: Vec::new(),
        }
    }

    fn metadata_for_url(url: &str, title: &str) -> WebMetadata {
        WebMetadata {
            url: url.into(),
            title: title.into(),
            description: String::new(),
            image: String::new(),
            site_name: String::new(),
            author: String::new(),
            favicon: String::new(),
            fetched_at: 1,
        }
    }

    fn profile_metadata(pubkey: &str, name: &str) -> ProfileMetadata {
        ProfileMetadata {
            pubkey: pubkey.into(),
            name: name.into(),
            display_name: String::new(),
            about: String::new(),
            picture: String::new(),
            banner: String::new(),
            nip05: String::new(),
            website: String::new(),
            lud16: String::new(),
            created_at: Some(1),
        }
    }

    fn community_summary(id: &str, name: &str, picture: &str) -> CommunitySummary {
        CommunitySummary {
            id: id.into(),
            name: name.into(),
            about: String::new(),
            picture: picture.into(),
            access: "open".into(),
            visibility: "public".into(),
            admin_pubkeys: Vec::new(),
            member_count: Some(1),
            relay_url: "wss://relay.example".into(),
            metadata_event_id: String::new(),
            created_at: Some(1),
        }
    }

    fn hydrated_highlight(
        event_id: &str,
        source_reference_key: &str,
        artifact_address: &str,
        created_at: u64,
    ) -> HydratedHighlight {
        HydratedHighlight {
            highlight: HighlightRecord {
                event_id: event_id.into(),
                pubkey: "reader".into(),
                quote: format!("Quote {event_id}"),
                context: String::new(),
                note: String::new(),
                artifact_address: artifact_address.into(),
                event_reference: String::new(),
                external_reference: String::new(),
                source_url: String::new(),
                source_reference_key: source_reference_key.into(),
                clip_start_seconds: None,
                clip_end_seconds: None,
                clip_speaker: String::new(),
                clip_transcript_segment_ids: Vec::new(),
                image_url: String::new(),
                created_at: Some(created_at),
            },
            artifact: None,
            shared_by_event_id: None,
            shared_by_pubkey: None,
        }
    }

    fn reading_feed_item(identifier: &str, latest_activity_at: u64) -> ReadingFeedItem {
        ReadingFeedItem {
            article: ArticleRecord {
                event_id: format!("event-{identifier}"),
                pubkey: "author".into(),
                identifier: identifier.into(),
                title: format!("Article {identifier}"),
                summary: String::new(),
                image: String::new(),
                content: String::new(),
                hashtags: Vec::new(),
                published_at: Some(latest_activity_at),
                created_at: Some(latest_activity_at),
            },
            author_followed: true,
            interactor_pubkeys: Vec::new(),
            latest_activity_at,
        }
    }

    fn article_record(
        identifier: &str,
        created_at: u64,
        published_at: Option<u64>,
    ) -> ArticleRecord {
        ArticleRecord {
            event_id: format!("event-{identifier}"),
            pubkey: "author".into(),
            identifier: identifier.into(),
            title: format!("Article {identifier}"),
            summary: String::new(),
            image: String::new(),
            content: String::new(),
            hashtags: Vec::new(),
            published_at,
            created_at: Some(created_at),
        }
    }

    fn article_record_for(
        pubkey: &str,
        identifier: &str,
        created_at: u64,
        published_at: Option<u64>,
    ) -> ArticleRecord {
        ArticleRecord {
            pubkey: pubkey.into(),
            ..article_record(identifier, created_at, published_at)
        }
    }

    // =======================================================================
    // Actor-blocking-fix acceptance tests (design §4.6).
    //
    // These prove the OpRunner keeps the single-writer actor loop live while a
    // network-class operation is wedged against a dead relay, that superseded
    // resolutions are dropped, that logout cancels in-flight work, and that no
    // single handler dispatch blows the loop-stall budget.
    // =======================================================================

    use std::io::Read as _;
    use std::net::TcpListener;

    /// A "network up, relay dead" black hole: accepts TCP connections and then
    /// never writes a byte back (no WebSocket / HTTP handshake completes).
    /// Returns the bound port; the accept loop runs on a detached thread for
    /// the lifetime of the process (test binaries are ephemeral).
    fn spawn_black_hole_listener() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind black-hole listener");
        let port = listener.local_addr().expect("local addr").port();
        std::thread::Builder::new()
            .name("black-hole-relay".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    if let Ok(mut stream) = stream {
                        // Hold the connection open and swallow any request
                        // bytes, but never respond — the client blocks until
                        // its own deadline fires.
                        std::thread::spawn(move || {
                            let mut buf = [0u8; 1024];
                            // One non-fatal read; then just keep the socket
                            // alive by parking. Dropping `stream` would RST.
                            let _ = stream.read(&mut buf);
                            loop {
                                std::thread::sleep(Duration::from_secs(3600));
                            }
                        });
                    }
                }
            })
            .expect("spawn black-hole listener");
        port
    }

    /// Relay policy JSON pointing every role at the black hole.
    fn black_hole_relay_policy(port: u16) -> String {
        let url = format!("ws://127.0.0.1:{port}");
        format!(
            r#"{{
  "highlighter_relay": "{url}",
  "purple_pages_relay": "{url}",
  "negentropy_sync_relays": ["{url}"],
  "nostr_connect_relay": "{url}",
  "seed_defaults": [
    {{ "url": "{url}", "read": true, "write": true, "rooms": true, "indexer": true }}
  ]
}}"#
        )
    }

    fn black_hole_app(port: u16) -> Arc<HighlighterNmpApp> {
        let tmp = tempdir().expect("tempdir");
        let data_dir = tmp.keep().join("ndb");
        // Install the black-hole policy process-wide for this test, then build
        // the app (which reads seed_defaults at construction).
        crate::relays::set_relay_policy_for_test(&black_hole_relay_policy(port));
        HighlighterNmpApp::new(HighlighterAppConfig {
            data_dir: Some(data_dir.to_string_lossy().into_owned()),
            visible_limit: 8,
            emit_hz: 30,
            relay_policy_json: Some(black_hole_relay_policy(port)),
        })
    }

    /// Drain whatever snapshots have already been emitted, returning the last.
    fn drain_states(rx: &std::sync::mpsc::Receiver<TestUpdate>) -> Option<HighlighterAppState> {
        let mut last = None;
        while let Ok(update) = rx.recv_timeout(Duration::from_millis(50)) {
            if let TestUpdate::State(state) = update {
                last = Some(state);
            }
        }
        last
    }

    /// Acceptance 1 — liveness under wedge. With every relay pointed at the
    /// black hole, a probe op wedges off-actor; meanwhile a burst of local
    /// actions must each produce a snapshot promptly, and the actor watchdog
    /// must never see a slow dispatch.
    #[test]
    fn acceptance_liveness_under_wedge() {
        let port = spawn_black_hole_listener();
        reset_actor_max_handler_ms();
        let app = black_hole_app(port);
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        // Wedge: probe a black-hole relay (HTTP GET that never completes).
        app.dispatch(HighlighterAppAction::OpenNetworkSettings);
        let _ = drain_states(&rx);
        app.dispatch(HighlighterAppAction::ProbeNetworkRelayNip11 {
            url: format!("ws://127.0.0.1:{port}"),
        });
        // The loading snapshot must arrive immediately (op submitted, not awaited).
        let loading = next_state(&rx);
        assert!(
            loading.network.nip11.iter().any(|row| row.is_loading),
            "probe should set a loading row without blocking"
        );

        // While the probe is wedged off-actor, fire local actions; each must
        // yield a snapshot well under a second.
        for i in 0..10 {
            let query = format!("q{i}");
            let before = Instant::now();
            app.dispatch(HighlighterAppAction::SetSearchQuery { query });
            let _ = next_state(&rx);
            assert!(
                before.elapsed() < Duration::from_secs(1),
                "local action {i} must not be blocked by the wedged probe"
            );
        }

        // No actor dispatch should have blown the loop-stall budget (the probe
        // ran off-actor). Generous CI bound; design target is < 250ms.
        let max_ms = actor_max_handler_ms();
        assert!(
            max_ms < 2000,
            "max actor handler duration {max_ms}ms exceeded budget while a probe was wedged"
        );

        crate::relays::reset_relay_policy_for_test();
    }

    /// Acceptance 2 — account-creation regression (the on-device repro). With
    /// relays black-holed, a create-account flow is interleaved with a wedged
    /// Phase-1 probe; the account-create resolution must still reach a snapshot
    /// within its deadline. On the PRE-FIX code this fails by construction: the
    /// actor would be blocked inside the probe handler's `runtime.block_on`, so
    /// `AccountCreateResolved` could not be dequeued until the probe's 6s wait
    /// returned — and with a Class-A op ahead (360s) it would never return in
    /// time. Here the probe runs off-actor, so the create-account worker's own
    /// resolution is processed promptly.
    #[test]
    fn acceptance_account_creation_regression() {
        let port = spawn_black_hole_listener();
        let app = black_hole_app(port);
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        // Interleave a wedged probe ahead of the account-create flow.
        app.dispatch(HighlighterAppAction::OpenNetworkSettings);
        let _ = drain_states(&rx);
        app.dispatch(HighlighterAppAction::ProbeNetworkRelayNip11 {
            url: format!("ws://127.0.0.1:{port}"),
        });
        let _ = drain_states(&rx);

        app.dispatch(HighlighterAppAction::SetCreateAccountDisplayName {
            display_name: "Acceptance Tester".into(),
        });
        let _ = drain_states(&rx);
        app.dispatch(HighlighterAppAction::SubmitCreateAccount);

        // The create-account worker has a 30s deadline; the actor must dequeue
        // its resolution. Poll until `is_creating` clears (success or D6 error),
        // bounded generously above the worker deadline.
        let deadline = Instant::now() + Duration::from_secs(45);
        let mut resolved = false;
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_secs(2)) {
                Ok(TestUpdate::State(state)) => {
                    if !state.create_account.is_creating
                        && (state.create_account.created_user.is_some()
                            || state.create_account.error_message.is_some())
                    {
                        resolved = true;
                        break;
                    }
                }
                Ok(_) => {}
                Err(_) => {}
            }
        }
        assert!(
            resolved,
            "AccountCreateResolved must reach a snapshot within deadline even with a probe wedged"
        );

        crate::relays::reset_relay_policy_for_test();
    }

    /// Acceptance 3 — keyed supersession. `RelayProbe` slots are keyed by
    /// URL: probes for two different relays run independently (the relay
    /// list probes every row on appear), so BOTH must resolve. Same-URL
    /// supersession is still exercised: re-probing `/first` mid-flight
    /// bumps its generation and the row still resolves exactly once.
    #[test]
    fn acceptance_keyed_probes_resolve_independently() {
        let port = spawn_black_hole_listener();
        let app = black_hole_app(port);
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::OpenNetworkSettings);
        let _ = drain_states(&rx);

        // Two different relays back-to-back, then a same-URL re-probe of the
        // first (supersedes only its own slot, never the second relay's).
        app.dispatch(HighlighterAppAction::ProbeNetworkRelayNip11 {
            url: format!("ws://127.0.0.1:{port}/first"),
        });
        app.dispatch(HighlighterAppAction::ProbeNetworkRelayNip11 {
            url: format!("ws://127.0.0.1:{port}/second"),
        });
        app.dispatch(HighlighterAppAction::ProbeNetworkRelayNip11 {
            url: format!("ws://127.0.0.1:{port}/first"),
        });

        // Wait past the probe deadline; collect all resolved (non-loading) rows.
        let deadline = Instant::now() + Duration::from_secs(12);
        let mut resolved_urls: Vec<String> = Vec::new();
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(TestUpdate::State(state)) => {
                    for row in &state.network.nip11 {
                        if !row.is_loading && !resolved_urls.contains(&row.url) {
                            resolved_urls.push(row.url.clone());
                        }
                    }
                }
                Ok(_) => {}
                Err(_) => {}
            }
            // Stop early once both relays have resolved.
            if resolved_urls.iter().any(|u| u.ends_with("/first"))
                && resolved_urls.iter().any(|u| u.ends_with("/second"))
            {
                break;
            }
        }

        assert!(
            resolved_urls.iter().any(|u| u.ends_with("/second")),
            "second relay's probe must resolve"
        );
        assert!(
            resolved_urls.iter().any(|u| u.ends_with("/first")),
            "first relay's probe must resolve — keyed slots must not let another relay's probe abort it"
        );

        crate::relays::reset_relay_policy_for_test();
    }

    /// Acceptance 4 — logout mid-flight. Start a wedged probe, then sign out;
    /// after logout the op's domain data must not appear and busy flags clear.
    #[test]
    fn acceptance_logout_cancels_in_flight() {
        let port = spawn_black_hole_listener();
        let app = black_hole_app(port);
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::OpenNetworkSettings);
        let _ = drain_states(&rx);
        app.dispatch(HighlighterAppAction::ProbeNetworkRelayNip11 {
            url: format!("ws://127.0.0.1:{port}/inflight"),
        });
        let _ = drain_states(&rx);

        // Sign out mid-flight; cancel_all aborts the in-flight probe and bumps
        // its generation so any late resolution is dropped.
        app.dispatch(HighlighterAppAction::Logout);
        let post_logout = next_state(&rx);
        assert!(
            post_logout.network.nip11.is_empty(),
            "post-logout snapshot must not carry the in-flight probe's domain data"
        );

        // Give the aborted probe past its deadline; no resurrected row may appear.
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Ok(TestUpdate::State(state)) = rx.recv_timeout(Duration::from_secs(1)) {
                assert!(
                    state.network.nip11.is_empty(),
                    "a cancelled probe must not write into a logged-out session"
                );
            }
        }

        crate::relays::reset_relay_policy_for_test();
    }

    /// Acceptance 5 — loop-stall watchdog. Across a representative sequence of
    /// local actions (no network), the max single-handler dispatch duration
    /// must stay under the D8 budget. Generous 1000ms CI bound; design target
    /// is < 250ms; the 2000ms assertion absorbs CI variance.
    #[test]
    fn acceptance_loop_stall_watchdog() {
        reset_actor_max_handler_ms();
        let app = test_app();
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        let actions = [
            HighlighterAppAction::OpenHomeFeed,
            HighlighterAppAction::SetSearchQuery {
                query: "alpha".into(),
            },
            HighlighterAppAction::SetSearchQuery {
                query: "beta".into(),
            },
            HighlighterAppAction::OpenNetworkSettings,
            HighlighterAppAction::SetNetworkImportNpub {
                npub: "npub1xyz".into(),
            },
            HighlighterAppAction::AppForegrounded,
            HighlighterAppAction::RefreshAppChrome,
        ];
        for action in actions {
            app.dispatch(action);
            let _ = drain_states(&rx);
        }

        let max_ms = actor_max_handler_ms();
        assert!(
            max_ms < 2000,
            "max actor handler duration {max_ms}ms exceeded the loop-stall budget"
        );
    }

    /// Regression mechanism proof (design §4.6, account-creation repro).
    ///
    /// Demonstrates *by construction* why the pre-fix code froze on a dead
    /// network and why the fix resolves it, without depending on the full
    /// pre-fix tree. It models the kernel as a single loop thread draining a
    /// queue, plus a worker that posts a resolution — exactly the
    /// account-create worker / `AccountCreateResolved` relationship.
    ///
    /// - PRE-FIX shape: the loop runs a network handler INLINE via
    ///   `runtime.block_on(wedged_future)`. The worker posts its resolution
    ///   ~50ms in, but the loop cannot observe it until the wedge returns. We
    ///   assert the observe-latency is dominated by the wedge (the on-device
    ///   "stuck on Creating…" freeze). On the real pre-fix code this is the
    ///   360s/65s wait; here we bound the wedge so the suite can never hang.
    /// - FIXED shape: the loop submits the network work off-thread (like
    ///   `OpRunner::submit_op`) and keeps draining, so the resolution is
    ///   observed within the worker's ~50ms latency.
    #[test]
    fn regression_inline_block_on_starves_queued_resolution() {
        use std::sync::mpsc::sync_channel as sc;
        use tokio::io::AsyncReadExt;

        let port = spawn_black_hole_listener();
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("rt");

        // Bounded wedge: connect to the black hole, then a read that never
        // completes — models a relay ack that never arrives.
        let wedge = move || async move {
            let addr = format!("127.0.0.1:{port}");
            if let Ok(mut stream) = tokio::net::TcpStream::connect(&addr).await {
                let mut buf = [0u8; 1];
                let _ = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut buf)).await;
            }
        };

        // --- PRE-FIX shape: inline block_on on the loop thread. ---
        let prefix_latency = {
            let (qtx, qrx) = sc::<Instant>(8);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(50));
                let _ = qtx.send(Instant::now()); // resolution "posted" time
            });
            let rt_handle = rt.handle().clone();
            let wedge_fut = wedge.clone();
            let loop_start = Instant::now();
            // Inline blocking handler (the defect) on the loop thread.
            rt_handle.block_on(async {
                wedge_fut().await;
            });
            // Only now can the loop drain the resolution.
            let _posted_at = qrx
                .recv_timeout(Duration::from_millis(50))
                .expect("resolution queued");
            loop_start.elapsed()
        };

        // --- FIXED shape: submit off-thread, keep draining. ---
        let fixed_latency = {
            let (qtx, qrx) = sc::<Instant>(8);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(50));
                let _ = qtx.send(Instant::now());
            });
            let wedge_fut = wedge.clone();
            let loop_start = Instant::now();
            rt.spawn(async move {
                wedge_fut().await;
            });
            // Loop keeps draining immediately.
            let _posted_at = qrx
                .recv_timeout(Duration::from_secs(1))
                .expect("resolution queued");
            loop_start.elapsed()
        };

        // Pre-fix: the loop was blocked for the whole wedge (~3s) before it
        // could observe a resolution that was ready in 50ms.
        assert!(
            prefix_latency >= Duration::from_millis(2500),
            "pre-fix inline block_on must starve the queued resolution for the wedge duration \
             (observed {prefix_latency:?})"
        );
        // Fixed: the resolution is observed within the worker's latency.
        assert!(
            fixed_latency < Duration::from_millis(500),
            "fixed submit-off-thread must observe the queued resolution promptly \
             (observed {fixed_latency:?})"
        );

        crate::relays::reset_relay_policy_for_test();
    }

    // =======================================================================
    // Phase 2 (Class B publish helpers) acceptance coverage.
    // =======================================================================

    /// Phase 2 — OpDomain keying for toggles. A toggle on comment A must NOT
    /// share a slot with a toggle on comment B (independent), while the same
    /// comment + same kind shares a slot (supersession). Different kinds on the
    /// same comment are independent slots. This is the correctness contract
    /// that makes "last tap wins per target" hold without cross-target aborts.
    #[test]
    fn comment_interaction_keying_is_per_target_and_kind() {
        let a_like = OpDomain::CommentInteraction {
            target: op_target_hash("comment-a"),
            kind: CommentInteractionKind::Like,
        };
        let a_like_again = OpDomain::CommentInteraction {
            target: op_target_hash("comment-a"),
            kind: CommentInteractionKind::Like,
        };
        let b_like = OpDomain::CommentInteraction {
            target: op_target_hash("comment-b"),
            kind: CommentInteractionKind::Like,
        };
        let a_bookmark = OpDomain::CommentInteraction {
            target: op_target_hash("comment-a"),
            kind: CommentInteractionKind::Bookmark,
        };
        // Same comment + same kind => same slot (a double-tap supersedes).
        assert_eq!(a_like, a_like_again);
        // Different comment => different slot (no cross-target abort).
        assert_ne!(a_like, b_like);
        // Same comment, different kind => different slot.
        assert_ne!(a_like, a_bookmark);
        // Article-bookmark keying is likewise per-address.
        assert_ne!(
            OpDomain::ArticleBookmarkToggle {
                target: op_target_hash("addr-1")
            },
            OpDomain::ArticleBookmarkToggle {
                target: op_target_hash("addr-2")
            },
        );
    }

    /// Phase 2 — rapid-double-toggle supersession (last tap wins). Submits two
    /// ops for the SAME domain back-to-back against the black hole; the first
    /// is aborted and its (stale-generation) resolution dropped, so only the
    /// LAST submission's generation may resolve. Modeled directly on `OpRunner`
    /// so it needs no signed-in fixture, while exercising the exact mechanism
    /// the comment-like / follow toggles rely on.
    #[test]
    fn rapid_double_toggle_last_tap_wins() {
        use std::sync::mpsc::sync_channel;

        let (tx, rx) = sync_channel::<KernelMsg>(16);
        let mut ops = OpRunner::new(tx);

        // First tap: a long-running op (would resolve "stale").
        ops.submit_op(
            OpDomain::FollowToggle,
            Duration::from_secs(30),
            OpOutcome::FollowToggle {
                desired_following: true,
                previous_following: false,
                result: Err(op_timeout_message(OpDomain::FollowToggle)),
            },
            async {
                tokio::time::sleep(Duration::from_secs(10)).await;
                OpOutcome::FollowToggle {
                    desired_following: true,
                    previous_following: false,
                    result: Ok(()),
                }
            },
        );
        let first_generation = *ops.generations.get(&OpDomain::FollowToggle).unwrap();

        // Second tap (the LAST one): resolves quickly. This bumps the generation
        // and aborts the first.
        ops.submit_op(
            OpDomain::FollowToggle,
            Duration::from_secs(30),
            OpOutcome::FollowToggle {
                desired_following: false,
                previous_following: true,
                result: Err(op_timeout_message(OpDomain::FollowToggle)),
            },
            async {
                OpOutcome::FollowToggle {
                    desired_following: false,
                    previous_following: true,
                    result: Ok(()),
                }
            },
        );
        let last_generation = *ops.generations.get(&OpDomain::FollowToggle).unwrap();
        assert!(
            last_generation > first_generation,
            "second tap bumps generation"
        );

        // Collect resolutions for ~2s; only the last generation may appear, and
        // the first generation must be stale (dropped by the actor's check).
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut saw_last = false;
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(KernelMsg::OpResolved {
                    domain, generation, ..
                }) => {
                    assert_eq!(domain, OpDomain::FollowToggle);
                    // The first tap, if it sends at all, must be stale.
                    if generation == first_generation {
                        assert!(
                            ops.is_stale(domain, generation),
                            "first-tap resolution must be stale (superseded)"
                        );
                    }
                    if generation == last_generation {
                        assert!(
                            !ops.is_stale(domain, generation),
                            "last tap must be current"
                        );
                        saw_last = true;
                        break;
                    }
                }
                Ok(_) => {}
                Err(_) => {}
            }
        }
        assert!(
            saw_last,
            "the last tap's resolution must arrive and be current"
        );
    }

    /// Phase 2 — publish under a dead network. A Class-B publish domain
    /// (NetworkRelayWrite) is dispatched against the black hole: the busy flag
    /// must set immediately (op submitted, not awaited), unrelated local
    /// actions must each stay well under a second, and the busy flag must clear
    /// (success or D6 toast) within the deadline + epsilon. On the pre-fix code
    /// the actor would block inside the publish handler for up to 65s.
    #[test]
    fn acceptance_publish_under_dead_network() {
        let port = spawn_black_hole_listener();
        reset_actor_max_handler_ms();
        let app = black_hole_app(port);
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        app.dispatch(HighlighterAppAction::OpenNetworkSettings);
        let _ = drain_states(&rx);

        // Dispatch a relay-list write (Class B publish) against the black hole.
        app.dispatch(HighlighterAppAction::UpsertNetworkRelay {
            config: RelayConfig {
                url: format!("ws://127.0.0.1:{port}"),
                read: true,
                write: true,
                rooms: false,
                indexer: false,
            },
        });
        // The busy snapshot must arrive immediately (submitted, not awaited).
        let busy = next_state(&rx);
        assert!(
            busy.network.is_saving,
            "publish should set the saving flag without blocking the actor"
        );

        // Unrelated local actions must stay live while the publish is in flight.
        for i in 0..10 {
            let before = Instant::now();
            app.dispatch(HighlighterAppAction::SetSearchQuery {
                query: format!("q{i}"),
            });
            let _ = next_state(&rx);
            assert!(
                before.elapsed() < Duration::from_secs(1),
                "local action {i} must not be blocked by the in-flight publish"
            );
        }

        // The actor watchdog must never have seen a slow dispatch.
        let max_ms = actor_max_handler_ms();
        assert!(
            max_ms < 2000,
            "max actor handler {max_ms}ms exceeded budget while a publish was in flight"
        );

        // The busy flag must clear within the op deadline + epsilon (success or
        // D6 error), proving the resolution reached the actor.
        let deadline = Instant::now() + OP_DEADLINE_NETWORK + Duration::from_secs(15);
        let mut cleared = false;
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_secs(2)) {
                Ok(TestUpdate::State(state)) => {
                    if !state.network.is_saving {
                        cleared = true;
                        break;
                    }
                }
                Ok(_) => {}
                Err(_) => {}
            }
        }
        assert!(
            cleared,
            "the publish busy flag must clear within the deadline even on a dead network"
        );

        crate::relays::reset_relay_policy_for_test();
    }

    /// Phase 2 — loop-stall watchdog extended with one of each newly migrated
    /// action dispatched against the black hole. None may wedge the actor: the
    /// max single-handler dispatch must stay under the budget while every op is
    /// in flight off-actor.
    #[test]
    fn acceptance_phase2_actions_keep_loop_live() {
        let port = spawn_black_hole_listener();
        reset_actor_max_handler_ms();
        let app = black_hole_app(port);
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        // One representative action per newly migrated Class-B domain. Each must
        // submit-and-return; none may block the loop. (Validation-gated handlers
        // that early-return without a network op are still safe to dispatch.)
        let actions = [
            HighlighterAppAction::OpenNetworkSettings,
            HighlighterAppAction::UpsertNetworkRelay {
                config: RelayConfig {
                    url: format!("ws://127.0.0.1:{port}"),
                    read: true,
                    write: true,
                    rooms: false,
                    indexer: false,
                },
            },
            HighlighterAppAction::RemoveNetworkRelay {
                url: format!("ws://127.0.0.1:{port}"),
            },
            HighlighterAppAction::SetNetworkRelayRoles {
                url: format!("ws://127.0.0.1:{port}"),
                read: true,
                write: false,
                rooms: false,
                indexer: false,
            },
            HighlighterAppAction::ToggleArticleBookmark {
                address: "30023:deadbeef:slug".into(),
            },
            HighlighterAppAction::ToggleProfileFollow,
            HighlighterAppAction::PublishComment {
                parent_event_id: None,
            },
            HighlighterAppAction::PublishFeedbackNewThread,
            HighlighterAppAction::SubmitEditProfile,
            HighlighterAppAction::PublishArtifactShare {
                preview: preview_for_isbn("9780000000000", "Test Book"),
                group_id: "group".into(),
                note: None,
            },
            HighlighterAppAction::PublishRoomDiscussion {
                title: "Title".into(),
                body: "Body".into(),
                attachment_url: None,
            },
        ];
        for action in actions {
            let before = Instant::now();
            app.dispatch(action);
            let _ = drain_states(&rx);
            assert!(
                before.elapsed() < Duration::from_secs(2),
                "a migrated action wedged the actor loop"
            );
        }

        let max_ms = actor_max_handler_ms();
        assert!(
            max_ms < 2000,
            "max actor handler {max_ms}ms exceeded the loop-stall budget under Phase-2 actions"
        );

        crate::relays::reset_relay_policy_for_test();
    }

    /// Phase 3 — `handle_core_delta` is non-blocking under a black-holed relay.
    /// With every relay pointed at the black hole, open the heaviest set of
    /// panels (so the `CommunityUpserted` cascade actually runs home-feed,
    /// room-explorer, profile-view and article-reader refreshes), then inject a
    /// burst of `CommunityUpserted` deltas straight into the actor queue. Each
    /// delta cascade is pure ndb reads + synchronous subscription registration
    /// (Phase-3 audit), so the per-message watchdog must stay well under the
    /// loop-stall budget even though the network is dead. On a delta path that
    /// still awaited the network this would blow the budget (or hang).
    #[test]
    fn acceptance_phase3_core_delta_non_blocking_under_dead_network() {
        let port = spawn_black_hole_listener();
        reset_actor_max_handler_ms();
        let app = black_hole_app(port);
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        // Sign in so signed-in-scoped refreshes have a user, then open every
        // panel the heaviest delta arm cascades into.
        let keys = Keys::generate();
        let nsec = keys.secret_key().to_bech32().expect("nsec");
        app.dispatch(HighlighterAppAction::SignInNsec {
            nsec,
            persist: false,
            clear_stored_on_failure: false,
        });
        let _ = drain_states(&rx);
        app.dispatch(HighlighterAppAction::OpenHomeFeed);
        let _ = drain_states(&rx);
        app.dispatch(HighlighterAppAction::OpenRoomExplorer);
        let _ = drain_states(&rx);
        app.dispatch(HighlighterAppAction::OpenBookmarks);
        let _ = drain_states(&rx);

        // The watchdog may have recorded sign-in / panel-open work; reset so the
        // assertion measures only the delta cascades.
        reset_actor_max_handler_ms();

        // Inject a burst of heaviest-cascade deltas. Each must dispatch promptly;
        // the surrounding loop stays live (snapshots keep draining).
        for i in 0..10 {
            let before = Instant::now();
            app.inject_delta_for_test(DataChangeType::CommunityUpserted {
                group_id: format!("group-{i}"),
            });
            let _ = drain_states(&rx);
            assert!(
                before.elapsed() < Duration::from_secs(1),
                "core delta {i} cascade must not block on the dead network"
            );
        }

        let max_ms = actor_max_handler_ms();
        assert!(
            max_ms < 2000,
            "handle_core_delta cascade max {max_ms}ms blew the loop-stall budget under a dead relay"
        );

        crate::relays::reset_relay_policy_for_test();
    }

    /// Phase 4 — auth supersession across sign-in methods. nsec and bunker
    /// share `OpDomain::Auth` (preserving today's single `auth_generation`
    /// semantics): a rapid nsec-then-bunker sequence must drop the superseded
    /// nsec attempt's RESOLUTION — concretely, its `persist` side effect must
    /// never fire (credential persistence happens only in the resolution
    /// success arm, which the staleness check skips).
    ///
    /// Deliberately NOT asserted: that the nsec user is absent from chrome.
    /// `login_nsec` succeeds synchronously at the core level and surfaces
    /// through the `SignerConnected` delta, which is (by long-standing design)
    /// independent of facade-level supersession — a later failed bunker
    /// pairing must not silently log out a successfully signed-in user.
    #[test]
    fn auth_supersession_nsec_then_bunker_drops_stale_resolution() {
        let port = spawn_black_hole_listener();
        let app = black_hole_app(port);
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        let keys = Keys::generate();
        let nsec_pubkey = keys.public_key().to_hex();
        let nsec = keys.secret_key().to_bech32().expect("nsec");

        // Rapid sequence: nsec sign-in (persist: true — the canary) is
        // immediately superseded by a bunker pairing, which will time out
        // against the black hole. Both bump the shared auth generation; the
        // nsec resolution carries the stale generation and must be dropped,
        // so the nsec credential must never be persisted.
        app.dispatch(HighlighterAppAction::SignInNsec {
            nsec,
            persist: true,
            clear_stored_on_failure: false,
        });
        app.dispatch(HighlighterAppAction::PairBunker {
            uri: format!("bunker://{nsec_pubkey}?relay=ws://127.0.0.1:{port}"),
            persist: false,
            clear_stored_on_failure: false,
        });

        // Drain updates until the bunker attempt settles (error toast +
        // signing-in cleared) or the deadline passes, recording every
        // credential-persistence callback along the way.
        let deadline = Instant::now() + Duration::from_secs(40);
        let mut persisted_nsec = false;
        let mut settled_with_error = false;
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_secs(2)) {
                Ok(TestUpdate::PersistSessionCredential(HighlighterSessionCredential::Nsec {
                    ..
                })) => {
                    persisted_nsec = true;
                }
                Ok(TestUpdate::State(state)) => {
                    let errored = state
                        .toast
                        .as_ref()
                        .is_some_and(|t| matches!(t.kind, HighlighterToastKind::Error));
                    if !state.auth.is_signing_in && errored {
                        settled_with_error = true;
                        break;
                    }
                }
                Ok(_) => {}
                Err(_) => {}
            }
        }

        assert!(
            settled_with_error,
            "the bunker (last) attempt must settle: signing-in cleared with an error toast"
        );
        assert!(
            !persisted_nsec,
            "superseded nsec resolution must be dropped: its persist side effect must not fire"
        );

        crate::relays::reset_relay_policy_for_test();
    }

    /// Timeout-outcome regression: a room join wedged on a dead network must
    /// resolve through `apply_op_outcome` at the OpRunner deadline with its
    /// REAL payload — the live `group_id`, not the old zeroed fallback that
    /// the staleness/bookkeeping paths silently discarded. Observables: the
    /// loop stays live while the join is in flight, and the timeout surfaces
    /// as the room-explorer join error + Error toast (which only the Err arm
    /// of the JoinRoom outcome writes).
    #[test]
    fn join_room_timeout_resolves_with_live_payload() {
        let port = spawn_black_hole_listener();
        let app = black_hole_app(port);
        let (tx, rx) = channel();
        app.listen_for_updates(Arc::new(TestReconciler { tx }));
        let _ = next_state(&rx);

        let keys = Keys::generate();
        let nsec = keys.secret_key().to_bech32().expect("nsec");
        app.dispatch(HighlighterAppAction::SignInNsec {
            nsec,
            persist: false,
            clear_stored_on_failure: false,
        });
        let _ = wait_for_state(&rx, "signed-in state before join timeout", |state| {
            state.chrome.current_user.is_some()
        });
        app.dispatch(HighlighterAppAction::OpenRoomExplorer);
        let _ = drain_states(&rx);

        app.dispatch(HighlighterAppAction::RequestJoinRoom {
            group_id: "test-group-timeout".into(),
            room_name: "Timeout Room".into(),
        });

        // The busy snapshot must arrive promptly (design §4.1 invariant 2):
        // the explorer marks which group is mid-join before the op is awaited.
        // Late emissions from the sign-in/open sequence may still be queued,
        // so poll a bounded number of snapshots for the marker.
        let _ = wait_for_state(&rx, "join busy marker", |state| {
            state.room_explorer.joining_group_id == "test-group-timeout"
        });

        // The loop must stay live while the join waits on the black hole.
        let before_rev = app.state().rev;
        let before = Instant::now();
        app.dispatch(HighlighterAppAction::SearchOpened);
        let _ = wait_for_state(&rx, "actor processed search while join pending", |state| {
            state.rev > before_rev
        });
        assert!(
            before.elapsed() < Duration::from_secs(2),
            "actor must stay live while the join is in flight"
        );

        // The join timeout (30s deadline) must resolve via the Err arm:
        // room-explorer join error + Error toast.
        let _ = wait_for_state_with_timeout(
            &rx,
            "join timeout error state",
            OP_DEADLINE_NETWORK + Duration::from_secs(15),
            |state| {
                state.room_explorer.error_message.is_some()
                    && state
                        .toast
                        .as_ref()
                        .is_some_and(|t| matches!(t.kind, HighlighterToastKind::Error))
            },
        );
        // And the joining marker must have been cleared by the Err arm.
        let settled = app.state();
        assert!(
            settled.room_explorer.joining_group_id.is_empty(),
            "join timeout must clear the joining marker"
        );

        crate::relays::reset_relay_policy_for_test();
    }
}
