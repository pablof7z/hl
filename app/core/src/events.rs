//! Push-based change notifications from the Rust core into Swift. Mirrors
//! TENEX's `EventCallback` + `DataChangeType` pattern, with one extra layer:
//! every delta is wrapped in a [`Delta`] record that carries a
//! `subscription_id`, so Swift can route the change to the view-scoped store
//! that installed the subscription.

use crate::models::{
    ArtifactRecord, CommunitySummary, CurrentUser, DiscussionRecord, FeedbackEventRecord,
    HighlightRecord, HydratedHighlight,
};

#[derive(Debug, Clone, uniffi::Enum)]
pub enum DataChangeType {
    CommunityUpserted { community: CommunitySummary },
    MembershipChanged { group_id: String },
    ArtifactUpserted { group_id: String, artifact: ArtifactRecord },
    DiscussionUpserted {
        group_id: String,
        discussion: DiscussionRecord,
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
    MyHighlightUpserted { highlight: HighlightRecord },
    /// Something that affects the profile view for `pubkey` arrived. `kind`
    /// is the event kind (0 metadata, 3 contacts, 30023 article, 9802
    /// highlight, 39001/39002 membership) so the Swift store can re-query
    /// just the affected slice.
    UserProfileUpdated { pubkey: String, kind: u32 },
    /// Something that affects the article reader for `address`
    /// (`30023:<pubkey>:<d>`) arrived. `kind` is `30023` when the article
    /// body/metadata itself changed (replaceable supersession) or `9802`
    /// when a new highlight was published against it.
    ArticleUpdated { address: String, kind: u32 },
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
    FeedbackThreadEventUpserted { event: FeedbackEventRecord },
    /// NIP-46 signer connected — fires after a remote signer completes the
    /// `nostrconnect://` or `bunker://` handshake.
    SignerConnected { user: CurrentUser },
    /// NIP-46 signer is requesting user approval to sign an event (for the
    /// rare case our own core is acting as a signer — MVP does not act as
    /// one, but keeping the variant here matches TENEX's shape).
    BunkerSignRequest { request_id: String },
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
