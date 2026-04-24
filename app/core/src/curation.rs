//! NIP-51 curated communities list. The "featured rooms" shelf is backed by a
//! single replaceable event published by the relay's pubkey; the event's
//! `group` tags enumerate the rooms that appear in the shelf.
//!
//! kind: 10009 — the NIP-51 "simple groups" list ("NIP-29 groups the author
//! is in"). A curator publishing kind:10009 is semantically "here are the
//! groups I host that I recommend"; the iOS explorer reads the newest
//! 10009 from the configured curator pubkey and renders its `group` tags
//! as the Featured shelf. Friends also publish 10009s to enumerate their
//! own memberships; `recommendations.rs` reads those with an author-in-
//! follow-set filter to power the "Friends are here" shelf. Same kind,
//! same tag shape, different author → different meaning.

use std::collections::HashSet;

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;
use crate::groups::{build_community_summary, KIND_GROUP_METADATA};
use crate::models::CommunitySummary;

/// NIP-51 "simple groups" list kind. Replaceable — one list per pubkey,
/// newest wins. Shared with `recommendations.rs` (friends' self-lists).
pub const KIND_CURATED_COMMUNITIES: u16 = 10009;

/// Read the latest kind:10012 event authored by `curator_pubkey_hex` from
/// nostrdb and resolve each referenced group into a `CommunitySummary`. Rooms
/// whose kind:39000 metadata isn't cached yet are dropped silently — the
/// relay sub installed on explorer appear backfills them, and the next call
/// after the backfill lands returns the full list.
pub fn fetch_curated_rooms_from_ndb(
    ndb: &Ndb,
    curator_pubkey_hex: &str,
) -> Result<Vec<CommunitySummary>, CoreError> {
    let curator_pubkey_hex = curator_pubkey_hex.trim();
    if curator_pubkey_hex.is_empty() {
        return Ok(Vec::new());
    }

    let curator = PublicKey::from_hex(curator_pubkey_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid curator pubkey: {e}")))?;

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let pk_bytes: [u8; 32] = curator.to_bytes();
    let list_filter = NdbFilter::new()
        .kinds([KIND_CURATED_COMMUNITIES as u64])
        .authors([&pk_bytes])
        .build();

    let results = ndb
        .query(&txn, &[list_filter], 16)
        .map_err(|e| CoreError::Cache(format!("query curated list: {e}")))?;

    // Replaceable event — newest `created_at` wins.
    let mut newest: Option<Event> = None;
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        newest = Some(match newest {
            Some(prev) if prev.created_at >= event.created_at => prev,
            _ => event,
        });
    }

    let Some(list_event) = newest else {
        return Ok(Vec::new());
    };

    let group_ids = extract_group_ids(&list_event);
    if group_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Fetch kind:39000 metadata for the referenced group ids, in list order.
    let id_refs: Vec<&str> = group_ids.iter().map(String::as_str).collect();
    let metadata_filter = NdbFilter::new()
        .kinds([KIND_GROUP_METADATA as u64])
        .tags(id_refs, 'd')
        .build();

    let metadata_results = ndb
        .query(&txn, &[metadata_filter], 1024)
        .map_err(|e| CoreError::Cache(format!("query curated metadata: {e}")))?;

    // Dedup by `d` tag, keeping newest per id — same rule as joined communities.
    use std::collections::BTreeMap;
    let mut newest_by_id: BTreeMap<String, Event> = BTreeMap::new();
    for r in &metadata_results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        let Some(d) = first_tag_value(&event, "d").map(|s| s.trim().to_string()) else {
            continue;
        };
        match newest_by_id.get(&d) {
            Some(prev) if prev.created_at >= event.created_at => {}
            _ => {
                newest_by_id.insert(d, event);
            }
        }
    }

    // Preserve curator's order. Rooms missing metadata are dropped until the
    // backfill lands.
    let mut out: Vec<CommunitySummary> = Vec::with_capacity(group_ids.len());
    for id in &group_ids {
        let Some(event) = newest_by_id.get(id) else {
            continue;
        };
        if let Ok(summary) = build_community_summary(event) {
            out.push(summary);
        }
    }
    Ok(out)
}

/// All group ids referenced by a kind:10012 list, in curator order, deduped.
/// Accepts two tag shapes:
/// - `["group", "<id>", "<relay>"]` — NIP-29 native shape
/// - `["a", "39000:<pubkey>:<id>"]` — NIP-33 address shape, fallback
pub(crate) fn extract_group_ids(event: &Event) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for tag in event.tags.iter() {
        let slice = tag.as_slice();
        let name = slice.first().map(String::as_str);
        match name {
            Some("group") => {
                if let Some(id) = slice.get(1).map(String::as_str) {
                    let id = id.trim();
                    if !id.is_empty() && seen.insert(id.to_string()) {
                        out.push(id.to_string());
                    }
                }
            }
            Some("a") => {
                let Some(coord) = slice.get(1).map(String::as_str) else {
                    continue;
                };
                let mut parts = coord.splitn(3, ':');
                let Some(kind_str) = parts.next() else { continue };
                if kind_str != "39000" {
                    continue;
                }
                let _ = parts.next();
                let Some(id) = parts.next() else { continue };
                let id = id.trim();
                if !id.is_empty() && seen.insert(id.to_string()) {
                    out.push(id.to_string());
                }
            }
            _ => {}
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

    fn isolated_ndb() -> (Ndb, tempfile::TempDir) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("ndb");
        std::fs::create_dir_all(&path).expect("mkdir");
        let cfg = nostrdb::Config::new().set_mapsize(32 * 1024 * 1024);
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

    fn sign(keys: &Keys, kind: u16, tags: Vec<Tag>, content: &str) -> Event {
        EventBuilder::new(Kind::Custom(kind), content)
            .tags(tags)
            .sign_with_keys(keys)
            .expect("sign")
    }

    fn meta(keys: &Keys, id: &str, name: &str) -> Event {
        sign(
            keys,
            KIND_GROUP_METADATA,
            vec![
                Tag::identifier(id),
                Tag::parse(vec!["name".to_string(), name.to_string()]).unwrap(),
                Tag::parse(vec!["public".to_string()]).unwrap(),
                Tag::parse(vec!["open".to_string()]).unwrap(),
            ],
            "",
        )
    }

    fn list_event(
        keys: &Keys,
        group_ids: &[&str],
        relay_url: &str,
        created_at: u64,
    ) -> Event {
        let mut tags: Vec<Tag> = Vec::new();
        for id in group_ids {
            tags.push(
                Tag::parse(vec![
                    "group".to_string(),
                    (*id).to_string(),
                    relay_url.to_string(),
                ])
                .unwrap(),
            );
        }
        EventBuilder::new(Kind::Custom(KIND_CURATED_COMMUNITIES), "")
            .tags(tags)
            .custom_created_at(Timestamp::from(created_at))
            .sign_with_keys(keys)
            .expect("sign")
    }

    #[test]
    fn empty_curator_returns_empty() {
        let (ndb, _tmp) = isolated_ndb();
        let out = fetch_curated_rooms_from_ndb(&ndb, "").expect("empty");
        assert!(out.is_empty());
    }

    #[test]
    fn resolves_list_in_curator_order() {
        let (ndb, _tmp) = isolated_ndb();
        let curator = Keys::generate();
        let author = Keys::generate();

        // Metadata for three groups.
        ingest(&ndb, &meta(&author, "alpha", "Alpha"));
        ingest(&ndb, &meta(&author, "bravo", "Bravo"));
        ingest(&ndb, &meta(&author, "charlie", "Charlie"));
        // Curator's featured list: bravo, alpha (charlie omitted).
        ingest(
            &ndb,
            &list_event(&curator, &["bravo", "alpha"], "wss://relay.example", 100),
        );
        wait_for_ndb();

        let out =
            fetch_curated_rooms_from_ndb(&ndb, &curator.public_key().to_hex()).expect("ok");
        let ids: Vec<_> = out.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["bravo", "alpha"], "curator order preserved");
    }

    #[test]
    fn drops_rooms_without_cached_metadata() {
        let (ndb, _tmp) = isolated_ndb();
        let curator = Keys::generate();
        let author = Keys::generate();

        // Only alpha metadata is cached; bravo isn't.
        ingest(&ndb, &meta(&author, "alpha", "Alpha"));
        ingest(
            &ndb,
            &list_event(&curator, &["alpha", "bravo"], "wss://relay.example", 100),
        );
        wait_for_ndb();

        let out =
            fetch_curated_rooms_from_ndb(&ndb, &curator.public_key().to_hex()).expect("ok");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "alpha");
    }

    #[test]
    fn newest_list_wins() {
        let (ndb, _tmp) = isolated_ndb();
        let curator = Keys::generate();
        let author = Keys::generate();

        ingest(&ndb, &meta(&author, "alpha", "Alpha"));
        ingest(&ndb, &meta(&author, "bravo", "Bravo"));
        // Older list — should be superseded.
        ingest(
            &ndb,
            &list_event(&curator, &["alpha"], "wss://relay.example", 100),
        );
        // Newer list has both.
        ingest(
            &ndb,
            &list_event(&curator, &["alpha", "bravo"], "wss://relay.example", 200),
        );
        wait_for_ndb();

        let out =
            fetch_curated_rooms_from_ndb(&ndb, &curator.public_key().to_hex()).expect("ok");
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn accepts_a_tag_addressing_fallback() {
        let (ndb, _tmp) = isolated_ndb();
        let curator = Keys::generate();
        let author = Keys::generate();

        ingest(&ndb, &meta(&author, "alpha", "Alpha"));
        // Use `a` tag instead of `group` tag.
        let list = EventBuilder::new(Kind::Custom(KIND_CURATED_COMMUNITIES), "")
            .tags(vec![
                Tag::parse(vec![
                    "a".to_string(),
                    format!("39000:{}:alpha", author.public_key().to_hex()),
                ])
                .unwrap(),
            ])
            .custom_created_at(Timestamp::from(100))
            .sign_with_keys(&curator)
            .expect("sign");
        ingest(&ndb, &list);
        wait_for_ndb();

        let out =
            fetch_curated_rooms_from_ndb(&ndb, &curator.public_key().to_hex()).expect("ok");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "alpha");
    }

    #[test]
    fn dedups_duplicate_group_tags() {
        let (ndb, _tmp) = isolated_ndb();
        let curator = Keys::generate();
        let author = Keys::generate();

        ingest(&ndb, &meta(&author, "alpha", "Alpha"));
        ingest(
            &ndb,
            &list_event(&curator, &["alpha", "alpha", "alpha"], "wss://r", 100),
        );
        wait_for_ndb();

        let out =
            fetch_curated_rooms_from_ndb(&ndb, &curator.public_key().to_hex()).expect("ok");
        assert_eq!(out.len(), 1);
    }
}
