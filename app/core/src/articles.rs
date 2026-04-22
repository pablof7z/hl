//! NIP-23 long-form articles (kind:30023) — the "Writing" tab on a user
//! profile.

use std::collections::BTreeMap;

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;
use crate::models::ArticleRecord;

pub const KIND_LONG_FORM: u16 = 30023;

/// Read a single NIP-23 article by its NIP-33 addressable id (`pubkey:d`).
/// Returns the newest `created_at` event with a matching `d` tag. `None` if
/// nostrdb has no matching event cached — the reader view spawns a relay
/// subscription on the article's address to backfill, at which point a later
/// call returns `Some`.
pub fn query_article(
    ndb: &Ndb,
    pubkey_hex: &str,
    d_tag: &str,
) -> Result<Option<ArticleRecord>, CoreError> {
    let pubkey_hex = pubkey_hex.trim();
    let d_tag = d_tag.trim();
    if pubkey_hex.is_empty() || d_tag.is_empty() {
        return Ok(None);
    }
    let author = PublicKey::from_hex(pubkey_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let pk_bytes: [u8; 32] = author.to_bytes();
    let filter = NdbFilter::new()
        .kinds([KIND_LONG_FORM as u64])
        .authors([&pk_bytes])
        .tags([d_tag], 'd')
        .build();

    let results = ndb
        .query(&txn, &[filter], 32)
        .map_err(|e| CoreError::Cache(format!("query article: {e}")))?;

    let mut events: Vec<Event> = Vec::with_capacity(results.len());
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        events.push(event);
    }

    Ok(build_articles(&events, 1).into_iter().next())
}

/// Read a pubkey's long-form articles from nostrdb, deduped by `d` tag
/// (newest wins, matching NIP-33 parameterized replaceable semantics) and
/// sorted desc by `published_at` (falling back to `created_at`).
pub fn query_articles_by_author(
    ndb: &Ndb,
    pubkey_hex: &str,
    limit: u32,
) -> Result<Vec<ArticleRecord>, CoreError> {
    if pubkey_hex.is_empty() {
        return Ok(Vec::new());
    }
    let author = PublicKey::from_hex(pubkey_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let pk_bytes: [u8; 32] = author.to_bytes();
    // Fetch generously so the dedupe step has enough history to pick newest
    // per `d`; the final slice honors `limit`.
    let ndb_cap = limit.saturating_mul(4).max(64) as i32;
    let filter = NdbFilter::new()
        .kinds([KIND_LONG_FORM as u64])
        .authors([&pk_bytes])
        .build();

    let results = ndb
        .query(&txn, &[filter], ndb_cap)
        .map_err(|e| CoreError::Cache(format!("query articles: {e}")))?;

    let mut events: Vec<Event> = Vec::with_capacity(results.len());
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        events.push(event);
    }

    Ok(build_articles(&events, limit as usize))
}

/// Pure: dedupe by `d`, keep newest per `d`, sort desc by `published_at ?? created_at`.
pub fn build_articles(events: &[Event], limit: usize) -> Vec<ArticleRecord> {
    // Keep newest event per `d` identifier. Events missing `d` are skipped —
    // they're not conformant NIP-23 articles.
    let mut latest_by_d: BTreeMap<String, &Event> = BTreeMap::new();
    for event in events {
        let Some(d) = first_tag_value(event, "d") else {
            continue;
        };
        let key = d.trim();
        if key.is_empty() {
            continue;
        }
        match latest_by_d.get(key) {
            Some(prev) if prev.created_at >= event.created_at => {}
            _ => {
                latest_by_d.insert(key.to_string(), event);
            }
        }
    }

    let mut records: Vec<ArticleRecord> =
        latest_by_d.into_values().map(record_from_event).collect();
    records.sort_by(|a, b| {
        b.published_at
            .unwrap_or(b.created_at.unwrap_or(0))
            .cmp(&a.published_at.unwrap_or(a.created_at.unwrap_or(0)))
    });
    records.truncate(limit);
    records
}

fn record_from_event(event: &Event) -> ArticleRecord {
    let identifier = first_tag_value(event, "d").unwrap_or("").trim().to_string();
    let title = first_tag_value(event, "title").unwrap_or("").trim().to_string();
    let summary = first_tag_value(event, "summary").unwrap_or("").trim().to_string();
    let image = first_tag_value(event, "image").unwrap_or("").trim().to_string();
    let published_at = first_tag_value(event, "published_at")
        .and_then(|s| s.trim().parse::<u64>().ok());
    let hashtags: Vec<String> = event
        .tags
        .iter()
        .filter_map(|tag| {
            let s = tag.as_slice();
            if s.first().map(String::as_str) == Some("t") {
                s.get(1).map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
            } else {
                None
            }
        })
        .collect();

    ArticleRecord {
        event_id: event.id.to_hex(),
        pubkey: event.pubkey.to_hex(),
        identifier,
        title,
        summary,
        image,
        content: event.content.clone(),
        hashtags,
        published_at,
        created_at: Some(event.created_at.as_secs()),
    }
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

    fn sign_article(keys: &Keys, d: &str, tags: Vec<Tag>, ts: u64, content: &str) -> Event {
        let mut all = vec![Tag::identifier(d)];
        all.extend(tags);
        EventBuilder::new(Kind::Custom(KIND_LONG_FORM), content)
            .tags(all)
            .custom_created_at(Timestamp::from(ts))
            .sign_with_keys(keys)
            .expect("sign")
    }

    fn named(name: &str, value: &str) -> Tag {
        Tag::parse(vec![name.to_string(), value.to_string()]).expect("named tag")
    }

    #[test]
    fn dedupes_by_d_keeping_newest() {
        let keys = Keys::generate();
        let older = sign_article(&keys, "post-1", vec![named("title", "Old")], 1_000, "old body");
        let newer = sign_article(&keys, "post-1", vec![named("title", "New")], 2_000, "new body");
        let distinct = sign_article(&keys, "post-2", vec![named("title", "Other")], 1_500, "x");
        let out = build_articles(&[older, newer, distinct], 10);
        assert_eq!(out.len(), 2);
        let p1 = out.iter().find(|a| a.identifier == "post-1").unwrap();
        assert_eq!(p1.title, "New");
        assert_eq!(p1.content, "new body");
    }

    #[test]
    fn skips_events_missing_d_tag() {
        let keys = Keys::generate();
        let good = sign_article(&keys, "ok", vec![named("title", "Ok")], 1_000, "body");
        let orphan = EventBuilder::new(Kind::Custom(KIND_LONG_FORM), "orphan")
            .tags(vec![named("title", "Orphan")])
            .custom_created_at(Timestamp::from(1_500))
            .sign_with_keys(&keys)
            .expect("sign");
        let out = build_articles(&[good, orphan], 10);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].identifier, "ok");
    }

    #[test]
    fn sorts_desc_by_published_at_then_created_at() {
        let keys = Keys::generate();
        // a published earlier but created later → should sort after b
        let a = sign_article(
            &keys,
            "a",
            vec![named("title", "A"), named("published_at", "1000")],
            9_000,
            "",
        );
        let b = sign_article(
            &keys,
            "b",
            vec![named("title", "B"), named("published_at", "2000")],
            8_000,
            "",
        );
        // c has no published_at → falls back to created_at
        let c = sign_article(&keys, "c", vec![named("title", "C")], 10_000, "");
        let out = build_articles(&[a, b, c], 10);
        let order: Vec<_> = out.iter().map(|r| r.identifier.as_str()).collect();
        assert_eq!(order, vec!["c", "b", "a"]);
    }

    #[test]
    fn extracts_hashtags_from_t_tags() {
        let keys = Keys::generate();
        let event = sign_article(
            &keys,
            "post",
            vec![
                named("title", "T"),
                named("t", "nostr"),
                named("t", "rust"),
                named("t", ""),
            ],
            1_000,
            "",
        );
        let out = build_articles(&[event], 10);
        assert_eq!(out[0].hashtags, vec!["nostr", "rust"]);
    }

    #[test]
    fn query_article_returns_newest_event_for_d_tag() {
        use nostrdb::{Config as NdbConfig, Ndb};
        use tempfile::tempdir;

        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().to_str().unwrap();
        let ndb = Ndb::new(db_path, &NdbConfig::new().set_mapsize(64 * 1024 * 1024))
            .expect("open ndb");

        let keys = Keys::generate();
        let older = sign_article(&keys, "post-x", vec![named("title", "Older")], 1_000, "v1");
        let newer = sign_article(&keys, "post-x", vec![named("title", "Newer")], 2_000, "v2");
        let other = sign_article(&keys, "post-y", vec![named("title", "Other")], 1_500, "w");

        for e in [&older, &newer, &other] {
            let relay_line = format!("[\"EVENT\",\"s\",{}]", e.as_json());
            ndb.process_event(&relay_line).expect("process event");
        }

        // Give ndb a moment to index — it processes writes async.
        std::thread::sleep(std::time::Duration::from_millis(100));

        let got = query_article(&ndb, &keys.public_key().to_hex(), "post-x")
            .expect("query_article")
            .expect("found article");
        assert_eq!(got.title, "Newer");
        assert_eq!(got.content, "v2");

        let missing = query_article(&ndb, &keys.public_key().to_hex(), "does-not-exist")
            .expect("query_article");
        assert!(missing.is_none());
    }

    #[test]
    fn limit_is_applied_after_dedup_and_sort() {
        let keys = Keys::generate();
        let mut events = Vec::new();
        for i in 0..5 {
            events.push(sign_article(
                &keys,
                &format!("p{i}"),
                vec![named("title", &format!("T{i}"))],
                1_000 + i,
                "",
            ));
        }
        let out = build_articles(&events, 2);
        assert_eq!(out.len(), 2);
        // newest two identifiers
        assert_eq!(out[0].identifier, "p4");
        assert_eq!(out[1].identifier, "p3");
    }
}
