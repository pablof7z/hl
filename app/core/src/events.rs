//! Push-based change notifications from the Rust core into Swift. Mirrors
//! TENEX's `EventCallback` + `DataChangeType` pattern, with one extra layer:
//! every delta is wrapped in a [`Delta`] record that carries a
//! `subscription_id`, so Swift can route the change to the view-scoped store
//! that installed the subscription.

use crate::models::{
    ArticleUpdateAction, ArtifactRecord, ChatMessageRecord, CommunitySummary, CurrentUser,
    DiscussionRecord, FeedbackEventRecord, HighlightRecord, HydratedHighlight, ProfileUpdateAction,
    RelayDiagnostic, RelayStatus,
};
use crate::nostr_entities::NostrEntityEvent;

const KIND_METADATA: u32 = 0;
const KIND_CONTACTS: u32 = 3;
const KIND_HIGHLIGHT: u32 = 9802;
const KIND_LONG_FORM: u32 = 30023;
const KIND_GROUP_ADMINS: u32 = 39001;
const KIND_GROUP_MEMBERS: u32 = 39002;

pub fn article_update_action(kind: u32) -> ArticleUpdateAction {
    match kind {
        KIND_LONG_FORM => ArticleUpdateAction::RefreshArticle,
        KIND_HIGHLIGHT => ArticleUpdateAction::RefreshHighlights,
        _ => ArticleUpdateAction::Ignore,
    }
}

pub fn profile_update_action(kind: u32) -> ProfileUpdateAction {
    match kind {
        KIND_METADATA => ProfileUpdateAction::RefreshProfile,
        KIND_CONTACTS => ProfileUpdateAction::RefreshFollowState,
        KIND_LONG_FORM => ProfileUpdateAction::RefreshArticles,
        KIND_HIGHLIGHT => ProfileUpdateAction::RefreshHighlights,
        KIND_GROUP_ADMINS | KIND_GROUP_MEMBERS => ProfileUpdateAction::RefreshCommunities,
        _ => ProfileUpdateAction::Ignore,
    }
}

// UniFFI serializes these deltas as bounded FFI records. Keeping the enum
// payloads inline preserves the generated Swift shape and avoids moving Rust
// event ownership into native shell code.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, uniffi::Enum)]
pub enum DataChangeType {
    CommunityUpserted {
        community: CommunitySummary,
    },
    MembershipChanged {
        group_id: String,
    },
    /// Rust-owned app toast. Native shell renders and dismisses it; Rust owns
    /// when the message exists and what it says.
    AppToastRequested {
        message: String,
    },
    ArtifactUpserted {
        group_id: String,
        artifact: ArtifactRecord,
    },
    DiscussionUpserted {
        group_id: String,
        discussion: DiscussionRecord,
    },
    /// A NIP-29 kind:9 chat message arrived for `group_id`. The Swift
    /// chat store appends it to its message list (ordered by `created_at`).
    ChatMessageUpserted {
        group_id: String,
        message: ChatMessageRecord,
    },
    HighlightUpserted {
        group_id: String,
        highlight: HydratedHighlight,
    },
    /// A kind:16 cross-community share of a highlight was received.
    HighlightShared {
        group_id: String,
        highlight_id: String,
        shared_by_pubkey: String,
    },
    MyHighlightUpserted {
        highlight: HighlightRecord,
    },
    /// Something that affects the profile view for `pubkey` arrived. `kind`
    /// is the event kind; Rust's `profile_update_action` defines the
    /// reload slice so native shells don't duplicate protocol policy.
    UserProfileUpdated {
        pubkey: String,
        kind: u32,
    },
    /// Something that affects the article reader for `address`
    /// (`30023:<pubkey>:<d>`) arrived. `kind` is the event kind; Rust's
    /// `article_update_action` defines which reader slice to refresh.
    ArticleUpdated {
        address: String,
        kind: u32,
    },
    /// The Following Reads feed has a new data point — either a follow
    /// published a new article, or a follow interacted with one. The Swift
    /// store re-queries the full feed on each delta (dedupe + sort is
    /// cheap). No payload beyond the trigger — keep deltas small.
    FollowingReadsUpdated,
    /// A new kind:9802 highlight showed up from a follow or in a joined
    /// room — trigger a re-query of the Highlights home feed.
    FollowingHighlightsUpdated,
    /// A kind:1 root note authored by the user, or a kind:513 metadata event
    /// for any of their threads, arrived. The Swift store re-queries the
    /// thread list on each (the 513 may have updated a title/summary on an
    /// existing row, which is easier to handle with a re-query than an in-place
    /// patch).
    FeedbackThreadsUpdated,
    /// A kind:1 message inside an open feedback thread arrived. The Swift
    /// store inserts/upserts it into the chat view ordered by `created_at`.
    FeedbackThreadEventUpserted {
        event: FeedbackEventRecord,
    },
    /// A NIP-50 relay search returned new kind:30023 events. The Swift store
    /// re-reads Rust's article snapshot on receipt; payload is the query the
    /// subscription was opened with (so a stale pump can't update a newer
    /// query's bucket).
    SearchArticlesUpdated {
        query: String,
    },
    /// The current user's NIP-51 kind:10003 bookmark list was updated
    /// (either by us via `toggle_bookmark` or by another client relaying a
    /// newer event). App-scope delta — Swift re-queries the authoritative
    /// list from nostrdb.
    BookmarksUpdated,
    /// One of the current user's kind:30003 / kind:30004 sets changed.
    /// View-scoped — the BookmarkStore re-queries on receipt.
    BookmarkSetsUpdated,
    /// A kind:30004 curation set from a followed author arrived.
    /// View-scoped — the BookmarkStore re-queries the explore list.
    FollowingCurationSetsUpdated,
    /// A NIP-B0 kind:39701 web bookmark from the current user changed.
    /// View-scoped — the BookmarkStore re-queries on receipt.
    WebBookmarksUpdated,
    /// A referenced NIP-19 entity resolved from nostrdb after its
    /// view-scoped subscription warmed the cache. Swift applies the payload
    /// directly to the card that installed the subscription.
    NostrEntityResolved {
        event: NostrEntityEvent,
    },
    /// NIP-46 signer connected — fires after a remote signer completes the
    /// `nostrconnect://` or `bunker://` handshake.
    SignerConnected {
        user: CurrentUser,
    },
    /// NIP-46 signer is requesting user approval to sign an event (for the
    /// rare case our own core is acting as a signer — MVP does not act as
    /// one, but keeping the variant here matches TENEX's shape).
    BunkerSignRequest {
        request_id: String,
    },
    /// A relay in the user's pool changed connection state. Swift re-reads
    /// `get_relay_diagnostics` on receipt to refresh per-row status dots,
    /// latency, and traffic counters.
    RelayStatusChanged {
        url: String,
        state: RelayStatus,
    },
    /// Bounded app-scope relay diagnostics projection. Emitted by the Rust
    /// diagnostics task when any relay row changes, including RTT / traffic
    /// counters that do not necessarily alter connection state.
    RelayDiagnosticsUpdated {
        diagnostics: Vec<RelayDiagnostic>,
    },
}

/// Every delta delivered to Swift. The `subscription_id` routes the change
/// to the specific Swift store that installed the subscription. `0` is
/// reserved for app-scoped deltas (signer state, joined-communities summary).
#[derive(Debug, Clone, uniffi::Record)]
pub struct Delta {
    pub subscription_id: u64,
    pub change: DataChangeType,
}

#[uniffi::export(with_foreign)]
pub trait EventCallback: Send + Sync {
    fn on_data_changed(&self, delta: Delta);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn article_update_action_maps_reader_slices() {
        assert_eq!(
            article_update_action(KIND_LONG_FORM),
            ArticleUpdateAction::RefreshArticle
        );
        assert_eq!(
            article_update_action(KIND_HIGHLIGHT),
            ArticleUpdateAction::RefreshHighlights
        );
        assert_eq!(article_update_action(1), ArticleUpdateAction::Ignore);
    }

    #[test]
    fn profile_update_action_maps_profile_slices() {
        assert_eq!(
            profile_update_action(KIND_METADATA),
            ProfileUpdateAction::RefreshProfile
        );
        assert_eq!(
            profile_update_action(KIND_CONTACTS),
            ProfileUpdateAction::RefreshFollowState
        );
        assert_eq!(
            profile_update_action(KIND_LONG_FORM),
            ProfileUpdateAction::RefreshArticles
        );
        assert_eq!(
            profile_update_action(KIND_HIGHLIGHT),
            ProfileUpdateAction::RefreshHighlights
        );
        assert_eq!(
            profile_update_action(KIND_GROUP_ADMINS),
            ProfileUpdateAction::RefreshCommunities
        );
        assert_eq!(
            profile_update_action(KIND_GROUP_MEMBERS),
            ProfileUpdateAction::RefreshCommunities
        );
        assert_eq!(profile_update_action(1), ProfileUpdateAction::Ignore);
    }
}
