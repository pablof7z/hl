//! Following Reads feed — kind:30023 articles surfaced through the NIP-02
//! follow graph. Two streams merge into one feed:
//!
//! 1. **Direct authorship** — articles the user's follows published
//!    (`kind:30023` with `authors ∈ follows`).
//! 2. **Social signal** — articles follows interacted with via
//!    `kind:1` (notes), `kind:7` (reactions), `kind:16` (reposts),
//!    or `kind:1111` (NIP-22 comments), each carrying `#k=30023` + an
//!    `a` / `A` tag pointing at the article's `30023:<pubkey>:<d>` address.
//!
//! Dedupes by article address. Each feed item carries the interaction
//! breakdown so the UI can render the "mentioned by @alice and 3 others"
//! line below the article card.

use std::collections::{BTreeMap, HashSet};

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::articles::{build_articles, KIND_LONG_FORM};
use crate::errors::CoreError;
use crate::follows;
use crate::models::{ArticleRecord, ReadingFeedItem};

/// kind:1 — plain note; quote/mention of the article surfaces it.
pub const KIND_NOTE: u16 = 1;
/// kind:7 — NIP-25 reaction.
pub const KIND_REACTION: u16 = 7;
/// kind:16 — NIP-18 generic repost.
pub const KIND_GENERIC_REPOST: u16 = 16;
/// kind:1111 — NIP-22 comment.
pub const KIND_NIP22_COMMENT: u16 = 1111;

/// The interaction kinds we monitor, in one place so the ndb + relay filters
/// and the pump agree on what counts as an interaction.
pub const INTERACTION_KINDS: [u16; 4] = [
    KIND_NOTE,
    KIND_REACTION,
    KIND_GENERIC_REPOST,
    KIND_NIP22_COMMENT,
];

/// Build the reads feed for the logged-in user from nostrdb. Returns items
/// sorted by most recent activity first. `limit` caps the final list; the
/// intermediate scans pull generously so dedupe + sort have enough material.
pub fn query_following_reads(
    ndb: &Ndb,
    user_pubkey_hex: &str,
    limit: u32,
) -> Result<Vec<ReadingFeedItem>, CoreError> {
    let user_pubkey_hex = user_pubkey_hex.trim();
    if user_pubkey_hex.is_empty() {
        return Ok(Vec::new());
    }

    let follows_hex = follows::query_follows(ndb, user_pubkey_hex)?;
    if follows_hex.is_empty() {
        return Ok(Vec::new());
    }

    let follows_pks: Vec<PublicKey> = follows_hex
        .iter()
        .filter_map(|s| PublicKey::from_hex(s.trim()).ok())
        .collect();
    if follows_pks.is_empty() {
        return Ok(Vec::new());
    }
    let follows_set: HashSet<String> = follows_pks
        .iter()
        .map(|pk| pk.to_hex().to_ascii_lowercase())
        .collect();

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    // Pull generously per stream so dedupe has headroom. The final slice
    // honors `limit` after sort.
    let per_stream_cap = (limit.saturating_mul(4)).max(128) as i32;

    // -- Stream A: articles authored by follows. ------------------------------
    let follow_bytes: Vec<[u8; 32]> = follows_pks.iter().map(|pk| pk.to_bytes()).collect();
    let follow_byte_refs: Vec<&[u8; 32]> = follow_bytes.iter().collect();
    let articles_filter = NdbFilter::new()
        .kinds([KIND_LONG_FORM as u64])
        .authors(follow_byte_refs.iter().copied())
        .build();

    let article_results = ndb
        .query(&txn, &[articles_filter], per_stream_cap)
        .map_err(|e| CoreError::Cache(format!("query follow articles: {e}")))?;

    let mut article_events: Vec<Event> = Vec::with_capacity(article_results.len());
    for r in &article_results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        article_events.push(event);
    }

    // -- Stream B: interactions by follows on kind:30023 content. -------------
    // ndb filters lack an "authors AND #k" combinator that matches exactly,
    // so we filter by kind + authors, then check `#k=30023` in Rust. Cheap —
    // follows-authored events are a small slice of the cache.
    let interactions_filter = NdbFilter::new()
        .kinds(INTERACTION_KINDS.iter().map(|k| *k as u64))
        .authors(follow_byte_refs.iter().copied())
        .build();
    let interaction_results = ndb
        .query(&txn, &[interactions_filter], per_stream_cap)
        .map_err(|e| CoreError::Cache(format!("query follow interactions: {e}")))?;

    let mut interaction_events: Vec<Event> = Vec::with_capacity(interaction_results.len());
    for r in &interaction_results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        if !references_article_kind(&event) {
            continue;
        }
        interaction_events.push(event);
    }

    // -- Assemble the feed. ---------------------------------------------------
    // Articles published directly by follows — dedupe by `d`, newest wins.
    let direct_articles = build_articles(&article_events, usize::MAX);
    let mut by_address: BTreeMap<String, PendingItem> = BTreeMap::new();

    for article in direct_articles {
        let address = format!("30023:{}:{}", article.pubkey, article.identifier);
        let activity_at = article
            .published_at
            .or(article.created_at)
            .unwrap_or(0);
        by_address
            .entry(address)
            .and_modify(|item| {
                item.author_followed = true;
                if activity_at > item.latest_activity_at {
                    item.latest_activity_at = activity_at;
                }
                item.article = Some(article.clone());
            })
            .or_insert(PendingItem {
                article: Some(article),
                author_followed: true,
                interactors: Vec::new(),
                latest_activity_at: activity_at,
            });
    }

    // Folding in interactions. Each one may point at an article we have not
    // seen yet; if so, look it up by (pubkey, d) on demand.
    for event in &interaction_events {
        let Some(addr) = referenced_article_address(event) else {
            continue;
        };
        let Some((author_hex, d_tag)) = parse_article_address(&addr) else {
            continue;
        };

        // If we haven't loaded the article for this address yet, query ndb.
        // Misses are fine — they'll appear later once the relay subscription
        // backfills and the pump re-triggers a query.
        let needs_article = by_address
            .get(&addr)
            .map(|p| p.article.is_none())
            .unwrap_or(true);
        if needs_article {
            let article = look_up_article(ndb, &txn, &author_hex, &d_tag);
            let activity_at = event.created_at.as_secs();
            let interactor_hex = event.pubkey.to_hex().to_ascii_lowercase();
            let author_followed = follows_set.contains(&author_hex.to_ascii_lowercase());
            by_address
                .entry(addr.clone())
                .and_modify(|item| {
                    if item.article.is_none() {
                        item.article = article.clone();
                    }
                    if !item.interactors.contains(&interactor_hex) {
                        item.interactors.push(interactor_hex.clone());
                    }
                    if activity_at > item.latest_activity_at {
                        item.latest_activity_at = activity_at;
                    }
                    item.author_followed = item.author_followed || author_followed;
                })
                .or_insert(PendingItem {
                    article,
                    author_followed,
                    interactors: vec![interactor_hex],
                    latest_activity_at: activity_at,
                });
        } else {
            let activity_at = event.created_at.as_secs();
            let interactor_hex = event.pubkey.to_hex().to_ascii_lowercase();
            if let Some(item) = by_address.get_mut(&addr) {
                if !item.interactors.contains(&interactor_hex) {
                    item.interactors.push(interactor_hex);
                }
                if activity_at > item.latest_activity_at {
                    item.latest_activity_at = activity_at;
                }
            }
        }
    }

    // Drop items whose article couldn't be resolved — showing a blank card
    // would be worse than showing nothing; backfill refreshes the feed.
    let mut items: Vec<ReadingFeedItem> = by_address
        .into_values()
        .filter_map(|p| {
            let article = p.article?;
            Some(ReadingFeedItem {
                article,
                author_followed: p.author_followed,
                interactor_pubkeys: p.interactors,
                latest_activity_at: p.latest_activity_at,
            })
        })
        .collect();

    items.sort_by(|a, b| b.latest_activity_at.cmp(&a.latest_activity_at));
    items.truncate(limit as usize);
    Ok(items)
}

/// Resolve the NIP-23 article at `pubkey:d` from nostrdb, newest replaceable
/// version. `None` if the cache hasn't seen the article yet — the caller is
/// expected to backfill via relay sub.
fn look_up_article(
    ndb: &Ndb,
    txn: &Transaction,
    pubkey_hex: &str,
    d_tag: &str,
) -> Option<ArticleRecord> {
    let author = PublicKey::from_hex(pubkey_hex).ok()?;
    let pk_bytes: [u8; 32] = author.to_bytes();
    let filter = NdbFilter::new()
        .kinds([KIND_LONG_FORM as u64])
        .authors([&pk_bytes])
        .tags([d_tag], 'd')
        .build();
    let results = ndb.query(txn, &[filter], 16).ok()?;

    let mut events: Vec<Event> = Vec::with_capacity(results.len());
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        events.push(event);
    }

    build_articles(&events, 1).into_iter().next()
}

/// Extract the first `a` or uppercase `A` tag value from an interaction
/// event, which for kind:30023 interactions is the addressable article
/// identifier `30023:<pubkey>:<d>`.
fn referenced_article_address(event: &Event) -> Option<String> {
    for tag in event.tags.iter() {
        let s = tag.as_slice();
        match s.first().map(String::as_str) {
            Some("a") | Some("A") => {
                let v = s.get(1).map(String::as_str)?.trim();
                if v.starts_with("30023:") {
                    return Some(v.to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_article_address(address: &str) -> Option<(String, String)> {
    let mut parts = address.splitn(3, ':');
    let kind = parts.next()?;
    if kind != "30023" {
        return None;
    }
    let pubkey = parts.next()?.trim();
    let d_tag = parts.next()?.trim();
    if pubkey.is_empty() || d_tag.is_empty() {
        return None;
    }
    Some((pubkey.to_string(), d_tag.to_string()))
}

/// True if any `k` tag on `event` names kind:30023. The filter we subscribe
/// with (`#k=30023`) guarantees this relay-side, but the ndb pump sees every
/// follow-authored event for these kinds, so we re-check locally.
fn references_article_kind(event: &Event) -> bool {
    event.tags.iter().any(|tag| {
        let s = tag.as_slice();
        s.first().map(String::as_str) == Some("k")
            && s.get(1).map(String::as_str) == Some("30023")
    })
}

struct PendingItem {
    article: Option<ArticleRecord>,
    author_followed: bool,
    interactors: Vec<String>,
    latest_activity_at: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostrdb::{Config as NdbConfig, Ndb};
    use tempfile::TempDir;

    fn isolated_ndb() -> (Ndb, TempDir) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("ndb");
        std::fs::create_dir_all(&path).expect("mkdir");
        let cfg = NdbConfig::new().set_mapsize(32 * 1024 * 1024);
        let ndb = Ndb::new(path.to_str().unwrap(), &cfg).expect("open ndb");
        (ndb, tmp)
    }

    fn ingest(ndb: &Ndb, event: &Event) {
        let line = format!("[\"EVENT\",\"sub\",{}]", event.as_json());
        ndb.process_event(&line).expect("process event");
    }

    fn sign(keys: &Keys, kind: u16, tags: Vec<Tag>, ts: u64, content: &str) -> Event {
        EventBuilder::new(Kind::Custom(kind), content)
            .tags(tags)
            .custom_created_at(Timestamp::from(ts))
            .sign_with_keys(keys)
            .expect("sign")
    }

    fn article_event(keys: &Keys, d: &str, title: &str, published_at: u64, ts: u64) -> Event {
        sign(
            keys,
            KIND_LONG_FORM,
            vec![
                Tag::identifier(d),
                Tag::parse(vec!["title".to_string(), title.to_string()]).unwrap(),
                Tag::parse(vec![
                    "published_at".to_string(),
                    published_at.to_string(),
                ])
                .unwrap(),
            ],
            ts,
            "body",
        )
    }

    fn contact_list(me: &Keys, follows: &[&Keys], ts: u64) -> Event {
        let tags: Vec<Tag> = follows.iter().map(|k| Tag::public_key(k.public_key())).collect();
        sign(me, 3, tags, ts, "")
    }

    fn interaction_event(
        actor: &Keys,
        kind: u16,
        article_address: &str,
        ts: u64,
        content: &str,
    ) -> Event {
        sign(
            actor,
            kind,
            vec![
                Tag::parse(vec!["a".to_string(), article_address.to_string()]).unwrap(),
                Tag::parse(vec!["k".to_string(), "30023".to_string()]).unwrap(),
            ],
            ts,
            content,
        )
    }

    #[test]
    fn empty_follows_returns_empty_feed() {
        let (ndb, _tmp) = isolated_ndb();
        let me = Keys::generate();
        // Don't publish a contact list.
        let feed =
            query_following_reads(&ndb, &me.public_key().to_hex(), 20).expect("query");
        assert!(feed.is_empty());
    }

    #[test]
    fn surfaces_direct_articles_by_follows() {
        let (ndb, _tmp) = isolated_ndb();
        let me = Keys::generate();
        let alice = Keys::generate();
        let bob = Keys::generate();
        let stranger = Keys::generate();

        ingest(&ndb, &contact_list(&me, &[&alice, &bob], 1));

        // Two articles by follows + one by a stranger (must be filtered out).
        let a1 = article_event(&alice, "post-1", "Hello From Alice", 1_000, 1_100);
        let b1 = article_event(&bob, "post-2", "Hello From Bob", 2_000, 2_100);
        let s1 = article_event(&stranger, "post-x", "Stranger", 3_000, 3_100);
        ingest(&ndb, &a1);
        ingest(&ndb, &b1);
        ingest(&ndb, &s1);

        let feed =
            query_following_reads(&ndb, &me.public_key().to_hex(), 20).expect("query");
        assert_eq!(feed.len(), 2, "only follow-authored articles surface");
        assert!(feed.iter().all(|item| item.author_followed));
        assert!(feed.iter().all(|item| item.interactor_pubkeys.is_empty()));

        // Newest published_at first.
        assert_eq!(feed[0].article.title, "Hello From Bob");
        assert_eq!(feed[1].article.title, "Hello From Alice");
    }

    #[test]
    fn surfaces_articles_via_interactions_by_follows() {
        let (ndb, _tmp) = isolated_ndb();
        let me = Keys::generate();
        let alice = Keys::generate();
        let outsider = Keys::generate();

        ingest(&ndb, &contact_list(&me, &[&alice], 1));

        // Article authored by someone the user does NOT follow.
        let outsider_post =
            article_event(&outsider, "gem", "Gem Article", 500, 500);
        ingest(&ndb, &outsider_post);
        let address = format!("30023:{}:gem", outsider.public_key().to_hex());

        // Alice (a follow) reacts + comments. Each should surface the article.
        let reaction = interaction_event(&alice, KIND_REACTION, &address, 1_000, "+");
        let comment = interaction_event(
            &alice,
            KIND_NIP22_COMMENT,
            &address,
            1_500,
            "great read",
        );
        ingest(&ndb, &reaction);
        ingest(&ndb, &comment);

        let feed =
            query_following_reads(&ndb, &me.public_key().to_hex(), 20).expect("query");
        assert_eq!(feed.len(), 1);
        let item = &feed[0];
        assert_eq!(item.article.title, "Gem Article");
        assert!(!item.author_followed);
        // Alice's pubkey must surface once — deduped even though she sent
        // two interactions.
        assert_eq!(item.interactor_pubkeys.len(), 1);
        assert_eq!(
            item.interactor_pubkeys[0],
            alice.public_key().to_hex().to_ascii_lowercase()
        );
        // Latest activity is the comment timestamp, not the reaction.
        assert_eq!(item.latest_activity_at, 1_500);
    }

    #[test]
    fn dedupes_and_ranks_across_streams() {
        let (ndb, _tmp) = isolated_ndb();
        let me = Keys::generate();
        let alice = Keys::generate();
        let bob = Keys::generate();

        ingest(&ndb, &contact_list(&me, &[&alice, &bob], 1));

        // Alice's article — surfaces via direct authorship. Bob reacts,
        // adding social signal to the same item.
        let post = article_event(&alice, "post", "Shared Article", 1_000, 1_000);
        ingest(&ndb, &post);
        let address = format!("30023:{}:post", alice.public_key().to_hex());
        let reaction = interaction_event(&bob, KIND_REACTION, &address, 5_000, "+");
        ingest(&ndb, &reaction);

        let feed =
            query_following_reads(&ndb, &me.public_key().to_hex(), 20).expect("query");
        assert_eq!(feed.len(), 1);
        let item = &feed[0];
        assert!(item.author_followed);
        assert_eq!(item.interactor_pubkeys.len(), 1);
        // Most recent timestamp wins — the interaction.
        assert_eq!(item.latest_activity_at, 5_000);
    }

    #[test]
    fn ignores_interactions_without_article_reference() {
        let (ndb, _tmp) = isolated_ndb();
        let me = Keys::generate();
        let alice = Keys::generate();

        ingest(&ndb, &contact_list(&me, &[&alice], 1));

        // A reaction without any `a` tag — has `#k=30023` but no addressable
        // reference. Must be ignored.
        let orphan = sign(
            &alice,
            KIND_REACTION,
            vec![
                Tag::parse(vec!["e".to_string(), "a".repeat(64)]).unwrap(),
                Tag::parse(vec!["k".to_string(), "30023".to_string()]).unwrap(),
            ],
            1_000,
            "+",
        );
        ingest(&ndb, &orphan);

        let feed =
            query_following_reads(&ndb, &me.public_key().to_hex(), 20).expect("query");
        assert!(feed.is_empty());
    }

    #[test]
    fn excludes_items_whose_article_is_uncached() {
        let (ndb, _tmp) = isolated_ndb();
        let me = Keys::generate();
        let alice = Keys::generate();
        let outsider = Keys::generate();

        ingest(&ndb, &contact_list(&me, &[&alice], 1));

        // Interaction references an article that was never ingested.
        let address = format!("30023:{}:ghost", outsider.public_key().to_hex());
        let reaction = interaction_event(&alice, KIND_REACTION, &address, 1_000, "+");
        ingest(&ndb, &reaction);

        let feed =
            query_following_reads(&ndb, &me.public_key().to_hex(), 20).expect("query");
        // Article isn't cached yet; surfacing a blank card is worse than
        // showing nothing. Expect empty until backfill lands.
        assert!(feed.is_empty());
    }

    #[test]
    fn honors_limit_after_sort() {
        let (ndb, _tmp) = isolated_ndb();
        let me = Keys::generate();
        let alice = Keys::generate();
        ingest(&ndb, &contact_list(&me, &[&alice], 1));
        for i in 0..5u64 {
            ingest(
                &ndb,
                &article_event(
                    &alice,
                    &format!("p{i}"),
                    &format!("T{i}"),
                    1_000 + i,
                    1_000 + i,
                ),
            );
        }
        let feed =
            query_following_reads(&ndb, &me.public_key().to_hex(), 2).expect("query");
        assert_eq!(feed.len(), 2);
        assert_eq!(feed[0].article.identifier, "p4");
        assert_eq!(feed[1].article.identifier, "p3");
    }
}
