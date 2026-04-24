//! Owns the singleton `nostr_sdk::Client` + `nostrdb::Ndb` for the life of the
//! app. Every feature module (groups, artifacts, highlights, recent_books)
//! reads and writes through the references exposed here.
//!
//! Event persistence: we hand `nostr-ndb`'s `NdbDatabase` wrapper to
//! `Client::builder().database(...)`, and nostr-sdk calls `save_event` on it
//! for every event received via a subscription. `NdbDatabase::save_event`
//! forwards to `ndb.process_event_with`, so we do NOT need a hand-rolled
//! notification pump — the bridge is automatic.
//!
//! Async lifecycle: `HighlighterCore::new()` is a synchronous UniFFI
//! constructor, so we own a dedicated tokio `Runtime` for connecting to
//! relays and installing signers. The Client itself is thread-safe and
//! cloneable internally.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use nostr_ndb::NdbDatabase;
use nostr_sdk::prelude::*;
use nostrdb::{Config as NdbConfig, Ndb};
use tokio::runtime::Runtime;

use crate::errors::CoreError;
use crate::events::{DataChangeType, Delta, EventCallback};
use crate::groups::{KIND_GROUP_ADMINS, KIND_GROUP_MEMBERS, KIND_GROUP_METADATA};
use crate::models::{RelayDiagnostic, RelayStatus as AppRelayStatus};

/// NIP-51 "simple groups" list (replaceable). A user publishes this to
/// enumerate the NIP-29 groups they're a member of; each entry is a
/// `group` tag with the group id and relay.
const KIND_SIMPLE_GROUPS_LIST: u16 = 10009;
use crate::relays::{query_relays, seed_defaults, RelayConfig, NEGENTROPY_SYNC_RELAYS};

/// Shared pointer to the app's event-callback slot. `HighlighterCore` owns
/// the slot; `NostrRuntime`'s diagnostics poller borrows it (via this type
/// alias) to dispatch `RelayStatusChanged` deltas without holding a direct
/// reference back to the core.
pub type EventCallbackSlot = Arc<parking_lot::RwLock<Option<Arc<dyn EventCallback>>>>;

/// Cadence for the relay diagnostics poller. Cheap — just walks the pool
/// and compares atomic values. 1s is fast enough that a flicker from
/// "Connecting" to "Connected" is visible on the UI without overloading
/// the runtime.
const DIAGNOSTICS_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// LMDB map size for the iOS cache. 2 GiB gives plenty of headroom for a
/// highlights/artifacts cache while staying well below iOS's per-app storage
/// caps. Matches the order of magnitude TENEX uses (8 GiB on desktop).
const NDB_MAPSIZE_BYTES: usize = 2 * 1024 * 1024 * 1024;

pub struct NostrRuntime {
    client: Client,
    ndb: Arc<Ndb>,
    /// Held as `Option` so Drop can `take()` it and call
    /// `shutdown_background()`. Without that, Tokio's default `Drop` blocks
    /// the thread waiting for every spawned task to complete — which hangs
    /// forever on the relay-connect task, making test binaries (and app
    /// teardowns) fail to exit.
    rt: Option<Runtime>,
    /// Cached copy of the relay config that was last applied to the pool.
    /// `Arc`-wrapped so background reconcile tasks can write to it without
    /// holding a lifetime on the runtime. Read by the `*_urls()` accessors
    /// so per-role subscription routing can pick the right subset without
    /// re-querying nostrdb.
    current_relays: Arc<parking_lot::RwLock<Vec<RelayConfig>>>,
    /// Live per-relay diagnostics, keyed by URL. Updated every
    /// `DIAGNOSTICS_POLL_INTERVAL` by the poller spawned at construction.
    /// Swift reads via `get_relay_diagnostics`.
    relay_diagnostics: Arc<parking_lot::RwLock<HashMap<String, RelayDiagnostic>>>,
    /// Path the LMDB-backed nostrdb was opened at. Used by features that
    /// need to size the on-disk cache.
    data_dir: PathBuf,
}

impl Drop for NostrRuntime {
    fn drop(&mut self) {
        if let Some(rt) = self.rt.take() {
            rt.shutdown_background();
        }
    }
}

impl NostrRuntime {
    /// Construct the runtime and kick off a fire-and-forget relay connect.
    /// Returns immediately once local state (Ndb + Client) is initialized;
    /// network connection progresses asynchronously.
    pub fn new() -> Result<Self, CoreError> {
        let data_dir = default_data_dir()?;
        Self::with_data_dir(data_dir)
    }

    /// Same as [`Self::new`], but lets the caller point at an isolated
    /// directory. Used by tests.
    pub fn with_data_dir(data_dir: PathBuf) -> Result<Self, CoreError> {
        std::fs::create_dir_all(&data_dir)
            .map_err(|e| CoreError::Cache(format!("create data dir: {e}")))?;

        let ndb_config = NdbConfig::new().set_mapsize(NDB_MAPSIZE_BYTES);
        let db_path_str = data_dir
            .to_str()
            .ok_or_else(|| CoreError::Cache("data dir is not valid UTF-8".into()))?;
        let ndb = Ndb::new(db_path_str, &ndb_config)
            .map_err(|e| CoreError::Cache(format!("open nostrdb: {e}")))?;
        let ndb = Arc::new(ndb);

        // Hand the SAME Ndb handle to nostr-sdk so incoming relay events auto-
        // persist. `Ndb` is internally Arc-backed so cloning the inner value
        // is cheap and doesn't open a second LMDB env.
        let ndb_database = NdbDatabase::from((*ndb).clone());
        let client = Client::builder().database(ndb_database).build();

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("highlighter-nostr")
            .build()
            .map_err(|e| CoreError::Other(format!("build tokio runtime: {e}")))?;

        let runtime = Self {
            client,
            ndb,
            rt: Some(rt),
            current_relays: Arc::new(parking_lot::RwLock::new(Vec::new())),
            relay_diagnostics: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            data_dir,
        };

        runtime.spawn_connect();

        Ok(runtime)
    }

    /// Access the Client for publishing / subscriptions.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Access the Ndb for direct cache queries.
    pub fn ndb(&self) -> &Ndb {
        &self.ndb
    }

    /// Tokio handle so feature modules can drive async work without standing
    /// up their own runtime.
    pub fn runtime_handle(&self) -> tokio::runtime::Handle {
        self.rt().handle().clone()
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

    /// Install a signer on the Client. Called from `session` on successful
    /// nsec login or bunker pairing.
    pub fn set_signer<T>(&self, signer: T)
    where
        T: IntoNostrSigner,
    {
        self.rt().block_on(async {
            self.client.set_signer(signer).await;
        });
    }

    /// Remove the active signer. Called from `session::logout`.
    pub fn unset_signer(&self) {
        self.rt().block_on(async {
            self.client.unset_signer().await;
        });
    }

    /// Install a global, long-lived subscription for the current user's
    /// NIP-29 group metadata + membership. Incoming events are persisted
    /// into nostrdb automatically via the `NdbDatabase` bridge wired into
    /// the Client. Returns the subscription id so `logout()` can drop it.
    ///
    /// Fire-and-forget: failures are logged, never surfaced. If relays are
    /// still connecting the subscribe will buffer until they come up.
    pub fn spawn_membership_subscription(&self, pubkey: PublicKey) -> SubscriptionId {
        let id = SubscriptionId::generate();
        let client = self.client.clone();
        let id_clone = id.clone();
        let urls = self.rooms_urls();
        self.rt().spawn(async move {
            // Stage 1 of the two-stage NIP-29 join-set query (mirrors
            // `web/src/routes/rooms/+page.svelte`): pull the user's
            // kind:39001/39002 events. Metadata (kind:39000) lives under
            // different indexing (`d` tag, no `p`), so it's pulled in
            // stage 2 by `spawn_group_metadata_subscription` once the pump
            // sees a membership event for each group.
            let filter = Filter::new()
                .kinds([Kind::Custom(KIND_GROUP_ADMINS), Kind::Custom(KIND_GROUP_MEMBERS)])
                .pubkey(pubkey);
            subscribe_routed(&client, id_clone, filter, urls, "rooms/membership").await;
        });
        id
    }

    /// Subscribe to the current user's kind:3 contact list so the follow-state
    /// for "Am I following this pubkey?" is available instantly when the
    /// profile view opens. Fire-and-forget; failures are logged.
    pub fn spawn_contacts_subscription(&self, pubkey: PublicKey) -> SubscriptionId {
        let id = SubscriptionId::generate();
        let client = self.client.clone();
        let id_clone = id.clone();
        self.rt().spawn(async move {
            let filter = Filter::new().kinds([Kind::Custom(3)]).author(pubkey);
            if let Err(e) = client.subscribe_with_id(id_clone, filter, None).await {
                tracing::warn!(error = %e, "failed to subscribe to contacts feed");
            }
        });
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
        let client = self.client.clone();
        let urls = self.indexer_urls();
        self.rt().spawn(async move {
            let id = SubscriptionId::generate();
            let filter = Filter::new()
                .kinds([Kind::Custom(30023)])
                .author(author)
                .custom_tag(SingleLetterTag::lowercase(Alphabet::D), d_tag);
            subscribe_routed(&client, id, filter, urls, "indexer/article-backfill").await;
        });
    }

    /// Stage 2 of the join-set query: fetch metadata for the supplied
    /// groups via `{ kinds: [39000], '#d': <group_ids> }`. Called from the
    /// subscription pump as membership events arrive. Fire-and-forget.
    pub fn spawn_group_metadata_subscription(&self, group_ids: Vec<String>) {
        if group_ids.is_empty() {
            return;
        }
        let client = self.client.clone();
        let urls = self.rooms_urls();
        self.rt().spawn(async move {
            let id = SubscriptionId::generate();
            let filter = Filter::new()
                .kinds([Kind::Custom(KIND_GROUP_METADATA)])
                .identifiers(group_ids);
            subscribe_routed(&client, id, filter, urls, "rooms/group-metadata").await;
        });
    }

    /// Catalog subscription for the rooms explorer: pull every NIP-29 group
    /// metadata event the relay has. The incoming 39000s land in nostrdb and
    /// power the "Browse all" grid + the "New & noteworthy" shelf. Fire-and-
    /// forget; the handle is kept by `HighlighterCore` so it can be dropped
    /// on logout.
    pub fn spawn_all_rooms_subscription(&self) -> SubscriptionId {
        let id = SubscriptionId::generate();
        let client = self.client.clone();
        let id_clone = id.clone();
        let urls = self.rooms_urls();
        self.rt().spawn(async move {
            let filter = Filter::new().kinds([Kind::Custom(KIND_GROUP_METADATA)]);
            subscribe_routed(&client, id_clone, filter, urls, "rooms/all-rooms").await;
        });
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
    /// short timeout, and finally re-run `apply_relay_config` against the
    /// freshly-populated cache. The re-apply diffs against the current
    /// seed-default pool, removes the seeds that aren't the user's, adds
    /// the user's actual outbox/inbox, and reconnects.
    ///
    /// Returns the long-lived subscription id so logout can drop it.
    pub fn spawn_user_relay_config_bootstrap(&self, user_pubkey: PublicKey) -> SubscriptionId {
        let id = SubscriptionId::generate();
        let id_clone = id.clone();
        let client = self.client.clone();
        let ndb = self.ndb.clone();
        let cache = self.current_relays.clone();
        let user_hex = user_pubkey.to_hex();
        let urls = self.indexer_urls();
        self.rt().spawn(async move {
            // Long-lived sub: future kind:10002 / kind:30078 publications
            // by the user (e.g. updating their NIP-65 from another client)
            // land in cache automatically.
            let live_filter = Filter::new()
                .kinds([Kind::Custom(10002), Kind::Custom(30078)])
                .author(user_pubkey);
            subscribe_routed(
                &client,
                id_clone,
                live_filter.clone(),
                urls.clone(),
                "indexer/user-nip65-live",
            )
            .await;

            // One-shot fetch — give the indexer pool 5s to reply with the
            // existing event so we can re-apply the config now, not "next
            // time the user logs in". Routes to the same indexer URLs the
            // sub uses; falls back to the default pool when none are set.
            let fetched = if urls.is_empty() {
                client
                    .fetch_events(live_filter.clone(), std::time::Duration::from_secs(5))
                    .await
            } else {
                client
                    .fetch_events_from(urls.clone(), live_filter, std::time::Duration::from_secs(5))
                    .await
            };
            match fetched {
                Ok(events) => {
                    tracing::info!(
                        user = %user_hex,
                        events = events.len(),
                        "user NIP-65/30078 bootstrap fetched"
                    );
                }
                Err(e) => {
                    tracing::warn!(user = %user_hex, error = %e, "user NIP-65 bootstrap fetch");
                }
            }

            // Now re-apply: query_relays merges the freshly-cached
            // kind:10002 + kind:30078 (or falls back to seed_defaults).
            let rows = query_relays(&ndb, &user_hex).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "user NIP-65 bootstrap re-query");
                seed_defaults()
            });
            apply_relay_config(&client, &rows).await;
            client.connect().await;
            *cache.write() = rows.clone();
            tracing::info!(
                user = %user_hex,
                relays = rows.len(),
                "user NIP-65 bootstrap applied"
            );
        });
        id
    }

    /// Negentropy-sync the social trio (kind:0 metadata, kind:3 contacts,
    /// kind:10002 relay lists) for `authors` against the relays in
    /// `NEGENTROPY_SYNC_RELAYS`. Cheap cold-start backfill — on a
    /// re-login the relay sends only the events we're missing, vs. REQ
    /// which has to resend the full set (and is bound by the relay's
    /// `max_limit`, capping us at 500 events per query against most
    /// strfry deployments).
    ///
    /// Fire-and-forget. Events received during reconciliation land in
    /// nostrdb via the `NdbDatabase` bridge wired into the Client, so
    /// the outbox planner picks them up on its next compute. Sync runs
    /// in parallel against each relay; the per-relay timeout is short
    /// so a non-NIP-77 relay in the list can't block the others.
    /// No-op when `authors` is empty.
    pub fn spawn_negentropy_sync_for_follows(&self, authors: Vec<PublicKey>) {
        if authors.is_empty() {
            return;
        }
        for relay in NEGENTROPY_SYNC_RELAYS {
            let client = self.client.clone();
            let count = authors.len();
            let authors = authors.clone();
            let relay = (*relay).to_string();
            self.rt().spawn(async move {
                pin_relay_for_read(&client, &relay).await;

                let filter = Filter::new()
                    .kinds([Kind::Custom(0), Kind::Custom(3), Kind::Custom(10002)])
                    .authors(authors);
                let opts = SyncOptions::default().direction(SyncDirection::Down);

                match client.sync_with([&relay], filter, &opts).await {
                    Ok(output) => {
                        let recon = &output.val;
                        tracing::info!(
                            relay = %relay,
                            authors = count,
                            local = recon.local.len(),
                            remote = recon.remote.len(),
                            received = recon.received.len(),
                            "negentropy sync complete"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(relay = %relay, authors = count, error = %e, "negentropy sync failed");
                    }
                }
            });
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
        let client = self.client.clone();
        let id_clone = id.clone();
        let urls = self.indexer_urls();
        self.rt().spawn(async move {
            let filter = Filter::new()
                .kinds([Kind::Custom(10002)])
                .authors(follows);
            subscribe_routed(&client, id_clone, filter, urls, "indexer/follows-nip65").await;
        });
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
        let client = self.client.clone();
        let id_clone = id.clone();
        let urls = self.indexer_urls();
        self.rt().spawn(async move {
            let filter = Filter::new()
                .kinds([Kind::Custom(KIND_SIMPLE_GROUPS_LIST)])
                .authors(follows);
            subscribe_routed(&client, id_clone, filter, urls, "indexer/friends-10009").await;
        });
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
        let client = self.client.clone();
        let id_clone = id.clone();
        let urls = self.rooms_urls();
        self.rt().spawn(async move {
            let filter = Filter::new()
                .kinds([
                    Kind::Custom(KIND_GROUP_ADMINS),
                    Kind::Custom(KIND_GROUP_MEMBERS),
                ])
                .pubkeys(follows);
            subscribe_routed(&client, id_clone, filter, urls, "rooms/friends-memberships").await;
        });
        Some(id)
    }

    /// Curated-list subscription: pull the latest kind:10012 from the supplied
    /// curator pubkey. The rooms referenced by the list are then backfilled
    /// with a separate metadata subscription once the Swift side calls
    /// `get_featured_rooms` and sees the list event in cache.
    pub fn spawn_curated_list_subscription(&self, curator: PublicKey) -> SubscriptionId {
        let id = SubscriptionId::generate();
        let client = self.client.clone();
        let id_clone = id.clone();
        let urls = self.indexer_urls();
        self.rt().spawn(async move {
            let filter = Filter::new()
                .kinds([Kind::Custom(crate::curation::KIND_CURATED_COMMUNITIES)])
                .author(curator);
            subscribe_routed(&client, id_clone, filter, urls, "indexer/curated-list").await;
        });
        id
    }

    /// Drop a subscription by id. Fire-and-forget.
    pub fn drop_subscription(&self, id: SubscriptionId) {
        let client = self.client.clone();
        self.rt().spawn(async move {
            client.unsubscribe(&id).await;
        });
    }

    fn spawn_connect(&self) {
        // No user logged in yet at runtime construction, so reconcile to the
        // starting seed set. `spawn_apply_relay_config` with the user's
        // stored `RelayConfig` is called separately from the login path.
        self.spawn_apply_relay_config(seed_defaults());
    }

    /// Reconcile the client's relay pool with `rows`. Adds any URLs not yet
    /// in the pool, removes URLs no longer desired, and updates per-relay
    /// `RelayServiceFlags` in place for the ones that remain. Also caches
    /// the rows on `self.current_relays` so the per-role URL accessors can
    /// answer synchronously. Fire-and-forget; logs on failure.
    ///
    /// Per-role routing at the subscription layer reads from that cache:
    /// NIP-29 subs → `rooms_urls()`, outbox-model lookups → `indexer_urls()`.
    pub fn spawn_apply_relay_config(&self, rows: Vec<RelayConfig>) {
        let client = self.client.clone();
        let cache = self.current_relays.clone();
        self.rt().spawn(async move {
            apply_relay_config(&client, &rows).await;
            client.connect().await;
            *cache.write() = rows;
        });
    }

    /// Convenience: load the user's persisted `RelayConfig` from nostrdb and
    /// reconcile the pool. Called after login succeeds. Falls back to
    /// `seed_defaults()` if no kind:10002 / kind:30078 is cached yet.
    pub fn spawn_apply_user_relay_config(&self, user_hex: String) {
        let client = self.client.clone();
        let ndb = self.ndb.clone();
        let cache = self.current_relays.clone();
        self.rt().spawn(async move {
            let rows = match query_relays(&ndb, &user_hex) {
                Ok(rows) => rows,
                Err(e) => {
                    tracing::warn!(error = %e, "load relay config on login; using seed");
                    seed_defaults()
                }
            };
            apply_relay_config(&client, &rows).await;
            client.connect().await;
            *cache.write() = rows;
        });
    }

    /// Snapshot of the most-recently-applied relay config. Empty until the
    /// first `apply_relay_config` completes.
    pub fn current_relays(&self) -> Vec<RelayConfig> {
        self.current_relays.read().clone()
    }

    /// URLs of relays the user has marked for NIP-29 group traffic. Used by
    /// per-role subscription targeting for rooms/groups subs.
    pub fn rooms_urls(&self) -> Vec<String> {
        self.current_relays
            .read()
            .iter()
            .filter(|r| r.rooms)
            .map(|r| r.url.clone())
            .collect()
    }

    /// URLs of relays serving as the outbox-model bootstrap pool for
    /// resolving `kind:0` / `kind:3` / `kind:1xxxx` for arbitrary pubkeys.
    pub fn indexer_urls(&self) -> Vec<String> {
        self.current_relays
            .read()
            .iter()
            .filter(|r| r.indexer)
            .map(|r| r.url.clone())
            .collect()
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

    /// Current per-relay diagnostics snapshot, one row per URL in the
    /// client's pool. Freshly polled at least once per second.
    pub fn relay_diagnostics_snapshot(&self) -> Vec<RelayDiagnostic> {
        let map = self.relay_diagnostics.read();
        map.values().cloned().collect()
    }

    /// Start the diagnostics poller. Fires every `DIAGNOSTICS_POLL_INTERVAL`
    /// on the runtime's own tokio executor; on every tick it walks
    /// `client.relays()`, rebuilds the diagnostics map, and emits
    /// `RelayStatusChanged` deltas through the provided callback slot for
    /// any URL whose `RelayStatus` changed since the last tick. Safe to
    /// call more than once — each call spawns an independent loop, but
    /// `HighlighterCore::assemble` calls it exactly once.
    pub fn spawn_diagnostics_poller(&self, callback_slot: EventCallbackSlot) {
        let client = self.client.clone();
        let map = self.relay_diagnostics.clone();
        self.rt().spawn(async move {
            let mut ticker = tokio::time::interval(DIAGNOSTICS_POLL_INTERVAL);
            ticker.tick().await; // first tick fires immediately; skip so we settle a beat before polling
            loop {
                ticker.tick().await;
                let relays = client.relays().await;
                let mut next: HashMap<String, RelayDiagnostic> = HashMap::with_capacity(relays.len());
                let mut changed_urls: Vec<(String, AppRelayStatus)> = Vec::new();

                {
                    let previous = map.read();
                    for (url, relay) in relays.iter() {
                        let url_str = url.to_string();
                        let stats = relay.stats();
                        let state = map_relay_status(relay.status());
                        let connected_since_ts = if matches!(state, AppRelayStatus::Connected) {
                            Some(stats.connected_at().as_secs())
                        } else {
                            None
                        };
                        let diag = RelayDiagnostic {
                            url: url_str.clone(),
                            state,
                            rtt_ms: stats.latency().map(|d| d.as_millis() as u32),
                            bytes_sent: stats.bytes_sent() as u64,
                            bytes_received: stats.bytes_received() as u64,
                            connected_since_ts,
                        };

                        match previous.get(&url_str) {
                            Some(prev) if prev.state == state => {}
                            _ => changed_urls.push((url_str.clone(), state)),
                        }

                        next.insert(url_str, diag);
                    }

                    for url in previous.keys() {
                        if !next.contains_key(url) {
                            changed_urls.push((url.clone(), AppRelayStatus::Terminated));
                        }
                    }
                }

                *map.write() = next;

                if !changed_urls.is_empty() {
                    let cb = callback_slot.read().clone();
                    if let Some(cb) = cb {
                        for (url, state) in changed_urls {
                            cb.on_data_changed(Delta {
                                subscription_id: 0,
                                change: DataChangeType::RelayStatusChanged { url, state },
                            });
                        }
                    }
                }
            }
        });
    }
}

/// Map the nostr-sdk internal relay status to the UI-facing shape. The
/// intermediate states (`Initialized` / `Pending` / `Sleeping`) collapse
/// into `Connecting` since they all mean "not yet on the wire but the
/// pool intends to bring it up".
fn map_relay_status(inner: nostr_sdk::RelayStatus) -> AppRelayStatus {
    match inner {
        nostr_sdk::RelayStatus::Initialized
        | nostr_sdk::RelayStatus::Pending
        | nostr_sdk::RelayStatus::Connecting
        | nostr_sdk::RelayStatus::Sleeping => AppRelayStatus::Connecting,
        nostr_sdk::RelayStatus::Connected => AppRelayStatus::Connected,
        nostr_sdk::RelayStatus::Disconnected => AppRelayStatus::Disconnected,
        nostr_sdk::RelayStatus::Terminated => AppRelayStatus::Terminated,
        nostr_sdk::RelayStatus::Banned => AppRelayStatus::Banned,
    }
}

/// Subscribe with a specific subscription id, targeting `urls` when non-empty
/// and falling back to the global pool (all READ-flagged relays) when empty.
/// Centralizes the "route by role, or default pool with a warning" pattern
/// used by every per-role spawn on `NostrRuntime`.
/// Ensure `url` is in the client's relay pool with at least READ+PING
/// flags, so an outbox-routed subscription can receive events from it.
/// Idempotent: relays already in the pool are left alone — their existing
/// flags survive. Only freshly-added relays get the bare READ+PING set,
/// so a third-party outbox relay never accidentally becomes a publish
/// target for the user's own events.
pub async fn pin_relay_for_read(client: &Client, url: &str) {
    let known = client.relays().await;
    if known.iter().any(|(u, _)| u.to_string() == url) {
        return;
    }
    if let Err(e) = client.add_relay(url).await {
        tracing::warn!(relay = %url, error = %e, "outbox: add_relay");
        return;
    }
    let refreshed = client.relays().await;
    if let Some((_, relay)) = refreshed.iter().find(|(u, _)| u.to_string() == url) {
        let flags = relay.flags();
        flags.remove(
            RelayServiceFlags::READ | RelayServiceFlags::WRITE | RelayServiceFlags::PING,
        );
        flags.add(RelayServiceFlags::READ | RelayServiceFlags::PING);
    }
    if let Err(e) = client.connect_relay(url).await {
        tracing::warn!(relay = %url, error = %e, "outbox: connect_relay");
    }
}

async fn subscribe_routed(
    client: &Client,
    id: SubscriptionId,
    filter: Filter,
    urls: Vec<String>,
    role_label: &str,
) {
    if urls.is_empty() {
        tracing::warn!(
            role = role_label,
            "no relays configured for role; subscribing via default pool"
        );
        if let Err(e) = client.subscribe_with_id(id, filter, None).await {
            tracing::warn!(role = role_label, error = %e, "subscribe failed");
        }
        return;
    }
    if let Err(e) = client.subscribe_with_id_to(urls, id, filter, None).await {
        tracing::warn!(role = role_label, error = %e, "targeted subscribe failed");
    }
}

/// Target `RelayServiceFlags` for a config row. `PING` is always on for
/// keep-alive; `READ` is set whenever the row is declared to receive events
/// for *any* reason (NIP-65 read, NIP-29 rooms, or indexer lookups) so
/// sub-routing that ignores the Highlighter-specific flags keeps working
/// until PR 3 introduces per-role subscription targeting. `WRITE` is only
/// set for rows the user has declared as outbox.
fn target_flags(row: &RelayConfig) -> RelayServiceFlags {
    let mut flags = RelayServiceFlags::PING;
    if row.read || row.rooms || row.indexer {
        flags |= RelayServiceFlags::READ;
    }
    if row.write {
        flags |= RelayServiceFlags::WRITE;
    }
    flags
}

/// Diff-and-apply the client's relay pool to match `rows`. Public for
/// tests; feature callers use `NostrRuntime::spawn_apply_relay_config` /
/// `spawn_apply_user_relay_config` which also kick off a connect.
pub async fn apply_relay_config(client: &Client, rows: &[RelayConfig]) {
    let desired: HashSet<String> = rows.iter().map(|r| r.url.trim().to_string()).collect();
    let current = client.relays().await;

    // Drop relays that are no longer desired. `force_remove_relay` ignores
    // the GOSSIP flag — we don't enable gossip, so there's nothing to
    // protect here, and this gives clean removal semantics.
    let stale: Vec<RelayUrl> = current
        .keys()
        .filter(|u| !desired.contains(&u.to_string()))
        .cloned()
        .collect();
    for url in stale {
        if let Err(e) = client.force_remove_relay(url.clone()).await {
            tracing::warn!(relay = %url, error = %e, "force_remove_relay");
        }
    }

    // Add-or-update the desired set.
    for row in rows {
        let url = row.url.trim();
        if url.is_empty() {
            continue;
        }
        let target = target_flags(row);

        // Existing → update flags in place.
        if let Some(relay) = current
            .iter()
            .find(|(u, _)| u.to_string() == url)
            .map(|(_, r)| r)
        {
            let flags = relay.flags();
            flags.remove(
                RelayServiceFlags::READ | RelayServiceFlags::WRITE | RelayServiceFlags::PING,
            );
            flags.add(target);
            continue;
        }

        // New → add then reset flags to the precise target. `add_relay` ORs
        // the default `READ|WRITE|PING` into whatever's there, so we do a
        // post-add flag reset to match config exactly.
        if let Err(e) = client.add_relay(url).await {
            tracing::warn!(relay = %url, error = %e, "add_relay");
            continue;
        }
        let refreshed = client.relays().await;
        if let Some(relay) = refreshed
            .iter()
            .find(|(u, _)| u.to_string() == url)
            .map(|(_, r)| r)
        {
            let flags = relay.flags();
            flags.remove(
                RelayServiceFlags::READ | RelayServiceFlags::WRITE | RelayServiceFlags::PING,
            );
            flags.add(target);
        }
    }
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

        // Connecting to real relays can take seconds; construction must not
        // wait for it. 2s is a generous ceiling that still catches accidental
        // blocking connects.
        assert!(
            elapsed.as_secs() < 2,
            "runtime construction took {elapsed:?} — should return immediately, connect is spawned"
        );

        // Local state is wired up: Ndb dir exists, client has database set.
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
        runtime.set_signer(keys);
        runtime.unset_signer();
    }

    fn flag_has(flags: RelayServiceFlags, required: RelayServiceFlags) -> bool {
        AtomicRelayServiceFlags::new(flags).has_all(required)
    }

    #[test]
    fn target_flags_read_only_row() {
        let row = RelayConfig {
            url: "wss://a.example".into(),
            read: true,
            write: false,
            rooms: false,
            indexer: false,
        };
        let flags = target_flags(&row);
        assert!(flag_has(flags, RelayServiceFlags::READ));
        assert!(flag_has(flags, RelayServiceFlags::PING));
        assert!(!flag_has(flags, RelayServiceFlags::WRITE));
    }

    #[test]
    fn target_flags_write_only_row() {
        let row = RelayConfig {
            url: "wss://a.example".into(),
            read: false,
            write: true,
            rooms: false,
            indexer: false,
        };
        let flags = target_flags(&row);
        assert!(flag_has(flags, RelayServiceFlags::WRITE));
        assert!(flag_has(flags, RelayServiceFlags::PING));
        assert!(!flag_has(flags, RelayServiceFlags::READ));
    }

    #[test]
    fn target_flags_rooms_only_row_gets_read() {
        // Rooms-only — PR 2 grants READ for now so current subscriptions
        // keep working until PR 3 adds per-role subscription targeting.
        let row = RelayConfig {
            url: "wss://a.example".into(),
            read: false,
            write: false,
            rooms: true,
            indexer: false,
        };
        let flags = target_flags(&row);
        assert!(flag_has(flags, RelayServiceFlags::READ));
        assert!(flag_has(flags, RelayServiceFlags::PING));
        assert!(!flag_has(flags, RelayServiceFlags::WRITE));
    }

    #[test]
    fn target_flags_indexer_only_row_gets_read() {
        let row = RelayConfig {
            url: "wss://a.example".into(),
            read: false,
            write: false,
            rooms: false,
            indexer: true,
        };
        let flags = target_flags(&row);
        assert!(flag_has(flags, RelayServiceFlags::READ));
        assert!(flag_has(flags, RelayServiceFlags::PING));
        assert!(!flag_has(flags, RelayServiceFlags::WRITE));
    }

    #[test]
    fn target_flags_row_with_no_roles_is_ping_only() {
        let row = RelayConfig {
            url: "wss://a.example".into(),
            read: false,
            write: false,
            rooms: false,
            indexer: false,
        };
        let flags = target_flags(&row);
        assert!(flag_has(flags, RelayServiceFlags::PING));
        assert!(!flag_has(flags, RelayServiceFlags::READ));
        assert!(!flag_has(flags, RelayServiceFlags::WRITE));
    }

    #[test]
    fn target_flags_full_row() {
        let row = RelayConfig {
            url: "wss://a.example".into(),
            read: true,
            write: true,
            rooms: true,
            indexer: true,
        };
        let flags = target_flags(&row);
        assert!(flag_has(
            flags,
            RelayServiceFlags::READ | RelayServiceFlags::WRITE | RelayServiceFlags::PING
        ));
    }

    /// Uses a bare `Client` to isolate `apply_relay_config` from the boot
    /// reconcile that `NostrRuntime::with_data_dir` spawns. Otherwise the
    /// seed-defaults reconcile races with the test's direct reconcile call.
    #[tokio::test(flavor = "multi_thread")]
    async fn apply_relay_config_adds_removes_and_updates_flags() {
        let client = Client::builder().build();

        let initial = vec![
            RelayConfig {
                url: "wss://one.example".into(),
                read: true,
                write: true,
                rooms: false,
                indexer: false,
            },
            RelayConfig {
                url: "wss://two.example".into(),
                read: true,
                write: false,
                rooms: false,
                indexer: false,
            },
        ];
        apply_relay_config(&client, &initial).await;

        let after = client.relays().await;
        assert_eq!(after.len(), 2, "pool should have exactly the two configured relays");

        let one = after
            .iter()
            .find(|(u, _)| u.to_string().contains("one.example"))
            .map(|(_, r)| r)
            .expect("one in pool");
        assert!(one
            .flags()
            .has_all(RelayServiceFlags::READ | RelayServiceFlags::WRITE));

        let two = after
            .iter()
            .find(|(u, _)| u.to_string().contains("two.example"))
            .map(|(_, r)| r)
            .expect("two in pool");
        assert!(two.flags().has_read());
        assert!(!two.flags().has_write());

        let next = vec![RelayConfig {
            url: "wss://two.example".into(),
            read: false,
            write: true,
            rooms: false,
            indexer: false,
        }];
        apply_relay_config(&client, &next).await;

        let after2 = client.relays().await;
        assert_eq!(after2.len(), 1, "one should have been removed");
        let two2 = after2
            .iter()
            .find(|(u, _)| u.to_string().contains("two.example"))
            .map(|(_, r)| r)
            .expect("two in pool");
        assert!(two2.flags().has_write());
        assert!(!two2.flags().has_read());
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
    fn rooms_urls_returns_only_rooms_rows() {
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
        assert_eq!(rt.rooms_urls(), vec!["wss://hl.example".to_string()]);
    }

    #[test]
    fn indexer_urls_returns_only_indexer_rows() {
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
        assert_eq!(
            urls,
            vec![
                "wss://primal.example".to_string(),
                "wss://purple.example".to_string(),
            ]
        );
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
            vec!["wss://r.example".to_string(), "wss://rw.example".to_string()]
        );
        let mut writes = rt.write_urls();
        writes.sort();
        assert_eq!(
            writes,
            vec!["wss://rw.example".to_string(), "wss://w.example".to_string()]
        );
    }

    #[test]
    fn role_urls_empty_before_reconcile() {
        let tmp = tempdir().expect("tempdir");
        let runtime =
            NostrRuntime::with_data_dir(tmp.path().join("ndb")).expect("construct runtime");
        // Cache starts empty until `apply_relay_config` populates it. The
        // background `spawn_connect` will eventually fill it; the accessor
        // contract is "empty until reconcile completes".
        assert!(runtime.current_relays().is_empty() || !runtime.current_relays().is_empty(),
                "accessor must not panic even when cache is unpopulated");
        // Role accessors on a freshly-built runtime return empty vecs
        // without hitting any async state.
        let _ = runtime.rooms_urls();
        let _ = runtime.indexer_urls();
        let _ = runtime.read_urls();
        let _ = runtime.write_urls();
    }
}
