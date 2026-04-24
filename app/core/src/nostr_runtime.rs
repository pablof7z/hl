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

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use nostr_ndb::NdbDatabase;
use nostr_sdk::prelude::*;
use nostrdb::{Config as NdbConfig, Ndb};
use tokio::runtime::Runtime;

use crate::errors::CoreError;
use crate::groups::{KIND_GROUP_ADMINS, KIND_GROUP_MEMBERS, KIND_GROUP_METADATA};

/// NIP-51 "simple groups" list (replaceable). A user publishes this to
/// enumerate the NIP-29 groups they're a member of; each entry is a
/// `group` tag with the group id and relay.
const KIND_SIMPLE_GROUPS_LIST: u16 = 10009;
use crate::relays::{query_relays, seed_defaults, RelayConfig};

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
    #[cfg(test)]
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
            #[cfg(test)]
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

    #[cfg(test)]
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
            if let Err(e) = client.subscribe_with_id(id_clone, filter, None).await {
                tracing::warn!(error = %e, "failed to subscribe to membership feed");
            }
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
        self.rt().spawn(async move {
            let id = SubscriptionId::generate();
            let filter = Filter::new()
                .kinds([Kind::Custom(30023)])
                .author(author)
                .custom_tag(SingleLetterTag::lowercase(Alphabet::D), d_tag);
            if let Err(e) = client.subscribe_with_id(id, filter, None).await {
                tracing::warn!(error = %e, "failed to spawn article backfill sub");
            }
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
        self.rt().spawn(async move {
            let id = SubscriptionId::generate();
            let filter = Filter::new()
                .kinds([Kind::Custom(KIND_GROUP_METADATA)])
                .identifiers(group_ids);
            if let Err(e) = client.subscribe_with_id(id, filter, None).await {
                tracing::warn!(error = %e, "failed to subscribe to group metadata feed");
            }
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
        self.rt().spawn(async move {
            let filter = Filter::new().kinds([Kind::Custom(KIND_GROUP_METADATA)]);
            if let Err(e) = client.subscribe_with_id(id_clone, filter, None).await {
                tracing::warn!(error = %e, "failed to subscribe to all rooms feed");
            }
        });
        id
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
        self.rt().spawn(async move {
            let filter = Filter::new()
                .kinds([Kind::Custom(KIND_SIMPLE_GROUPS_LIST)])
                .authors(follows);
            if let Err(e) = client.subscribe_with_id(id_clone, filter, None).await {
                tracing::warn!(error = %e, "failed to subscribe to friends groups list feed");
            }
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
        self.rt().spawn(async move {
            let filter = Filter::new()
                .kinds([
                    Kind::Custom(KIND_GROUP_ADMINS),
                    Kind::Custom(KIND_GROUP_MEMBERS),
                ])
                .pubkeys(follows);
            if let Err(e) = client.subscribe_with_id(id_clone, filter, None).await {
                tracing::warn!(error = %e, "failed to subscribe to friends memberships feed");
            }
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
        self.rt().spawn(async move {
            let filter = Filter::new()
                .kinds([Kind::Custom(crate::curation::KIND_CURATED_COMMUNITIES)])
                .author(curator);
            if let Err(e) = client.subscribe_with_id(id_clone, filter, None).await {
                tracing::warn!(error = %e, "failed to subscribe to curated list feed");
            }
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
    /// `RelayServiceFlags` in place for the ones that remain. Fire-and-
    /// forget; logs on failure.
    ///
    /// PR 2 scope: sets READ/WRITE based on the NIP-65 meaning of each row,
    /// with the relaxation that relays carrying only `rooms` or `indexer`
    /// still get the READ flag so current subscriptions continue to work.
    /// PR 3 swaps that global-pool behavior for per-role subscription
    /// targeting and removes the relaxation.
    pub fn spawn_apply_relay_config(&self, rows: Vec<RelayConfig>) {
        let client = self.client.clone();
        self.rt().spawn(async move {
            apply_relay_config(&client, &rows).await;
            client.connect().await;
        });
    }

    /// Convenience: load the user's persisted `RelayConfig` from nostrdb and
    /// reconcile the pool. Called after login succeeds. Falls back to
    /// `seed_defaults()` if no kind:10002 / kind:30078 is cached yet.
    pub fn spawn_apply_user_relay_config(&self, user_hex: String) {
        let client = self.client.clone();
        let ndb = self.ndb.clone();
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
        });
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
}
