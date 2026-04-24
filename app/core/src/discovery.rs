//! Room discovery — catalog of all known NIP-29 groups (kind:39000) cached
//! in nostrdb. Powers the "Browse all" grid and the derived "New & noteworthy"
//! shelf on the explorer.
//!
//! The local query is pure: it scans cached metadata, dedups by `d` tag, and
//! returns `CommunitySummary` sorted by `created_at` descending. Fresh rooms
//! arrive via a long-lived relay subscription installed from `client.rs` on
//! explorer appearance.

use std::collections::BTreeMap;

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;
use crate::groups::{build_community_summary, KIND_GROUP_METADATA};
use crate::models::CommunitySummary;

/// Return every cached kind:39000 as a `CommunitySummary`, newest first,
/// truncated to `limit`. Dedup by group id with the newest `created_at`
/// winning.
pub fn query_all_rooms_from_ndb(
    ndb: &Ndb,
    limit: u32,
) -> Result<Vec<CommunitySummary>, CoreError> {
    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    // Cap the raw scan at 4x the requested limit — metadata events can collide
    // on `d` (same group, newer supersession), and we want enough headroom to
    // dedup down to `limit` without scanning the whole index.
    let cap = ((limit.saturating_mul(4)).max(256)) as i32;
    let filter = NdbFilter::new().kinds([KIND_GROUP_METADATA as u64]).build();
    let results = ndb
        .query(&txn, &[filter], cap)
        .map_err(|e| CoreError::Cache(format!("query all rooms: {e}")))?;

    let mut newest_by_id: BTreeMap<String, Event> = BTreeMap::new();
    for r in &results {
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
        if d.is_empty() {
            continue;
        }
        match newest_by_id.get(&d) {
            Some(prev) if prev.created_at >= event.created_at => {}
            _ => {
                newest_by_id.insert(d, event);
            }
        }
    }

    let mut summaries: Vec<CommunitySummary> = newest_by_id
        .values()
        .filter_map(|e| build_community_summary(e).ok())
        .collect();

    summaries.sort_by(|a, b| {
        let a_ts = a.created_at.unwrap_or(0);
        let b_ts = b.created_at.unwrap_or(0);
        b_ts.cmp(&a_ts)
    });
    summaries.truncate(limit as usize);
    Ok(summaries)
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

    fn meta(keys: &Keys, id: &str, name: &str, ts: u64) -> Event {
        EventBuilder::new(Kind::Custom(KIND_GROUP_METADATA), "")
            .tags(vec![
                Tag::identifier(id),
                Tag::parse(vec!["name".to_string(), name.to_string()]).unwrap(),
                Tag::parse(vec!["public".to_string()]).unwrap(),
                Tag::parse(vec!["open".to_string()]).unwrap(),
            ])
            .custom_created_at(Timestamp::from(ts))
            .sign_with_keys(keys)
            .expect("sign")
    }

    #[test]
    fn returns_empty_on_cold_cache() {
        let (ndb, _tmp) = isolated_ndb();
        let out = query_all_rooms_from_ndb(&ndb, 32).expect("ok");
        assert!(out.is_empty());
    }

    #[test]
    fn sorts_newest_first() {
        let (ndb, _tmp) = isolated_ndb();
        let author = Keys::generate();
        ingest(&ndb, &meta(&author, "alpha", "Alpha", 100));
        ingest(&ndb, &meta(&author, "bravo", "Bravo", 300));
        ingest(&ndb, &meta(&author, "charlie", "Charlie", 200));
        wait_for_ndb();

        let out = query_all_rooms_from_ndb(&ndb, 32).expect("ok");
        let ids: Vec<_> = out.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["bravo", "charlie", "alpha"]);
    }

    #[test]
    fn honors_limit() {
        let (ndb, _tmp) = isolated_ndb();
        let author = Keys::generate();
        for i in 0..10u64 {
            ingest(&ndb, &meta(&author, &format!("room{i}"), &format!("R{i}"), 100 + i));
        }
        wait_for_ndb();

        let out = query_all_rooms_from_ndb(&ndb, 4).expect("ok");
        assert_eq!(out.len(), 4);
    }

    #[test]
    fn dedups_by_d_tag_keeping_newest() {
        let (ndb, _tmp) = isolated_ndb();
        let author = Keys::generate();
        ingest(&ndb, &meta(&author, "alpha", "Old Alpha", 100));
        ingest(&ndb, &meta(&author, "alpha", "New Alpha", 200));
        wait_for_ndb();

        let out = query_all_rooms_from_ndb(&ndb, 32).expect("ok");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "New Alpha");
    }
}
