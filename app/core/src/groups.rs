//! NIP-29 group metadata + membership. Ports `web/src/lib/ndk/groups.ts`.
//!
//! The pure helpers here take `nostr::Event`s so they're trivial to unit-test
//! with `EventBuilder::sign_with_keys`. The live client path in `client.rs`
//! queries nostrdb directly and re-hydrates each matching note into an
//! `Event` via JSON (nostrdb strips signatures, so we splice a valid-shape
//! placeholder `sig` and rely on `Event::from_json` which does NOT verify).

use std::collections::{BTreeMap, HashSet};

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;
use crate::models::CommunitySummary;
use crate::nostr_runtime::NostrRuntime;
use crate::relays::HIGHLIGHTER_RELAY;

pub const KIND_GROUP_METADATA: u16 = 39000;
pub const KIND_GROUP_ADMINS: u16 = 39001;
pub const KIND_GROUP_MEMBERS: u16 = 39002;
/// NIP-29 join-request event. Published by a user asking to join a room;
/// the relay either auto-admits (open rooms) by publishing a 39002 that
/// includes the requester's pubkey, or holds the request for moderator
/// approval (closed rooms).
pub const KIND_JOIN_REQUEST: u16 = 9021;

/// Query the local nostrdb cache for the current user's joined communities.
/// Returns `[]` if the cache has no relevant events yet (cold start).
pub fn query_joined_communities_from_ndb(
    ndb: &Ndb,
    current_pubkey_hex: &str,
) -> Result<Vec<CommunitySummary>, CoreError> {
    if current_pubkey_hex.is_empty() {
        return Ok(Vec::new());
    }

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    // 1. Fetch all admin+member events and manually check the p-tag.
    //    nostrdb's tag index for the 'p' character does not reliably match
    //    hex pubkeys in parameterized-replaceable event kinds (39001/39002),
    //    so we scan by kind only and filter in Rust.
    let membership_filter = NdbFilter::new()
        .kinds([KIND_GROUP_ADMINS as u64, KIND_GROUP_MEMBERS as u64])
        .build();

    let membership_results = ndb
        .query(&txn, &[membership_filter], 4096)
        .map_err(|e| CoreError::Cache(format!("query membership: {e}")))?;

    let mut membership_events: Vec<Event> = Vec::with_capacity(membership_results.len());
    let mut group_ids: HashSet<String> = HashSet::new();
    for result in &membership_results {
        let Ok(note) = ndb.get_note_by_key(&txn, result.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        let has_me = event.tags.iter().any(|tag| {
            let s = tag.as_slice();
            s.first().map(String::as_str) == Some("p")
                && s.get(1).map(String::as_str) == Some(current_pubkey_hex)
        });
        if !has_me {
            continue;
        }
        if let Some(id) = group_id_from_event(&event) {
            group_ids.insert(id);
        }
        membership_events.push(event);
    }

    if group_ids.is_empty() {
        return Ok(Vec::new());
    }

    // 2. Fetch kind:39000 metadata for the discovered group ids. nostrdb
    //    filters with a `d` tag match work the same way as `#d` on a relay.
    let group_id_refs: Vec<&str> = group_ids.iter().map(|s| s.as_str()).collect();
    let metadata_filter = NdbFilter::new()
        .kinds([KIND_GROUP_METADATA as u64])
        .tags(group_id_refs, 'd')
        .build();

    let metadata_results = ndb
        .query(&txn, &[metadata_filter], 1024)
        .map_err(|e| CoreError::Cache(format!("query metadata: {e}")))?;

    let mut metadata_events: Vec<Event> = Vec::with_capacity(metadata_results.len());
    for result in &metadata_results {
        let Ok(note) = ndb.get_note_by_key(&txn, result.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        metadata_events.push(event);
    }

    Ok(build_joined_communities(
        current_pubkey_hex,
        &metadata_events,
        &membership_events,
    ))
}

/// Port of `buildJoinedCommunities` (`web/src/lib/ndk/groups.ts:102-143`).
///
/// Rules (must match the TS line-for-line):
/// - Empty pubkey → `[]`.
/// - Dedup metadata/admin/member events by group id, keeping the newest
///   (greatest `created_at`) per id.
/// - A group is "joined" iff `current_pubkey` appears in the admin list OR
///   the member list for that group id.
/// - Sort output by `name` ascending (`localeCompare` → simple `cmp` on Rust
///   `String`s is close enough for ASCII; matches the webapp for realistic
///   group names).
pub fn build_joined_communities(
    current_pubkey: &str,
    metadata_events: &[Event],
    membership_events: &[Event],
) -> Vec<CommunitySummary> {
    if current_pubkey.trim().is_empty() {
        return Vec::new();
    }

    let metadata_by_id = latest_by_group_id(metadata_events.iter());
    let admin_by_id = latest_by_group_id(
        membership_events
            .iter()
            .filter(|e| e.kind.as_u16() == KIND_GROUP_ADMINS),
    );
    let member_by_id = latest_by_group_id(
        membership_events
            .iter()
            .filter(|e| e.kind.as_u16() == KIND_GROUP_MEMBERS),
    );

    // Membership events are the source of truth for "am I in this group?".
    // Metadata enriches the row but must never gate its existence — a group
    // without a cached kind:39000 should still appear in the list.
    let mut all_group_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for id in admin_by_id.keys().chain(member_by_id.keys()).chain(metadata_by_id.keys()) {
        all_group_ids.insert(id.clone());
    }

    let mut joined: Vec<CommunitySummary> = Vec::new();
    for group_id in all_group_ids {
        let admin_event = admin_by_id.get(&group_id).copied();
        let member_event = member_by_id.get(&group_id).copied();
        let is_admin = event_has_p_tag(admin_event, current_pubkey);
        let is_member = event_has_p_tag(member_event, current_pubkey);
        if !is_admin && !is_member {
            continue;
        }
        let metadata_event = metadata_by_id.get(&group_id).copied();
        match build_summary(&group_id, metadata_event, admin_event, member_event) {
            Ok(summary) => joined.push(summary),
            Err(_) => continue,
        }
    }

    joined.sort_by(|a, b| a.name.cmp(&b.name));
    joined
}

/// Publish a NIP-29 kind:9021 join-request event for `group_id`. Fire-and-
/// forget from the UI's perspective: returns the event id once the relay
/// accepts the event. The user's actual membership state flips when a
/// matching kind:39002 arrives in the ndb stream — the subscription pump
/// delivers that as `MembershipChanged` and the UI promotes the
/// "Join requested" toast to "You're in ✓".
pub async fn publish_join_request(
    runtime: &NostrRuntime,
    group_id: &str,
) -> Result<String, CoreError> {
    let group_id = group_id.trim();
    if group_id.is_empty() {
        return Err(CoreError::InvalidInput("group_id must not be empty".into()));
    }

    let builder = EventBuilder::new(Kind::Custom(KIND_JOIN_REQUEST), "")
        .tags(vec![Tag::parse(vec!["h".to_string(), group_id.to_string()])
            .map_err(|e| CoreError::Other(format!("build h tag: {e}")))?]);

    let client = runtime.client();
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign join request: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish join request: {e}")))?;
    Ok(event.id.to_hex())
}

/// Port of `buildCommunitySummary`. Returns `CoreError::InvalidInput` if the
/// metadata event is missing its `d` tag.
pub fn build_community_summary(event: &Event) -> Result<CommunitySummary, CoreError> {
    let id = first_tag_value(event, "d")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| CoreError::InvalidInput("Group metadata missing d tag.".into()))?;
    build_summary(&id, Some(event), None, None)
}

fn build_summary(
    group_id: &str,
    metadata_event: Option<&Event>,
    admin_event: Option<&Event>,
    member_event: Option<&Event>,
) -> Result<CommunitySummary, CoreError> {
    let id = group_id.to_string();

    // Tag presence, per the webapp's getters (`["public"]` / `["closed"]` etc.)
    let has_public = metadata_event.map(|e| has_marker_tag(e, "public")).unwrap_or(false);
    let has_private = metadata_event.map(|e| has_marker_tag(e, "private")).unwrap_or(false);
    let has_open = metadata_event.map(|e| has_marker_tag(e, "open")).unwrap_or(false);
    let has_closed = metadata_event.map(|e| has_marker_tag(e, "closed")).unwrap_or(false);

    // Deviation from the TS defaults: we use paranoid defaults
    // (`closed` / `private` when both visibility/access tags are missing)
    // per the Phase 2 #2 spec. The webapp defaults to `public`/`open`.
    let visibility = if has_public {
        "public"
    } else if has_private {
        "private"
    } else {
        "private"
    };
    let access = if visibility == "private" {
        "closed"
    } else if has_open {
        "open"
    } else if has_closed {
        "closed"
    } else {
        "closed"
    };

    let name = {
        let raw = metadata_event.map(|e| clean_text(first_tag_value(e, "name"))).unwrap_or_default();
        if raw.is_empty() { id.clone() } else { raw }
    };
    let about = metadata_event.map(|e| clean_text(first_tag_value(e, "about"))).unwrap_or_default();
    let picture = metadata_event.map(|e| clean_text(first_tag_value(e, "picture"))).unwrap_or_default();

    let admin_pubkeys = unique_p_tag_values(admin_event);
    let member_pubkeys = unique_p_tag_values(member_event);

    // Unknown unless we actually have a kind:39002 event to count from.
    // A private group might hide its member list entirely, and absence of a
    // member event in the cache is not evidence that the group is empty.
    let member_count: Option<u64> = member_event.map(|_| member_pubkeys.len() as u64);

    Ok(CommunitySummary {
        id,
        name,
        about,
        picture,
        access: access.to_string(),
        visibility: visibility.to_string(),
        admin_pubkeys,
        member_count,
        relay_url: HIGHLIGHTER_RELAY.to_string(),
        metadata_event_id: metadata_event.map(|e| e.id.to_hex()).unwrap_or_default(),
        created_at: metadata_event.map(|e| e.created_at.as_secs()),
    })
}

/// Keep the newest event per `d` tag (falling back to `h` tag for parity with
/// `groupIdFromEvent` in the TS source).
fn latest_by_group_id<'a, I>(events: I) -> BTreeMap<String, &'a Event>
where
    I: Iterator<Item = &'a Event>,
{
    let mut latest: BTreeMap<String, &'a Event> = BTreeMap::new();
    for event in events {
        let Some(group_id) = group_id_from_event(event) else {
            continue;
        };
        match latest.get(&group_id) {
            Some(existing) if existing.created_at >= event.created_at => {}
            _ => {
                latest.insert(group_id, event);
            }
        }
    }
    latest
}

fn group_id_from_event(event: &Event) -> Option<String> {
    first_tag_value(event, "d")
        .or_else(|| first_tag_value(event, "h"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
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

fn has_marker_tag(event: &Event, name: &str) -> bool {
    event
        .tags
        .iter()
        .any(|tag| tag.as_slice().first().map(String::as_str) == Some(name))
}

fn event_has_p_tag(event: Option<&Event>, pubkey: &str) -> bool {
    let Some(event) = event else {
        return false;
    };
    if pubkey.is_empty() {
        return false;
    }
    event.tags.iter().any(|tag| {
        let slice = tag.as_slice();
        slice.first().map(String::as_str) == Some("p")
            && slice.get(1).map(String::as_str) == Some(pubkey)
    })
}

fn unique_p_tag_values(event: Option<&Event>) -> Vec<String> {
    let Some(event) = event else {
        return Vec::new();
    };
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for tag in event.tags.iter() {
        let slice = tag.as_slice();
        if slice.first().map(String::as_str) != Some("p") {
            continue;
        }
        if let Some(pk) = slice.get(1) {
            let pk = pk.as_str();
            if pk.is_empty() {
                continue;
            }
            if seen.insert(pk.to_string()) {
                out.push(pk.to_string());
            }
        }
    }
    out
}

fn clean_text(v: Option<&str>) -> String {
    v.map(|s| s.trim().to_string()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sign(keys: &Keys, kind: u16, tags: Vec<Tag>, content: &str) -> Event {
        EventBuilder::new(Kind::Custom(kind), content)
            .tags(tags)
            .sign_with_keys(keys)
            .expect("sign")
    }

    fn d(id: &str) -> Tag {
        Tag::identifier(id)
    }

    fn p(pubkey: &PublicKey) -> Tag {
        Tag::public_key(*pubkey)
    }

    fn marker(name: &str) -> Tag {
        Tag::parse(vec![name.to_string()]).expect("marker tag")
    }

    fn named(name: &str, value: &str) -> Tag {
        Tag::parse(vec![name.to_string(), value.to_string()]).expect("named tag")
    }

    #[test]
    fn empty_pubkey_returns_empty() {
        let out = build_joined_communities("", &[], &[]);
        assert!(out.is_empty());
    }

    #[test]
    fn includes_groups_where_user_is_admin_or_member() {
        let me = Keys::generate();
        let other = Keys::generate();

        let meta_a = sign(
            &other,
            39000,
            vec![d("alpha"), named("name", "Alpha"), marker("open"), marker("public")],
            "",
        );
        let meta_b = sign(
            &other,
            39000,
            vec![d("bravo"), named("name", "Bravo"), marker("closed"), marker("private")],
            "",
        );
        let meta_c = sign(&other, 39000, vec![d("charlie"), named("name", "Charlie")], "");

        // admin list for alpha includes `me`
        let admins_a = sign(&other, 39001, vec![d("alpha"), p(&me.public_key())], "");
        // member list for bravo includes `me`
        let members_b = sign(&other, 39002, vec![d("bravo"), p(&me.public_key())], "");
        // charlie has memberships but not for me
        let members_c = sign(&other, 39002, vec![d("charlie"), p(&other.public_key())], "");

        let out = build_joined_communities(
            &me.public_key().to_hex(),
            &[meta_a.clone(), meta_b.clone(), meta_c.clone()],
            &[admins_a, members_b, members_c],
        );

        let ids: Vec<_> = out.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["alpha", "bravo"], "sorted by name, charlie excluded");
        assert_eq!(out[0].name, "Alpha");
        assert_eq!(out[0].admin_pubkeys, vec![me.public_key().to_hex()]);
        assert_eq!(out[0].access, "open");
        assert_eq!(out[0].visibility, "public");
        assert_eq!(out[1].name, "Bravo");
        assert_eq!(out[1].access, "closed");
        assert_eq!(out[1].visibility, "private");
        assert_eq!(out[1].member_count, Some(1));
    }

    #[test]
    fn duplicate_metadata_keeps_newest() {
        let me = Keys::generate();
        let other = Keys::generate();

        // older name = "Old"
        let older = EventBuilder::new(Kind::Custom(39000), "")
            .tags(vec![d("alpha"), named("name", "Old")])
            .custom_created_at(Timestamp::from(1_000))
            .sign_with_keys(&other)
            .expect("sign older");
        // newer name = "New"
        let newer = EventBuilder::new(Kind::Custom(39000), "")
            .tags(vec![d("alpha"), named("name", "New")])
            .custom_created_at(Timestamp::from(2_000))
            .sign_with_keys(&other)
            .expect("sign newer");

        let members = sign(&other, 39002, vec![d("alpha"), p(&me.public_key())], "");

        let out = build_joined_communities(
            &me.public_key().to_hex(),
            &[older, newer],
            &[members],
        );

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "New", "newest metadata event wins");
    }

    #[test]
    fn paranoid_defaults_when_tags_missing() {
        let me = Keys::generate();
        let other = Keys::generate();

        let meta = sign(&other, 39000, vec![d("plain"), named("name", "Plain")], "");
        let members = sign(&other, 39002, vec![d("plain"), p(&me.public_key())], "");

        let out = build_joined_communities(
            &me.public_key().to_hex(),
            &[meta],
            &[members],
        );

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].access, "closed", "access defaults to closed when tag missing");
        assert_eq!(out[0].visibility, "private", "visibility defaults to private when tag missing");
    }

    #[test]
    fn metadata_without_d_tag_is_skipped() {
        let me = Keys::generate();
        let other = Keys::generate();

        // Missing `d` → skipped. Also provide a valid one so sort still works.
        let orphan = sign(&other, 39000, vec![named("name", "Orphan")], "");
        let good_meta = sign(&other, 39000, vec![d("good"), named("name", "Good")], "");
        let good_members = sign(&other, 39002, vec![d("good"), p(&me.public_key())], "");

        let out = build_joined_communities(
            &me.public_key().to_hex(),
            &[orphan, good_meta],
            &[good_members],
        );

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "good");
    }

    #[test]
    fn membership_without_metadata_still_produces_summary() {
        // Regression: prior implementation iterated over metadata_by_id, so a
        // member event without a matching 39000 in cache would silently hide
        // the group — the core cause of the iOS "sometimes no communities"
        // flakiness. Memberships must be the source of truth; metadata
        // enriches the row but never gates its existence.
        let me = Keys::generate();
        let other = Keys::generate();

        let members = sign(&other, 39002, vec![d("alpha"), p(&me.public_key())], "");

        let out = build_joined_communities(&me.public_key().to_hex(), &[], &[members]);

        assert_eq!(out.len(), 1, "missing metadata must not hide membership");
        assert_eq!(out[0].id, "alpha");
        assert_eq!(out[0].name, "alpha", "name falls back to id when 39000 absent");
        assert_eq!(out[0].about, "");
        assert_eq!(out[0].picture, "");
        assert!(out[0].metadata_event_id.is_empty());
        assert_eq!(out[0].created_at, None);
        assert_eq!(out[0].member_count, Some(1));
    }

    #[test]
    fn admin_without_metadata_still_produces_summary() {
        // Same protection as `membership_without_metadata_still_produces_summary`
        // but for the admin (39001) path.
        let me = Keys::generate();
        let other = Keys::generate();

        let admins = sign(&other, 39001, vec![d("solo"), p(&me.public_key())], "");

        let out = build_joined_communities(&me.public_key().to_hex(), &[], &[admins]);

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "solo");
        assert_eq!(out[0].name, "solo");
        assert_eq!(out[0].admin_pubkeys, vec![me.public_key().to_hex()]);
    }

    #[test]
    fn build_community_summary_handles_single_event() {
        let other = Keys::generate();
        let meta = sign(
            &other,
            39000,
            vec![
                d("solo"),
                named("name", "Solo"),
                named("about", "about text"),
                named("picture", "https://example.com/p.png"),
                marker("open"),
                marker("public"),
            ],
            "",
        );
        let summary = build_community_summary(&meta).expect("summary");
        assert_eq!(summary.id, "solo");
        assert_eq!(summary.name, "Solo");
        assert_eq!(summary.about, "about text");
        assert_eq!(summary.picture, "https://example.com/p.png");
        assert_eq!(summary.access, "open");
        assert_eq!(summary.visibility, "public");
        assert_eq!(summary.admin_pubkeys, Vec::<String>::new());
        assert_eq!(summary.member_count, None, "no member event → None");
    }
}
