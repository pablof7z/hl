//! "Recent books across all my communities" — iOS-only capture-flow feature,
//! not in the webapp. Indexed from nostrdb for instant display in the book
//! picker.

use std::collections::{HashMap, HashSet};

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::artifacts::{artifact_record_from_event, first_tag_value};
use crate::errors::CoreError;
use crate::groups::{KIND_GROUP_ADMINS, KIND_GROUP_MEMBERS};
use crate::models::ArtifactRecord;

const KIND_ARTIFACT_SHARE: u16 = 11;

/// 1. Joined group IDs are derived from cached kind:39001/39002 events.
/// 2. Query nostrdb for kind:11 artifact shares within those groups.
/// 3. Keep artifacts where `source == "book"` OR the reference tag is `i=isbn:…`.
/// 4. Dedupe by `(reference_tag_name, reference_tag_value)`, keeping the
///    most-recent occurrence.
/// 5. Sort by `created_at` desc, cap at `limit`.
pub fn query_recent_books(
    ndb: &Ndb,
    user_pubkey_hex: &str,
    limit: u32,
) -> Result<Vec<ArtifactRecord>, CoreError> {
    let user = user_pubkey_hex.trim();
    if user.is_empty() {
        return Ok(Vec::new());
    }
    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let joined: HashSet<String> = joined_group_ids(ndb, &txn, user)?;
    if joined.is_empty() {
        return Ok(Vec::new());
    }

    let cap = (limit.saturating_mul(8)).max(256) as i32;
    let filter = NdbFilter::new()
        .kinds([KIND_ARTIFACT_SHARE as u64])
        .build();
    let results = ndb
        .query(&txn, &[filter], cap)
        .map_err(|e| CoreError::Cache(format!("query artifacts: {e}")))?;

    // Dedupe by reference key — same book referenced from multiple groups
    // collapses to the most-recent share.
    let mut by_ref: HashMap<String, ArtifactRecord> = HashMap::new();
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else { continue };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else { continue };

        let Some(group_id) = first_tag_value(&event, "h") else { continue };
        if !joined.contains(group_id) {
            continue;
        }
        if crate::discussions::is_discussion(&event) {
            continue;
        }
        if !is_book(&event) {
            continue;
        }

        let Some(rec) = artifact_record_from_event(&event, group_id) else { continue };
        let key = reference_key(&rec);
        if key.is_empty() {
            continue;
        }
        match by_ref.get(&key) {
            Some(existing)
                if existing.created_at.unwrap_or(0) >= rec.created_at.unwrap_or(0) =>
            {
                // keep existing newer
            }
            _ => {
                by_ref.insert(key, rec);
            }
        }
    }

    let mut out: Vec<ArtifactRecord> = by_ref.into_values().collect();
    out.sort_by(|a, b| b.created_at.unwrap_or(0).cmp(&a.created_at.unwrap_or(0)));
    out.truncate(limit as usize);
    Ok(out)
}

/// Collect group ids the user appears in (admin or member). Pure scan over
/// kind:39001/39002 events with manual `p` tag check — same approach
/// `groups::query_joined_communities_from_ndb` uses, kept local so we don't
/// have to refactor it for sharing.
fn joined_group_ids(
    ndb: &Ndb,
    txn: &Transaction,
    user_pubkey_hex: &str,
) -> Result<HashSet<String>, CoreError> {
    let filter = NdbFilter::new()
        .kinds([KIND_GROUP_ADMINS as u64, KIND_GROUP_MEMBERS as u64])
        .build();
    let results = ndb
        .query(txn, &[filter], 4096)
        .map_err(|e| CoreError::Cache(format!("query membership: {e}")))?;

    let mut ids: HashSet<String> = HashSet::new();
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(txn, r.note_key) else { continue };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else { continue };

        let has_user = event.tags.iter().any(|tag| {
            let s = tag.as_slice();
            s.first().map(String::as_str) == Some("p")
                && s.get(1).map(String::as_str) == Some(user_pubkey_hex)
        });
        if !has_user {
            continue;
        }
        if let Some(d) = first_tag_value(&event, "d") {
            ids.insert(d.to_string());
        }
    }
    Ok(ids)
}

/// True if the kind:11 event represents a book — either `source=="book"` or
/// the `i` reference tag value starts with `isbn:`.
fn is_book(event: &Event) -> bool {
    if let Some(source) = first_tag_value(event, "source") {
        if source.eq_ignore_ascii_case("book") {
            return true;
        }
    }
    if let Some(i) = first_tag_value(event, "i") {
        if i.to_ascii_lowercase().starts_with("isbn:") {
            return true;
        }
    }
    false
}

/// Stable key for deduping shares of the same book across communities.
/// Mirrors `ArtifactPreview.highlight_reference_key` semantics.
fn reference_key(rec: &ArtifactRecord) -> String {
    let name = rec.preview.reference_tag_name.trim();
    let value = rec.preview.reference_tag_value.trim();
    if name.is_empty() || value.is_empty() {
        return String::new();
    }
    format!("{name}:{value}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostrdb::{Config as NdbConfig, Ndb};
    use tempfile::tempdir;

    fn isolated_ndb() -> (Ndb, tempfile::TempDir) {
        let tmp = tempdir().expect("tempdir");
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

    fn wait_for_ndb() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    fn membership(keys: &Keys, group_id: &str, user_hex: &str, ts: u64) -> Event {
        EventBuilder::new(Kind::Custom(KIND_GROUP_MEMBERS), "")
            .tags(vec![
                Tag::parse(vec!["d".to_string(), group_id.to_string()]).unwrap(),
                Tag::parse(vec!["p".to_string(), user_hex.to_string()]).unwrap(),
            ])
            .custom_created_at(Timestamp::from(ts))
            .sign_with_keys(keys)
            .expect("sign membership")
    }

    fn book_share(
        keys: &Keys,
        group_id: &str,
        d: &str,
        ref_value: &str,
        title: &str,
        ts: u64,
    ) -> Event {
        EventBuilder::new(Kind::Custom(KIND_ARTIFACT_SHARE), "")
            .tags(vec![
                Tag::parse(vec!["h".to_string(), group_id.to_string()]).unwrap(),
                Tag::identifier(d),
                Tag::parse(vec!["title".to_string(), title.to_string()]).unwrap(),
                Tag::parse(vec!["source".to_string(), "book".to_string()]).unwrap(),
                Tag::parse(vec!["i".to_string(), ref_value.to_string()]).unwrap(),
            ])
            .custom_created_at(Timestamp::from(ts))
            .sign_with_keys(keys)
            .expect("sign share")
    }

    fn article_share(keys: &Keys, group_id: &str, d: &str, ts: u64) -> Event {
        EventBuilder::new(Kind::Custom(KIND_ARTIFACT_SHARE), "")
            .tags(vec![
                Tag::parse(vec!["h".to_string(), group_id.to_string()]).unwrap(),
                Tag::identifier(d),
                Tag::parse(vec!["title".to_string(), "Some Article".to_string()]).unwrap(),
                Tag::parse(vec!["source".to_string(), "article".to_string()]).unwrap(),
                Tag::parse(vec!["r".to_string(), "https://example.com".to_string()]).unwrap(),
            ])
            .custom_created_at(Timestamp::from(ts))
            .sign_with_keys(keys)
            .expect("sign article")
    }

    #[test]
    fn returns_empty_when_no_groups_joined() {
        let (ndb, _tmp) = isolated_ndb();
        let user = "a".repeat(64);
        let out = query_recent_books(&ndb, &user, 10).expect("query");
        assert!(out.is_empty());
    }

    #[test]
    fn filters_to_books_only() {
        let (ndb, _tmp) = isolated_ndb();
        let user_keys = Keys::generate();
        let user = user_keys.public_key().to_hex();
        let admin = Keys::generate();

        ingest(&ndb, &membership(&admin, "alpha", &user, 1));
        ingest(&ndb, &book_share(&admin, "alpha", "b1", "isbn:111", "Book A", 100));
        ingest(&ndb, &article_share(&admin, "alpha", "a1", 200));
        wait_for_ndb();

        let out = query_recent_books(&ndb, &user, 10).expect("query");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].preview.title, "Book A");
    }

    #[test]
    fn dedupes_by_reference_keeping_newest() {
        let (ndb, _tmp) = isolated_ndb();
        let user_keys = Keys::generate();
        let user = user_keys.public_key().to_hex();
        let admin = Keys::generate();

        ingest(&ndb, &membership(&admin, "alpha", &user, 1));
        ingest(&ndb, &membership(&admin, "bravo", &user, 1));
        // Same book (isbn:111) shared in two groups, bravo is newer.
        ingest(&ndb, &book_share(&admin, "alpha", "b1", "isbn:111", "Old Title", 100));
        ingest(&ndb, &book_share(&admin, "bravo", "b2", "isbn:111", "New Title", 200));
        wait_for_ndb();

        let out = query_recent_books(&ndb, &user, 10).expect("query");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].preview.title, "New Title");
        assert_eq!(out[0].group_id, "bravo");
    }

    #[test]
    fn skips_groups_user_is_not_in() {
        let (ndb, _tmp) = isolated_ndb();
        let user_keys = Keys::generate();
        let user = user_keys.public_key().to_hex();
        let admin = Keys::generate();

        ingest(&ndb, &membership(&admin, "alpha", &user, 1));
        ingest(&ndb, &book_share(&admin, "alpha", "b1", "isbn:111", "Mine", 100));
        ingest(&ndb, &book_share(&admin, "other", "b2", "isbn:222", "Theirs", 200));
        wait_for_ndb();

        let out = query_recent_books(&ndb, &user, 10).expect("query");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].preview.title, "Mine");
    }

    #[test]
    fn sorts_newest_first_and_caps_at_limit() {
        let (ndb, _tmp) = isolated_ndb();
        let user_keys = Keys::generate();
        let user = user_keys.public_key().to_hex();
        let admin = Keys::generate();

        ingest(&ndb, &membership(&admin, "alpha", &user, 1));
        for i in 0..5u64 {
            ingest(
                &ndb,
                &book_share(
                    &admin,
                    "alpha",
                    &format!("b{i}"),
                    &format!("isbn:{i:03}"),
                    &format!("Book {i}"),
                    100 + i,
                ),
            );
        }
        wait_for_ndb();

        let out = query_recent_books(&ndb, &user, 3).expect("query");
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].preview.title, "Book 4");
        assert_eq!(out[1].preview.title, "Book 3");
        assert_eq!(out[2].preview.title, "Book 2");
    }
}
