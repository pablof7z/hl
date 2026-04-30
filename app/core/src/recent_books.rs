//! "Recent books across all my communities" — iOS-only capture-flow feature,
//! not in the webapp. Indexed from nostrdb for instant display in the book
//! picker.

use std::collections::{HashMap, HashSet};

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::artifacts::{artifact_record_from_event, first_tag_value};
use crate::errors::CoreError;
use crate::groups::{KIND_GROUP_ADMINS, KIND_GROUP_MEMBERS};
use crate::models::{ArtifactPreview, ArtifactRecord};

const KIND_ARTIFACT_SHARE: u16 = 11;
const KIND_HIGHLIGHT: u16 = 9802;

/// Source signals (combined, deduped by `(reference_tag_name, reference_tag_value)`):
/// 1. kind:11 artifact shares in joined groups (the original "books from my rooms" set).
/// 2. kind:9802 highlights authored by the user — a book the user highlighted is by
///    definition recent, even if it was never formally "shared" as a kind:11.
///
/// For each book key we keep the most recent timestamp across both signals
/// and prefer the kind:11 record (with title/cover) when one exists, falling
/// back to a synthesized record carrying just the catalog id when not.
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

    // Dedupe by reference key — same book referenced from multiple groups or
    // from both kind:11 and kind:9802 collapses to the most-recent occurrence.
    let mut by_ref: HashMap<String, ArtifactRecord> = HashMap::new();
    // Full kind:11 book index regardless of joined-group membership. Used to
    // backfill title/cover for books seen via highlight signals when their
    // kind:11 share lives in a group the user isn't in.
    let mut metadata_by_ref: HashMap<String, ArtifactRecord> = HashMap::new();

    // 1. kind:11 artifact shares. We scan everything once and split into two
    //    buckets — `by_ref` requires the share to be in a joined group, while
    //    `metadata_by_ref` collects every book share for later backfill.
    {
        let cap = (limit.saturating_mul(8)).max(256) as i32;
        let filter = NdbFilter::new()
            .kinds([KIND_ARTIFACT_SHARE as u64])
            .build();
        let results = ndb
            .query(&txn, &[filter], cap)
            .map_err(|e| CoreError::Cache(format!("query artifacts: {e}")))?;

        for r in &results {
            let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else { continue };
            let Ok(json) = note.json() else { continue };
            let Ok(event) = Event::from_json(&json) else { continue };

            let Some(group_id) = first_tag_value(&event, "h") else { continue };
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

            // Always remember the share for metadata, newest wins.
            match metadata_by_ref.get(&key) {
                Some(existing)
                    if existing.created_at.unwrap_or(0) >= rec.created_at.unwrap_or(0) => {}
                _ => {
                    metadata_by_ref.insert(key.clone(), rec.clone());
                }
            }

            if !joined.contains(group_id) {
                continue;
            }
            match by_ref.get(&key) {
                Some(existing)
                    if existing.created_at.unwrap_or(0) >= rec.created_at.unwrap_or(0) => {}
                _ => {
                    by_ref.insert(key, rec);
                }
            }
        }
    }

    // 2. kind:9802 highlights authored by the user. Bypasses the joined-group
    //    filter — a vault-only highlight still counts as "I'm reading this".
    //    For each book we haven't already surfaced via kind:11, prefer pulling
    //    title/cover from any cached kind:11 share before falling back to a
    //    synthesized ISBN-only record.
    if let Ok(author) = PublicKey::from_hex(user) {
        let cap = (limit.saturating_mul(16)).max(512) as i32;
        let author_bytes = author.to_bytes();
        let filter = NdbFilter::new()
            .kinds([KIND_HIGHLIGHT as u64])
            .authors([&author_bytes])
            .build();

        if let Ok(results) = ndb.query(&txn, &[filter], cap) {
            for r in &results {
                let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else { continue };
                let Ok(json) = note.json() else { continue };
                let Ok(event) = Event::from_json(&json) else { continue };

                let Some(catalog_id) = book_catalog_id_from_highlight(&event) else {
                    continue;
                };
                let key = format!("i:{catalog_id}");
                let ts = event.created_at.as_secs();

                if let Some(existing) = by_ref.get_mut(&key) {
                    // Bump the recency to whichever signal is newer.
                    if existing.created_at.unwrap_or(0) < ts {
                        existing.created_at = Some(ts);
                    }
                    continue;
                }

                let mut record = metadata_by_ref
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| synthesize_book_record(&catalog_id, ts));
                if record.created_at.unwrap_or(0) < ts {
                    record.created_at = Some(ts);
                }
                by_ref.insert(key, record);
            }
        }
    }

    let mut out: Vec<ArtifactRecord> = by_ref.into_values().collect();
    out.sort_by(|a, b| b.created_at.unwrap_or(0).cmp(&a.created_at.unwrap_or(0)));
    out.truncate(limit as usize);
    Ok(out)
}

/// Pull a book catalog id (e.g. `isbn:9780…`) off a kind:9802 highlight. Looks
/// at every `i` tag because the canonical NIP-73 tag may sit alongside the
/// primary `i` reference (build_highlight_event mirrors the catalog id).
fn book_catalog_id_from_highlight(event: &Event) -> Option<String> {
    for tag in event.tags.iter() {
        let slice = tag.as_slice();
        if slice.first().map(String::as_str) != Some("i") {
            continue;
        }
        let Some(value) = slice.get(1) else { continue };
        if value.to_ascii_lowercase().starts_with("isbn:") {
            return Some(value.to_string());
        }
    }
    None
}

/// Minimal ArtifactRecord we can build when we have only an `isbn:…` reference
/// and no kind:11 cover/title source. Title and image are empty — the picker
/// renders a placeholder cell — but the highlight reference tags are correct
/// so picking it still produces a valid kind:9802.
fn synthesize_book_record(catalog_id: &str, created_at: u64) -> ArtifactRecord {
    let preview = ArtifactPreview {
        id: String::new(),
        url: String::new(),
        title: String::new(),
        author: String::new(),
        image: String::new(),
        description: String::new(),
        source: "book".to_string(),
        domain: String::new(),
        catalog_id: catalog_id.to_string(),
        catalog_kind: "isbn".to_string(),
        podcast_guid: String::new(),
        podcast_item_guid: String::new(),
        podcast_show_title: String::new(),
        audio_url: String::new(),
        audio_preview_url: String::new(),
        transcript_url: String::new(),
        feed_url: String::new(),
        published_at: String::new(),
        duration_seconds: None,
        reference_tag_name: "i".to_string(),
        reference_tag_value: catalog_id.to_string(),
        reference_kind: "isbn".to_string(),
        highlight_tag_name: "i".to_string(),
        highlight_tag_value: catalog_id.to_string(),
        highlight_reference_key: format!("i:{catalog_id}"),
        chapters: Vec::new(),
    };
    ArtifactRecord {
        preview,
        group_id: String::new(),
        share_event_id: String::new(),
        pubkey: String::new(),
        created_at: Some(created_at),
        note: String::new(),
    }
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

    fn book_highlight(keys: &Keys, isbn: &str, ts: u64) -> Event {
        EventBuilder::new(Kind::Custom(KIND_HIGHLIGHT), "quote")
            .tags(vec![
                Tag::parse(vec!["i".to_string(), format!("isbn:{isbn}")]).unwrap(),
            ])
            .custom_created_at(Timestamp::from(ts))
            .sign_with_keys(keys)
            .expect("sign highlight")
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
    fn highlight_surfaces_book_with_no_share_in_joined_group() {
        // Reproduces the user-reported bug: I publish highlights of a book I'm
        // reading but the picker never lists it because no kind:11 share exists
        // in any group I'm in. The highlight itself should be enough.
        let (ndb, _tmp) = isolated_ndb();
        let user_keys = Keys::generate();
        let user = user_keys.public_key().to_hex();

        ingest(&ndb, &book_highlight(&user_keys, "111", 500));
        wait_for_ndb();

        let out = query_recent_books(&ndb, &user, 10).expect("query");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].preview.catalog_id, "isbn:111");
        assert_eq!(out[0].preview.highlight_tag_name, "i");
        assert_eq!(out[0].preview.highlight_tag_value, "isbn:111");
        assert_eq!(out[0].created_at, Some(500));
    }

    #[test]
    fn highlight_backfills_metadata_from_kind11_in_other_group() {
        // Highlight signal exists for the book, but the only kind:11 share for
        // it lives in a group the user isn't joined to. We should still surface
        // the book with the title/cover from that share rather than a blank
        // placeholder.
        let (ndb, _tmp) = isolated_ndb();
        let user_keys = Keys::generate();
        let user = user_keys.public_key().to_hex();
        let other = Keys::generate();

        ingest(&ndb, &book_share(&other, "elsewhere", "b1", "isbn:111", "Real Title", 100));
        ingest(&ndb, &book_highlight(&user_keys, "111", 500));
        wait_for_ndb();

        let out = query_recent_books(&ndb, &user, 10).expect("query");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].preview.title, "Real Title");
        // Recency reflects the highlight, which is newer than the kind:11.
        assert_eq!(out[0].created_at, Some(500));
    }

    #[test]
    fn highlight_bumps_recency_for_existing_share() {
        // A book is already in recents via a kind:11 share in a joined group.
        // A newer highlight on the same ISBN should bump it back to the top.
        let (ndb, _tmp) = isolated_ndb();
        let user_keys = Keys::generate();
        let user = user_keys.public_key().to_hex();
        let admin = Keys::generate();

        ingest(&ndb, &membership(&admin, "alpha", &user, 1));
        ingest(&ndb, &book_share(&admin, "alpha", "b1", "isbn:111", "Older Share", 100));
        ingest(&ndb, &book_share(&admin, "alpha", "b2", "isbn:222", "Newer Share", 200));
        ingest(&ndb, &book_highlight(&user_keys, "111", 300));
        wait_for_ndb();

        let out = query_recent_books(&ndb, &user, 10).expect("query");
        assert_eq!(out.len(), 2);
        // The book with the recent highlight is now first.
        assert_eq!(out[0].preview.catalog_id, "isbn:111");
        assert_eq!(out[0].created_at, Some(300));
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
