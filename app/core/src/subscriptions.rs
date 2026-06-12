//! Subscription registry + per-subscription pump tasks.
//!
//! A "subscription" here is a Swift-visible handle (`u64`) that ties together
//! three things:
//! 1. a `nostrdb::Subscription` that receives new notes matching a filter,
//! 2. one or more NMP logical interests, keyed by `nostr_sdk::SubscriptionId`
//!    owners so the legacy FFI cancellation surface stays stable while NMP
//!    owns relay planning and wire subscriptions,
//! 3. a tokio pump task that drains the nostrdb subscription, builds the
//!    matching `DataChangeType` variants, and delivers them via the
//!    [`EventCallback`] stored on the core.
//!
//! Handles are allocated monotonically starting at 1. `0` is reserved for
//! app-scope deltas (signer state, joined-communities summary).

use std::sync::Arc;

use futures::StreamExt;
use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Subscription as NdbSub, Transaction};
use parking_lot::Mutex;
use tokio::task::JoinHandle;

use crate::errors::CoreError;
use crate::events::{DataChangeType, Delta, EventCallback};
use crate::feedback::{FEEDBACK_RELAY, KIND_FEEDBACK_NOTE, KIND_FEEDBACK_THREAD_META};
use crate::groups::{
    build_community_summary, KIND_GROUP_ADMINS, KIND_GROUP_MEMBERS, KIND_GROUP_METADATA,
};
use crate::nostr_runtime::NostrRuntime;
use crate::reads::INTERACTION_KINDS;

const KIND_METADATA: u16 = 0;
const KIND_CONTACTS: u16 = 3;
const KIND_LONG_FORM: u16 = 30023;
const KIND_HIGHLIGHT: u16 = 9802;

/// First handle allocated for view-scope subscriptions. `0` is reserved for
/// the app-scope bus (no specific subscription).
const FIRST_HANDLE: u64 = 1;

/// Hold-all for one live subscription.
struct Entry {
    /// Pump task draining the nostrdb subscription. Aborted on `remove`.
    task: JoinHandle<()>,
    /// NMP interest-owner ids to cancel when unsubscribing. Empty when the
    /// pump rides app-scope interests already installed at login.
    nmp_interest_ids: Vec<SubscriptionId>,
}

pub(crate) struct SubscriptionRegistry {
    inner: Mutex<Inner>,
    callback: Arc<parking_lot::RwLock<Option<Arc<dyn EventCallback>>>>,
}

struct Inner {
    next_handle: u64,
    entries: std::collections::HashMap<u64, Entry>,
}

#[derive(Clone)]
pub(crate) enum SubscriptionKind {
    JoinedCommunities {
        user_pubkey: PublicKey,
    },
    Room {
        group_id: String,
    },
    RoomDiscussions {
        group_id: String,
    },
    /// Powers the chat tab: kind:9 messages tagged `#h=group_id`. Lives on
    /// the same rooms-relays set as `Room` / `RoomDiscussions`.
    RoomChat {
        group_id: String,
    },
    Vault {
        user_pubkey: PublicKey,
    },
    /// Powers the profile page: listens for every event that could affect
    /// what renders on a profile (kind:0 metadata, kind:3 contacts, kind:30023
    /// articles, kind:9802 highlights authored by `pubkey`, plus kind:39001 /
    /// kind:39002 membership events that #p-tag `pubkey`). Delivers generic
    /// `UserProfileUpdated` deltas; the Swift store re-queries on each.
    UserProfile {
        pubkey: PublicKey,
    },
    /// Powers the article reader: listens for replaceable supersessions of
    /// this specific article (kind:30023 by `author` with matching `d`) and
    /// every kind:9802 highlight referencing the article's NIP-33 address
    /// (`30023:<pubkey>:<d>`). Delivers `ArticleUpdated` deltas; the Swift
    /// store re-queries body / highlights on each.
    Article {
        author: PublicKey,
        d_tag: String,
        /// Precomputed `30023:<pubkey_hex>:<d>` — convenient so the pump
        /// doesn't rebuild it on every delta.
        address: String,
    },
    /// Powers the "Reads" tab: articles authored by follows plus articles
    /// interacted with by follows (kinds 1/7/16/1111 + `#k=30023`). `follows`
    /// is a snapshot at subscribe time; if the user's contact list changes,
    /// the Swift store should drop + re-install the subscription.
    FollowingReads {
        follows: Vec<PublicKey>,
    },
    /// Powers the "Highlights" home tab: kind:9802 events authored by
    /// follows plus kind:9802 events tagged with `#h=<group_id>` for any
    /// joined room. Both sets are snapshots at subscribe time.
    FollowingHighlights {
        follows: Vec<PublicKey>,
        group_ids: Vec<String>,
    },
    /// Powers the shake-to-share feedback list: roots are kind:1 notes the
    /// current user authored that `a`-tag the project coordinate; titles come
    /// from kind:513 metadata events sharing the same `a` tag (any author —
    /// the project's agent emits these).
    FeedbackThreads {
        coordinate: String,
        current_user_pubkey: PublicKey,
    },
    /// Powers the open-thread chat: every kind:1 `e`-tagged to the root
    /// (regardless of author) plus the root itself. Author-agnostic so agent
    /// replies appear inline.
    FeedbackThread {
        root_event_id: EventId,
    },
    /// NIP-50 relay search for kind:30023 articles. `query` is the raw search
    /// term the user typed; `relays` is the resolved set to target (default
    /// `wss://relay.highlighter.com` plus any kind:10007 entries). The pump
    /// just forwards a `SearchArticlesUpdated { query }` trigger when matching
    /// events ingest into nostrdb — the Swift store re-runs the local ndb
    /// substring match on each delta to merge relay-delivered events into the
    /// Articles bucket.
    SearchArticles {
        query: String,
        relays: Vec<String>,
    },
    /// Current user's NIP-51 kind:10003 bookmark list. Fires
    /// `BookmarksUpdated` (app-scope) when a newer list lands in nostrdb —
    /// the Swift bookmarks store re-queries the authoritative list and every
    /// observing row reacts.
    Bookmarks {
        user_pubkey: PublicKey,
    },
    /// Current user's kind:30003 / kind:30004 sets. Fires `BookmarkSetsUpdated`
    /// (view-scoped) when any of the user's sets change.
    BookmarkSets {
        user_pubkey: PublicKey,
    },
    /// kind:30004 curation sets from followed authors. Fires
    /// `FollowingCurationSetsUpdated` (view-scoped) on each new set event.
    FollowingCurationSets {
        follows: Vec<PublicKey>,
    },
    /// Current user's NIP-B0 kind:39701 web bookmarks. Fires
    /// `WebBookmarksUpdated` (view-scoped) when any web bookmark changes.
    WebBookmarks {
        user_pubkey: PublicKey,
    },
}

impl SubscriptionRegistry {
    pub fn new(callback: Arc<parking_lot::RwLock<Option<Arc<dyn EventCallback>>>>) -> Self {
        Self {
            inner: Mutex::new(Inner {
                next_handle: FIRST_HANDLE,
                entries: std::collections::HashMap::new(),
            }),
            callback,
        }
    }

    /// Register a new subscription: opens an nostrdb sub matching the kind's
    /// filter, spawns a pump task, and (for `Room`) installs a relay sub.
    /// Returns the u64 handle for Swift.
    pub fn register(
        self: &Arc<Self>,
        runtime: &Arc<NostrRuntime>,
        kind: SubscriptionKind,
    ) -> Result<u64, crate::errors::CoreError> {
        let handle = {
            let mut guard = self.inner.lock();
            let h = guard.next_handle;
            guard.next_handle = guard.next_handle.saturating_add(1);
            h
        };

        let ndb_filters = build_ndb_filters(&kind);
        let sub = runtime
            .ndb()
            .subscribe(&ndb_filters)
            .map_err(|e| crate::errors::CoreError::Cache(format!("ndb subscribe: {e}")))?;

        let nmp_interest_ids = install_nmp_interests(runtime, &kind)?;

        let task = self.spawn_pump(runtime.clone(), sub, kind, handle);

        self.inner.lock().entries.insert(
            handle,
            Entry {
                task,
                nmp_interest_ids,
            },
        );

        Ok(handle)
    }

    /// Drop a subscription by handle. Idempotent.
    pub fn remove(&self, runtime: &NostrRuntime, handle: u64) {
        let Some(entry) = self.inner.lock().entries.remove(&handle) else {
            return;
        };
        entry.task.abort();
        for id in entry.nmp_interest_ids {
            runtime.drop_subscription(id);
        }
    }

    /// Remove every registered subscription. Used on logout / shutdown.
    pub fn clear(&self, runtime: &NostrRuntime) {
        let entries: Vec<(u64, Entry)> = {
            let mut guard = self.inner.lock();
            guard.entries.drain().collect()
        };
        for (_, entry) in entries {
            entry.task.abort();
            for id in entry.nmp_interest_ids {
                runtime.drop_subscription(id);
            }
        }
    }

    fn spawn_pump(
        self: &Arc<Self>,
        runtime: Arc<NostrRuntime>,
        sub: NdbSub,
        kind: SubscriptionKind,
        handle: u64,
    ) -> JoinHandle<()> {
        let callback_slot = self.callback.clone();
        runtime.runtime_handle().spawn(async move {
            run_pump(runtime, callback_slot, sub, kind, handle).await;
        })
    }
}

fn install_nmp_interests(
    runtime: &NostrRuntime,
    kind: &SubscriptionKind,
) -> Result<Vec<SubscriptionId>, CoreError> {
    let mut ids = Vec::new();
    match kind {
        SubscriptionKind::Room { group_id } => {
            let filter = Filter::new()
                .kinds([Kind::Custom(11), Kind::Custom(9802), Kind::Custom(16)])
                .custom_tag(SingleLetterTag::lowercase(Alphabet::H), group_id.clone());
            open_nmp_on_relays(
                runtime,
                &mut ids,
                "rooms/feed",
                filter,
                runtime.rooms_urls(),
            )?;
        }
        SubscriptionKind::RoomDiscussions { group_id } => {
            // Discussions are also kind:11 but distinguished by the
            // `t=discussion` marker. The relay-side filter matches kind:11
            // for this group; the pump filters to discussions in
            // `build_change`. Reusing `#h` means the Room sub and this one
            // receive the same events from nostrdb — each pump decides what
            // it cares about.
            let filter = Filter::new()
                .kinds([Kind::Custom(11)])
                .custom_tag(SingleLetterTag::lowercase(Alphabet::H), group_id.clone());
            open_nmp_on_relays(
                runtime,
                &mut ids,
                "rooms/discussions",
                filter,
                runtime.rooms_urls(),
            )?;
        }
        SubscriptionKind::RoomChat { group_id } => {
            // NIP-29 chat lives at kind:9 with `#h=<group_id>`. Targets the
            // rooms-flagged relays so the room host sees the REQ — chat is
            // not part of any user's outbox set, the room-relay is the
            // canonical home.
            let filter = Filter::new()
                .kinds([Kind::Custom(crate::chat::KIND_CHAT_MESSAGE)])
                .custom_tag(SingleLetterTag::lowercase(Alphabet::H), group_id.clone());
            open_nmp_on_relays(
                runtime,
                &mut ids,
                "rooms/chat",
                filter,
                runtime.rooms_urls(),
            )?;
        }
        SubscriptionKind::Vault { user_pubkey } => {
            let filter = Filter::new()
                .kinds([Kind::Custom(9802)])
                .author(*user_pubkey);
            open_nmp_auto(runtime, &mut ids, "vault/highlights", filter)?;
        }
        SubscriptionKind::UserProfile { pubkey } => {
            // Two separate filters: author-based for self-published kinds,
            // #p-based for membership events.
            let pk = *pubkey;
            let author_filter = Filter::new()
                .kinds([
                    Kind::Custom(KIND_METADATA),
                    Kind::Custom(KIND_CONTACTS),
                    Kind::Custom(KIND_LONG_FORM),
                    Kind::Custom(KIND_HIGHLIGHT),
                ])
                .author(pk);
            open_nmp_auto(runtime, &mut ids, "profile/author", author_filter)?;

            let membership_filter = Filter::new()
                .kinds([
                    Kind::Custom(KIND_GROUP_ADMINS),
                    Kind::Custom(KIND_GROUP_MEMBERS),
                ])
                .pubkey(pk);
            open_nmp_on_relays(
                runtime,
                &mut ids,
                "profile/membership",
                membership_filter,
                runtime.rooms_urls(),
            )?;
        }
        SubscriptionKind::Article {
            author,
            d_tag,
            address,
        } => {
            // Two relay filters: the article body (kind:30023 by author with
            // matching `d`) and its highlights (kind:9802 referencing the
            // `a`-tag). Keeping them as distinct subs keeps relay-side
            // filtering precise and costs no more than the single sub the
            // Room/Vault kinds install — the pool batches.
            let author_pk = *author;
            let body_filter = Filter::new()
                .kinds([Kind::Custom(KIND_LONG_FORM)])
                .author(author_pk)
                .custom_tag(SingleLetterTag::lowercase(Alphabet::D), d_tag.clone());
            open_nmp_auto(runtime, &mut ids, "article/body", body_filter)?;

            let highlights_filter = Filter::new()
                .kinds([Kind::Custom(KIND_HIGHLIGHT)])
                .custom_tag(SingleLetterTag::lowercase(Alphabet::A), address.clone());
            open_nmp_auto(runtime, &mut ids, "article/highlights", highlights_filter)?;
        }
        SubscriptionKind::FollowingReads { follows, .. } => {
            if follows.is_empty() {
                return Ok(ids);
            }
            let articles = Filter::new()
                .kinds([Kind::Custom(KIND_LONG_FORM)])
                .authors(follows.clone());
            open_nmp_auto(runtime, &mut ids, "feed/following-reads/articles", articles)?;

            let interactions = Filter::new()
                .kinds(INTERACTION_KINDS.iter().map(|k| Kind::Custom(*k)))
                .authors(follows.clone())
                .custom_tag(SingleLetterTag::lowercase(Alphabet::K), "30023");
            open_nmp_auto(
                runtime,
                &mut ids,
                "feed/following-reads/interactions",
                interactions,
            )?;
        }
        SubscriptionKind::FollowingHighlights { follows, group_ids } => {
            if !follows.is_empty() {
                let filter = Filter::new()
                    .kinds([Kind::Custom(KIND_HIGHLIGHT)])
                    .authors(follows.clone());
                open_nmp_auto(
                    runtime,
                    &mut ids,
                    "feed/following-highlights/authors",
                    filter,
                )?;
            }

            // Room-tagged highlights stay on the rooms-flagged relays where
            // the groups actually live — outbox routing doesn't apply here
            // because the authoritative source is the room host, not the
            // highlighters' personal outboxes.
            if !group_ids.is_empty() {
                let filter = Filter::new()
                    .kinds([Kind::Custom(KIND_HIGHLIGHT)])
                    .custom_tags(SingleLetterTag::lowercase(Alphabet::H), group_ids.clone());
                open_nmp_on_relays(
                    runtime,
                    &mut ids,
                    "feed/following-highlights/rooms",
                    filter,
                    runtime.rooms_urls(),
                )?;
            }
        }
        SubscriptionKind::FeedbackThreads {
            coordinate,
            current_user_pubkey,
        } => {
            // Both subs target only FEEDBACK_RELAY so the kind:1/513 traffic
            // for the project never fans out across the user's relay set.
            let pk = *current_user_pubkey;
            let roots = Filter::new()
                .kinds([Kind::Custom(KIND_FEEDBACK_NOTE)])
                .author(pk)
                .custom_tag(SingleLetterTag::lowercase(Alphabet::A), coordinate.clone());
            open_nmp_on_relays(
                runtime,
                &mut ids,
                "feedback/threads/roots",
                roots,
                vec![FEEDBACK_RELAY.to_string()],
            )?;

            let meta = Filter::new()
                .kinds([Kind::Custom(KIND_FEEDBACK_THREAD_META)])
                .custom_tag(SingleLetterTag::lowercase(Alphabet::A), coordinate.clone());
            open_nmp_on_relays(
                runtime,
                &mut ids,
                "feedback/threads/meta",
                meta,
                vec![FEEDBACK_RELAY.to_string()],
            )?;
        }
        SubscriptionKind::FeedbackThread { root_event_id } => {
            let root_hex = root_event_id.to_hex();
            let replies = Filter::new()
                .kinds([Kind::Custom(KIND_FEEDBACK_NOTE)])
                .custom_tag(SingleLetterTag::lowercase(Alphabet::E), root_hex);
            open_nmp_on_relays(
                runtime,
                &mut ids,
                "feedback/thread/replies",
                replies,
                vec![FEEDBACK_RELAY.to_string()],
            )?;
            let root = Filter::new().ids([*root_event_id]);
            open_nmp_on_relays(
                runtime,
                &mut ids,
                "feedback/thread/root",
                root,
                vec![FEEDBACK_RELAY.to_string()],
            )?;
        }
        SubscriptionKind::SearchArticles { query, relays } => {
            if relays.is_empty() || query.trim().is_empty() {
                return Ok(ids);
            }
            let filter = Filter::new()
                .kinds([Kind::Custom(KIND_LONG_FORM)])
                .search(query.clone())
                .limit(60);
            open_nmp_on_relays(runtime, &mut ids, "search/articles", filter, relays.clone())?;
        }
        // JoinedCommunities rides the membership relay-sub already installed
        // at login — no additional relay subscription needed.
        SubscriptionKind::JoinedCommunities { .. } => {}
        SubscriptionKind::Bookmarks { user_pubkey } => {
            let filter = Filter::new()
                .kinds([Kind::Custom(crate::bookmarks::KIND_BOOKMARKS)])
                .author(*user_pubkey);
            open_nmp_auto(runtime, &mut ids, "bookmarks/list", filter)?;
        }
        SubscriptionKind::BookmarkSets { user_pubkey } => {
            let filter = Filter::new()
                .kinds([
                    Kind::Custom(crate::lists::KIND_BOOKMARK_SETS),
                    Kind::Custom(crate::lists::KIND_CURATION_SETS),
                ])
                .author(*user_pubkey);
            open_nmp_auto(runtime, &mut ids, "bookmarks/sets", filter)?;
        }
        SubscriptionKind::FollowingCurationSets { follows } => {
            if follows.is_empty() {
                return Ok(ids);
            }
            let filter = Filter::new()
                .kinds([Kind::Custom(crate::lists::KIND_CURATION_SETS)])
                .authors(follows.clone());
            open_nmp_auto(runtime, &mut ids, "curation/following-sets", filter)?;
        }
        SubscriptionKind::WebBookmarks { user_pubkey } => {
            let filter = Filter::new()
                .kinds([Kind::Custom(crate::lists::KIND_WEB_BOOKMARK)])
                .author(*user_pubkey);
            open_nmp_auto(runtime, &mut ids, "bookmarks/web", filter)?;
        }
    }
    Ok(ids)
}

fn open_nmp_auto(
    runtime: &NostrRuntime,
    ids: &mut Vec<SubscriptionId>,
    label: &'static str,
    filter: Filter,
) -> Result<(), CoreError> {
    let id = SubscriptionId::generate();
    if let Err(e) = runtime.open_nmp_filter(&id, label, filter, None) {
        close_nmp_interests(runtime, ids);
        return Err(e);
    }
    ids.push(id);
    Ok(())
}

fn open_nmp_on_relays(
    runtime: &NostrRuntime,
    ids: &mut Vec<SubscriptionId>,
    label: &'static str,
    filter: Filter,
    relays: Vec<String>,
) -> Result<(), CoreError> {
    let id = SubscriptionId::generate();
    if let Err(e) = runtime.open_nmp_filter_on_relays(&id, label, filter, relays) {
        close_nmp_interests(runtime, ids);
        return Err(e);
    }
    ids.push(id);
    Ok(())
}

fn close_nmp_interests(runtime: &NostrRuntime, ids: &mut Vec<SubscriptionId>) {
    for id in ids.drain(..) {
        runtime.drop_subscription(id);
    }
}

fn build_ndb_filters(kind: &SubscriptionKind) -> Vec<NdbFilter> {
    match kind {
        SubscriptionKind::JoinedCommunities { .. } => {
            // Any 39000/39001/39002 event — the pump itself decides whether
            // it's relevant (kind:39000 for community upserts, 39001/39002
            // when the user is #p-tagged for membership changes).
            vec![NdbFilter::new()
                .kinds([
                    KIND_GROUP_METADATA as u64,
                    KIND_GROUP_ADMINS as u64,
                    KIND_GROUP_MEMBERS as u64,
                ])
                .build()]
        }
        // Room / RoomDiscussions: kind-only filter — nostrdb's `#h` tag index
        // is unreliable so we skip it here. `build_change` already checks
        // `first_tag_value(event, "h")` against the group_id in Rust, which
        // is the authoritative filter.
        SubscriptionKind::Room { .. } => {
            vec![NdbFilter::new().kinds([11u64, 9802u64, 16u64]).build()]
        }
        SubscriptionKind::RoomDiscussions { .. } => vec![NdbFilter::new().kinds([11u64]).build()],
        // RoomChat: kind:9 only — `build_change` re-checks `#h` against
        // the group_id so misrouted events can't poison another room.
        SubscriptionKind::RoomChat { .. } => vec![NdbFilter::new()
            .kinds([crate::chat::KIND_CHAT_MESSAGE as u64])
            .build()],
        SubscriptionKind::Vault { user_pubkey } => {
            let pk_bytes: [u8; 32] = user_pubkey.to_bytes();
            vec![NdbFilter::new()
                .kinds([9802u64])
                .authors([&pk_bytes])
                .build()]
        }
        SubscriptionKind::UserProfile { pubkey } => {
            let pk_bytes: [u8; 32] = pubkey.to_bytes();
            let pk_hex = pubkey.to_hex();
            let author_filter = NdbFilter::new()
                .kinds([
                    KIND_METADATA as u64,
                    KIND_CONTACTS as u64,
                    KIND_LONG_FORM as u64,
                    KIND_HIGHLIGHT as u64,
                ])
                .authors([&pk_bytes])
                .build();
            let membership_filter = NdbFilter::new()
                .kinds([KIND_GROUP_ADMINS as u64, KIND_GROUP_MEMBERS as u64])
                .tags([pk_hex.as_str()], 'p')
                .build();
            vec![author_filter, membership_filter]
        }
        SubscriptionKind::Article {
            author,
            d_tag,
            address,
        } => {
            let pk_bytes: [u8; 32] = author.to_bytes();
            let body_filter = NdbFilter::new()
                .kinds([KIND_LONG_FORM as u64])
                .authors([&pk_bytes])
                .tags([d_tag.as_str()], 'd')
                .build();
            let highlights_filter = NdbFilter::new()
                .kinds([KIND_HIGHLIGHT as u64])
                .tags([address.as_str()], 'a')
                .build();
            vec![body_filter, highlights_filter]
        }
        SubscriptionKind::FollowingReads { follows, .. } => {
            if follows.is_empty() {
                // Pump won't receive anything; still build a sentinel so the
                // SubscriptionRegistry bookkeeping is consistent.
                return vec![NdbFilter::new().kinds([KIND_LONG_FORM as u64]).build()];
            }
            let author_bytes: Vec<[u8; 32]> = follows.iter().map(|pk| pk.to_bytes()).collect();
            let author_refs: Vec<&[u8; 32]> = author_bytes.iter().collect();

            let articles_filter = NdbFilter::new()
                .kinds([KIND_LONG_FORM as u64])
                .authors(author_refs.iter().copied())
                .build();
            let interactions_filter = NdbFilter::new()
                .kinds(INTERACTION_KINDS.iter().map(|k| *k as u64))
                .authors(author_refs.iter().copied())
                .build();
            vec![articles_filter, interactions_filter]
        }
        SubscriptionKind::FollowingHighlights { .. } => {
            // Kind:9802 only — we filter by author / #h in `build_change`.
            // ndb's tag index for `h` is unreliable, hence the broad filter.
            vec![NdbFilter::new().kinds([KIND_HIGHLIGHT as u64]).build()]
        }
        SubscriptionKind::FeedbackThreads {
            current_user_pubkey,
            ..
        } => {
            // Kind:1 by current user (root or reply) and kind:513 from anyone.
            // The pump's `build_change` re-runs query_threads on each delta so
            // the title/summary/last_activity_at recompute after a 513 lands.
            let pk_bytes: [u8; 32] = current_user_pubkey.to_bytes();
            let roots = NdbFilter::new()
                .kinds([KIND_FEEDBACK_NOTE as u64])
                .authors([&pk_bytes])
                .build();
            let meta = NdbFilter::new()
                .kinds([KIND_FEEDBACK_THREAD_META as u64])
                .build();
            vec![roots, meta]
        }
        SubscriptionKind::FeedbackThread { .. } => {
            // Kind:1 only — `build_change` checks the `e` tag points at the
            // root before forwarding.
            vec![NdbFilter::new().kinds([KIND_FEEDBACK_NOTE as u64]).build()]
        }
        SubscriptionKind::SearchArticles { .. } => {
            // Every kind:30023 — the pump's `build_change` narrows by substring
            // match so a generic long-form event that happens to share a word
            // with the query still triggers a re-query (the Swift side does the
            // authoritative filtering against its own query-at-time).
            vec![NdbFilter::new().kinds([KIND_LONG_FORM as u64]).build()]
        }
        SubscriptionKind::Bookmarks { user_pubkey } => {
            let pk_bytes: [u8; 32] = user_pubkey.to_bytes();
            vec![NdbFilter::new()
                .kinds([crate::bookmarks::KIND_BOOKMARKS as u64])
                .authors([&pk_bytes])
                .build()]
        }
        SubscriptionKind::BookmarkSets { user_pubkey } => {
            let pk_bytes: [u8; 32] = user_pubkey.to_bytes();
            vec![NdbFilter::new()
                .kinds([
                    crate::lists::KIND_BOOKMARK_SETS as u64,
                    crate::lists::KIND_CURATION_SETS as u64,
                ])
                .authors([&pk_bytes])
                .build()]
        }
        SubscriptionKind::FollowingCurationSets { follows } => {
            if follows.is_empty() {
                return vec![];
            }
            let pk_bytes: Vec<[u8; 32]> = follows.iter().map(|pk| pk.to_bytes()).collect();
            let pk_refs: Vec<&[u8; 32]> = pk_bytes.iter().collect();
            vec![NdbFilter::new()
                .kinds([crate::lists::KIND_CURATION_SETS as u64])
                .authors(pk_refs.iter().copied())
                .build()]
        }
        SubscriptionKind::WebBookmarks { user_pubkey } => {
            let pk_bytes: [u8; 32] = user_pubkey.to_bytes();
            vec![NdbFilter::new()
                .kinds([crate::lists::KIND_WEB_BOOKMARK as u64])
                .authors([&pk_bytes])
                .build()]
        }
    }
}

async fn run_pump(
    runtime: Arc<NostrRuntime>,
    callback_slot: Arc<parking_lot::RwLock<Option<Arc<dyn EventCallback>>>>,
    sub: NdbSub,
    kind: SubscriptionKind,
    handle: u64,
) {
    // `SubscriptionStream::Drop` unsubscribes from nostrdb automatically, so
    // aborting this task cleans up the nostrdb side too.
    let mut stream = sub.stream(runtime.ndb()).notes_per_await(32);

    // JoinedCommunities deltas belong on the app-scope bus (`subscription_id
    // == 0`) — the CommunitySummary / MembershipChanged payloads mutate
    // `HighlighterStore.joinedCommunities`, not a view-scoped store. The
    // handle returned to Swift is only for cancellation. Room / Vault deltas
    // are view-scoped and route by handle.
    let delivery_id = match kind {
        SubscriptionKind::JoinedCommunities { .. } => 0,
        SubscriptionKind::Bookmarks { .. } => 0,
        _ => handle,
    };
    // BookmarkSets / FollowingCurationSets / WebBookmarks use `handle` (view-scoped)

    // Stage 2 of the NIP-29 join-set query: as we see membership events
    // (kind:39001/39002) for this user, install a `{ kinds: [39000], '#d':
    // [group_id] }` relay sub so metadata for the group streams in. Dedup
    // by `group_id`. On pump startup seed from cached membership events
    // so a relaunch with a warm nostrdb re-hydrates known groups without
    // waiting for new membership activity.
    let mut hydrated: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let SubscriptionKind::JoinedCommunities { user_pubkey } = &kind {
        let seeds = collect_cached_group_ids(runtime.ndb(), user_pubkey);
        if !seeds.is_empty() {
            runtime.spawn_group_metadata_subscription(seeds.iter().cloned().collect());
            hydrated.extend(seeds);
        }
    }

    // Per-pump dedupe set for FollowingReads article backfill: only fire the
    // one-shot relay sub per article address once for the life of the pump.
    let mut backfilled_articles: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    while let Some(note_keys) = stream.next().await {
        // Resolve and ship each delta. Open a fresh txn per batch so the
        // pump never holds one longer than necessary.
        let Ok(txn) = Transaction::new(runtime.ndb()) else {
            continue;
        };
        let mut deltas: Vec<Delta> = Vec::new();
        // FollowingReads / FollowingHighlights / FeedbackThreads collapse many
        // per-event deltas into one re-query trigger per batch — the Swift
        // store re-queries the whole feed on each, so one delta per batch is
        // plenty.
        let mut saw_following_reads_update = false;
        let mut saw_following_highlights_update = false;
        let mut saw_feedback_threads_update = false;
        let mut saw_search_articles_update = false;
        for key in note_keys {
            let Ok(note) = runtime.ndb().get_note_by_key(&txn, key) else {
                continue;
            };
            let Ok(json) = note.json() else { continue };
            let Ok(event) = Event::from_json(&json) else {
                continue;
            };
            // Article backfill: when a follow interacts with an uncached
            // article, pull that article so the next re-query surfaces it.
            if matches!(kind, SubscriptionKind::FollowingReads { .. }) {
                maybe_backfill_article(&runtime, &event, &mut backfilled_articles);
            }
            if let Some(change) = build_change(&kind, &event) {
                // Stage-2 hydrate: if this membership event introduces a new
                // group, record it and fetch its 39000 metadata from the relay.
                if let DataChangeType::MembershipChanged { group_id } = &change {
                    if hydrated.insert(group_id.clone()) {
                        runtime.spawn_group_metadata_subscription(vec![group_id.clone()]);
                    }
                }
                // Guard: only forward CommunityUpserted for groups the user is
                // actually a member of. The ndb filter catches all 39000 events
                // (no pubkey filter exists at that level), so a 39000 for an
                // unrelated group — e.g., landing from a profile-view relay sub
                // — must not pollute joinedCommunities.
                if let DataChangeType::CommunityUpserted { group_id } = &change {
                    if !hydrated.contains(group_id) {
                        continue;
                    }
                }
                if matches!(change, DataChangeType::FollowingReadsUpdated) {
                    saw_following_reads_update = true;
                    continue;
                }
                if matches!(change, DataChangeType::FollowingHighlightsUpdated) {
                    saw_following_highlights_update = true;
                    continue;
                }
                if matches!(change, DataChangeType::FeedbackThreadsUpdated) {
                    saw_feedback_threads_update = true;
                    continue;
                }
                if matches!(change, DataChangeType::SearchArticlesUpdated { .. }) {
                    saw_search_articles_update = true;
                    continue;
                }
                deltas.push(Delta {
                    subscription_id: delivery_id,
                    change,
                });
            }
        }
        drop(txn);

        if saw_following_reads_update {
            deltas.push(Delta {
                subscription_id: delivery_id,
                change: DataChangeType::FollowingReadsUpdated,
            });
        }
        if saw_following_highlights_update {
            deltas.push(Delta {
                subscription_id: delivery_id,
                change: DataChangeType::FollowingHighlightsUpdated,
            });
        }
        if saw_feedback_threads_update {
            deltas.push(Delta {
                subscription_id: delivery_id,
                change: DataChangeType::FeedbackThreadsUpdated,
            });
        }
        if saw_search_articles_update {
            if let SubscriptionKind::SearchArticles { query, .. } = &kind {
                deltas.push(Delta {
                    subscription_id: delivery_id,
                    change: DataChangeType::SearchArticlesUpdated {
                        query: query.clone(),
                    },
                });
            }
        }

        if deltas.is_empty() {
            continue;
        }

        let cb = { callback_slot.read().clone() };
        if let Some(cb) = cb {
            for delta in deltas {
                cb.on_data_changed(delta);
            }
        }
    }
}

/// For a FollowingReads interaction event, parse the referenced article
/// address and — if we haven't backfilled that address yet — spawn a
/// one-shot relay sub so the article body lands in nostrdb. No-op for
/// events that aren't kind:1 / 7 / 16 / 1111 or that don't carry an
/// `a`/`A` tag naming a `30023:…` address.
fn maybe_backfill_article(
    runtime: &NostrRuntime,
    event: &Event,
    already_backfilled: &mut std::collections::HashSet<String>,
) {
    if !INTERACTION_KINDS.contains(&event.kind.as_u16()) {
        return;
    }
    for tag in event.tags.iter() {
        let s = tag.as_slice();
        match s.first().map(String::as_str) {
            Some("a") | Some("A") => {
                let Some(addr) = s.get(1).map(String::as_str) else {
                    continue;
                };
                let addr = addr.trim();
                if !addr.starts_with("30023:") {
                    continue;
                }
                if !already_backfilled.insert(addr.to_string()) {
                    continue;
                }
                let mut parts = addr.splitn(3, ':');
                let _ = parts.next();
                let Some(pk_hex) = parts.next() else { continue };
                let Some(d_tag) = parts.next() else { continue };
                let Ok(author) = PublicKey::from_hex(pk_hex.trim()) else {
                    continue;
                };
                runtime.spawn_article_address_backfill(author, d_tag.trim().to_string());
            }
            _ => {}
        }
    }
}

fn build_change(kind: &SubscriptionKind, event: &Event) -> Option<DataChangeType> {
    match kind {
        SubscriptionKind::JoinedCommunities { user_pubkey } => {
            match event.kind.as_u16() {
                KIND_GROUP_METADATA => {
                    // Metadata only reaches nostrdb via the stage-2 relay sub
                    // (`#d=<learned_ids>`), so by construction every 39000 we
                    // see belongs to a group this user is in.
                    let summary = build_community_summary(event).ok()?;
                    Some(DataChangeType::CommunityUpserted {
                        group_id: summary.id,
                    })
                }
                KIND_GROUP_ADMINS | KIND_GROUP_MEMBERS => {
                    // Only relevant to this user — the relay-side sub already
                    // filters by #p, but the ndb pump sees every admin/member
                    // event so we re-check here.
                    let me_hex = user_pubkey.to_hex();
                    let has_me = event.tags.iter().any(|tag| {
                        let s = tag.as_slice();
                        s.first().map(String::as_str) == Some("p")
                            && s.get(1).map(String::as_str) == Some(me_hex.as_str())
                    });
                    if !has_me {
                        return None;
                    }
                    let group_id = first_tag_value(event, "d")?.to_string();
                    Some(DataChangeType::MembershipChanged { group_id })
                }
                _ => None,
            }
        }
        SubscriptionKind::Room { group_id } => {
            // Defensive: ndb filter already matched #h, but verify so a
            // misrouted event can't deliver into the wrong store.
            let event_group = first_tag_value(event, "h")?;
            if event_group != group_id {
                return None;
            }
            match event.kind.as_u16() {
                11 => {
                    // kind:11 with `t=discussion` belongs to the Discussions
                    // pump, not the Library pump — skip it here so the same
                    // event doesn't deliver twice under different semantics.
                    if crate::discussions::is_discussion(event) {
                        return None;
                    }
                    Some(DataChangeType::ArtifactUpserted {
                        group_id: group_id.clone(),
                    })
                }
                9802 => Some(DataChangeType::HighlightUpserted {
                    group_id: group_id.clone(),
                }),
                16 => {
                    // kind:16 repost is only a highlight share if its k=9802.
                    let k = first_tag_value(event, "k")?;
                    if k != "9802" {
                        return None;
                    }
                    let highlight_id = first_tag_value(event, "e")?.to_string();
                    Some(DataChangeType::HighlightShared {
                        group_id: group_id.clone(),
                        highlight_id,
                        shared_by_pubkey: event.pubkey.to_hex(),
                    })
                }
                _ => None,
            }
        }
        SubscriptionKind::RoomDiscussions { group_id } => {
            let event_group = first_tag_value(event, "h")?;
            if event_group != group_id {
                return None;
            }
            if event.kind.as_u16() != 11 {
                return None;
            }
            crate::discussions::record_from_event(event)?;
            Some(DataChangeType::DiscussionUpserted {
                group_id: group_id.clone(),
            })
        }
        SubscriptionKind::RoomChat { group_id } => {
            if event.kind.as_u16() != crate::chat::KIND_CHAT_MESSAGE {
                return None;
            }
            let event_group = first_tag_value(event, "h")?;
            if event_group != group_id {
                return None;
            }
            crate::chat::record_from_event(event)?;
            Some(DataChangeType::ChatMessageUpserted {
                group_id: group_id.clone(),
            })
        }
        SubscriptionKind::Vault { user_pubkey } => {
            if event.pubkey != *user_pubkey {
                return None;
            }
            if event.kind.as_u16() != 9802 {
                return None;
            }
            Some(DataChangeType::MyHighlightUpserted {
                event_id: event.id.to_hex(),
            })
        }
        SubscriptionKind::UserProfile { pubkey } => {
            let pk_hex = pubkey.to_hex();
            let relevant_kind = event.kind.as_u16();
            let is_self_authored = matches!(
                relevant_kind,
                KIND_METADATA | KIND_CONTACTS | KIND_LONG_FORM | KIND_HIGHLIGHT
            ) && event.pubkey == *pubkey;
            let is_membership = matches!(relevant_kind, KIND_GROUP_ADMINS | KIND_GROUP_MEMBERS)
                && event.tags.iter().any(|t| {
                    let s = t.as_slice();
                    s.first().map(String::as_str) == Some("p")
                        && s.get(1).map(String::as_str) == Some(pk_hex.as_str())
                });
            if !is_self_authored && !is_membership {
                return None;
            }
            Some(DataChangeType::UserProfileUpdated {
                pubkey: pk_hex,
                kind: relevant_kind as u32,
            })
        }
        SubscriptionKind::Article {
            author,
            d_tag,
            address,
        } => {
            let event_kind = event.kind.as_u16();
            match event_kind {
                KIND_LONG_FORM => {
                    // Same-author + same-`d` is the replaceable identity.
                    if event.pubkey != *author {
                        return None;
                    }
                    let d = first_tag_value(event, "d")?;
                    if d != d_tag {
                        return None;
                    }
                    Some(DataChangeType::ArticleUpdated {
                        address: address.clone(),
                        kind: event_kind as u32,
                    })
                }
                KIND_HIGHLIGHT => {
                    // Must reference this article's `a`-tag.
                    let references = event.tags.iter().any(|t| {
                        let s = t.as_slice();
                        s.first().map(String::as_str) == Some("a")
                            && s.get(1).map(String::as_str) == Some(address.as_str())
                    });
                    if !references {
                        return None;
                    }
                    Some(DataChangeType::ArticleUpdated {
                        address: address.clone(),
                        kind: event_kind as u32,
                    })
                }
                _ => None,
            }
        }
        SubscriptionKind::FollowingReads { follows, .. } => {
            // Follows are small (≤ a few thousand); linear scan is fine.
            let is_follow = follows.contains(&event.pubkey);
            if !is_follow {
                return None;
            }
            let event_kind = event.kind.as_u16();
            if event_kind == KIND_LONG_FORM {
                return Some(DataChangeType::FollowingReadsUpdated);
            }
            if INTERACTION_KINDS.contains(&event_kind) {
                // Require the `#k=30023` sentinel so a follow's random
                // unrelated kind:1 doesn't spam this feed.
                let references_article_kind = event.tags.iter().any(|t| {
                    let s = t.as_slice();
                    s.first().map(String::as_str) == Some("k")
                        && s.get(1).map(String::as_str) == Some("30023")
                });
                if !references_article_kind {
                    return None;
                }
                return Some(DataChangeType::FollowingReadsUpdated);
            }
            None
        }
        SubscriptionKind::FollowingHighlights { follows, group_ids } => {
            if event.kind.as_u16() != KIND_HIGHLIGHT {
                return None;
            }
            let is_follow = follows.contains(&event.pubkey);
            let in_joined_group = first_tag_value(event, "h")
                .map(|h| group_ids.iter().any(|g| g == h))
                .unwrap_or(false);
            if !is_follow && !in_joined_group {
                return None;
            }
            Some(DataChangeType::FollowingHighlightsUpdated)
        }
        SubscriptionKind::FeedbackThreads {
            coordinate,
            current_user_pubkey,
        } => {
            let has_coord = first_tag_value(event, "a")
                .map(|v| v == coordinate)
                .unwrap_or(false);
            if !has_coord {
                return None;
            }
            match event.kind.as_u16() {
                KIND_FEEDBACK_NOTE => {
                    if event.pubkey != *current_user_pubkey {
                        return None;
                    }
                    Some(DataChangeType::FeedbackThreadsUpdated)
                }
                KIND_FEEDBACK_THREAD_META => Some(DataChangeType::FeedbackThreadsUpdated),
                _ => None,
            }
        }
        SubscriptionKind::SearchArticles { query, .. } => {
            if event.kind.as_u16() != KIND_LONG_FORM {
                return None;
            }
            let q_lower = query.to_lowercase();
            if q_lower.is_empty() {
                return None;
            }
            let title = first_tag_value(event, "title").unwrap_or("").to_lowercase();
            let summary = first_tag_value(event, "summary")
                .unwrap_or("")
                .to_lowercase();
            let body_lower = event.content.to_lowercase();
            let tag_hit = event.tags.iter().any(|t| {
                let s = t.as_slice();
                s.first().map(String::as_str) == Some("t")
                    && s.get(1)
                        .map(|v| v.to_lowercase().contains(&q_lower))
                        .unwrap_or(false)
            });
            if !(title.contains(&q_lower)
                || summary.contains(&q_lower)
                || body_lower.contains(&q_lower)
                || tag_hit)
            {
                return None;
            }
            Some(DataChangeType::SearchArticlesUpdated {
                query: query.clone(),
            })
        }
        SubscriptionKind::Bookmarks { user_pubkey } => {
            if event.kind.as_u16() != crate::bookmarks::KIND_BOOKMARKS {
                return None;
            }
            if event.pubkey != *user_pubkey {
                return None;
            }
            Some(DataChangeType::BookmarksUpdated)
        }
        SubscriptionKind::BookmarkSets { user_pubkey } => {
            let k = event.kind.as_u16();
            if k != crate::lists::KIND_BOOKMARK_SETS && k != crate::lists::KIND_CURATION_SETS {
                return None;
            }
            if event.pubkey != *user_pubkey {
                return None;
            }
            Some(DataChangeType::BookmarkSetsUpdated)
        }
        SubscriptionKind::FollowingCurationSets { follows } => {
            if event.kind.as_u16() != crate::lists::KIND_CURATION_SETS {
                return None;
            }
            if !follows.contains(&event.pubkey) {
                return None;
            }
            Some(DataChangeType::FollowingCurationSetsUpdated)
        }
        SubscriptionKind::WebBookmarks { user_pubkey } => {
            if event.kind.as_u16() != crate::lists::KIND_WEB_BOOKMARK {
                return None;
            }
            if event.pubkey != *user_pubkey {
                return None;
            }
            Some(DataChangeType::WebBookmarksUpdated)
        }
        SubscriptionKind::FeedbackThread { root_event_id } => {
            if event.kind.as_u16() != KIND_FEEDBACK_NOTE {
                return None;
            }
            let root_hex = root_event_id.to_hex();
            let matches_root = event.id == *root_event_id
                || event.tags.iter().any(|tag| {
                    let s = tag.as_slice();
                    s.first().map(String::as_str) == Some("e")
                        && s.get(1).map(String::as_str) == Some(root_hex.as_str())
                });
            if !matches_root {
                return None;
            }
            Some(DataChangeType::FeedbackThreadEventUpserted {
                event_id: event.id.to_hex(),
            })
        }
    }
}

/// Scan nostrdb for all cached kind:39001 / 39002 events where `user_pubkey`
/// appears in a `p` tag and return the set of group ids. Used both by the
/// pump on startup (to seed the hydrated set) and by `client.rs` at login
/// time (to eagerly install stage-2 relay subscriptions before any delta
/// fires, so a warm cache populates immediately without waiting for new
/// membership events to arrive from the relay).
pub(crate) fn collect_cached_group_ids(
    ndb: &nostrdb::Ndb,
    user_pubkey: &PublicKey,
) -> std::collections::HashSet<String> {
    let mut out: std::collections::HashSet<String> = std::collections::HashSet::new();
    let me_hex = user_pubkey.to_hex();
    let Ok(txn) = Transaction::new(ndb) else {
        return out;
    };
    let filter = NdbFilter::new()
        .kinds([KIND_GROUP_ADMINS as u64, KIND_GROUP_MEMBERS as u64])
        .build();
    let Ok(results) = ndb.query(&txn, &[filter], 512) else {
        return out;
    };
    for r in results {
        let Ok(json) = r.note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        let has_me = event.tags.iter().any(|t| {
            let s = t.as_slice();
            s.first().map(String::as_str) == Some("p")
                && s.get(1).map(String::as_str) == Some(me_hex.as_str())
        });
        if !has_me {
            continue;
        }
        if let Some(group_id) = first_tag_value(&event, "d") {
            out.insert(group_id.to_string());
        }
    }
    out
}

fn first_tag_value<'a>(event: &'a Event, name: &str) -> Option<&'a str> {
    for tag in event.tags.iter() {
        let slice = tag.as_slice();
        if slice.first().map(String::as_str) == Some(name) {
            return slice.get(1).map(String::as_str);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::HighlighterCore;
    use nostrdb::Ndb;
    use std::sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    };
    use std::time::Duration;
    use tempfile::TempDir;

    /// Test-friendly callback that funnels deltas into a sync channel.
    struct ChannelCallback {
        tx: Sender<Delta>,
    }

    impl EventCallback for ChannelCallback {
        fn on_data_changed(&self, delta: Delta) {
            let _ = self.tx.send(delta);
        }
    }

    fn channel_callback() -> (Arc<ChannelCallback>, Receiver<Delta>) {
        let (tx, rx) = channel();
        (Arc::new(ChannelCallback { tx }), rx)
    }

    fn isolated_core() -> (Arc<HighlighterCore>, TempDir) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let core = HighlighterCore::new_with_data_dir(tmp.path().join("ndb"));
        (core, tmp)
    }

    fn sign(keys: &Keys, kind: u16, tags: Vec<Tag>, content: &str) -> Event {
        EventBuilder::new(Kind::Custom(kind), content)
            .tags(tags)
            .sign_with_keys(keys)
            .expect("sign")
    }

    fn process(ndb: &Ndb, event: &Event) {
        let relay_line = format!("[\"EVENT\",\"sub\",{}]", event.as_json());
        ndb.process_event(&relay_line).expect("process event");
    }

    /// Receive the next delta, skipping app-scope seeds (`subscription_id == 0`
    /// is reserved for `SignerConnected` / community-upsert bus events).
    fn recv_view_delta(rx: &Receiver<Delta>, timeout: Duration) -> Option<Delta> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return None;
            }
            let d = rx.recv_timeout(remaining).ok()?;
            if d.subscription_id != 0 {
                return Some(d);
            }
        }
    }

    #[test]
    fn subscribe_joined_communities_delivers_community_upserted() {
        let (core, _tmp) = isolated_core();
        let me = Keys::generate();
        let other = Keys::generate();

        core.login_nsec(me.secret_key().to_bech32().unwrap())
            .expect("login");

        let (cb, rx) = channel_callback();
        core.set_event_callback(cb);

        let handle = {
            let core = core.clone();
            std::thread::spawn(move || {
                futures::executor::block_on(core.subscribe_joined_communities())
            })
            .join()
            .expect("join")
            .expect("subscribe")
        };
        assert!(
            handle >= FIRST_HANDLE,
            "handle must start at 1 (0 is reserved)"
        );

        // Seed a valid 39000 + 39002 pair. nostrdb ingest will deliver via
        // the pump.
        let meta = sign(
            &other,
            39000,
            vec![
                Tag::identifier("alpha"),
                Tag::parse(vec!["name".to_string(), "Alpha".to_string()]).unwrap(),
                Tag::parse(vec!["public".to_string()]).unwrap(),
                Tag::parse(vec!["open".to_string()]).unwrap(),
            ],
            "",
        );
        let members = sign(
            &other,
            39002,
            vec![Tag::identifier("alpha"), Tag::public_key(me.public_key())],
            "",
        );

        // Membership must arrive before metadata so "alpha" is in the hydrated
        // set when the 39000 fires — the pump only delivers CommunityUpserted
        // for groups it has confirmed the user belongs to.
        process(core.runtime().ndb(), &members);
        process(core.runtime().ndb(), &meta);

        // JoinedCommunities deltas ride the app-scope bus (subscription_id
        // == 0); the handle is kept only for `unsubscribe()`. Skip the
        // `SignerConnected` seed the callback fires on `set_event_callback`.
        let mut saw_community = false;
        let mut saw_membership = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline && (!saw_community || !saw_membership) {
            let Ok(delta) = rx.recv_timeout(Duration::from_millis(200)) else {
                continue;
            };
            if matches!(delta.change, DataChangeType::SignerConnected { .. }) {
                assert_eq!(delta.subscription_id, 0);
                continue;
            }
            assert_eq!(
                delta.subscription_id, 0,
                "joined-communities rides app-scope bus"
            );
            match delta.change {
                DataChangeType::CommunityUpserted { group_id } => {
                    assert_eq!(group_id, "alpha");
                    saw_community = true;
                }
                DataChangeType::MembershipChanged { group_id } => {
                    assert_eq!(group_id, "alpha");
                    saw_membership = true;
                }
                _ => {}
            }
        }
        assert!(saw_community, "community upsert delta must arrive");
        assert!(saw_membership, "membership change delta must arrive");
    }

    #[test]
    fn unsubscribe_stops_further_deliveries() {
        let (core, _tmp) = isolated_core();
        let me = Keys::generate();
        let other = Keys::generate();

        core.login_nsec(me.secret_key().to_bech32().unwrap())
            .expect("login");
        let (cb, rx) = channel_callback();
        core.set_event_callback(cb);

        let handle = {
            let core = core.clone();
            std::thread::spawn(move || {
                futures::executor::block_on(core.subscribe_joined_communities())
            })
            .join()
            .expect("join")
            .expect("subscribe")
        };

        // Membership first — this adds "alpha" to the hydrated set so the
        // subsequent 39000 is allowed through.
        let members = sign(
            &other,
            39002,
            vec![Tag::identifier("alpha"), Tag::public_key(me.public_key())],
            "",
        );
        let meta = sign(
            &other,
            39000,
            vec![
                Tag::identifier("alpha"),
                Tag::parse(vec!["name".to_string(), "Alpha".to_string()]).unwrap(),
            ],
            "",
        );
        process(core.runtime().ndb(), &members);
        process(core.runtime().ndb(), &meta);

        // Drain until we see a community-shaped delta (skip SignerConnected).
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut drained = false;
        while std::time::Instant::now() < deadline {
            let Ok(delta) = rx.recv_timeout(Duration::from_millis(200)) else {
                continue;
            };
            if matches!(
                delta.change,
                DataChangeType::CommunityUpserted { .. } | DataChangeType::MembershipChanged { .. }
            ) {
                drained = true;
                break;
            }
        }
        assert!(
            drained,
            "expected at least one community delivery before unsubscribe"
        );

        core.unsubscribe(handle);

        // Drain any stragglers so the window starts clean.
        while rx.try_recv().is_ok() {}

        // After unsubscribe, a new matching event must not deliver.
        let meta2 = sign(
            &other,
            39000,
            vec![
                Tag::identifier("bravo"),
                Tag::parse(vec!["name".to_string(), "Bravo".to_string()]).unwrap(),
            ],
            "",
        );
        process(core.runtime().ndb(), &meta2);

        // Nothing community-shaped should arrive within the window.
        let deadline = std::time::Instant::now() + Duration::from_millis(500);
        while std::time::Instant::now() < deadline {
            let Ok(delta) = rx.recv_timeout(Duration::from_millis(100)) else {
                continue;
            };
            if matches!(delta.change, DataChangeType::CommunityUpserted { .. }) {
                panic!("no community delivery expected after unsubscribe");
            }
        }
    }

    #[test]
    fn subscribe_room_filters_by_h_tag() {
        let (core, _tmp) = isolated_core();
        let me = Keys::generate();
        let other = Keys::generate();

        core.login_nsec(me.secret_key().to_bech32().unwrap())
            .expect("login");
        let (cb, rx) = channel_callback();
        core.set_event_callback(cb);

        // Subscribe to group "alpha" only.
        let handle = {
            let core = core.clone();
            std::thread::spawn(move || {
                futures::executor::block_on(core.subscribe_room("alpha".to_string()))
            })
            .join()
            .expect("join")
            .expect("subscribe")
        };

        // kind:11 event for alpha
        let share_alpha = sign(
            &other,
            11,
            vec![
                Tag::parse(vec!["h".to_string(), "alpha".to_string()]).unwrap(),
                Tag::identifier("art-1"),
                Tag::parse(vec!["title".to_string(), "Alpha Book".to_string()]).unwrap(),
                Tag::parse(vec!["source".to_string(), "book".to_string()]).unwrap(),
            ],
            "",
        );
        // kind:11 event for bravo (must be ignored)
        let share_bravo = sign(
            &other,
            11,
            vec![
                Tag::parse(vec!["h".to_string(), "bravo".to_string()]).unwrap(),
                Tag::identifier("art-2"),
                Tag::parse(vec!["title".to_string(), "Bravo Book".to_string()]).unwrap(),
            ],
            "",
        );

        process(core.runtime().ndb(), &share_alpha);
        process(core.runtime().ndb(), &share_bravo);

        let mut alpha_seen = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            let Some(delta) = recv_view_delta(&rx, Duration::from_millis(200)) else {
                continue;
            };
            assert_eq!(delta.subscription_id, handle);
            match delta.change {
                DataChangeType::ArtifactUpserted { group_id } => {
                    assert_eq!(group_id, "alpha");
                    alpha_seen = true;
                }
                other => panic!("unexpected delta: {other:?}"),
            }
        }
        assert!(alpha_seen, "expected alpha artifact delta");
    }

    #[test]
    fn subscribe_vault_filters_by_author() {
        let (core, _tmp) = isolated_core();
        let me = Keys::generate();
        let other = Keys::generate();

        core.login_nsec(me.secret_key().to_bech32().unwrap())
            .expect("login");
        let (cb, rx) = channel_callback();
        core.set_event_callback(cb);

        let handle = {
            let core = core.clone();
            std::thread::spawn(move || futures::executor::block_on(core.subscribe_vault()))
                .join()
                .expect("join")
                .expect("subscribe")
        };

        // Highlight authored by `me` — should deliver.
        let mine = sign(
            &me,
            9802,
            vec![Tag::parse(vec!["r".to_string(), "https://example.com".to_string()]).unwrap()],
            "my quote",
        );
        // Highlight authored by `other` — must not deliver.
        let theirs = sign(
            &other,
            9802,
            vec![Tag::parse(vec!["r".to_string(), "https://example.com".to_string()]).unwrap()],
            "their quote",
        );

        process(core.runtime().ndb(), &mine);
        process(core.runtime().ndb(), &theirs);

        let mut mine_seen = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            let Some(delta) = recv_view_delta(&rx, Duration::from_millis(200)) else {
                continue;
            };
            assert_eq!(delta.subscription_id, handle);
            match delta.change {
                DataChangeType::MyHighlightUpserted { event_id } => {
                    assert_eq!(event_id, mine.id.to_hex());
                    mine_seen = true;
                }
                other => panic!("unexpected delta: {other:?}"),
            }
        }
        assert!(mine_seen, "vault sub must deliver self-authored highlight");
    }

    #[test]
    fn joined_communities_pump_ignores_metadata_for_unjoined_groups() {
        // Regression: the pump subscribed to ALL 39000/39001/39002 events in
        // ndb with no pubkey filter. Any 39000 — even for a group the user is
        // not a member of — would blindly emit CommunityUpserted and pollute
        // the list. Only groups where the user has a matching 39001/39002 (with
        // their pubkey in #p) should produce deltas.
        let (core, _tmp) = isolated_core();
        let me = Keys::generate();
        let other = Keys::generate();
        let stranger = Keys::generate();

        core.login_nsec(me.secret_key().to_bech32().unwrap())
            .expect("login");
        let (cb, rx) = channel_callback();
        core.set_event_callback(cb);

        let _handle = {
            let core = core.clone();
            std::thread::spawn(move || {
                futures::executor::block_on(core.subscribe_joined_communities())
            })
            .join()
            .expect("join")
            .expect("subscribe");
        };

        // A 39000 for a group where ONLY `stranger` is a member — user `me`
        // has no membership event for this group.
        let meta_strangers_group = sign(
            &other,
            39000,
            vec![
                Tag::identifier("strangers-only"),
                Tag::parse(vec!["name".to_string(), "Strangers Only".to_string()]).unwrap(),
            ],
            "",
        );
        let members_stranger_only = sign(
            &other,
            39002,
            vec![
                Tag::identifier("strangers-only"),
                Tag::public_key(stranger.public_key()),
            ],
            "",
        );

        process(core.runtime().ndb(), &meta_strangers_group);
        process(core.runtime().ndb(), &members_stranger_only);

        // Now ingest a group that `me` IS actually a member of, to confirm
        // the pump is still alive and does deliver for real memberships.
        let meta_mine = sign(
            &other,
            39000,
            vec![
                Tag::identifier("mine"),
                Tag::parse(vec!["name".to_string(), "Mine".to_string()]).unwrap(),
            ],
            "",
        );
        let members_mine = sign(
            &other,
            39002,
            vec![Tag::identifier("mine"), Tag::public_key(me.public_key())],
            "",
        );
        process(core.runtime().ndb(), &meta_mine);
        process(core.runtime().ndb(), &members_mine);

        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut saw_mine = false;
        while std::time::Instant::now() < deadline {
            let Ok(delta) = rx.recv_timeout(Duration::from_millis(200)) else {
                if saw_mine {
                    break;
                }
                continue;
            };
            if matches!(delta.change, DataChangeType::SignerConnected { .. }) {
                continue;
            }
            match &delta.change {
                DataChangeType::CommunityUpserted { group_id } => {
                    assert_ne!(
                        group_id, "strangers-only",
                        "must not emit CommunityUpserted for a group the user is not a member of"
                    );
                    if group_id == "mine" {
                        saw_mine = true;
                    }
                }
                DataChangeType::MembershipChanged { group_id } => {
                    assert_ne!(
                        group_id, "strangers-only",
                        "must not emit MembershipChanged for a group the user is not a member of"
                    );
                    if group_id == "mine" {
                        saw_mine = true;
                    }
                }
                _ => {}
            }
        }
        assert!(
            saw_mine,
            "must still deliver deltas for groups the user is in"
        );
    }

    #[test]
    fn subscribe_room_chat_emits_chat_message_upserted_for_matching_h_tag() {
        let (core, _tmp) = isolated_core();
        let me = Keys::generate();
        let other = Keys::generate();

        core.login_nsec(me.secret_key().to_bech32().unwrap())
            .expect("login");
        let (cb, rx) = channel_callback();
        core.set_event_callback(cb);

        let handle = {
            let core = core.clone();
            std::thread::spawn(move || {
                futures::executor::block_on(core.subscribe_room_chat("alpha".to_string()))
            })
            .join()
            .expect("join")
            .expect("subscribe")
        };

        // kind:9 event for alpha — must deliver as ChatMessageUpserted.
        let chat_alpha = sign(
            &other,
            crate::chat::KIND_CHAT_MESSAGE,
            vec![Tag::parse(vec!["h".to_string(), "alpha".to_string()]).unwrap()],
            "hello alpha",
        );
        // kind:9 event for bravo — must be ignored by the alpha pump.
        let chat_bravo = sign(
            &other,
            crate::chat::KIND_CHAT_MESSAGE,
            vec![Tag::parse(vec!["h".to_string(), "bravo".to_string()]).unwrap()],
            "hello bravo",
        );

        process(core.runtime().ndb(), &chat_alpha);
        process(core.runtime().ndb(), &chat_bravo);

        let mut alpha_seen = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            let Some(delta) = recv_view_delta(&rx, Duration::from_millis(200)) else {
                continue;
            };
            assert_eq!(delta.subscription_id, handle);
            match delta.change {
                DataChangeType::ChatMessageUpserted { group_id } => {
                    assert_eq!(group_id, "alpha");
                    alpha_seen = true;
                }
                other => panic!("unexpected delta on chat sub: {other:?}"),
            }
        }
        assert!(alpha_seen, "expected alpha chat delta");
    }
}
