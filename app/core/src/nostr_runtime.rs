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

use std::path::PathBuf;
use std::sync::Arc;

use nostr_ndb::NdbDatabase;
use nostr_sdk::prelude::*;
use nostrdb::{Config as NdbConfig, Ndb};
use tokio::runtime::Runtime;

use crate::errors::CoreError;
use crate::groups::{KIND_GROUP_ADMINS, KIND_GROUP_MEMBERS, KIND_GROUP_METADATA};
use crate::relays::DEFAULT_RELAYS;

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

    /// Drop a subscription by id. Fire-and-forget.
    pub fn drop_subscription(&self, id: SubscriptionId) {
        let client = self.client.clone();
        self.rt().spawn(async move {
            client.unsubscribe(&id).await;
        });
    }

    fn spawn_connect(&self) {
        let client = self.client.clone();
        self.rt().spawn(async move {
            for url in DEFAULT_RELAYS {
                if let Err(e) = client.add_relay(*url).await {
                    tracing::warn!(relay = %url, error = %e, "failed to add relay");
                }
            }
            client.connect().await;
        });
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
}
