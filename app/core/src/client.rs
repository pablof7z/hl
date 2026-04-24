//! Top-level UniFFI-exposed object. Swift holds one `HighlighterCore` for
//! the life of the app.
//!
//! State discipline: async methods never hold the `parking_lot` guard across
//! an `.await` point (the guard isn't `Send`). Long-running protocol work
//! happens in `Session` / feature modules, which own their own async state.

use std::sync::Arc;

use nostr_sdk::prelude::*;
use parking_lot::RwLock;

use crate::articles;
use crate::blossom;
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
    ArticleRecord, ArtifactPreview, ArtifactRecord, BlossomUpload, CommunitySummary, CurrentUser,
    DiscussionRecord, FeedbackEventRecord, FeedbackThreadRecord, HighlightDraft, HighlightRecord,
    HydratedHighlight, NostrConnectOptions, PictureDraft, PictureRecord, ProfileMetadata,
    ReadingFeedItem, RoomRecommendation,
};
use crate::reads;
use crate::recommendations;
use crate::nip46::{self, BunkerSigner};
use crate::nostr_runtime::NostrRuntime;
use crate::profile;
use crate::relays::NOSTR_CONNECT_RELAY;
use crate::session::{current_user_from_pubkey, Session};
use crate::subscriptions::{SubscriptionKind, SubscriptionRegistry};

#[derive(uniffi::Object)]
pub struct HighlighterCore {
    inner: Arc<RwLock<Inner>>,
    runtime: Arc<NostrRuntime>,
    /// Shared with every pump task so `set_event_callback` can replace the
    /// callback atomically mid-flight.
    callback_slot: Arc<RwLock<Option<Arc<dyn EventCallback>>>>,
    subscriptions: Arc<SubscriptionRegistry>,
}

struct Inner {
    session: Session,
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

    pub fn login_nsec(&self, nsec: String) -> Result<CurrentUser, CoreError> {
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
            self.runtime
                .spawn_apply_user_relay_config(pubkey.to_hex());
            let sub_id = self.runtime.spawn_membership_subscription(pubkey);
            let contacts_id = self.runtime.spawn_contacts_subscription(pubkey);
            // Eagerly fetch 39000 metadata for any groups already in the
            // nostrdb cache. Without this, the first `getJoinedCommunities`
            // call on a warm cache would return summaries with name=id because
            // the stage-2 metadata sub would only be installed after the pump
            // sees a live membership delta.
            let cached_ids = crate::subscriptions::collect_cached_group_ids(
                self.runtime.ndb(),
                &pubkey,
            );
            if !cached_ids.is_empty() {
                self.runtime
                    .spawn_group_metadata_subscription(cached_ids.into_iter().collect());
            }
            let mut guard = self.inner.write();
            guard.session.set_membership_subscription(sub_id);
            guard.session.set_contacts_subscription(contacts_id);
        }
        Ok(user)
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
        }
        self.inner.write().session.logout();
        self.runtime.unset_signer();
    }

    pub fn current_user(&self) -> Option<CurrentUser> {
        self.inner.read().session.current_user()
    }

    // -- Auth (async) --
    // Async auth flows delegate without holding the parking_lot guard across
    // await. The session module is responsible for thread-safe internal state.

    pub async fn start_nostr_connect(
        &self,
        options: NostrConnectOptions,
    ) -> Result<String, CoreError> {
        // Local ephemeral keypair. The remote signer uses this pubkey to
        // address its messages to us over the relay; after pair completion
        // the user's pubkey comes from the remote signer via GetPublicKey.
        let local_keys = Keys::generate();
        let secret = nip46::random_secret();

        let uri = nip46::build_nostr_connect_uri(
            local_keys.public_key(),
            NOSTR_CONNECT_RELAY,
            &options.name,
            &options.url,
            &options.image,
            &options.perms,
            &secret,
        )?;

        // Ensure the NIP-46 relay is part of the pool before we start
        // listening for the inbound `connect` request. `add_relay` is a
        // no-op if the relay is already known — but we can't rely on the
        // initial pool reconcile having completed yet.
        let client = self.runtime.client().clone();
        if let Err(e) = client.add_relay(NOSTR_CONNECT_RELAY).await {
            tracing::warn!(relay = %NOSTR_CONNECT_RELAY, error = %e, "add_relay");
        }
        client.connect().await;

        // Spawn a background task that waits for the remote signer to
        // connect and then installs the resulting BunkerSigner. The task
        // must own: the client (for set_signer after pairing), the callback
        // slot (to fire SignerConnected), the Session guard slot (to store
        // the active signer), and the local keys.
        let inner = self.inner.clone();
        let runtime = self.runtime.clone();
        let callback_slot = self.callback_slot.clone();
        self.runtime
            .runtime_handle()
            .spawn(async move {
                let result =
                    BunkerSigner::await_inbound(client.clone(), local_keys, Some(secret)).await;
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
                        runtime.set_signer((*signer).clone());
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

        Ok(uri)
    }

    pub async fn pair_bunker(&self, uri: String) -> Result<CurrentUser, CoreError> {
        let normalized = normalize_bunker_uri(&uri);
        if normalized.is_empty() {
            return Err(CoreError::InvalidInput("empty bunker URI".into()));
        }

        let client = self.runtime.client().clone();
        let (signer, user_pubkey) = BunkerSigner::pair(client, &normalized).await?;
        let user = current_user_from_pubkey(&user_pubkey)?;

        let signer = Arc::new(signer);
        self.runtime.set_signer((*signer).clone());
        self.runtime
            .spawn_apply_user_relay_config(user_pubkey.to_hex());

        let sub_id = self
            .runtime
            .spawn_membership_subscription(user_pubkey);
        let contacts_id = self.runtime.spawn_contacts_subscription(user_pubkey);
        let cached_ids = crate::subscriptions::collect_cached_group_ids(
            self.runtime.ndb(),
            &user_pubkey,
        );
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

    /// App-scope subscription for the joined-communities view. Returns a
    /// handle; fires CommunityUpserted / MembershipChanged deltas tagged
    /// with that handle. Re-uses the relay sub installed at login; this
    /// call is about setting up the nostrdb notification pump.
    pub async fn subscribe_joined_communities(&self) -> Result<u64, CoreError> {
        let user_pubkey = self.require_user_pubkey()?;
        self.subscriptions.register(
            &self.runtime,
            SubscriptionKind::JoinedCommunities { user_pubkey },
        )
    }

    /// Per-room view-scope subscription. Returns a handle; fires
    /// ArtifactUpserted / HighlightUpserted / HighlightShared for this
    /// specific group.
    pub async fn subscribe_room(&self, group_id: String) -> Result<u64, CoreError> {
        if group_id.trim().is_empty() {
            return Err(CoreError::InvalidInput("group_id must not be empty".into()));
        }
        self.subscriptions
            .register(&self.runtime, SubscriptionKind::Room { group_id })
    }

    /// Per-room Discussions view-scope subscription. Returns a handle; fires
    /// `DiscussionUpserted` deltas for kind:11 threads in this group that
    /// carry the `t=discussion` marker.
    pub async fn subscribe_room_discussions(&self, group_id: String) -> Result<u64, CoreError> {
        if group_id.trim().is_empty() {
            return Err(CoreError::InvalidInput("group_id must not be empty".into()));
        }
        self.subscriptions
            .register(&self.runtime, SubscriptionKind::RoomDiscussions { group_id })
    }

    /// Vault view-scope subscription for the current user's own highlights.
    pub async fn subscribe_vault(&self) -> Result<u64, CoreError> {
        let user_pubkey = self.require_user_pubkey()?;
        self.subscriptions
            .register(&self.runtime, SubscriptionKind::Vault { user_pubkey })
    }

    /// Profile view-scope subscription. Fires `UserProfileUpdated` deltas
    /// when any event relevant to `pubkey_hex`'s profile arrives. Install on
    /// profile view appearance; `unsubscribe(handle)` on disappearance.
    pub async fn subscribe_user_profile(&self, pubkey_hex: String) -> Result<u64, CoreError> {
        let pubkey = PublicKey::from_hex(pubkey_hex.trim())
            .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;
        self.subscriptions
            .register(&self.runtime, SubscriptionKind::UserProfile { pubkey })
    }

    /// Following Reads view-scope subscription. Snapshots the user's current
    /// follow list, then listens for: (a) new articles authored by a follow,
    /// (b) interactions by a follow against any kind:30023 content. Fires
    /// `FollowingReadsUpdated` deltas; the Swift store re-queries the feed.
    /// Install on tab appearance; `unsubscribe(handle)` on disappearance.
    pub async fn subscribe_following_reads(&self) -> Result<u64, CoreError> {
        let user_pubkey = self.require_user_pubkey()?;
        let follow_hex_strings =
            follows::query_follows(self.runtime.ndb(), &user_pubkey.to_hex())?;
        let follows_pks: Vec<PublicKey> = follow_hex_strings
            .iter()
            .filter_map(|s| PublicKey::from_hex(s.trim()).ok())
            .collect();
        self.subscriptions.register(
            &self.runtime,
            SubscriptionKind::FollowingReads {
                follows: follows_pks,
            },
        )
    }

    /// Highlights home-feed view-scope subscription. Snapshots the user's
    /// current follow list (plus self — nobody lists themselves in kind:3
    /// but we want our own highlights in the home feed) and joined-group
    /// ids, then listens for kind:9802 events authored by anyone in that
    /// set or tagged into any joined room.
    pub async fn subscribe_following_highlights(&self) -> Result<u64, CoreError> {
        let user_pubkey = self.require_user_pubkey()?;
        let follow_hex_strings =
            follows::query_follows(self.runtime.ndb(), &user_pubkey.to_hex())?;
        let mut follows_pks: Vec<PublicKey> = follow_hex_strings
            .iter()
            .filter_map(|s| PublicKey::from_hex(s.trim()).ok())
            .collect();
        if !follows_pks.iter().any(|pk| *pk == user_pubkey) {
            follows_pks.push(user_pubkey);
        }
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
    }

    /// Article-reader view-scope subscription. Fires `ArticleUpdated` deltas
    /// whenever the article's replaceable body supersedes OR a new kind:9802
    /// highlighting this article's `a`-tag arrives. Install on reader view
    /// appearance; `unsubscribe(handle)` on disappearance.
    pub async fn subscribe_article(
        &self,
        pubkey_hex: String,
        d_tag: String,
    ) -> Result<u64, CoreError> {
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
    }

    /// Feedback-threads subscription for the shake-to-share surface. Fires
    /// `FeedbackThreadsUpdated` deltas whenever a kind:1 root authored by
    /// the current user (with the project `a` tag) or any kind:513 metadata
    /// for the same project arrives. Swift re-queries on each.
    pub async fn subscribe_feedback_threads(
        &self,
        coordinate: String,
    ) -> Result<u64, CoreError> {
        let coordinate = coordinate.trim();
        if coordinate.is_empty() {
            return Err(CoreError::InvalidInput("coordinate must not be empty".into()));
        }
        let user_pubkey = self.require_user_pubkey()?;
        self.subscriptions.register(
            &self.runtime,
            SubscriptionKind::FeedbackThreads {
                coordinate: coordinate.to_string(),
                current_user_pubkey: user_pubkey,
            },
        )
    }

    /// Per-thread feedback subscription. Fires `FeedbackThreadEventUpserted`
    /// deltas for every kind:1 `e`-tagged to the root (regardless of author).
    pub async fn subscribe_feedback_thread(
        &self,
        root_event_id: String,
    ) -> Result<u64, CoreError> {
        let root_event_id = root_event_id.trim();
        if root_event_id.is_empty() {
            return Err(CoreError::InvalidInput(
                "root_event_id must not be empty".into(),
            ));
        }
        let root = EventId::from_hex(root_event_id)
            .map_err(|e| CoreError::InvalidInput(format!("invalid event id: {e}")))?;
        self.subscriptions
            .register(&self.runtime, SubscriptionKind::FeedbackThread { root_event_id: root })
    }

    /// Drop a subscription by handle. Idempotent.
    pub fn unsubscribe(&self, handle: u64) {
        self.subscriptions.remove(&self.runtime, handle);
    }

    // -- Reads --

    pub async fn get_joined_communities(&self) -> Result<Vec<CommunitySummary>, CoreError> {
        let Some(user) = self.inner.read().session.current_user() else {
            return Err(CoreError::NotAuthenticated);
        };
        groups::query_joined_communities_from_ndb(self.runtime.ndb(), &user.pubkey)
    }

    pub async fn get_artifacts(
        &self,
        group_id: String,
        limit: u32,
    ) -> Result<Vec<ArtifactRecord>, CoreError> {
        crate::artifacts::query_for_group(self.runtime.ndb(), &group_id, limit)
    }

    pub async fn get_highlights(
        &self,
        group_id: String,
        limit: u32,
    ) -> Result<Vec<HydratedHighlight>, CoreError> {
        crate::highlights::query_for_group(self.runtime.ndb(), &group_id, limit)
    }

    pub async fn get_my_highlights(&self, limit: u32) -> Result<Vec<HighlightRecord>, CoreError> {
        let Some(user) = self.inner.read().session.current_user() else {
            return Err(CoreError::NotAuthenticated);
        };
        highlights::query_highlights_by_author(self.runtime.ndb(), &user.pubkey, limit)
    }

    /// Following Reads feed — articles surfaced through the user's follow
    /// graph. See `reads::query_following_reads` for semantics. Returns an
    /// empty list if the user isn't logged in or has no follows cached yet.
    pub async fn get_following_reads(
        &self,
        limit: u32,
    ) -> Result<Vec<ReadingFeedItem>, CoreError> {
        let Some(user) = self.inner.read().session.current_user() else {
            return Err(CoreError::NotAuthenticated);
        };
        reads::query_following_reads(self.runtime.ndb(), &user.pubkey, limit)
    }

    /// Highlights home feed — kind:9802 events authored by follows plus
    /// highlights tagged into joined rooms. See
    /// `highlights::query_following_highlights` for semantics.
    pub async fn get_following_highlights(
        &self,
        limit: u32,
    ) -> Result<Vec<HydratedHighlight>, CoreError> {
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
    }

    // -- Profile reads (per-pubkey, no auth required) --

    pub async fn get_user_profile(
        &self,
        pubkey_hex: String,
    ) -> Result<Option<ProfileMetadata>, CoreError> {
        profile::query_profile_from_ndb(self.runtime.ndb(), pubkey_hex.trim())
    }

    pub async fn get_user_articles(
        &self,
        pubkey_hex: String,
        limit: u32,
    ) -> Result<Vec<ArticleRecord>, CoreError> {
        articles::query_articles_by_author(self.runtime.ndb(), pubkey_hex.trim(), limit)
    }

    /// Read a single NIP-23 article by author + `d` tag from nostrdb. `None`
    /// if ndb hasn't cached it yet — the reader's `subscribe_article` pump
    /// backfills via relays, and a later call returns `Some`.
    pub async fn get_article(
        &self,
        pubkey_hex: String,
        d_tag: String,
    ) -> Result<Option<ArticleRecord>, CoreError> {
        articles::query_article(self.runtime.ndb(), pubkey_hex.trim(), d_tag.trim())
    }

    /// Read all highlights referencing the given NIP-23 article address
    /// (`30023:<pubkey>:<d>`) from nostrdb, newest first.
    pub async fn get_highlights_for_article(
        &self,
        address: String,
        limit: u32,
    ) -> Result<Vec<HighlightRecord>, CoreError> {
        highlights::query_for_article(self.runtime.ndb(), address.trim(), limit)
    }

    pub async fn get_user_highlights(
        &self,
        pubkey_hex: String,
        limit: u32,
    ) -> Result<Vec<HighlightRecord>, CoreError> {
        highlights::query_highlights_by_author(self.runtime.ndb(), pubkey_hex.trim(), limit)
    }

    pub async fn get_user_communities(
        &self,
        pubkey_hex: String,
    ) -> Result<Vec<CommunitySummary>, CoreError> {
        groups::query_joined_communities_from_ndb(self.runtime.ndb(), pubkey_hex.trim())
    }

    // -- Follow state (kind:3) --

    /// Returns true if the logged-in user's cached contact list currently
    /// includes `target_pubkey_hex`.
    pub async fn is_following(&self, target_pubkey_hex: String) -> Result<bool, CoreError> {
        let Some(user) = self.inner.read().session.current_user() else {
            return Err(CoreError::NotAuthenticated);
        };
        follows::is_following(self.runtime.ndb(), &user.pubkey, target_pubkey_hex.trim())
    }

    /// Publish a new kind:3 that adds (`follow=true`) or removes
    /// (`follow=false`) `target_pubkey_hex` from the logged-in user's contact
    /// list. Returns the new event id, or `None` if already in the desired
    /// state (no republish).
    pub async fn set_follow(
        &self,
        target_pubkey_hex: String,
        follow: bool,
    ) -> Result<Option<String>, CoreError> {
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

    /// Recent books across the user's joined communities — drives the
    /// capture-flow book picker. Returns `[]` if no books are cached or the
    /// user isn't logged in.
    pub async fn get_recent_books(&self, limit: u32) -> Result<Vec<ArtifactRecord>, CoreError> {
        let Some(user) = self.inner.read().session.current_user() else {
            return Ok(Vec::new());
        };
        crate::recent_books::query_recent_books(self.runtime.ndb(), &user.pubkey, limit)
    }

    pub async fn search_artifacts(
        &self,
        _query: String,
        _limit: u32,
    ) -> Result<Vec<ArtifactRecord>, CoreError> {
        Err(CoreError::NotInitialized)
    }

    // -- Search: across local nostrdb (all four surfaces) + NIP-50 relay ---

    pub async fn search_highlights(
        &self,
        query: String,
        limit: u32,
    ) -> Result<Vec<HighlightRecord>, CoreError> {
        crate::search::search_highlights(self.runtime.ndb(), &query, limit)
    }

    pub async fn search_articles(
        &self,
        query: String,
        limit: u32,
    ) -> Result<Vec<ArticleRecord>, CoreError> {
        crate::search::search_articles(self.runtime.ndb(), &query, limit)
    }

    pub async fn search_communities(
        &self,
        query: String,
        limit: u32,
    ) -> Result<Vec<CommunitySummary>, CoreError> {
        crate::search::search_communities(self.runtime.ndb(), &query, limit)
    }

    pub async fn search_profiles(
        &self,
        query: String,
        limit: u32,
    ) -> Result<Vec<ProfileMetadata>, CoreError> {
        crate::search::search_profiles(self.runtime.ndb(), &query, limit)
    }

    /// Resolve the merged set of NIP-50 search relays for the current user —
    /// always includes `wss://relay.highlighter.com`, plus every `relay` tag
    /// from the newest cached kind:10007 (NIP-51 search relay list).
    pub async fn get_search_relays(&self) -> Result<Vec<String>, CoreError> {
        let user_hex = self
            .inner
            .read()
            .session
            .current_user()
            .map(|u| u.pubkey)
            .unwrap_or_default();
        crate::search::query_search_relays(self.runtime.ndb(), &user_hex)
    }

    /// Open a NIP-50 relay subscription for kind:30023 against the user's
    /// search relays. Returns a handle; the pump fires
    /// `SearchArticlesUpdated { query }` deltas as matching events ingest,
    /// and the Swift store responds by re-running `search_articles` locally
    /// to merge the new events into its Articles bucket.
    pub async fn subscribe_article_search(&self, query: String) -> Result<u64, CoreError> {
        let trimmed = query.trim().to_string();
        if trimmed.is_empty() {
            return Err(CoreError::InvalidInput("search query must not be empty".into()));
        }
        let relays = self.get_search_relays().await?;
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

    pub async fn lookup_isbn(&self, isbn: String) -> Result<ArtifactPreview, CoreError> {
        isbn_lookup::lookup_isbn(&isbn).await
    }

    /// Build an `ArtifactPreview` from a bare URL. Used by the iOS Share
    /// Extension flow — the main app drains the share queue, normalizes each
    /// URL through this, then calls `publish_artifact` to post the kind:11.
    pub async fn build_preview_from_url(
        &self,
        url: String,
    ) -> Result<ArtifactPreview, CoreError> {
        crate::artifacts::build_preview(&url)
    }

    pub async fn get_discussions(
        &self,
        group_id: String,
        limit: u32,
    ) -> Result<Vec<DiscussionRecord>, CoreError> {
        crate::discussions::query_for_group(self.runtime.ndb(), &group_id, limit)
    }

    // -- Feedback (shake-to-share) --

    /// Threads scoped to `coordinate` authored by the current user. Returns
    /// an empty list if not logged in.
    pub async fn get_feedback_threads(
        &self,
        coordinate: String,
    ) -> Result<Vec<FeedbackThreadRecord>, CoreError> {
        let user = match self.inner.read().session.current_user() {
            Some(u) => u,
            None => return Ok(Vec::new()),
        };
        feedback::query_threads(self.runtime.ndb(), &coordinate, &user.pubkey)
    }

    /// Every message in a feedback thread, ordered ascending by `created_at`.
    pub async fn get_feedback_thread_events(
        &self,
        root_event_id: String,
    ) -> Result<Vec<FeedbackEventRecord>, CoreError> {
        feedback::query_thread_events(self.runtime.ndb(), &root_event_id)
    }

    /// First `p` tag of the project's kind:31933 event by addressable
    /// coordinate. The shake-to-share composer uses this to pick the agent
    /// pubkey for the root note's `p` tag. `None` if the project event isn't
    /// cached or has no agents.
    pub async fn get_project_first_agent_pubkey(
        &self,
        coordinate: String,
    ) -> Result<Option<String>, CoreError> {
        feedback::query_first_agent_pubkey(self.runtime.ndb(), &coordinate)
    }

    // -- Writes --

    pub async fn publish_artifact(
        &self,
        preview: ArtifactPreview,
        group_id: String,
        note: Option<String>,
    ) -> Result<ArtifactRecord, CoreError> {
        let _ = self.require_user_pubkey()?;
        crate::artifacts::publish(&self.runtime, preview, &group_id, note.as_deref()).await
    }

    pub async fn publish_discussion(
        &self,
        group_id: String,
        title: String,
        body: String,
        attachment: Option<ArtifactPreview>,
    ) -> Result<DiscussionRecord, CoreError> {
        let _ = self.require_user_pubkey()?;
        crate::discussions::publish(&self.runtime, &group_id, &title, &body, attachment).await
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
    ) -> Result<FeedbackEventRecord, CoreError> {
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

    pub async fn publish_highlights_and_share(
        &self,
        artifact: ArtifactRecord,
        drafts: Vec<HighlightDraft>,
        target_group_id: String,
    ) -> Result<Vec<HighlightRecord>, CoreError> {
        // Guard: user must be logged in.
        let _ = self.require_user_pubkey()?;
        crate::highlights::publish_and_share(
            &self.runtime,
            artifact,
            drafts,
            &target_group_id,
        )
        .await
    }

    /// Publish a solo NIP-84 highlight without a NIP-29 repost. Used by the
    /// article reader's text-selection flow: user highlights → event lands in
    /// their vault; sharing into a community is a later explicit action.
    pub async fn publish_highlight(
        &self,
        draft: HighlightDraft,
        artifact: ArtifactRecord,
    ) -> Result<HighlightRecord, CoreError> {
        let _ = self.require_user_pubkey()?;
        crate::highlights::publish(&self.runtime, draft, artifact).await
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
    pub async fn start_friends_rooms_discovery(&self) -> Result<(), CoreError> {
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
    }

    /// Install (if not already installed) the kind:10012 curated-list sub for
    /// `curator_pubkey_hex`. Once the list lands in ndb, this method also
    /// spawns a metadata backfill for every group the list references, so a
    /// subsequent `get_featured_rooms` returns rich summaries rather than
    /// bare ids. Idempotent; the sub rides until logout.
    pub async fn start_featured_rooms(
        &self,
        curator_pubkey_hex: String,
    ) -> Result<(), CoreError> {
        let curator = PublicKey::from_hex(curator_pubkey_hex.trim())
            .map_err(|e| CoreError::InvalidInput(format!("invalid curator pubkey: {e}")))?;

        let already = self.inner.read().session.has_curation_subscription();
        if !already {
            let sub_id = self.runtime.spawn_curated_list_subscription(curator);
            self.inner
                .write()
                .session
                .set_curation_subscription(sub_id);
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
    }

    /// Curator's latest kind:10012 list, resolved into `CommunitySummary`
    /// items in curator-chosen order. Rooms without cached metadata are
    /// dropped; the next call after `start_featured_rooms` has backfilled
    /// metadata returns the full list.
    pub async fn get_featured_rooms(
        &self,
        curator_pubkey_hex: String,
    ) -> Result<Vec<CommunitySummary>, CoreError> {
        curation::fetch_curated_rooms_from_ndb(self.runtime.ndb(), curator_pubkey_hex.trim())
    }

    /// Every cached room, newest first, truncated to `limit`. Powers the
    /// explorer's "Browse all" grid.
    pub async fn get_all_rooms(
        &self,
        limit: u32,
    ) -> Result<Vec<CommunitySummary>, CoreError> {
        discovery::query_all_rooms_from_ndb(self.runtime.ndb(), limit)
    }

    /// The N most-recently-seen rooms. Same underlying query as
    /// `get_all_rooms` with a tighter limit — kept as a distinct method so
    /// the Swift explorer store's shelves remain single-purpose.
    pub async fn get_new_rooms(
        &self,
        limit: u32,
    ) -> Result<Vec<CommunitySummary>, CoreError> {
        discovery::query_all_rooms_from_ndb(self.runtime.ndb(), limit)
    }

    /// Rooms where 2+ of the user's follows are members. Empty when the user
    /// isn't logged in, has no follows cached, or no room satisfies the
    /// threshold.
    pub async fn get_rooms_with_friends(
        &self,
        limit: u32,
    ) -> Result<Vec<RoomRecommendation>, CoreError> {
        let Some(user) = self.inner.read().session.current_user() else {
            return Ok(Vec::new());
        };
        recommendations::query_rooms_with_friends(self.runtime.ndb(), &user.pubkey, limit)
    }

    /// Rooms where authors of articles the user has highlighted post
    /// artifacts. Empty when the user hasn't highlighted any articles yet.
    pub async fn get_rooms_from_read_authors(
        &self,
        limit: u32,
    ) -> Result<Vec<RoomRecommendation>, CoreError> {
        let Some(user) = self.inner.read().session.current_user() else {
            return Ok(Vec::new());
        };
        recommendations::query_rooms_from_read_authors(self.runtime.ndb(), &user.pubkey, limit)
    }

    /// Publish a NIP-29 kind:9021 join-request for `group_id`. Returns the
    /// event id on success. The UI treats this as fire-and-forget: a
    /// subsequent `MembershipChanged` delta for this group with the user's
    /// pubkey is the signal that the relay admitted the request.
    pub async fn request_join_room(&self, group_id: String) -> Result<String, CoreError> {
        let _ = self.require_user_pubkey()?;
        groups::publish_join_request(&self.runtime, group_id.trim()).await
    }

    // -- Blossom (BUD-03, kind:10063) --

    /// Return the user's ordered Blossom server list from nostrdb. Empty if no
    /// kind:10063 has been cached yet (relay hasn't delivered it).
    pub async fn get_blossom_servers(&self) -> Result<Vec<String>, CoreError> {
        let user = self
            .inner
            .read()
            .session
            .current_user()
            .ok_or(CoreError::NotAuthenticated)?;
        blossom::query_blossom_servers(self.runtime.ndb(), &user.pubkey)
    }

    /// Replace the user's Blossom server list with `servers` (must be
    /// non-empty). Order is preserved — first server is the upload default.
    pub async fn set_blossom_servers(&self, servers: Vec<String>) -> Result<String, CoreError> {
        let _ = self.require_user_pubkey()?;
        blossom::publish_blossom_servers(&self.runtime, servers).await
    }

    /// Publish the default Blossom server list only if the user has no cached
    /// kind:10063. Called once after login; no-op when the list already exists.
    pub async fn init_default_blossom_servers(&self) -> Result<(), CoreError> {
        let user = self
            .inner
            .read()
            .session
            .current_user()
            .ok_or(CoreError::NotAuthenticated)?;
        blossom::init_default_blossom_servers(&self.runtime, &user.pubkey).await
    }

    /// Sign a NIP-98 HTTP auth event (kind:27235) for a Blossom upload
    /// request. Returns the raw JSON of the signed event; the Swift caller
    /// base64-encodes it and uses it as `Authorization: Nostr <base64>`.
    /// `payload_hash` is the hex SHA-256 of the file bytes (required for PUT).
    pub async fn sign_nip98_auth(
        &self,
        url: String,
        method: String,
        payload_hash: Option<String>,
    ) -> Result<String, CoreError> {
        let _ = self.require_user_pubkey()?;
        blossom::sign_nip98_auth(&self.runtime, &url, &method, payload_hash.as_deref()).await
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
    ) -> Result<BlossomUpload, CoreError> {
        let _ = self.require_user_pubkey()?;
        blossom::upload_blob(&self.runtime, bytes, mime, width, height, alt).await
    }

    /// Publish a NIP-68 `kind:20` picture event into a NIP-29 community.
    /// Used by the capture flow when the user opts not to (or can't) extract
    /// a highlight quote — the photo still ships to the community with all
    /// the imeta metadata.
    pub async fn publish_picture(
        &self,
        draft: PictureDraft,
    ) -> Result<PictureRecord, CoreError> {
        let _ = self.require_user_pubkey()?;
        crate::pictures::publish_picture(&self.runtime, draft).await
    }

    // -- Relay config (NIP-65 read/write + NIP-78 rooms/indexer) --

    /// Return the user's effective relay list, merging NIP-65 (read/write)
    /// with NIP-78 app-data (rooms/indexer). Falls back to `seed_defaults()`
    /// when neither has been cached yet (first login).
    pub async fn get_relays(&self) -> Result<Vec<crate::relays::RelayConfig>, CoreError> {
        let user = self
            .inner
            .read()
            .session
            .current_user()
            .ok_or(CoreError::NotAuthenticated)?;
        crate::relays::query_relays(self.runtime.ndb(), &user.pubkey)
    }

    /// Insert-or-update a single relay. Replaces the row with matching URL or
    /// appends a new one, re-publishes kind:10002 + kind:30078, and reconciles
    /// the live relay pool so the change takes effect immediately.
    pub async fn upsert_relay(
        &self,
        cfg: crate::relays::RelayConfig,
    ) -> Result<(), CoreError> {
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

    /// Remove a relay by URL.
    pub async fn remove_relay(&self, url: String) -> Result<(), CoreError> {
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

    /// Atomically update a single relay's role flags.
    pub async fn set_relay_roles(
        &self,
        url: String,
        read: bool,
        write: bool,
        rooms: bool,
        indexer: bool,
    ) -> Result<(), CoreError> {
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

    // -- Relay telemetry --

    /// Snapshot of the live per-relay diagnostics map. One row per URL
    /// currently in the client's pool. Refreshed by the background
    /// diagnostics poller at least once per second.
    pub async fn get_relay_diagnostics(&self) -> Result<Vec<crate::models::RelayDiagnostic>, CoreError> {
        Ok(self.runtime.relay_diagnostics_snapshot())
    }

    /// Handle the Swift side uses to match `RelayStatusChanged` deltas on the
    /// event bus. Relay status changes are app-scoped and ride
    /// `subscription_id == 0`, so this returns `0` unconditionally — the
    /// value is a stable contract, not a unique sub id.
    pub async fn subscribe_relay_status(&self) -> Result<u64, CoreError> {
        Ok(0)
    }
}

impl HighlighterCore {
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
        let callback_slot: Arc<RwLock<Option<Arc<dyn EventCallback>>>> =
            Arc::new(RwLock::new(None));
        let subscriptions = Arc::new(SubscriptionRegistry::new(callback_slot.clone()));
        // Start the diagnostics poller before handing out the Arc<Self>.
        // The callback slot starts empty; the poller updates its in-memory
        // map regardless, and fires deltas once Swift installs a callback
        // via `set_event_callback`.
        runtime.spawn_diagnostics_poller(callback_slot.clone());
        Arc::new(Self {
            inner: Arc::new(RwLock::new(Inner {
                session: Session::new(),
            })),
            runtime,
            callback_slot,
            subscriptions,
        })
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

/// Strip a leading `nostr:` prefix, trim whitespace. Olas does this before
/// handing a URI to `NDKBunkerSigner.bunker(...)`.
pub(crate) fn normalize_bunker_uri(input: &str) -> String {
    let t = input.trim();
    t.strip_prefix("nostr:").unwrap_or(t).to_string()
}
