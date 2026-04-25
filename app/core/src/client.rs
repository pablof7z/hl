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
use crate::comments;
use crate::follows;
use crate::groups;
use crate::highlights;
use crate::isbn_lookup;
use crate::models::{
    ArticleRecord, ArtifactPreview, ArtifactRecord, BlossomUpload, ChatMessageRecord,
    CommentRecord, CommunitySummary, CurrentUser, DiscussionRecord, FeedbackEventRecord,
    FeedbackThreadRecord, HighlightDraft, HighlightRecord, HydratedHighlight, NostrConnectOptions,
    PictureDraft, PictureRecord, ProfileMetadata, ReadingFeedItem, RoomRecommendation,
};
use crate::reads;
use crate::recommendations;
use crate::nip46::{self, BunkerSigner};
use crate::nostr_runtime::NostrRuntime;
use crate::profile;
use crate::relays::NOSTR_CONNECT_RELAY;
use crate::session::{current_user_from_pubkey, Session};
use crate::subscriptions::{SubscriptionKind, SubscriptionRegistry};
use crate::web_metadata::{self, WebMetadata, WebMetadataStore};

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
            // First-pass: apply whatever's in cache so subscriptions have a
            // pool to talk to immediately. The bootstrap below races to
            // fetch the user's actual NIP-65 from the network and re-apply
            // — without it, a fresh install with cold cache stays on
            // seed_defaults forever.
            self.runtime
                .spawn_apply_user_relay_config(pubkey.to_hex());
            let user_relay_config_id = self
                .runtime
                .spawn_user_relay_config_bootstrap(pubkey);
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

    /// Per-room Chat view-scope subscription. Returns a handle; fires
    /// `ChatMessageUpserted` deltas for kind:9 messages tagged
    /// `#h=<group_id>`.
    pub async fn subscribe_room_chat(&self, group_id: String) -> Result<u64, CoreError> {
        if group_id.trim().is_empty() {
            return Err(CoreError::InvalidInput("group_id must not be empty".into()));
        }
        self.subscriptions
            .register(&self.runtime, SubscriptionKind::RoomChat { group_id })
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
        self.refresh_follows_nip65_subscription(&follows_pks);
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

    /// Publish a new kind:0 metadata event for the current user. Preserves
    /// any unknown JSON fields the user had set via other clients —
    /// only the canonical fields the edit form drives get overwritten.
    /// Empty strings clear the corresponding field. Returns the parsed
    /// metadata so the caller's UI can swap to the new state without
    /// waiting for the relay echo.
    pub async fn update_profile(
        &self,
        name: String,
        display_name: String,
        about: String,
        picture: String,
        banner: String,
        nip05: String,
        website: String,
        lud16: String,
    ) -> Result<ProfileMetadata, CoreError> {
        let _ = self.require_user_pubkey()?;
        profile::publish_profile(
            &self.runtime,
            &name,
            &display_name,
            &about,
            &picture,
            &banner,
            &nip05,
            &website,
            &lud16,
        )
        .await
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

    /// Read highlights whose `tag_name` tag holds `tag_value`, newest
    /// first. Generalizes `get_highlights_for_article`: pass `("a", "30023:pk:d")`
    /// for articles, `("i", "isbn:…")` for ISBN books, `("r", "<url>")` for
    /// podcasts. `tag_name` must be a single character.
    pub async fn get_highlights_for_reference(
        &self,
        tag_name: String,
        tag_value: String,
        limit: u32,
    ) -> Result<Vec<HighlightRecord>, CoreError> {
        let Some(ch) = tag_name.trim().chars().next() else {
            return Ok(Vec::new());
        };
        highlights::query_for_reference(self.runtime.ndb(), ch, tag_value.trim(), limit)
    }

    /// Read NIP-22 comments (kind:1111) rooted at the given uppercase
    /// scope tag — `("A", "30023:pk:d")` for articles, `("I", "isbn:…")` for
    /// books, etc. Newest first.
    pub async fn get_comments_for_reference(
        &self,
        tag_name: String,
        tag_value: String,
        limit: u32,
    ) -> Result<Vec<CommentRecord>, CoreError> {
        let Some(ch) = tag_name.trim().chars().next() else {
            return Ok(Vec::new());
        };
        comments::query_for_reference(self.runtime.ndb(), ch, tag_value.trim(), limit)
    }

    /// Publish a NIP-22 kind:1111 comment scoped to any artifact.
    ///
    /// `root_tag_name` is `"A"` (addressable, e.g. `30023:<pubkey>:<d>`),
    /// `"E"` (event id — a highlight, an event share), or `"I"` (external
    /// content like `url:…`, `podcast:item:guid:…`, `isbn:…`).
    /// `root_tag_value` is the corresponding scope value. `root_kind` is
    /// the kind of the root event (or 0 for purely external roots).
    /// `parent_event_id` is `None` for top-level comments and `Some(id)`
    /// for replies (the parent kind:1111 comment).
    pub async fn publish_comment(
        &self,
        root_tag_name: String,
        root_tag_value: String,
        root_kind: u16,
        parent_event_id: Option<String>,
        content: String,
    ) -> Result<CommentRecord, CoreError> {
        let _ = self.require_user_pubkey()?;
        let Some(scope) = root_tag_name.trim().chars().next() else {
            return Err(CoreError::InvalidInput("root tag must not be empty".into()));
        };
        let parent = parent_event_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        comments::publish_comment(
            &self.runtime,
            scope,
            root_tag_value.trim(),
            root_kind,
            parent,
            content.trim(),
        )
        .await
    }

    // -- Reactions (NIP-25 kind:7) ---------------------------------------

    /// All cached kind:7 reactions on `target_event_id`, newest first.
    pub async fn get_reactions_for_event(
        &self,
        target_event_id: String,
        limit: u32,
    ) -> Result<Vec<crate::reactions::ReactionRecord>, CoreError> {
        crate::reactions::query_reactions_for_event(
            self.runtime.ndb(),
            target_event_id.trim(),
            limit,
        )
    }

    /// Publish a kind:7 reaction targeting `event_id` authored by
    /// `author_pubkey_hex` of `target_kind`. `content` is the reaction
    /// body — pass `"+"` for a like.
    pub async fn publish_reaction(
        &self,
        event_id: String,
        author_pubkey_hex: String,
        target_kind: u16,
        content: String,
    ) -> Result<crate::reactions::ReactionRecord, CoreError> {
        let _ = self.require_user_pubkey()?;
        crate::reactions::publish_reaction(
            &self.runtime,
            event_id.trim(),
            author_pubkey_hex.trim(),
            target_kind,
            content.trim(),
        )
        .await
    }

    /// Delete one of the user's own kind:7 reactions via NIP-09.
    pub async fn unpublish_reaction(&self, reaction_event_id: String) -> Result<String, CoreError> {
        let _ = self.require_user_pubkey()?;
        crate::reactions::unpublish_reaction(&self.runtime, reaction_event_id.trim()).await
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

    // -- Bookmarks (NIP-51 kind:10003) -----------------------------------

    /// Return the set of article addresses the user has bookmarked in their
    /// newest kind:10003 list (empty when not logged in or no list cached).
    pub async fn get_bookmarked_article_addresses(&self) -> Result<Vec<String>, CoreError> {
        let user_hex = self
            .inner
            .read()
            .session
            .current_user()
            .map(|u| u.pubkey)
            .unwrap_or_default();
        let list = crate::bookmarks::query_bookmarks(self.runtime.ndb(), &user_hex)?;
        Ok(list.addresses)
    }

    /// Read-only predicate: is `address` currently bookmarked for the logged-in
    /// user? Always `false` when no user is logged in.
    pub async fn is_article_bookmarked(&self, address: String) -> Result<bool, CoreError> {
        let user_hex = self
            .inner
            .read()
            .session
            .current_user()
            .map(|u| u.pubkey)
            .unwrap_or_default();
        if user_hex.is_empty() {
            return Ok(false);
        }
        crate::bookmarks::is_bookmarked(self.runtime.ndb(), &user_hex, &address)
    }

    /// Toggle `address` in the user's kind:10003 list. Returns the new
    /// membership state — `true` if the address is now bookmarked, `false`
    /// if it was removed.
    pub async fn toggle_article_bookmark(&self, address: String) -> Result<bool, CoreError> {
        let user_hex = self
            .inner
            .read()
            .session
            .current_user()
            .map(|u| u.pubkey)
            .ok_or(CoreError::NotInitialized)?;
        crate::bookmarks::toggle_bookmark(&self.runtime, &user_hex, &address).await
    }

    /// Read-only predicate: is `event_id_hex` currently bookmarked for
    /// the logged-in user? Always `false` when no user is logged in.
    pub async fn is_event_bookmarked(&self, event_id_hex: String) -> Result<bool, CoreError> {
        let user_hex = self
            .inner
            .read()
            .session
            .current_user()
            .map(|u| u.pubkey)
            .unwrap_or_default();
        if user_hex.is_empty() {
            return Ok(false);
        }
        crate::bookmarks::is_event_bookmarked(self.runtime.ndb(), &user_hex, &event_id_hex)
    }

    /// Toggle `event_id_hex` in the user's kind:10003 list (for comments
    /// and other event-id-addressed targets). Returns the new membership
    /// state.
    pub async fn toggle_event_bookmark(&self, event_id_hex: String) -> Result<bool, CoreError> {
        let user_hex = self
            .inner
            .read()
            .session
            .current_user()
            .map(|u| u.pubkey)
            .ok_or(CoreError::NotInitialized)?;
        crate::bookmarks::toggle_event_bookmark(&self.runtime, &user_hex, &event_id_hex).await
    }

    /// Open a live subscription on the current user's kind:10003 bookmark
    /// events. Deltas land on the app-scope bus (`BookmarksUpdated`); the
    /// Swift bookmarks store re-queries on each.
    pub async fn subscribe_bookmarks(&self) -> Result<u64, CoreError> {
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

    /// Fetch OpenGraph + favicon metadata for a web URL. Backed by a
    /// JSON-on-disk cache (7-day positive TTL, 1-hour negative TTL) and
    /// in-flight coalescing — concurrent calls for the same URL share one
    /// HTTP request. Returns `CoreError::NotFound` when the page 404s,
    /// `CoreError::Network` on transport failure.
    pub async fn get_web_metadata(&self, url: String) -> Result<WebMetadata, CoreError> {
        web_metadata::get_or_fetch(self.web_metadata.clone(), &url).await
    }

    pub async fn get_discussions(
        &self,
        group_id: String,
        limit: u32,
    ) -> Result<Vec<DiscussionRecord>, CoreError> {
        crate::discussions::query_for_group(self.runtime.ndb(), &group_id, limit)
    }

    /// NIP-29 chat messages (kind:9) cached for `group_id`, ordered ascending
    /// by `created_at`. UI can also peek with `limit=1` to detect chat
    /// activity and decide whether to expose the chat tab at all.
    pub async fn get_chat_messages(
        &self,
        group_id: String,
        limit: u32,
    ) -> Result<Vec<ChatMessageRecord>, CoreError> {
        crate::chat::query_chat_messages(self.runtime.ndb(), &group_id, limit)
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

    /// Publish a NIP-29 kind:9 chat message into `group_id`. When
    /// `reply_to_event_id` is set, the published event carries a marked
    /// NIP-10 `["e", <id>, "", "reply"]` tag.
    pub async fn publish_chat_message(
        &self,
        group_id: String,
        content: String,
        reply_to_event_id: Option<String>,
    ) -> Result<ChatMessageRecord, CoreError> {
        let _ = self.require_user_pubkey()?;
        crate::chat::publish_chat_message(
            &self.runtime,
            &group_id,
            &content,
            reply_to_event_id.as_deref(),
        )
        .await
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
    ) -> Result<(), CoreError> {
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

    /// Create a brand-new NIP-29 room. Publishes kind:9007 (create-group) and
    /// kind:9002 (edit-metadata) signed by the current user. Returns the
    /// freshly-generated group id on success — the relay's 39000/39001/39002
    /// follow-up events drive the iOS membership stream automatically.
    pub async fn create_room(
        &self,
        name: String,
        about: String,
        picture: String,
        visibility: groups::RoomVisibility,
        access: groups::RoomAccess,
    ) -> Result<String, CoreError> {
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

    /// Add a Nostr user (by hex pubkey) to a room as a member. Must be
    /// signed by a room admin — the relay enforces this. Returns the
    /// kind:9000 event id on success.
    pub async fn add_room_member(
        &self,
        group_id: String,
        pubkey_hex: String,
    ) -> Result<String, CoreError> {
        let _ = self.require_user_pubkey()?;
        groups::add_member(&self.runtime, group_id.trim(), pubkey_hex.trim()).await
    }

    /// Decode a Nostr identifier (`npub1…`, `nprofile1…`, optionally with a
    /// `nostr:` URI prefix) to a 64-char hex pubkey. Returns
    /// `CoreError::InvalidInput` if the input isn't a recognised pubkey
    /// reference. Used by the room-invite picker to resolve a pasted handle.
    pub fn decode_npub(&self, input: String) -> Result<String, CoreError> {
        let trimmed = input
            .trim()
            .strip_prefix("nostr:")
            .unwrap_or(input.trim())
            .trim();
        if trimmed.is_empty() {
            return Err(CoreError::InvalidInput("empty pubkey reference".into()));
        }
        if let Ok(pk) = PublicKey::from_bech32(trimmed) {
            return Ok(pk.to_hex());
        }
        if let Ok(profile) = nostr_sdk::nips::nip19::Nip19Profile::from_bech32(trimmed) {
            return Ok(profile.public_key.to_hex());
        }
        if trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(trimmed.to_ascii_lowercase());
        }
        Err(CoreError::InvalidInput(format!(
            "unrecognised pubkey reference: {trimmed}"
        )))
    }

    /// Classify a NIP-19 entity (`npub1…`, `nprofile1…`, `note1…`,
    /// `nevent1…`, `naddr1…`) into a renderable variant. Strips an
    /// optional `nostr:` URI prefix. Used by the iOS rich-text renderer
    /// to walk event content for inline mentions and event-ref cards.
    pub fn decode_nostr_entity(
        &self,
        input: String,
    ) -> Result<crate::nostr_entities::NostrEntityRef, CoreError> {
        crate::nostr_entities::decode_nostr_entity(&input)
    }

    /// Best-effort cache lookup for a [`NostrEntityRef`]. Returns the
    /// resolved event when nostrdb already has it, `None` otherwise.
    /// The caller should pair this with `subscribe_nostr_entity` so a
    /// cold-cache reference warms up over the wire.
    pub async fn resolve_nostr_entity(
        &self,
        entity: crate::nostr_entities::NostrEntityRef,
    ) -> Result<Option<crate::nostr_entities::NostrEntityEvent>, CoreError> {
        crate::nostr_entities::resolve_from_cache(self.runtime.ndb(), &entity)
    }

    /// Install a one-shot REQ for the missing event behind an entity.
    /// Routes to relay hints first (when the bech32 carried any) plus
    /// the indexer pool. Events received are persisted to nostrdb via
    /// the `NdbDatabase` bridge; the caller polls
    /// `resolve_nostr_entity` again to pick them up. Fire-and-forget —
    /// no handle returned, no need to unsubscribe (the relay closes
    /// the REQ on EOSE).
    pub async fn subscribe_nostr_entity(
        &self,
        entity: crate::nostr_entities::NostrEntityRef,
    ) -> Result<(), CoreError> {
        let _ = self.require_user_pubkey()?;
        self.runtime.spawn_nostr_entity_backfill(entity)
    }

    /// Pubkeys (hex) the current user follows per their cached kind:3 contact
    /// list. Empty if the user isn't logged in or the cache hasn't seen a
    /// kind:3 yet. Used by the room-invite picker to surface "people you know"
    /// before any typing happens.
    pub async fn get_follows(&self) -> Result<Vec<String>, CoreError> {
        let user_pubkey = self.require_user_pubkey()?;
        crate::follows::query_follows(self.runtime.ndb(), &user_pubkey.to_hex())
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

    /// Nudge the relay pool to attempt a reconnect on every disconnected
    /// relay. `Client::connect` is idempotent — already-connected relays
    /// are unaffected; disconnected / terminated / banned relays get a
    /// fresh WebSocket attempt.
    pub async fn reconnect_all(&self) -> Result<(), CoreError> {
        self.runtime.client().connect().await;
        Ok(())
    }

    /// Close every WebSocket in the pool. Used by the Wi-Fi-only toggle
    /// when the device drops off Wi-Fi — the Swift side re-enables by
    /// calling `reconnect_all` once the path monitor reports Wi-Fi back.
    pub async fn disconnect_all(&self) -> Result<(), CoreError> {
        self.runtime.client().disconnect().await;
        Ok(())
    }

    /// Fetch the target relay's NIP-11 information document via an HTTPS
    /// GET to the `ws[s]://` URL's HTTP equivalent with
    /// `Accept: application/nostr+json`. Fails fast on timeout.
    pub async fn probe_relay_nip11(
        &self,
        url: String,
    ) -> Result<crate::models::Nip11Document, CoreError> {
        crate::relay_polish::probe_nip11(&url).await
    }

    /// Fetch another user's kind:10002 via the indexer pool and return the
    /// parsed `RelayConfig` rows. Useful for "adopt someone else's relay
    /// setup" flows — the Swift caller shows the list with checkboxes
    /// and upserts the selected subset through `upsert_relay`.
    pub async fn import_relays_from_npub(
        &self,
        npub: String,
    ) -> Result<Vec<crate::relays::RelayConfig>, CoreError> {
        crate::relay_polish::import_from_npub(&self.runtime, &npub).await
    }

    /// Size + event-count snapshot of the local nostrdb cache. Order-of-
    /// magnitude figures used by the Network Settings "Local cache" card.
    pub async fn get_cache_stats(&self) -> Result<crate::models::CacheStats, CoreError> {
        crate::relay_polish::cache_stats(self.runtime.ndb(), self.runtime.data_dir())
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
        let web_metadata = Arc::new(WebMetadataStore::open(runtime.data_dir()));
        Arc::new(Self {
            inner: Arc::new(RwLock::new(Inner {
                session: Session::new(),
            })),
            runtime,
            callback_slot,
            subscriptions,
            web_metadata,
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
fn current_followed_pubkeys(
    ndb: &nostrdb::Ndb,
    user_pubkey: &PublicKey,
) -> Vec<PublicKey> {
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
