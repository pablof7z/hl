//! Search across locally cached nostrdb content and (via NIP-50) across
//! relay-hosted long-form articles.
//!
//! Local scans are synchronous ndb reads — cheap, case-insensitive substring
//! matches over the fields a user would reasonably search for:
//!
//! - kind:9802 highlights — quote + note
//! - kind:30023 articles — title + summary + hashtags
//! - kind:39000 communities — name + about
//! - kind:0 profiles — name + display_name + nip05
//!
//! Relay-side search is a `SubscriptionKind::SearchArticles` in
//! `subscriptions.rs`; this module only provides the local reads and the
//! helper that resolves the user's kind:10007 NIP-51 search relay list
//! (merged with `wss://relay.highlighter.com` as a default).

use std::collections::{BTreeMap, HashSet};

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::articles::KIND_LONG_FORM;
use crate::errors::CoreError;
use crate::groups::KIND_GROUP_METADATA;
use crate::models::{ArticleRecord, CommunitySummary, HighlightRecord, ProfileMetadata};
use crate::profile;
use crate::relays::HIGHLIGHTER_RELAY;

/// NIP-51 kind for the user's curated list of search relays.
pub const KIND_SEARCH_RELAYS: u16 = 10007;
/// kind:9802 NIP-84 highlight.
const KIND_HIGHLIGHT: u16 = 9802;
/// kind:0 NIP-01 profile metadata.
const KIND_METADATA: u16 = 0;

/// How many candidate notes to pull from ndb before filtering. Higher than the
/// final `limit` so substring matches still surface when the candidate set is
/// dominated by non-matching notes.
const LOCAL_SCAN_MULTIPLIER: i32 = 8;
const LOCAL_SCAN_FLOOR: i32 = 256;
const LOCAL_SCAN_CEILING: i32 = 4096;

fn scan_cap(limit: u32) -> i32 {
    let raw = (limit as i32).saturating_mul(LOCAL_SCAN_MULTIPLIER);
    raw.clamp(LOCAL_SCAN_FLOOR, LOCAL_SCAN_CEILING)
}

/// Case-insensitive `needle in haystack`, ignoring leading/trailing whitespace
/// on the query.
fn contains_ci(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

// -- Highlights --------------------------------------------------------------

pub fn search_highlights(
    ndb: &Ndb,
    query: &str,
    limit: u32,
) -> Result<Vec<HighlightRecord>, CoreError> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let filter = NdbFilter::new()
        .kinds([KIND_HIGHLIGHT as u64])
        .build();
    let results = ndb
        .query(&txn, &[filter], scan_cap(limit))
        .map_err(|e| CoreError::Cache(format!("query highlights: {e}")))?;

    let mut records: Vec<HighlightRecord> = Vec::new();
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else { continue };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else { continue };
        let note_text = first_tag_value(&event, "comment").unwrap_or("");
        if !(contains_ci(&event.content, q) || contains_ci(note_text, q)) {
            continue;
        }
        if let Some(rec) = highlight_record_from_event(&event) {
            records.push(rec);
        }
    }

    records.sort_by(|a, b| b.created_at.unwrap_or(0).cmp(&a.created_at.unwrap_or(0)));
    records.truncate(limit as usize);
    Ok(records)
}

// -- Articles ----------------------------------------------------------------

pub fn search_articles(
    ndb: &Ndb,
    query: &str,
    limit: u32,
) -> Result<Vec<ArticleRecord>, CoreError> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let filter = NdbFilter::new()
        .kinds([KIND_LONG_FORM as u64])
        .build();
    let results = ndb
        .query(&txn, &[filter], scan_cap(limit))
        .map_err(|e| CoreError::Cache(format!("query articles: {e}")))?;

    // Collect into addressable-event dedupe map (newest per (pubkey, d) wins).
    let mut best_per_addr: BTreeMap<(String, String), Event> = BTreeMap::new();
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else { continue };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else { continue };
        let title = first_tag_value(&event, "title").unwrap_or("");
        let summary = first_tag_value(&event, "summary").unwrap_or("");
        let d_tag = first_tag_value(&event, "d").unwrap_or("");
        let hashtags_match = event.tags.iter().any(|t| {
            let s = t.as_slice();
            s.first().map(String::as_str) == Some("t")
                && s.get(1).map(|v| contains_ci(v, q)).unwrap_or(false)
        });
        if !(contains_ci(title, q) || contains_ci(summary, q) || hashtags_match) {
            continue;
        }
        let key = (event.pubkey.to_hex(), d_tag.to_string());
        match best_per_addr.get(&key) {
            Some(prev) if prev.created_at >= event.created_at => {}
            _ => {
                best_per_addr.insert(key, event);
            }
        }
    }

    let mut records: Vec<ArticleRecord> = best_per_addr
        .into_values()
        .filter_map(|ev| article_record_from_event(&ev))
        .collect();

    records.sort_by(|a, b| {
        b.published_at
            .or(b.created_at)
            .unwrap_or(0)
            .cmp(&a.published_at.or(a.created_at).unwrap_or(0))
    });
    records.truncate(limit as usize);
    Ok(records)
}

// -- Communities -------------------------------------------------------------

pub fn search_communities(
    ndb: &Ndb,
    query: &str,
    limit: u32,
) -> Result<Vec<CommunitySummary>, CoreError> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let filter = NdbFilter::new()
        .kinds([KIND_GROUP_METADATA as u64])
        .build();
    let results = ndb
        .query(&txn, &[filter], scan_cap(limit))
        .map_err(|e| CoreError::Cache(format!("query communities: {e}")))?;

    // Dedupe per `d` tag — kind:39000 is replaceable; newest wins.
    let mut best_per_d: BTreeMap<String, Event> = BTreeMap::new();
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else { continue };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else { continue };
        let name = first_tag_value(&event, "name").unwrap_or("");
        let about = first_tag_value(&event, "about").unwrap_or("");
        let d_tag = first_tag_value(&event, "d").unwrap_or("");
        if !(contains_ci(name, q) || contains_ci(about, q)) {
            continue;
        }
        match best_per_d.get(d_tag) {
            Some(prev) if prev.created_at >= event.created_at => {}
            _ => {
                best_per_d.insert(d_tag.to_string(), event);
            }
        }
    }

    let mut records: Vec<CommunitySummary> = best_per_d
        .into_values()
        .filter_map(|ev| crate::groups::build_community_summary(&ev).ok())
        .collect();
    records.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    records.truncate(limit as usize);
    Ok(records)
}

// -- Profiles ----------------------------------------------------------------

pub fn search_profiles(
    ndb: &Ndb,
    query: &str,
    limit: u32,
) -> Result<Vec<ProfileMetadata>, CoreError> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let filter = NdbFilter::new()
        .kinds([KIND_METADATA as u64])
        .build();
    let results = ndb
        .query(&txn, &[filter], scan_cap(limit))
        .map_err(|e| CoreError::Cache(format!("query profiles: {e}")))?;

    // Dedupe per author — kind:0 is replaceable; newest wins.
    let mut best_per_author: BTreeMap<String, Event> = BTreeMap::new();
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else { continue };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else { continue };
        let author = event.pubkey.to_hex();
        match best_per_author.get(&author) {
            Some(prev) if prev.created_at >= event.created_at => {}
            _ => {
                best_per_author.insert(author, event);
            }
        }
    }

    let mut records: Vec<ProfileMetadata> = Vec::new();
    for event in best_per_author.into_values() {
        let meta = profile::parse_metadata(&event);
        let matches = contains_ci(&meta.name, q)
            || contains_ci(&meta.display_name, q)
            || contains_ci(&meta.nip05, q)
            || contains_ci(&meta.about, q);
        if matches {
            records.push(meta);
        }
    }

    // Rank by "does the name start with the query" first, then alphabetical.
    let q_lower = q.to_lowercase();
    records.sort_by(|a, b| {
        let a_prefix = a.display_name.to_lowercase().starts_with(&q_lower)
            || a.name.to_lowercase().starts_with(&q_lower);
        let b_prefix = b.display_name.to_lowercase().starts_with(&q_lower)
            || b.name.to_lowercase().starts_with(&q_lower);
        match (a_prefix, b_prefix) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                let a_label = primary_label(a).to_lowercase();
                let b_label = primary_label(b).to_lowercase();
                a_label.cmp(&b_label)
            }
        }
    });
    records.truncate(limit as usize);
    Ok(records)
}

fn primary_label(p: &ProfileMetadata) -> &str {
    if !p.display_name.is_empty() {
        &p.display_name
    } else if !p.name.is_empty() {
        &p.name
    } else {
        &p.nip05
    }
}

// -- NIP-51 kind:10007 search relays ----------------------------------------

/// Resolve the set of relays to hit with NIP-50 queries. Always includes
/// `wss://relay.highlighter.com` (the app's default search host); additionally
/// includes every `relay` tag from the newest cached kind:10007 for `user_hex`.
/// Output is deduped, order-preserving (default first, then user list in tag
/// order).
pub fn query_search_relays(ndb: &Ndb, user_hex: &str) -> Result<Vec<String>, CoreError> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    let push = |url: String, out: &mut Vec<String>, seen: &mut HashSet<String>| {
        let trimmed = url.trim().trim_end_matches('/').to_string();
        if trimmed.is_empty() {
            return;
        }
        if seen.insert(trimmed.clone()) {
            out.push(trimmed);
        }
    };

    push(HIGHLIGHTER_RELAY.to_string(), &mut out, &mut seen);

    if user_hex.is_empty() {
        return Ok(out);
    }

    let Ok(author) = PublicKey::from_hex(user_hex) else {
        return Ok(out);
    };
    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let pk_bytes: [u8; 32] = author.to_bytes();
    let filter = NdbFilter::new()
        .kinds([KIND_SEARCH_RELAYS as u64])
        .authors([&pk_bytes])
        .build();
    let results = ndb
        .query(&txn, &[filter], 8)
        .map_err(|e| CoreError::Cache(format!("query search relays: {e}")))?;

    let mut newest: Option<Event> = None;
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else { continue };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else { continue };
        newest = Some(match newest {
            Some(prev) if prev.created_at >= event.created_at => prev,
            _ => event,
        });
    }

    if let Some(event) = newest {
        for tag in event.tags.iter() {
            let s = tag.as_slice();
            if s.first().map(String::as_str) != Some("relay") {
                continue;
            }
            if let Some(url) = s.get(1) {
                push(url.to_string(), &mut out, &mut seen);
            }
        }
    }

    Ok(out)
}

// -- Event → record helpers --------------------------------------------------

fn first_tag_value<'a>(event: &'a Event, name: &str) -> Option<&'a str> {
    for tag in event.tags.iter() {
        let slice = tag.as_slice();
        if slice.first().map(String::as_str) == Some(name) {
            return slice.get(1).map(String::as_str);
        }
    }
    None
}

fn highlight_record_from_event(event: &Event) -> Option<HighlightRecord> {
    let quote = event.content.clone();
    let context = first_tag_value(event, "context").unwrap_or("").to_string();
    let comment = first_tag_value(event, "comment").unwrap_or("").to_string();
    let artifact_address = first_tag_value(event, "a").unwrap_or("").to_string();
    let event_reference = first_tag_value(event, "e").unwrap_or("").to_string();
    let source_url = first_tag_value(event, "r").unwrap_or("").to_string();

    let source_reference_key = if !artifact_address.is_empty() {
        format!("a:{artifact_address}")
    } else if !event_reference.is_empty() {
        format!("e:{event_reference}")
    } else if !source_url.is_empty() {
        format!("r:{source_url}")
    } else {
        String::new()
    };

    Some(HighlightRecord {
        event_id: event.id.to_hex(),
        pubkey: event.pubkey.to_hex(),
        quote,
        context,
        note: comment,
        artifact_address,
        event_reference,
        external_reference: String::new(),
        source_url,
        source_reference_key,
        clip_start_seconds: first_tag_value(event, "start").and_then(|s| s.parse().ok()),
        clip_end_seconds: first_tag_value(event, "end").and_then(|s| s.parse().ok()),
        clip_speaker: first_tag_value(event, "speaker").unwrap_or("").to_string(),
        clip_transcript_segment_ids: event
            .tags
            .iter()
            .filter_map(|t| {
                let s = t.as_slice();
                if s.first().map(String::as_str) == Some("segment") {
                    s.get(1).map(|v| v.to_string())
                } else {
                    None
                }
            })
            .collect(),
        created_at: Some(event.created_at.as_secs()),
    })
}

fn article_record_from_event(event: &Event) -> Option<ArticleRecord> {
    let identifier = first_tag_value(event, "d").unwrap_or("").to_string();
    if identifier.is_empty() {
        return None;
    }
    let title = first_tag_value(event, "title").unwrap_or("").to_string();
    let summary = first_tag_value(event, "summary").unwrap_or("").to_string();
    let image = first_tag_value(event, "image").unwrap_or("").to_string();
    let published_at = first_tag_value(event, "published_at").and_then(|v| v.parse::<u64>().ok());
    let hashtags: Vec<String> = event
        .tags
        .iter()
        .filter_map(|t| {
            let s = t.as_slice();
            if s.first().map(String::as_str) == Some("t") {
                s.get(1).map(|v| v.to_string())
            } else {
                None
            }
        })
        .collect();

    Some(ArticleRecord {
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fresh_ndb() -> (Ndb, TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = nostrdb::Config::new();
        let ndb = Ndb::new(tmp.path().to_str().unwrap(), &cfg).unwrap();
        (ndb, tmp)
    }

    fn process(ndb: &Ndb, event: &Event) {
        let line = format!("[\"EVENT\",\"sub\",{}]", event.as_json());
        ndb.process_event(&line).unwrap();
    }

    #[test]
    fn search_highlights_matches_quote_and_note_case_insensitive() {
        let (ndb, _tmp) = fresh_ndb();
        let keys = Keys::generate();

        let match_quote = EventBuilder::new(Kind::Custom(KIND_HIGHLIGHT), "The Brothers Karamazov")
            .sign_with_keys(&keys)
            .unwrap();
        let match_note = EventBuilder::new(Kind::Custom(KIND_HIGHLIGHT), "an unrelated quote")
            .tags([Tag::parse(vec!["comment".to_string(), "dostoevsky fan club".to_string()]).unwrap()])
            .sign_with_keys(&keys)
            .unwrap();
        let no_match = EventBuilder::new(Kind::Custom(KIND_HIGHLIGHT), "Proust is the best")
            .sign_with_keys(&keys)
            .unwrap();

        process(&ndb, &match_quote);
        process(&ndb, &match_note);
        process(&ndb, &no_match);
        std::thread::sleep(std::time::Duration::from_millis(50));

        let hits = search_highlights(&ndb, "DOSTOEVSKY", 20).unwrap();
        assert!(hits.iter().any(|h| h.note.contains("dostoevsky")));
        let kara = search_highlights(&ndb, "karamazov", 20).unwrap();
        assert!(kara.iter().any(|h| h.quote.contains("Karamazov")));
    }

    #[test]
    fn search_articles_matches_title_and_hashtag_and_dedupes_by_address() {
        let (ndb, _tmp) = fresh_ndb();
        let keys = Keys::generate();

        let older = EventBuilder::new(Kind::Custom(KIND_LONG_FORM), "old body")
            .tags([
                Tag::parse(vec!["d".to_string(), "essay".to_string()]).unwrap(),
                Tag::parse(vec!["title".to_string(), "On Attention".to_string()]).unwrap(),
            ])
            .custom_created_at(Timestamp::from(1_000u64))
            .sign_with_keys(&keys)
            .unwrap();
        let newer = EventBuilder::new(Kind::Custom(KIND_LONG_FORM), "new body")
            .tags([
                Tag::parse(vec!["d".to_string(), "essay".to_string()]).unwrap(),
                Tag::parse(vec!["title".to_string(), "On Attention (revised)".to_string()]).unwrap(),
            ])
            .custom_created_at(Timestamp::from(2_000u64))
            .sign_with_keys(&keys)
            .unwrap();
        let hashtag_match = EventBuilder::new(Kind::Custom(KIND_LONG_FORM), "body")
            .tags([
                Tag::parse(vec!["d".to_string(), "other".to_string()]).unwrap(),
                Tag::parse(vec!["title".to_string(), "Entirely Unrelated".to_string()]).unwrap(),
                Tag::parse(vec!["t".to_string(), "attention".to_string()]).unwrap(),
            ])
            .sign_with_keys(&keys)
            .unwrap();

        process(&ndb, &older);
        process(&ndb, &newer);
        process(&ndb, &hashtag_match);
        std::thread::sleep(std::time::Duration::from_millis(50));

        let hits = search_articles(&ndb, "attention", 20).unwrap();
        assert_eq!(hits.len(), 2, "dedupe by (pubkey, d): 2 distinct addresses");
        let essay = hits.iter().find(|a| a.identifier == "essay").unwrap();
        assert_eq!(essay.title, "On Attention (revised)", "newest wins");
    }

    #[test]
    fn search_profiles_ranks_prefix_matches_first() {
        let (ndb, _tmp) = fresh_ndb();
        let a = Keys::generate();
        let b = Keys::generate();

        let contains_only = EventBuilder::new(
            Kind::Custom(KIND_METADATA),
            r#"{"name":"Prof. Aldous Huxley","display_name":"Aldous Huxley"}"#,
        )
        .sign_with_keys(&a)
        .unwrap();
        let prefix_match = EventBuilder::new(
            Kind::Custom(KIND_METADATA),
            r#"{"name":"huxley-fan","display_name":"Huxley's Fan"}"#,
        )
        .sign_with_keys(&b)
        .unwrap();

        process(&ndb, &contains_only);
        process(&ndb, &prefix_match);
        std::thread::sleep(std::time::Duration::from_millis(50));

        let hits = search_profiles(&ndb, "huxley", 20).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].display_name, "Huxley's Fan", "prefix match ranks first");
    }

    #[test]
    fn query_search_relays_always_includes_default_and_dedupes() {
        let (ndb, _tmp) = fresh_ndb();
        let user = Keys::generate();

        let event = EventBuilder::new(Kind::Custom(KIND_SEARCH_RELAYS), "")
            .tags([
                Tag::parse(vec!["relay".to_string(), "wss://relay.nostr.band".to_string()]).unwrap(),
                Tag::parse(vec!["relay".to_string(), HIGHLIGHTER_RELAY.to_string()]).unwrap(),
            ])
            .sign_with_keys(&user)
            .unwrap();
        process(&ndb, &event);
        std::thread::sleep(std::time::Duration::from_millis(50));

        let relays = query_search_relays(&ndb, &user.public_key().to_hex()).unwrap();
        assert_eq!(relays.first().map(String::as_str), Some(HIGHLIGHTER_RELAY));
        assert!(relays.iter().any(|r| r == "wss://relay.nostr.band"));
        // No duplicates for the default relay even though the user also listed it.
        let hl_count = relays.iter().filter(|r| *r == HIGHLIGHTER_RELAY).count();
        assert_eq!(hl_count, 1);
    }
}
