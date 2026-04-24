//! Social-graph room recommendations. Two shelves in the explorer come from
//! local computation over already-cached events:
//!
//! 1. **Rooms with friends** — rooms where at least two pubkeys the user
//!    follows (kind:3) appear in the members list (kind:39002).
//! 2. **Rooms from authors you read** — rooms where pubkeys the user has
//!    highlighted (the `a`-tag author in their kind:9802) have shared
//!    artifacts (kind:11). This is the "I've read things by X, so a room X
//!    posts in is probably relevant" signal.
//!
//! Both are pure over nostrdb; no relay calls. The explorer re-queries on
//! `MembershipChanged` / `FollowingReadsUpdated` so the shelves stay fresh.

use std::collections::{BTreeMap, HashSet};

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;
use crate::follows;
use crate::groups::{build_community_summary, KIND_GROUP_MEMBERS, KIND_GROUP_METADATA};
use crate::models::{RoomRecommendation, RoomRecommendationReason};

/// NIP-51 "simple groups" list (kind:10009). A user publishes this to
/// enumerate the NIP-29 rooms they're in; we read one from each follow to
/// build the "Friends are here" shelf.
const KIND_SIMPLE_GROUPS_LIST: u16 = 10009;

/// Rooms where 2+ of the user's follows are members. Rooms the user is
/// already in are excluded — the explorer's "Your rooms" shelf is elsewhere.
///
/// The signal is the union of two sources:
/// - **kind:10009 authored by each follow** — the NIP-51 "simple groups"
///   list, public and user-owned; denser when friends publish it.
/// - **kind:39002 with `#p=<follow>`** — the relay-owned members event,
///   falls back when a given friend hasn't published a 10009 yet.
///
/// The `reason_pubkeys` on each recommendation is the set of matching follows
/// (the avatar cluster on the card). Capped at 5 per room for UI.
pub fn query_rooms_with_friends(
    ndb: &Ndb,
    user_pubkey_hex: &str,
    limit: u32,
) -> Result<Vec<RoomRecommendation>, CoreError> {
    let user_pubkey_hex = user_pubkey_hex.trim();
    if user_pubkey_hex.is_empty() {
        return Ok(Vec::new());
    }

    let follows: HashSet<String> = follows::query_follows(ndb, user_pubkey_hex)?
        .into_iter()
        .map(|s| s.to_ascii_lowercase())
        .collect();
    if follows.is_empty() {
        return Ok(Vec::new());
    }

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let user_hex_lower = user_pubkey_hex.to_ascii_lowercase();

    // Aggregate map: group_id -> set of follow pubkeys (deduped across both
    // signals). We also track the set of group ids the user is a member of
    // so we can drop those at the end.
    let mut friends_in_group: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    let mut user_is_in: HashSet<String> = HashSet::new();

    // --- Signal 1: kind:10009 authored by follows (and the user themself) ---
    //
    // Each follow publishes one list naming the rooms they're in. We keep
    // the newest per author. The user's own 10009 is read alongside so
    // rooms they're already in get excluded without needing the 39002
    // signal to catch them.
    let mut list_pk_bytes: Vec<[u8; 32]> = follows
        .iter()
        .filter_map(|hex| PublicKey::from_hex(hex).ok())
        .map(|pk| pk.to_bytes())
        .collect();
    if let Ok(user_pk) = PublicKey::from_hex(user_pubkey_hex) {
        list_pk_bytes.push(user_pk.to_bytes());
    }
    if !list_pk_bytes.is_empty() {
        let refs: Vec<&[u8; 32]> = list_pk_bytes.iter().collect();
        let groups_list_filter = NdbFilter::new()
            .kinds([KIND_SIMPLE_GROUPS_LIST as u64])
            .authors(refs.iter().copied())
            .build();
        let groups_list_results = ndb
            .query(&txn, &[groups_list_filter], 8192)
            .map_err(|e| CoreError::Cache(format!("query friends groups list: {e}")))?;

        let mut newest_by_author: BTreeMap<String, Event> = BTreeMap::new();
        for r in &groups_list_results {
            let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
                continue;
            };
            let Ok(json) = note.json() else { continue };
            let Ok(event) = Event::from_json(&json) else {
                continue;
            };
            let author = event.pubkey.to_hex().to_ascii_lowercase();
            match newest_by_author.get(&author) {
                Some(prev) if prev.created_at >= event.created_at => {}
                _ => {
                    newest_by_author.insert(author, event);
                }
            }
        }

        for (author_lower, event) in newest_by_author {
            let author_display = event.pubkey.to_hex();
            for tag in event.tags.iter() {
                let s = tag.as_slice();
                if s.first().map(String::as_str) != Some("group") {
                    continue;
                }
                let Some(group_id) = s.get(1).map(|v| v.trim().to_string()) else {
                    continue;
                };
                if group_id.is_empty() {
                    continue;
                }
                if author_lower == user_hex_lower {
                    user_is_in.insert(group_id);
                } else {
                    friends_in_group
                        .entry(group_id)
                        .or_default()
                        .insert(author_display.clone());
                }
            }
        }
    }

    // --- Signal 2: kind:39002 (relay-owned members lists) ---
    //
    // For each members event, scan the `p` tags for the user's pubkey (→
    // they're in that room) and for any follow (→ that follow is in that
    // room). Replaceable; newest 39002 per group id wins, but since we only
    // care about which follows appear, the union across all 39002s for the
    // group is fine — a follow listed in the newest alone OR in an older
    // one still counts. We do prefer the newest when there's a conflict.
    let members_filter = NdbFilter::new()
        .kinds([KIND_GROUP_MEMBERS as u64])
        .build();
    let member_results = ndb
        .query(&txn, &[members_filter], 4096)
        .map_err(|e| CoreError::Cache(format!("query members: {e}")))?;

    let mut newest_member_event: BTreeMap<String, Event> = BTreeMap::new();
    for r in &member_results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        let Some(group_id) = first_tag_value(&event, "d").map(|s| s.trim().to_string()) else {
            continue;
        };
        if group_id.is_empty() {
            continue;
        }
        match newest_member_event.get(&group_id) {
            Some(prev) if prev.created_at >= event.created_at => {}
            _ => {
                newest_member_event.insert(group_id, event);
            }
        }
    }
    for (group_id, event) in &newest_member_event {
        for tag in event.tags.iter() {
            let s = tag.as_slice();
            if s.first().map(String::as_str) != Some("p") {
                continue;
            }
            let Some(pk) = s.get(1).map(String::as_str) else {
                continue;
            };
            let pk_lower = pk.to_ascii_lowercase();
            if pk_lower == user_hex_lower {
                user_is_in.insert(group_id.clone());
            } else if follows.contains(&pk_lower) {
                friends_in_group
                    .entry(group_id.clone())
                    .or_default()
                    .insert(pk.to_string());
            }
        }
    }

    // Drop rooms the user is already in and anything below the 2-follow
    // threshold.
    friends_in_group.retain(|group_id, pubkeys| {
        !user_is_in.contains(group_id) && pubkeys.len() >= 2
    });
    if friends_in_group.is_empty() {
        return Ok(Vec::new());
    }

    // Resolve metadata for the matching groups.
    let ids: Vec<String> = friends_in_group.keys().cloned().collect();
    let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let metadata_filter = NdbFilter::new()
        .kinds([KIND_GROUP_METADATA as u64])
        .tags(id_refs, 'd')
        .build();
    let metadata_results = ndb
        .query(&txn, &[metadata_filter], 1024)
        .map_err(|e| CoreError::Cache(format!("query members metadata: {e}")))?;
    let mut metadata_by_id: BTreeMap<String, Event> = BTreeMap::new();
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
        match metadata_by_id.get(&d) {
            Some(prev) if prev.created_at >= event.created_at => {}
            _ => {
                metadata_by_id.insert(d, event);
            }
        }
    }

    // Assemble. Sort by number of matching follows desc (strongest signal first).
    let mut out: Vec<RoomRecommendation> = Vec::new();
    for (group_id, pubkeys) in friends_in_group {
        let Some(meta_event) = metadata_by_id.get(&group_id) else {
            continue;
        };
        let Ok(summary) = build_community_summary(meta_event) else {
            continue;
        };
        let mut reasons: Vec<String> = pubkeys.into_iter().collect();
        reasons.sort();
        reasons.truncate(5);
        out.push(RoomRecommendation {
            summary,
            reason_pubkeys: reasons,
            reason_kind: RoomRecommendationReason::Friends,
        });
    }

    out.sort_by(|a, b| b.reason_pubkeys.len().cmp(&a.reason_pubkeys.len()));
    out.truncate(limit as usize);
    Ok(out)
}

/// Rooms where authors whose articles the user has highlighted have shared
/// artifacts. "Author" here is the pubkey half of the `a`-tag on a kind:9802
/// (`30023:<author_hex>:<d>`). For each unique author we collect the groups
/// they've posted a kind:11 into, excluding rooms the user is already in.
pub fn query_rooms_from_read_authors(
    ndb: &Ndb,
    user_pubkey_hex: &str,
    limit: u32,
) -> Result<Vec<RoomRecommendation>, CoreError> {
    let user_pubkey_hex = user_pubkey_hex.trim();
    if user_pubkey_hex.is_empty() {
        return Ok(Vec::new());
    }
    let user = PublicKey::from_hex(user_pubkey_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid user pubkey: {e}")))?;

    let user_hex_lower = user_pubkey_hex.to_ascii_lowercase();
    let mut authors_lower: HashSet<String> = HashSet::new();
    // group_id -> (authors contributing, newest_created_at_among_matches)
    let mut groups_to_authors: BTreeMap<String, HashSet<String>> = BTreeMap::new();

    // First pass: read highlights + shares inside one txn, then drop it so
    // downstream helpers can open their own (nostrdb doesn't nest).
    {
        let txn = Transaction::new(ndb)
            .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

        // 1. User's kind:9802 → collect author pubkeys from `a` tags.
        let pk_bytes: [u8; 32] = user.to_bytes();
        let highlights_filter = NdbFilter::new()
            .kinds([9802u64])
            .authors([&pk_bytes])
            .build();
        let highlight_results = ndb
            .query(&txn, &[highlights_filter], 1024)
            .map_err(|e| CoreError::Cache(format!("query highlights: {e}")))?;

        for r in &highlight_results {
            let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
                continue;
            };
            let Ok(json) = note.json() else { continue };
            let Ok(event) = Event::from_json(&json) else {
                continue;
            };
            for tag in event.tags.iter() {
                let slice = tag.as_slice();
                if slice.first().map(String::as_str) != Some("a") {
                    continue;
                }
                let Some(addr) = slice.get(1).map(String::as_str) else {
                    continue;
                };
                let mut parts = addr.splitn(3, ':');
                let kind_str = match parts.next() {
                    Some(v) => v,
                    None => continue,
                };
                if kind_str != "30023" {
                    continue;
                }
                let Some(author_hex) = parts.next().map(|s| s.trim().to_ascii_lowercase())
                else {
                    continue;
                };
                if author_hex.is_empty() || author_hex == user_hex_lower {
                    continue;
                }
                authors_lower.insert(author_hex);
            }
        }

        if authors_lower.is_empty() {
            return Ok(Vec::new());
        }

        // 2. Find kind:11 (artifact share) events authored by any of those
        //    authors. Reuses the same txn as the highlights scan above.
        let shares_filter = NdbFilter::new().kinds([11u64]).build();
        let share_results = ndb
            .query(&txn, &[shares_filter], 4096)
            .map_err(|e| CoreError::Cache(format!("query shares: {e}")))?;

        for r in &share_results {
            let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
                continue;
            };
            let Ok(json) = note.json() else { continue };
            let Ok(event) = Event::from_json(&json) else {
                continue;
            };
            let author_hex = event.pubkey.to_hex().to_ascii_lowercase();
            if !authors_lower.contains(&author_hex) {
                continue;
            }
            let Some(group_id) = first_tag_value(&event, "h").map(|s| s.trim().to_string())
            else {
                continue;
            };
            if group_id.is_empty() {
                continue;
            }
            groups_to_authors
                .entry(group_id)
                .or_default()
                .insert(event.pubkey.to_hex());
        }
    }
    // txn dropped — nostrdb does not support nested transactions, and the
    // next step calls a helper that opens its own.

    if groups_to_authors.is_empty() {
        return Ok(Vec::new());
    }

    // 3. Exclude groups the user is already a member of.
    let joined_ids: HashSet<String> = crate::groups::query_joined_communities_from_ndb(
        ndb,
        user_pubkey_hex,
    )?
    .into_iter()
    .map(|c| c.id)
    .collect();

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("reopen ndb txn: {e}")))?;

    // 4. Fetch metadata for candidate groups.
    let candidate_ids: Vec<String> = groups_to_authors
        .keys()
        .filter(|id| !joined_ids.contains(*id))
        .cloned()
        .collect();
    if candidate_ids.is_empty() {
        return Ok(Vec::new());
    }
    let id_refs: Vec<&str> = candidate_ids.iter().map(String::as_str).collect();
    let metadata_filter = NdbFilter::new()
        .kinds([KIND_GROUP_METADATA as u64])
        .tags(id_refs, 'd')
        .build();
    let metadata_results = ndb
        .query(&txn, &[metadata_filter], 1024)
        .map_err(|e| CoreError::Cache(format!("query author-rooms metadata: {e}")))?;
    let mut metadata_by_id: BTreeMap<String, Event> = BTreeMap::new();
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
        match metadata_by_id.get(&d) {
            Some(prev) if prev.created_at >= event.created_at => {}
            _ => {
                metadata_by_id.insert(d, event);
            }
        }
    }

    let mut out: Vec<RoomRecommendation> = Vec::new();
    for id in candidate_ids {
        let Some(meta_event) = metadata_by_id.get(&id) else {
            continue;
        };
        let Ok(summary) = build_community_summary(meta_event) else {
            continue;
        };
        let mut reasons: Vec<String> = groups_to_authors
            .get(&id)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default();
        reasons.sort();
        reasons.truncate(5);
        out.push(RoomRecommendation {
            summary,
            reason_pubkeys: reasons,
            reason_kind: RoomRecommendationReason::Authors,
        });
    }

    out.sort_by(|a, b| b.reason_pubkeys.len().cmp(&a.reason_pubkeys.len()));
    out.truncate(limit as usize);
    Ok(out)
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

    fn contacts(me: &Keys, follows: &[&Keys]) -> Event {
        let tags: Vec<Tag> = follows.iter().map(|k| Tag::public_key(k.public_key())).collect();
        sign(me, 3, tags, "")
    }

    fn meta(author: &Keys, id: &str, name: &str) -> Event {
        sign(
            author,
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

    fn members(author: &Keys, id: &str, members: &[&Keys]) -> Event {
        let mut tags = vec![Tag::identifier(id)];
        for m in members {
            tags.push(Tag::public_key(m.public_key()));
        }
        sign(author, KIND_GROUP_MEMBERS, tags, "")
    }

    #[test]
    fn no_follows_means_empty() {
        let (ndb, _tmp) = isolated_ndb();
        let me = Keys::generate();
        let out = query_rooms_with_friends(&ndb, &me.public_key().to_hex(), 32).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn returns_rooms_with_two_plus_follows() {
        let (ndb, _tmp) = isolated_ndb();
        let me = Keys::generate();
        let f1 = Keys::generate();
        let f2 = Keys::generate();
        let f3 = Keys::generate();
        let stranger = Keys::generate();
        let author = Keys::generate();

        ingest(&ndb, &contacts(&me, &[&f1, &f2, &f3]));
        ingest(&ndb, &meta(&author, "alpha", "Alpha"));
        // f1 + f2 are in alpha → match (threshold 2).
        ingest(&ndb, &members(&author, "alpha", &[&f1, &f2, &stranger]));
        // Only f1 in bravo → doesn't meet threshold.
        ingest(&ndb, &meta(&author, "bravo", "Bravo"));
        ingest(&ndb, &members(&author, "bravo", &[&f1, &stranger]));
        wait_for_ndb();

        let out = query_rooms_with_friends(&ndb, &me.public_key().to_hex(), 32).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].summary.id, "alpha");
        assert_eq!(out[0].reason_pubkeys.len(), 2);
    }

    fn groups_list(keys: &Keys, group_ids: &[&str], relay_url: &str, ts: u64) -> Event {
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
        EventBuilder::new(Kind::Custom(KIND_SIMPLE_GROUPS_LIST), "")
            .tags(tags)
            .custom_created_at(Timestamp::from(ts))
            .sign_with_keys(keys)
            .expect("sign")
    }

    #[test]
    fn surfaces_rooms_from_friends_kind_10009_lists() {
        // Two follows publish 10009 lists naming the same room → shelf match.
        let (ndb, _tmp) = isolated_ndb();
        let me = Keys::generate();
        let f1 = Keys::generate();
        let f2 = Keys::generate();
        let author = Keys::generate();

        ingest(&ndb, &contacts(&me, &[&f1, &f2]));
        ingest(&ndb, &meta(&author, "alpha", "Alpha"));
        // Both follows list "alpha" as a group they're in. No 39002 needed.
        ingest(&ndb, &groups_list(&f1, &["alpha"], "wss://relay", 100));
        ingest(&ndb, &groups_list(&f2, &["alpha"], "wss://relay", 100));
        wait_for_ndb();

        let out = query_rooms_with_friends(&ndb, &me.public_key().to_hex(), 32).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].summary.id, "alpha");
        assert_eq!(out[0].reason_pubkeys.len(), 2);
    }

    #[test]
    fn kind_10009_from_user_excludes_their_rooms() {
        // The user's own kind:10009 tells us which rooms to exclude from
        // the shelf without needing a 39002 for those rooms.
        let (ndb, _tmp) = isolated_ndb();
        let me = Keys::generate();
        let f1 = Keys::generate();
        let f2 = Keys::generate();
        let author = Keys::generate();

        ingest(&ndb, &contacts(&me, &[&f1, &f2]));
        ingest(&ndb, &meta(&author, "alpha", "Alpha"));
        ingest(&ndb, &groups_list(&me, &["alpha"], "wss://relay", 100));
        // Even with two follows matching, user is already in alpha → exclude.
        ingest(&ndb, &groups_list(&f1, &["alpha"], "wss://relay", 100));
        ingest(&ndb, &groups_list(&f2, &["alpha"], "wss://relay", 100));
        wait_for_ndb();

        let out = query_rooms_with_friends(&ndb, &me.public_key().to_hex(), 32).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn excludes_rooms_user_is_in() {
        let (ndb, _tmp) = isolated_ndb();
        let me = Keys::generate();
        let f1 = Keys::generate();
        let f2 = Keys::generate();
        let author = Keys::generate();

        ingest(&ndb, &contacts(&me, &[&f1, &f2]));
        ingest(&ndb, &meta(&author, "alpha", "Alpha"));
        // Me + two follows — user is already a member.
        ingest(&ndb, &members(&author, "alpha", &[&me, &f1, &f2]));
        wait_for_ndb();

        let out = query_rooms_with_friends(&ndb, &me.public_key().to_hex(), 32).unwrap();
        assert!(out.is_empty(), "rooms the user is already in must be excluded");
    }

    #[test]
    fn rooms_from_read_authors_matches_highlight_to_share() {
        let (ndb, _tmp) = isolated_ndb();
        let me = Keys::generate();
        let author_a = Keys::generate();
        let author_b = Keys::generate();
        let group_author = Keys::generate();

        // My kind:9802 referencing author_a's article via `a` tag.
        let article_addr = format!("30023:{}:essay-1", author_a.public_key().to_hex());
        let highlight = sign(
            &me,
            9802,
            vec![Tag::parse(vec!["a".to_string(), article_addr]).unwrap()],
            "a quote",
        );
        ingest(&ndb, &highlight);

        // author_a has shared a kind:11 into "alpha".
        let share_a = sign(
            &author_a,
            11,
            vec![
                Tag::parse(vec!["h".to_string(), "alpha".to_string()]).unwrap(),
                Tag::identifier("art-1"),
                Tag::parse(vec!["title".to_string(), "t".to_string()]).unwrap(),
            ],
            "",
        );
        ingest(&ndb, &share_a);

        // author_b has shared into "bravo" — must NOT match (I didn't highlight them).
        let share_b = sign(
            &author_b,
            11,
            vec![
                Tag::parse(vec!["h".to_string(), "bravo".to_string()]).unwrap(),
                Tag::identifier("art-2"),
            ],
            "",
        );
        ingest(&ndb, &share_b);

        ingest(&ndb, &meta(&group_author, "alpha", "Alpha"));
        ingest(&ndb, &meta(&group_author, "bravo", "Bravo"));
        wait_for_ndb();

        let out = query_rooms_from_read_authors(&ndb, &me.public_key().to_hex(), 32).unwrap();
        let ids: Vec<_> = out.iter().map(|r| r.summary.id.as_str()).collect();
        assert_eq!(ids, vec!["alpha"]);
        assert_eq!(out[0].reason_kind, RoomRecommendationReason::Authors);
        assert_eq!(out[0].reason_pubkeys.len(), 1);
    }
}
