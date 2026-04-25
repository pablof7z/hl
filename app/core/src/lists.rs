//! NIP-51 Bookmark sets (kind:30003), Curation sets (kind:30004), and
//! NIP-B0 Web bookmarks (kind:39701).
//!
//! These are all parameterized replaceable events (NIP-33), so one event
//! exists per (author, d-tag) pair. We read-only; writing is not supported yet.

use std::collections::HashMap;

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::artifacts::first_tag_value;
use crate::errors::CoreError;
use crate::models::{BookmarkSetRecord, WebBookmarkRecord};

pub const KIND_BOOKMARK_SETS: u16 = 30003;
pub const KIND_CURATION_SETS: u16 = 30004;
pub const KIND_WEB_BOOKMARK: u16 = 39701;

// -- Public query API --------------------------------------------------------

/// Return all kind:30003 or kind:30004 sets authored by `user_hex`, newest
/// first. Deduplicates in Rust so callers always get one record per d-tag.
pub fn query_user_sets(
    ndb: &Ndb,
    user_hex: &str,
    kind: u16,
) -> Result<Vec<BookmarkSetRecord>, CoreError> {
    if user_hex.is_empty() {
        return Ok(Vec::new());
    }
    let author = PublicKey::from_hex(user_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;
    let pk_bytes: [u8; 32] = author.to_bytes();

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let filter = NdbFilter::new()
        .kinds([kind as u64])
        .authors([&pk_bytes])
        .build();
    let results = ndb
        .query(&txn, &[filter], 256)
        .map_err(|e| CoreError::Cache(format!("query user sets: {e}")))?;

    let mut by_d: HashMap<String, Event> = HashMap::new();
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else { continue };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else { continue };
        let d = first_tag_value(&event, "d").unwrap_or("").to_string();
        let entry = by_d.entry(d).or_insert_with(|| event.clone());
        if event.created_at > entry.created_at {
            *entry = event;
        }
    }

    let mut records: Vec<BookmarkSetRecord> = by_d
        .into_values()
        .map(|ev| parse_set_event(ev, kind))
        .collect();
    records.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(records)
}

/// Return kind:30004 curation sets authored by any of `follow_hexes`, newest
/// first per (author, d-tag). Used for the Explore mode.
pub fn query_following_curation_sets(
    ndb: &Ndb,
    follow_hexes: &[String],
) -> Result<Vec<BookmarkSetRecord>, CoreError> {
    if follow_hexes.is_empty() {
        return Ok(Vec::new());
    }
    let pks: Vec<PublicKey> = follow_hexes
        .iter()
        .filter_map(|h| PublicKey::from_hex(h).ok())
        .collect();
    if pks.is_empty() {
        return Ok(Vec::new());
    }
    let pk_bytes: Vec<[u8; 32]> = pks.iter().map(|pk| pk.to_bytes()).collect();
    let pk_refs: Vec<&[u8; 32]> = pk_bytes.iter().collect();

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let filter = NdbFilter::new()
        .kinds([KIND_CURATION_SETS as u64])
        .authors(pk_refs.iter().copied())
        .build();
    let results = ndb
        .query(&txn, &[filter], 512)
        .map_err(|e| CoreError::Cache(format!("query following curation sets: {e}")))?;

    let mut by_key: HashMap<(String, String), Event> = HashMap::new();
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else { continue };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else { continue };
        let d = first_tag_value(&event, "d").unwrap_or("").to_string();
        let pk = event.pubkey.to_hex();
        let key = (pk, d);
        let entry = by_key.entry(key).or_insert_with(|| event.clone());
        if event.created_at > entry.created_at {
            *entry = event;
        }
    }

    let mut records: Vec<BookmarkSetRecord> = by_key
        .into_values()
        .map(|ev| parse_set_event(ev, KIND_CURATION_SETS))
        .collect();
    records.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(records)
}

/// Return all NIP-B0 kind:39701 web bookmarks authored by `user_hex`,
/// newest first. The `url` field is prefixed with `https://`.
pub fn query_user_web_bookmarks(
    ndb: &Ndb,
    user_hex: &str,
) -> Result<Vec<WebBookmarkRecord>, CoreError> {
    if user_hex.is_empty() {
        return Ok(Vec::new());
    }
    let author = PublicKey::from_hex(user_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;
    let pk_bytes: [u8; 32] = author.to_bytes();

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let filter = NdbFilter::new()
        .kinds([KIND_WEB_BOOKMARK as u64])
        .authors([&pk_bytes])
        .build();
    let results = ndb
        .query(&txn, &[filter], 256)
        .map_err(|e| CoreError::Cache(format!("query web bookmarks: {e}")))?;

    let mut by_d: HashMap<String, Event> = HashMap::new();
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else { continue };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else { continue };
        let d = first_tag_value(&event, "d").unwrap_or("").to_string();
        let entry = by_d.entry(d).or_insert_with(|| event.clone());
        if event.created_at > entry.created_at {
            *entry = event;
        }
    }

    let mut records: Vec<WebBookmarkRecord> = by_d
        .into_values()
        .map(parse_web_bookmark_event)
        .collect();
    records.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(records)
}

// -- Parsing -----------------------------------------------------------------

fn parse_set_event(event: Event, kind: u16) -> BookmarkSetRecord {
    let mut article_addresses = Vec::new();
    let mut note_ids = Vec::new();

    for tag in event.tags.iter() {
        let s = tag.as_slice();
        match s.first().map(String::as_str) {
            Some("a") => {
                if let Some(v) = s.get(1) {
                    article_addresses.push(v.clone());
                }
            }
            Some("e") => {
                if let Some(v) = s.get(1) {
                    note_ids.push(v.clone());
                }
            }
            _ => {}
        }
    }

    BookmarkSetRecord {
        id: first_tag_value(&event, "d").unwrap_or("").to_string(),
        pubkey: event.pubkey.to_hex(),
        kind: kind as u32,
        title: first_tag_value(&event, "title").unwrap_or("").to_string(),
        description: first_tag_value(&event, "description").unwrap_or("").to_string(),
        image: first_tag_value(&event, "image").unwrap_or("").to_string(),
        article_addresses,
        note_ids,
        created_at: Some(event.created_at.as_secs()),
    }
}

fn parse_web_bookmark_event(event: Event) -> WebBookmarkRecord {
    let d = first_tag_value(&event, "d").unwrap_or("").to_string();
    let url = if d.is_empty() {
        String::new()
    } else {
        format!("https://{d}")
    };

    let topics: Vec<String> = event
        .tags
        .iter()
        .filter_map(|tag| {
            let s = tag.as_slice();
            if s.first().map(String::as_str) == Some("t") {
                s.get(1).cloned()
            } else {
                None
            }
        })
        .collect();

    let published_at = first_tag_value(&event, "published_at")
        .and_then(|v| v.parse::<u64>().ok());

    WebBookmarkRecord {
        url,
        pubkey: event.pubkey.to_hex(),
        title: first_tag_value(&event, "title").unwrap_or("").to_string(),
        description: event.content.clone(),
        topics,
        published_at,
        created_at: Some(event.created_at.as_secs()),
    }
}
