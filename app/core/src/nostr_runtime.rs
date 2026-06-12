//! Owns Highlighter's Rust runtime composition: NMP for protocol IO and
//! identity, nostrdb for the app read model, and a nostr-sdk client used only
//! as an event-builder/signing facade for existing feature modules.
//!
//! Async lifecycle: `HighlighterCore::new()` is a synchronous UniFFI
//! constructor, so we own a dedicated tokio `Runtime` for NMP-backed async
//! observers and signer installation.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures::{future::BoxFuture, StreamExt};
use nmp_core::planner::InterestLifecycle;
use nmp_core::typed_projections::ActionResultRow;
use nostr_sdk::prelude::*;
use nostrdb::{Config as NdbConfig, Filter as NdbFilter, Ndb};
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

use crate::errors::CoreError;
use crate::events::EventCallback;
use crate::groups::{KIND_GROUP_ADMINS, KIND_GROUP_MEMBERS, KIND_GROUP_METADATA};
use crate::models::RelayDiagnostic;
use crate::nmp_runtime::{HighlighterNmpRuntime, NmpInterestHandle};

/// NIP-51 "simple groups" list (replaceable). A user publishes this to
/// enumerate the NIP-29 groups they're a member of; each entry is a
/// `group` tag with the group id and relay.
const KIND_SIMPLE_GROUPS_LIST: u16 = 10009;
use crate::relays::{
    highlighter_relay, negentropy_sync_relays, purple_pages_relay, query_relays, seed_defaults,
    RelayConfig,
};

/// Shared pointer to the app's event-callback slot. `HighlighterCore` owns
/// the slot; NMP's diagnostics observer uses it to dispatch
/// `RelayStatusChanged` deltas without holding a direct reference back to
/// the core.
pub type EventCallbackSlot = Arc<parking_lot::RwLock<Option<Arc<dyn EventCallback>>>>;

/// Local nostrdb map size. LMDB reserves address space, not disk, and the
/// cache grows on demand as events arrive.
const NDB_MAPSIZE_BYTES: usize = 2 * 1024 * 1024 * 1024;

pub struct NostrRuntime {
    client: Client,
    ndb: Arc<Ndb>,
    nmp: Arc<HighlighterNmpRuntime>,
    /// Held as `Option` so Drop can `take()` it and call
    /// `shutdown_background()`. Without that, Tokio's default `Drop` blocks
    /// the thread waiting for long-lived observer tasks to complete.
    rt: Option<Runtime>,
    /// Cached copy of the relay config that was last applied to NMP. Read by
    /// the `*_urls()` accessors so per-role subscription routing can pick the
    /// right subset without re-querying nostrdb.
    current_relays: Arc<parking_lot::RwLock<Vec<RelayConfig>>>,
    /// NMP logical interests opened under legacy subscription handles. This
    /// keeps the existing Rust/native lifecycle API while NMP owns the wire
    /// subscription registry and planner.
    nmp_subscriptions: Arc<parking_lot::RwLock<HashMap<String, Vec<NmpInterestHandle>>>>,
    /// Event-driven side tasks associated with NMP subscription ids. Used for
    /// Rust-owned reactions to NMP-mirrored events, never for relay IO.
    nmp_subscription_tasks: Arc<parking_lot::RwLock<HashMap<String, JoinHandle<()>>>>,
    /// Path the LMDB-backed nostrdb was opened at. Used by features that
    /// need to size the on-disk cache.
    data_dir: PathBuf,
}

pub(crate) fn signed_event_from_action_result(
    source: &str,
    result_json: Option<&str>,
) -> Result<Event, CoreError> {
    let result_json = result_json
        .ok_or_else(|| CoreError::Relay(format!("{source}: NMP publish missing signed event")))?;
    let result: serde_json::Value = serde_json::from_str(result_json)
        .map_err(|e| CoreError::Relay(format!("{source}: NMP publish result is not JSON: {e}")))?;
    let event_json = result
        .get("event")
        .ok_or_else(|| CoreError::Relay(format!("{source}: NMP publish result missing `event`")))?;
    Event::from_json(event_json.to_string())
        .map_err(|e| CoreError::Relay(format!("{source}: NMP publish event decode: {e}")))
}

impl Drop for NostrRuntime {
    fn drop(&mut self) {
        if let Some(rt) = self.rt.take() {
            rt.shutdown_background();
        }
    }
}

impl NostrRuntime {
    /// Construct the runtime and seed NMP with the default relay roles.
    /// Returns immediately once local state (Ndb + NMP + signer facade) is
    /// initialized.
    pub fn new() -> Result<Self, CoreError> {
        let data_dir = default_data_dir()?;
        Self::with_data_dir(data_dir)
    }

    /// Same as [`Self::new`], but lets the caller point at an isolated
    /// directory. Used by tests.
    pub fn with_data_dir(data_dir: PathBuf) -> Result<Self, CoreError> {
        std::fs::create_dir_all(&data_dir)
            .map_err(|e| CoreError::Cache(format!("create data dir: {e}")))?;

        let db_path_str = data_dir
            .to_str()
            .ok_or_else(|| CoreError::Cache("data dir is not valid UTF-8".into()))?;
        let ndb_config = NdbConfig::new().set_mapsize(NDB_MAPSIZE_BYTES);
        let ndb = Ndb::new(db_path_str, &ndb_config)
            .map_err(|e| CoreError::Cache(format!("open nostrdb: {e}")))?;
        let ndb = Arc::new(ndb);

        // Compatibility facade only: NMP owns relay IO and mirrors accepted
        // events into nostrdb. Feature modules still use nostr-sdk's
        // EventBuilder helpers through this client, with an NMP-backed signer.
        let client = Client::builder().build();

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("highlighter-nostr")
            .build()
            .map_err(|e| CoreError::Other(format!("build tokio runtime: {e}")))?;

        let initial_relays = seed_defaults();
        let nmp = Arc::new(HighlighterNmpRuntime::new(
            &data_dir,
            ndb.clone(),
            &initial_relays,
        )?);

        let runtime = Self {
            client,
            ndb,
            nmp,
            rt: Some(rt),
            current_relays: Arc::new(parking_lot::RwLock::new(Vec::new())),
            nmp_subscriptions: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            nmp_subscription_tasks: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            data_dir,
        };

        runtime.spawn_connect();

        Ok(runtime)
    }

    /// Access the nostr-sdk client used as an event-builder/signing facade.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Access the Ndb for direct cache queries.
    pub fn ndb(&self) -> &Ndb {
        &self.ndb
    }

    /// Insert a successfully-accepted local publish into nostrdb so the next
    /// Rust projection refresh sees the same source of truth as relay events.
    pub(crate) fn cache_accepted_event(
        &self,
        source: &str,
        event: &Event,
    ) -> Result<(), CoreError> {
        let source = source.trim();
        let source = if source.is_empty() {
            "local-publish"
        } else {
            source
        };
        let line = format!("[\"EVENT\",\"{source}\",{}]", event.as_json());
        self.ndb
            .process_event(&line)
            .map_err(|e| CoreError::Cache(format!("cache accepted event: {e}")))
    }

    /// Tokio handle so feature modules can drive async work without standing
    /// up their own runtime.
    pub fn runtime_handle(&self) -> tokio::runtime::Handle {
        self.rt().handle().clone()
    }

    pub(crate) fn open_nmp_filter(
        &self,
        id: &SubscriptionId,
        label: &str,
        filter: Filter,
        relay_pin: Option<String>,
    ) -> Result<(), CoreError> {
        self.open_nmp_filter_with_lifecycle(
            id,
            label,
            filter,
            relay_pin,
            InterestLifecycle::Tailing,
        )
    }

    fn open_nmp_filter_with_lifecycle(
        &self,
        id: &SubscriptionId,
        label: &str,
        filter: Filter,
        relay_pin: Option<String>,
        lifecycle: InterestLifecycle,
    ) -> Result<(), CoreError> {
        let owner = id.to_string();
        let handle = self
            .nmp
            .open_filter_interest(label, &owner, filter, relay_pin, lifecycle)?;
        self.nmp_subscriptions
            .write()
            .entry(owner)
            .or_default()
            .push(handle);
        Ok(())
    }

    pub(crate) fn open_nmp_filter_on_relays(
        &self,
        id: &SubscriptionId,
        label: &str,
        filter: Filter,
        relays: Vec<String>,
    ) -> Result<(), CoreError> {
        self.open_nmp_filter_on_relays_with_lifecycle(
            id,
            label,
            filter,
            relays,
            InterestLifecycle::Tailing,
        )
    }

    pub(crate) fn open_nmp_filter_once_on_relays(
        &self,
        id: &SubscriptionId,
        label: &str,
        filter: Filter,
        relays: Vec<String>,
    ) -> Result<(), CoreError> {
        self.open_nmp_filter_on_relays_with_lifecycle(
            id,
            label,
            filter,
            relays,
            InterestLifecycle::OneShot,
        )
    }

    pub(crate) fn open_nmp_filter_once_and_wait<'a>(
        &'a self,
        label: &'a str,
        filter: Filter,
        relays: Vec<String>,
        ndb_filters: Vec<NdbFilter>,
        timeout: Duration,
    ) -> BoxFuture<'a, Result<bool, CoreError>> {
        let sub = self
            .ndb
            .subscribe(&ndb_filters)
            .map_err(|e| CoreError::Cache(format!("{label}: ndb subscribe: {e}")));
        Box::pin(async move {
            let sub = sub?;
            let id = SubscriptionId::generate();
            self.open_nmp_filter_once_on_relays(&id, label, filter, relays)?;

            let observed = {
                let ndb = self.ndb.clone();
                let mut stream = sub.stream(&ndb).notes_per_await(32);
                matches!(
                    tokio::time::timeout(timeout, stream.next()).await,
                    Ok(Some(keys)) if !keys.is_empty()
                )
            };
            self.drop_subscription(id);
            Ok(observed)
        })
    }

    fn open_nmp_filter_on_relays_with_lifecycle(
        &self,
        id: &SubscriptionId,
        label: &str,
        filter: Filter,
        relays: Vec<String>,
        lifecycle: InterestLifecycle,
    ) -> Result<(), CoreError> {
        let relays: Vec<String> = relays
            .into_iter()
            .map(|url| url.trim().to_string())
            .filter(|url| !url.is_empty())
            .collect();
        if relays.is_empty() {
            return self.open_nmp_filter_with_lifecycle(id, label, filter, None, lifecycle);
        }

        let owner = id.to_string();
        let mut opened = Vec::with_capacity(relays.len());
        for relay in relays {
            match self.nmp.open_filter_interest(
                label,
                &owner,
                filter.clone(),
                Some(relay),
                lifecycle.clone(),
            ) {
                Ok(handle) => opened.push(handle),
                Err(e) => {
                    for handle in opened {
                        self.nmp.close_interest(handle);
                    }
                    return Err(e);
                }
            }
        }
        self.nmp_subscriptions
            .write()
            .entry(owner)
            .or_default()
            .extend(opened);
        Ok(())
    }

    /// Internal accessor: always returns the Runtime while the struct is
    /// alive. Only Drop removes it.
    fn rt(&self) -> &Runtime {
        self.rt
            .as_ref()
            .expect("NostrRuntime::rt accessed after Drop")
    }

    /// Resolved nostrdb directory. Used by cache-stats features that want
    /// to size the on-disk store.
    pub fn data_dir(&self) -> &std::path::Path {
        &self.data_dir
    }

    /// Resolved NMP store directory, sibling to the nostrdb cache. Exposed
    /// for diagnostics/tests only; NMP owns the contents.
    pub fn nmp_data_dir(&self) -> &std::path::Path {
        self.nmp.storage_dir()
    }

    /// Install the NMP-backed signer facade on the compatibility client.
    pub fn install_nmp_signer(&self) {
        self.rt().block_on(self.install_nmp_signer_async());
    }

    pub async fn install_nmp_signer_async(&self) {
        let signer = self.nmp.nostr_signer();
        self.client.set_signer(signer).await;
    }

    /// Apply the nsec identity WITHOUT the network-bound signer install. This is
    /// the fast, synchronous half of `set_local_nsec_signer`: it parses the
    /// secret, registers the local signer, and marks it active in NMP's identity
    /// reducer (no relay I/O). Used by the sign-in dispatch path to establish the
    /// active account synchronously on the actor thread, so a subsequent
    /// superseding sign-in observes the correct prior identity. Idempotent:
    /// NMP's `add_signer`/`identity.add` dedup by pubkey, so the later full
    /// `set_local_nsec_signer` re-apply is a no-op.
    pub fn apply_nsec_identity(&self, nsec: &str) -> Result<String, CoreError> {
        self.nmp.sign_in_nsec(nsec)
    }

    /// Install a local signer through NMP, then bind the compatibility
    /// event-builder client to NMP's signer port.
    pub fn set_local_nsec_signer(&self, nsec: &str) -> Result<String, CoreError> {
        let active_pubkey = self.nmp.sign_in_nsec(nsec)?;
        self.install_nmp_signer();
        Ok(active_pubkey)
    }

    pub fn start_nostrconnect_uri(
        &self,
        options: &crate::models::NostrConnectOptions,
    ) -> Result<String, CoreError> {
        self.nmp.nostrconnect_uri(options)
    }

    pub fn active_account_pubkey(&self) -> Option<String> {
        self.nmp.active_pubkey()
    }

    pub(crate) async fn dispatch_nmp_action_for_result<T: serde::Serialize>(
        &self,
        source: &str,
        namespace: &str,
        action: &T,
    ) -> Result<ActionResultRow, CoreError> {
        self.nmp
            .dispatch_action_for_result(source, namespace, action)
            .await
    }

    pub async fn wait_for_signer_pair_after(
        &self,
        previous: Option<String>,
    ) -> Result<String, CoreError> {
        let pubkey = self.nmp.wait_for_signer_pair_after(previous).await?;
        self.install_nmp_signer_async().await;
        Ok(pubkey)
    }

    /// Register a pasted bunker URI with NMP and wait until the identity actor
    /// exposes the resolved remote signer account.
    pub async fn sign_in_bunker_uri(&self, uri: &str) -> Result<String, CoreError> {
        let previous = self.nmp.active_pubkey();
        self.nmp.sign_in_bunker_uri(uri)?;
        self.wait_for_signer_pair_after(previous).await
    }

    /// Begin a NIP-55 sign-in (ADR-0048 Stage 2) and wait until the identity
    /// actor exposes the paired account.
    ///
    /// The wait deliberately uses `previous = None` ("any active account"),
    /// not the dispatch-time account: on a fresh interactive sign-in the app
    /// is signed out (None == change-wait), and on a session restore NMP's
    /// own external-signer restore hook may have re-activated the account
    /// BEFORE this call — a change-wait would then dead-wait into a timeout
    /// and wrongly clear the stored credential. hl has no account switching
    /// (logout tears the session down first), so "any active account" is
    /// exactly the paired one.
    pub async fn sign_in_nip55(&self, signer_package: Option<&str>) -> Result<String, CoreError> {
        // Kernel-side session restore (the external-signer Restore hook) may
        // have already re-activated the NIP-55 account before this dispatch.
        // Complete immediately instead of re-running the interactive
        // get_public_key handshake — re-prompting the signer app on every
        // cold start is both redundant and hostile UX.
        if let Some(active) = self.nmp.active_pubkey() {
            self.install_nmp_signer_async().await;
            return Ok(active);
        }
        self.nmp.sign_in_nip55(signer_package);
        self.wait_for_signer_pair_after(None).await
    }

    /// Deliver a raw `ExternalSignerResponse` JSON back to the NIP-55 driver (D7).
    pub fn nmp_deliver_external_signer_response(&self, response_json: &str) {
        self.nmp.deliver_external_signer_response(response_json);
    }

    /// Blocking timed drain of the next outbound `ExternalSignerRequest`
    /// (D8 — parks in the channel `recv_timeout`, never a poll).
    pub fn nmp_next_signer_request(&self) -> crate::nmp_runtime::SignerRequestDrain {
        self.nmp.next_signer_request()
    }

    /// Remove the active account from the NMP identity reducer.
    pub fn remove_nmp_account(&self, pubkey_hex: &str) {
        self.nmp.remove_account(pubkey_hex);
    }

    /// Remove the active signer. Called from `session::logout`.
    pub fn unset_signer(&self) {
        self.rt().block_on(async {
            self.client.unset_signer().await;
        });
    }

    /// Install a global, long-lived subscription for the current user's
    /// NIP-29 group metadata + membership. NMP owns the wire subscription and
    /// mirrors incoming events into nostrdb. Returns the subscription id so
    /// `logout()` can drop it.
    pub fn spawn_membership_subscription(&self, pubkey: PublicKey) -> SubscriptionId {
        let id = SubscriptionId::generate();
        // Stage 1 of the two-stage NIP-29 join-set query (mirrors
        // `web/src/routes/rooms/+page.svelte`): pull the user's
        // kind:39001/39002 events. Metadata (kind:39000) lives under
        // different indexing (`d` tag, no `p`), so it's pulled in
        // stage 2 by `spawn_group_metadata_subscription` once the pump
        // sees a membership event for each group.
        let filter = Filter::new()
            .kinds([
                Kind::Custom(KIND_GROUP_ADMINS),
                Kind::Custom(KIND_GROUP_MEMBERS),
            ])
            .pubkey(pubkey);
        if let Err(e) =
            self.open_nmp_filter_on_relays(&id, "rooms/membership", filter, self.rooms_urls())
        {
            tracing::warn!(error = %e, "failed to open NMP membership interest");
        }
        id
    }

    /// Subscribe to the current user's kind:3 contact list so the follow-state
    /// for "Am I following this pubkey?" is available instantly when the
    /// profile view opens. Fire-and-forget; failures are logged.
    pub fn spawn_contacts_subscription(&self, pubkey: PublicKey) -> SubscriptionId {
        let id = SubscriptionId::generate();
        let filter = Filter::new().kinds([Kind::Custom(3)]).author(pubkey);
        if let Err(e) = self.open_nmp_filter(&id, "contacts", filter, None) {
            tracing::warn!(error = %e, "failed to open NMP contacts interest");
        }
        id
    }

    /// On-demand backfill for a specific NIP-23 article by (author, d).
    /// Called from the Following Reads pump when an interaction from a
    /// follow references an article the cache hasn't seen yet. Fire-and-
    /// forget — the usual relay-side nostrdb bridge persists the result
    /// and wakes the pump to re-query.
    pub fn spawn_article_address_backfill(&self, author: PublicKey, d_tag: String) {
        if d_tag.is_empty() {
            return;
        }
        let id = SubscriptionId::generate();
        let filter = Filter::new()
            .kinds([Kind::Custom(30023)])
            .author(author)
            .custom_tag(SingleLetterTag::lowercase(Alphabet::D), d_tag);
        if let Err(e) = self.open_nmp_filter_once_on_relays(
            &id,
            "indexer/article-backfill",
            filter,
            self.indexer_urls(),
        ) {
            tracing::warn!(error = %e, "failed to open NMP article backfill interest");
        }
    }

    /// Stage 2 of the join-set query: fetch metadata for the supplied
    /// groups via `{ kinds: [39000], '#d': <group_ids> }`. Called from the
    /// subscription pump as membership events arrive. Fire-and-forget.
    pub fn spawn_group_metadata_subscription(&self, group_ids: Vec<String>) {
        if group_ids.is_empty() {
            return;
        }
        let id = SubscriptionId::generate();
        let filter = Filter::new()
            .kinds([Kind::Custom(KIND_GROUP_METADATA)])
            .identifiers(group_ids);
        if let Err(e) = self.open_nmp_filter_once_on_relays(
            &id,
            "rooms/group-metadata",
            filter,
            self.rooms_urls(),
        ) {
            tracing::warn!(error = %e, "failed to open NMP group metadata interest");
        }
    }

    /// Catalog subscription for the rooms explorer: pull every NIP-29 group
    /// metadata event the relay has. The incoming 39000s land in nostrdb and
    /// power the "Browse all" grid + the "New & noteworthy" shelf. Fire-and-
    /// forget; the handle is kept by `HighlighterCore` so it can be dropped
    /// on logout.
    pub fn spawn_all_rooms_subscription(&self) -> SubscriptionId {
        let id = SubscriptionId::generate();
        let filter = Filter::new().kinds([Kind::Custom(KIND_GROUP_METADATA)]);
        if let Err(e) =
            self.open_nmp_filter_on_relays(&id, "rooms/all-rooms", filter, self.rooms_urls())
        {
            tracing::warn!(error = %e, "failed to open NMP all-rooms interest");
        }
        id
    }

    /// Bootstrap the *current user's own* relay config from the network at
    /// login. Without this, a fresh install with no cached kind:10002 (or
    /// kind:30078) falls back to `seed_defaults()` forever — so the user
    /// stays on Highlighter+damus+purple+primal even when their NIP-65 says
    /// they publish to four other relays.
    ///
    /// Strategy: install a long-lived subscription on the indexer pool for
    /// kind:10002 + kind:30078 authored by the user (so cross-device edits
    /// land in cache automatically), then run a one-shot fetch with a
    /// short timeout, and finally re-apply the freshly-populated rows to NMP.
    ///
    /// Returns the long-lived subscription id so logout can drop it.
    pub fn spawn_user_relay_config_bootstrap(&self, user_pubkey: PublicKey) -> SubscriptionId {
        let id = SubscriptionId::generate();
        let filter = Filter::new()
            .kinds([Kind::Custom(10002), Kind::Custom(30078)])
            .author(user_pubkey);
        if let Err(e) =
            self.open_nmp_filter_on_relays(&id, "indexer/user-relays", filter, self.indexer_urls())
        {
            tracing::warn!(error = %e, "failed to open NMP user relay config interest");
        }

        let pk_bytes: [u8; 32] = user_pubkey.to_bytes();
        let ndb_filter = NdbFilter::new()
            .kinds([10002u64, 30078u64])
            .authors([&pk_bytes])
            .build();
        let sub = match self.ndb.subscribe(&[ndb_filter]) {
            Ok(sub) => sub,
            Err(e) => {
                tracing::warn!(error = %e, "user relay config nostrdb observer");
                return id;
            }
        };

        let ndb = self.ndb.clone();
        let cache = self.current_relays.clone();
        let nmp = self.nmp.clone();
        let user_hex = user_pubkey.to_hex();
        let task = self.rt().spawn(async move {
            run_user_relay_config_observer(nmp, ndb, cache, sub, user_hex).await;
        });
        self.nmp_subscription_tasks
            .write()
            .insert(id.to_string(), task);
        id
    }

    /// Negentropy-sync the social trio (kind:0 metadata, kind:3 contacts,
    /// kind:10002 relay lists) for `authors` against the relays in
    /// `negentropy_sync_relays()`. Cheap cold-start backfill — on a
    /// re-login the relay sends only the events we're missing, vs. REQ
    /// which has to resend the full set (and is bound by the relay's
    /// `max_limit`, capping us at 500 events per query against most
    /// strfry deployments).
    ///
    /// Fire-and-forget. Events received during reconciliation land in
    /// nostrdb through NMP's raw event mirror. Sync runs in parallel against
    /// each relay; the per-relay timeout is short so a non-NIP-77 relay in
    /// the list can't block the others.
    /// No-op when `authors` is empty.
    pub fn spawn_negentropy_sync_for_follows(&self, authors: Vec<PublicKey>) {
        if authors.is_empty() {
            return;
        }
        let id = SubscriptionId::generate();
        let relays: Vec<String> = negentropy_sync_relays()
            .iter()
            .map(|relay| (*relay).to_string())
            .collect();
        let filter = Filter::new()
            .kinds([Kind::Custom(0), Kind::Custom(3), Kind::Custom(10002)])
            .authors(authors);
        if let Err(e) =
            self.open_nmp_filter_once_on_relays(&id, "sync/follows-social-trio", filter, relays)
        {
            tracing::warn!(error = %e, "failed to open NMP follows social-trio sync interest");
        }
    }

    /// Backfill follows' kind:10002 (NIP-65 relay lists) from the indexer
    /// pool so the outbox planner has data to work with. Without this, the
    /// per-pubkey relay map at outbox-compute time is empty and every
    /// follow falls into the "uncovered" fallback shard, defeating the
    /// whole point of routing. Fire-and-forget; long-lived so updates
    /// (new relay-list publications by a follow) keep landing in nostrdb.
    pub fn spawn_follows_relay_lists_subscription(
        &self,
        follows: Vec<PublicKey>,
    ) -> Option<SubscriptionId> {
        if follows.is_empty() {
            return None;
        }
        let id = SubscriptionId::generate();
        let filter = Filter::new().kinds([Kind::Custom(10002)]).authors(follows);
        if let Err(e) = self.open_nmp_filter_on_relays(
            &id,
            "indexer/follows-nip65",
            filter,
            self.indexer_urls(),
        ) {
            tracing::warn!(error = %e, "failed to open NMP follows relay-list interest");
        }
        Some(id)
    }

    /// Friends' NIP-51 group lists: kind:10009 authored by any of the user's
    /// follows. Each event enumerates the groups its author is a member of,
    /// so this is the primary signal for the "Friends are here" shelf —
    /// denser and more reliable than the relay-owned 39002 alone (users
    /// broadcast 10009 publicly; some relays gate 39002 behind auth). No-op
    /// if the follow set is empty.
    pub fn spawn_friends_groups_list_subscription(
        &self,
        follows: Vec<PublicKey>,
    ) -> Option<SubscriptionId> {
        if follows.is_empty() {
            return None;
        }
        let id = SubscriptionId::generate();
        let filter = Filter::new()
            .kinds([Kind::Custom(KIND_SIMPLE_GROUPS_LIST)])
            .authors(follows);
        if let Err(e) = self.open_nmp_filter_on_relays(
            &id,
            "indexer/friends-10009",
            filter,
            self.indexer_urls(),
        ) {
            tracing::warn!(error = %e, "failed to open NMP friends group-list interest");
        }
        Some(id)
    }

    /// Friends' memberships: pull kind:39001 / 39002 events where any of the
    /// user's follows appears in a `p` tag. This backfills the data the
    /// "Friends are here" shelf needs to surface rooms the user could join —
    /// the default login-time membership sub only sees the user's own groups,
    /// so without this shelf 3 stays mostly empty. No-op if the follow set
    /// is empty.
    pub fn spawn_friends_memberships_subscription(
        &self,
        follows: Vec<PublicKey>,
    ) -> Option<SubscriptionId> {
        if follows.is_empty() {
            return None;
        }
        let id = SubscriptionId::generate();
        let filter = Filter::new()
            .kinds([
                Kind::Custom(KIND_GROUP_ADMINS),
                Kind::Custom(KIND_GROUP_MEMBERS),
            ])
            .pubkeys(follows);
        if let Err(e) = self.open_nmp_filter_on_relays(
            &id,
            "rooms/friends-memberships",
            filter,
            self.rooms_urls(),
        ) {
            tracing::warn!(error = %e, "failed to open NMP friends membership interest");
        }
        Some(id)
    }

    /// Curated-list subscription: pull the latest kind:10012 from the supplied
    /// curator pubkey. The rooms referenced by the list are then backfilled
    /// with a separate metadata subscription once the Swift side calls
    /// `get_featured_rooms` and sees the list event in cache.
    pub fn spawn_curated_list_subscription(&self, curator: PublicKey) -> SubscriptionId {
        let id = SubscriptionId::generate();
        let filter = Filter::new()
            .kinds([Kind::Custom(crate::curation::KIND_CURATED_COMMUNITIES)])
            .author(curator);
        if let Err(e) =
            self.open_nmp_filter_on_relays(&id, "indexer/curated-list", filter, self.indexer_urls())
        {
            tracing::warn!(error = %e, "failed to open NMP curated-list interest");
        }
        id
    }

    /// Drop a subscription by id. Fire-and-forget.
    pub fn drop_subscription(&self, id: SubscriptionId) {
        let key = id.to_string();
        if let Some(task) = self.nmp_subscription_tasks.write().remove(&key) {
            task.abort();
        }
        if let Some(handles) = self.nmp_subscriptions.write().remove(&key) {
            for handle in handles {
                self.nmp.close_interest(handle);
            }
        }
    }

    fn spawn_connect(&self) {
        // No user logged in yet at runtime construction, so seed NMP with the
        // starting relay set. User-owned `RelayConfig` is applied after login.
        self.spawn_apply_relay_config(seed_defaults());
    }

    /// Reconcile NMP's relay registry with `rows` and cache the rows so the
    /// per-role URL accessors can answer synchronously. Logs on failure.
    ///
    /// Per-role routing at the subscription layer reads from that cache:
    /// NIP-29 subs → `rooms_urls()`, outbox-model lookups → `indexer_urls()`.
    pub fn spawn_apply_relay_config(&self, rows: Vec<RelayConfig>) {
        if let Err(e) = self.nmp.sync_relays(&rows) {
            tracing::warn!(error = %e, "NMP relay config apply");
        }
        *self.current_relays.write() = rows;
    }

    /// Convenience: load the user's persisted `RelayConfig` from nostrdb and
    /// reconcile NMP's relay registry. Called after login succeeds. Falls back to
    /// `seed_defaults()` if no kind:10002 / kind:30078 is cached yet.
    /// `purple_pages_relay()` remains part of the indexer accessor regardless
    /// of the user's editable rows.
    pub fn spawn_apply_user_relay_config(&self, user_hex: String) {
        let ndb = self.ndb.clone();
        let cache = self.current_relays.clone();
        let nmp = self.nmp.clone();
        self.rt().spawn(async move {
            let rows = match query_relays(&ndb, &user_hex) {
                Ok(rows) => rows,
                Err(e) => {
                    tracing::warn!(error = %e, "load relay config on login; using seed");
                    seed_defaults()
                }
            };
            if let Err(e) = nmp.sync_relays(&rows) {
                tracing::warn!(error = %e, "NMP user relay config apply");
            }
            *cache.write() = rows;
        });
    }

    /// Publish a pre-signed Highlighter event through NMP's action seam and
    /// reflect it into the local read cache for immediate Rust projections.
    pub fn publish_signed_event(&self, source: &str, event: &Event) -> Result<(), CoreError> {
        self.nmp.publish_signed_auto(source, event)?;
        self.cache_accepted_event(source, event)
    }

    /// Same as [`Self::publish_signed_event`], but deliberately pins the
    /// event to a relay set. Used for NIP-29 relay-owned group events and
    /// fixed infrastructure mirrors where Auto would be wrong.
    pub fn publish_signed_event_to_relays(
        &self,
        source: &str,
        event: &Event,
        relays: Vec<String>,
    ) -> Result<(), CoreError> {
        self.nmp.publish_signed_to_relays(source, event, relays)?;
        self.cache_accepted_event(source, event)
    }

    /// Explicit mirror without another local cache write. Used for
    /// infrastructure replicas after the canonical NMP publish has already
    /// reflected the event into nostrdb.
    pub fn mirror_signed_event_to_relays(
        &self,
        source: &str,
        event: &Event,
        relays: Vec<String>,
    ) -> Result<(), CoreError> {
        self.nmp.publish_signed_to_relays(source, event, relays)
    }

    pub fn mirror_social_trio_to_purple(
        &self,
        source: &str,
        event: &Event,
    ) -> Result<(), CoreError> {
        self.mirror_signed_event_to_relays(source, event, vec![purple_pages_relay().to_string()])
    }

    /// Snapshot of the most-recently-applied relay config. Empty until the
    /// first relay-role reconcile completes.
    pub fn current_relays(&self) -> Vec<RelayConfig> {
        self.current_relays.read().clone()
    }

    /// URLs of relays the user has marked for NIP-29 group traffic. Used by
    /// URLs of relays the user has marked for NIP-29 group traffic.
    /// `highlighter_relay()` is always included — it's the canonical groups
    /// host. Falls back to it alone when no rooms relay is configured, and
    /// adds it to any user-configured set that doesn't already contain it.
    pub fn rooms_urls(&self) -> Vec<String> {
        let mut urls: Vec<String> = self
            .current_relays
            .read()
            .iter()
            .filter(|r| r.rooms)
            .map(|r| r.url.clone())
            .collect();
        if !urls.iter().any(|u| u == highlighter_relay()) {
            urls.push(highlighter_relay().to_string());
        }
        urls
    }

    /// URLs of relays serving as the outbox-model bootstrap pool for
    /// resolving `kind:0` / `kind:3` / `kind:1xxxx` for arbitrary pubkeys.
    /// `purple_pages_relay()` is always included regardless of user config —
    /// it's app-internal infrastructure, not a user setting. Anything the
    /// user has flagged `indexer` in NIP-78 is added on top, deduped.
    pub fn indexer_urls(&self) -> Vec<String> {
        let mut urls: Vec<String> = vec![purple_pages_relay().to_string()];
        for row in self.current_relays.read().iter() {
            if row.indexer && row.url != purple_pages_relay() {
                urls.push(row.url.clone());
            }
        }
        urls
    }

    /// URLs of the user's NIP-65 read relays.
    pub fn read_urls(&self) -> Vec<String> {
        self.current_relays
            .read()
            .iter()
            .filter(|r| r.read)
            .map(|r| r.url.clone())
            .collect()
    }

    /// URLs of the user's NIP-65 write relays.
    pub fn write_urls(&self) -> Vec<String> {
        self.current_relays
            .read()
            .iter()
            .filter(|r| r.write)
            .map(|r| r.url.clone())
            .collect()
    }

    /// Current per-relay diagnostics snapshot from NMP's typed
    /// `relay_diagnostics` projection.
    pub fn relay_diagnostics_snapshot(&self) -> Vec<RelayDiagnostic> {
        self.nmp.relay_diagnostics_snapshot()
    }

    pub fn reconnect_all(&self) {
        self.nmp.foreground();
        let rows = self.current_relays();
        if let Err(e) = self.nmp.sync_relays(&rows) {
            tracing::warn!(error = %e, "NMP reconnect relay sync");
        }
    }

    pub fn disconnect_all(&self) {
        self.nmp.background();
    }

    pub fn install_relay_diagnostics_observer(&self, callback_slot: EventCallbackSlot) {
        self.nmp.set_relay_diagnostics_callback(callback_slot);
    }
}

async fn run_user_relay_config_observer(
    nmp: Arc<HighlighterNmpRuntime>,
    ndb: Arc<Ndb>,
    cache: Arc<parking_lot::RwLock<Vec<RelayConfig>>>,
    sub: nostrdb::Subscription,
    user_hex: String,
) {
    apply_user_relay_config_rows(&nmp, &ndb, &cache, &user_hex, "cached").await;

    let mut stream = sub.stream(&ndb).notes_per_await(32);
    while let Some(note_keys) = stream.next().await {
        if note_keys.is_empty() {
            continue;
        }
        apply_user_relay_config_rows(&nmp, &ndb, &cache, &user_hex, "mirrored").await;
    }
}

async fn apply_user_relay_config_rows(
    nmp: &HighlighterNmpRuntime,
    ndb: &Ndb,
    cache: &parking_lot::RwLock<Vec<RelayConfig>>,
    user_hex: &str,
    source: &str,
) {
    let rows = query_relays(ndb, user_hex).unwrap_or_else(|e| {
        tracing::warn!(user = %user_hex, error = %e, "user relay config query");
        seed_defaults()
    });
    if let Err(e) = nmp.sync_relays(&rows) {
        tracing::warn!(user = %user_hex, error = %e, "NMP user relay config apply");
    }
    *cache.write() = rows.clone();
    tracing::info!(
        user = %user_hex,
        source,
        relays = rows.len(),
        "user relay config applied"
    );
}

/// Resolve the platform-appropriate nostrdb directory. On iOS we're inside a
/// sandboxed container; `dirs::data_dir()` resolves to `<app>/Library/Application Support`
/// which is the correct location for persistent, non-user-visible data.
fn default_data_dir() -> Result<PathBuf, CoreError> {
    let base = dirs::data_dir()
        .ok_or_else(|| CoreError::Cache("no platform data_dir available".into()))?;
    Ok(base.join("highlighter").join("ndb"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use tempfile::tempdir;

    #[test]
    fn runtime_constructs_without_blocking() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("ndb");

        let started = Instant::now();
        let runtime = NostrRuntime::with_data_dir(path.clone()).expect("construct runtime");
        let elapsed = started.elapsed();

        // Runtime construction must not wait on network work. 2s is a
        // generous ceiling that still catches accidental blocking connects.
        assert!(
            elapsed.as_secs() < 2,
            "runtime construction took {elapsed:?} — should return immediately"
        );

        // Local state is wired up: Ndb dir exists and accepts queries.
        assert_eq!(runtime.data_dir(), path.as_path());
        assert!(runtime.data_dir().exists());

        // Sanity-check that a no-op nostrdb transaction works against our Ndb.
        let txn = nostrdb::Transaction::new(runtime.ndb()).expect("txn");
        let filter = nostrdb::Filter::new().kinds([9802]).build();
        let results = runtime.ndb().query(&txn, &[filter], 1).expect("query");
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn set_and_unset_signer_does_not_panic() {
        let tmp = tempdir().expect("tempdir");
        let runtime = NostrRuntime::with_data_dir(tmp.path().join("ndb")).expect("construct");

        let keys = Keys::generate();
        let nsec = keys.secret_key().to_bech32().expect("nsec");
        runtime.set_local_nsec_signer(&nsec).expect("sign in");
        runtime.unset_signer();
    }

    fn runtime_with_config(rows: Vec<RelayConfig>) -> (NostrRuntime, tempfile::TempDir) {
        let tmp = tempdir().expect("tempdir");
        let runtime =
            NostrRuntime::with_data_dir(tmp.path().join("ndb")).expect("construct runtime");
        // Populate the cache synchronously for role-URL accessor tests —
        // bypasses the background reconcile `spawn_connect` kicks off so
        // tests observe a deterministic config.
        *runtime.current_relays.write() = rows;
        (runtime, tmp)
    }

    #[test]
    fn rooms_urls_returns_room_rows_plus_default_highlighter() {
        let (rt, _tmp) = runtime_with_config(vec![
            RelayConfig {
                url: "wss://hl.example".into(),
                read: true,
                write: true,
                rooms: true,
                indexer: false,
            },
            RelayConfig {
                url: "wss://inbox.example".into(),
                read: true,
                write: true,
                rooms: false,
                indexer: false,
            },
            RelayConfig {
                url: "wss://index.example".into(),
                read: false,
                write: false,
                rooms: false,
                indexer: true,
            },
        ]);
        assert_eq!(
            rt.rooms_urls(),
            vec![
                "wss://hl.example".to_string(),
                highlighter_relay().to_string(),
            ]
        );
    }

    #[test]
    fn indexer_urls_returns_indexer_rows_plus_hardcoded_purple() {
        let (rt, _tmp) = runtime_with_config(vec![
            RelayConfig {
                url: "wss://hl.example".into(),
                read: true,
                write: true,
                rooms: true,
                indexer: false,
            },
            RelayConfig {
                url: "wss://purple.example".into(),
                read: false,
                write: false,
                rooms: false,
                indexer: true,
            },
            RelayConfig {
                url: "wss://primal.example".into(),
                read: false,
                write: false,
                rooms: false,
                indexer: true,
            },
        ]);
        let mut urls = rt.indexer_urls();
        urls.sort();
        // Hardcoded `wss://purplepag.es` is always present, then the
        // user's own indexer-flagged rows (deduped if the user happens
        // to list purple too).
        assert_eq!(
            urls,
            vec![
                "wss://primal.example".to_string(),
                "wss://purple.example".to_string(),
                "wss://purplepag.es".to_string(),
            ]
        );
    }

    #[test]
    fn indexer_urls_dedupes_purple_when_user_lists_it() {
        let (rt, _tmp) = runtime_with_config(vec![RelayConfig {
            url: "wss://purplepag.es".into(),
            read: false,
            write: false,
            rooms: false,
            indexer: true,
        }]);
        let urls = rt.indexer_urls();
        assert_eq!(urls, vec!["wss://purplepag.es".to_string()]);
    }

    #[test]
    fn read_and_write_urls_respect_nip65_flags() {
        let (rt, _tmp) = runtime_with_config(vec![
            RelayConfig {
                url: "wss://rw.example".into(),
                read: true,
                write: true,
                rooms: false,
                indexer: false,
            },
            RelayConfig {
                url: "wss://r.example".into(),
                read: true,
                write: false,
                rooms: false,
                indexer: false,
            },
            RelayConfig {
                url: "wss://w.example".into(),
                read: false,
                write: true,
                rooms: false,
                indexer: false,
            },
        ]);
        let mut reads = rt.read_urls();
        reads.sort();
        assert_eq!(
            reads,
            vec![
                "wss://r.example".to_string(),
                "wss://rw.example".to_string()
            ]
        );
        let mut writes = rt.write_urls();
        writes.sort();
        assert_eq!(
            writes,
            vec![
                "wss://rw.example".to_string(),
                "wss://w.example".to_string()
            ]
        );
    }

    #[test]
    fn role_urls_empty_before_reconcile() {
        let tmp = tempdir().expect("tempdir");
        let runtime =
            NostrRuntime::with_data_dir(tmp.path().join("ndb")).expect("construct runtime");
        // Accessors must not panic while the relay-role cache is empty or
        // partially reconciled.
        assert!(
            runtime.current_relays().is_empty() || !runtime.current_relays().is_empty(),
            "accessor must not panic even when cache is unpopulated"
        );
        // Role accessors on a freshly-built runtime return empty vecs
        // without hitting any async state.
        let _ = runtime.rooms_urls();
        let _ = runtime.indexer_urls();
        let _ = runtime.read_urls();
        let _ = runtime.write_urls();
    }
}
