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
use crate::errors::CoreError;
use crate::events::{DataChangeType, Delta, EventCallback};
use crate::follows;
use crate::groups;
use crate::highlights;
use crate::isbn_lookup;
use crate::models::{
    ArticleRecord, ArtifactPreview, ArtifactRecord, CommunitySummary, CurrentUser,
    DiscussionRecord, HighlightDraft, HighlightRecord, HydratedHighlight, NostrConnectOptions,
    ProfileMetadata, ReadingFeedItem,
};
use crate::reads;
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
            let sub_id = self.runtime.spawn_membership_subscription(pubkey);
            let contacts_id = self.runtime.spawn_contacts_subscription(pubkey);
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
        // no-op if the relay is already known (which it is — Primal is in
        // DEFAULT_RELAYS — but we can't rely on the initial relay connect
        // having completed yet).
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

        let sub_id = self
            .runtime
            .spawn_membership_subscription(user_pubkey);
        let contacts_id = self.runtime.spawn_contacts_subscription(user_pubkey);
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
        _group_id: String,
        _limit: u32,
    ) -> Result<Vec<ArtifactRecord>, CoreError> {
        Err(CoreError::NotInitialized)
    }

    pub async fn get_highlights(
        &self,
        _group_id: String,
        _limit: u32,
    ) -> Result<Vec<HydratedHighlight>, CoreError> {
        Err(CoreError::NotInitialized)
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

    pub async fn get_recent_books(&self, _limit: u32) -> Result<Vec<ArtifactRecord>, CoreError> {
        Err(CoreError::NotInitialized)
    }

    pub async fn search_artifacts(
        &self,
        _query: String,
        _limit: u32,
    ) -> Result<Vec<ArtifactRecord>, CoreError> {
        Err(CoreError::NotInitialized)
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
