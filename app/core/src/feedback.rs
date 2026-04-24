//! In-app feedback threads scoped to a single project (a kind:31933 event).
//!
//! Each thread is rooted in a kind:1 note that `a`-tags the project's
//! addressable coordinate and `p`-tags the project's first registered agent.
//! Replies are kind:1 events `e`-tagged to the root (NIP-10 marked `root`).
//! A kind:513 metadata event (with an `e` tag matching the root) carries an
//! optional title/summary/status-label rendered in the conversation list.

use std::collections::HashMap;

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;
use crate::models::{FeedbackEventRecord, FeedbackThreadRecord};
use crate::nostr_runtime::NostrRuntime;

pub const HIGHLIGHTER_PROJECT_COORDINATE: &str =
    "31933:09d48a1a5dbe13404a729634f1d6ba722d40513468dd713c8ea38ca9b7b6f2c7:highlighter";

pub const KIND_FEEDBACK_NOTE: u16 = 1;
pub const KIND_FEEDBACK_THREAD_META: u16 = 513;
pub const KIND_PROJECT_DEFINITION: u16 = 31933;

/// Threads authored by `current_user_pubkey` that `a`-tag `coordinate`. Each
/// returned root is enriched with the latest matching kind:513 metadata
/// (title/summary/status-label) when one exists. Sorted by `last_activity_at`
/// descending — the most recently-updated thread comes first.
pub fn query_threads(
    ndb: &Ndb,
    coordinate: &str,
    current_user_pubkey: &str,
) -> Result<Vec<FeedbackThreadRecord>, CoreError> {
    let coordinate = coordinate.trim();
    let current_user_pubkey = current_user_pubkey.trim();
    if coordinate.is_empty() {
        return Err(CoreError::InvalidInput("coordinate must not be empty".into()));
    }
    if current_user_pubkey.is_empty() {
        return Ok(Vec::new());
    }

    let author = PublicKey::from_hex(current_user_pubkey)
        .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;
    let pk_bytes: [u8; 32] = author.to_bytes();

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let roots_filter = NdbFilter::new()
        .kinds([KIND_FEEDBACK_NOTE as u64])
        .authors([&pk_bytes])
        .tags([coordinate], 'a')
        .build();
    let meta_filter = NdbFilter::new()
        .kinds([KIND_FEEDBACK_THREAD_META as u64])
        .tags([coordinate], 'a')
        .build();

    let root_results = ndb
        .query(&txn, &[roots_filter], 256)
        .map_err(|e| CoreError::Cache(format!("query feedback roots: {e}")))?;
    let meta_results = ndb
        .query(&txn, &[meta_filter], 512)
        .map_err(|e| CoreError::Cache(format!("query feedback meta: {e}")))?;

    let mut roots: Vec<Event> = Vec::with_capacity(root_results.len());
    for r in &root_results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        // Roots are top-level kind:1 events — drop replies (events with a
        // root `e` marker) so a kind:1 reply that happens to also a-tag the
        // project doesn't surface as its own thread.
        if has_root_e_marker(&event) {
            continue;
        }
        roots.push(event);
    }

    let mut latest_meta_by_root: HashMap<String, Event> = HashMap::new();
    for r in &meta_results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        let Some(root_id) = first_tag_value(&event, "e") else {
            continue;
        };
        match latest_meta_by_root.get(root_id) {
            Some(prev) if prev.created_at >= event.created_at => {}
            _ => {
                latest_meta_by_root.insert(root_id.to_string(), event);
            }
        }
    }

    let mut records: Vec<FeedbackThreadRecord> = roots
        .into_iter()
        .map(|root| record_from_root(&root, latest_meta_by_root.get(&root.id.to_hex())))
        .collect();
    records.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    Ok(records)
}

/// Every event in a feedback thread, ordered ascending by `created_at` (chat
/// order). Includes the root note plus every kind:1 `e`-tagged to it,
/// regardless of author — so the project's agent replies appear inline with
/// the user's messages.
pub fn query_thread_events(
    ndb: &Ndb,
    root_event_id: &str,
) -> Result<Vec<FeedbackEventRecord>, CoreError> {
    let root_event_id = root_event_id.trim();
    if root_event_id.is_empty() {
        return Err(CoreError::InvalidInput("root_event_id must not be empty".into()));
    }
    let root_id = EventId::from_hex(root_event_id)
        .map_err(|e| CoreError::InvalidInput(format!("invalid event id: {e}")))?;

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    // ndb's `e` tag index is unreliable in this codebase (see the `h`-tag
    // note in subscriptions.rs::build_ndb_filters), so the replies filter is
    // kind-only and we post-filter by `e` tag in Rust.
    let root_filter = NdbFilter::new()
        .ids([root_id.as_bytes()])
        .build();
    let replies_filter = NdbFilter::new()
        .kinds([KIND_FEEDBACK_NOTE as u64])
        .build();

    let mut events: Vec<Event> = Vec::new();
    let root_results = ndb
        .query(&txn, &[root_filter], 1)
        .map_err(|e| CoreError::Cache(format!("query feedback root: {e}")))?;
    for r in &root_results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        events.push(event);
    }
    let reply_results = ndb
        .query(&txn, &[replies_filter], 4096)
        .map_err(|e| CoreError::Cache(format!("query feedback replies: {e}")))?;
    for r in &reply_results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        if event.id == root_id {
            continue;
        }
        let references_root = event.tags.iter().any(|tag| {
            let s = tag.as_slice();
            s.first().map(String::as_str) == Some("e")
                && s.get(1).map(String::as_str) == Some(root_event_id)
        });
        if !references_root {
            continue;
        }
        events.push(event);
    }

    let mut records: Vec<FeedbackEventRecord> = events
        .iter()
        .map(|e| event_record(e, root_event_id))
        .collect();
    records.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Ok(records)
}

/// Look up the project's kind:31933 by addressable coordinate and return the
/// hex of its first `p` tag. None if the project event isn't cached or has
/// no `p` tags.
pub fn query_first_agent_pubkey(
    ndb: &Ndb,
    coordinate: &str,
) -> Result<Option<String>, CoreError> {
    let (kind, pubkey_hex, d_tag) = parse_coordinate(coordinate)?;
    let project_pubkey = PublicKey::from_hex(&pubkey_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid project pubkey: {e}")))?;
    let pk_bytes: [u8; 32] = project_pubkey.to_bytes();

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let filter = NdbFilter::new()
        .kinds([kind as u64])
        .authors([&pk_bytes])
        .tags([d_tag.as_str()], 'd')
        .build();

    let results = ndb
        .query(&txn, &[filter], 8)
        .map_err(|e| CoreError::Cache(format!("query project event: {e}")))?;

    let mut latest: Option<Event> = None;
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        match &latest {
            Some(prev) if prev.created_at >= event.created_at => {}
            _ => latest = Some(event),
        }
    }
    Ok(latest.and_then(|e| first_tag_value(&e, "p").map(str::to_string)))
}

/// Build, sign and send a kind:1 feedback note. The event always carries an
/// `a` tag for the project coordinate and a `p` tag for the agent. When
/// `parent_event_id` is `Some`, an `["e", root, "", "root"]` marker is added
/// so the reply attaches to an existing thread.
pub async fn publish_note(
    runtime: &NostrRuntime,
    coordinate: &str,
    agent_pubkey: &str,
    parent_event_id: Option<&str>,
    body: &str,
) -> Result<FeedbackEventRecord, CoreError> {
    let coordinate = coordinate.trim();
    let agent_pubkey = agent_pubkey.trim();
    let body = body.trim();
    if coordinate.is_empty() {
        return Err(CoreError::InvalidInput("coordinate must not be empty".into()));
    }
    if agent_pubkey.is_empty() {
        return Err(CoreError::InvalidInput("agent_pubkey must not be empty".into()));
    }
    if body.is_empty() {
        return Err(CoreError::InvalidInput("feedback body must not be empty".into()));
    }
    // Sanity-check pubkey/coordinate before we try to sign.
    PublicKey::from_hex(agent_pubkey)
        .map_err(|e| CoreError::InvalidInput(format!("invalid agent pubkey: {e}")))?;
    parse_coordinate(coordinate)?;
    let parent_root = match parent_event_id.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Some(
            EventId::from_hex(s)
                .map_err(|e| CoreError::InvalidInput(format!("invalid parent event id: {e}")))?,
        ),
        None => None,
    };

    let mut tags: Vec<Tag> = Vec::with_capacity(3);
    tags.push(parse_tag(&["a", coordinate])?);
    tags.push(parse_tag(&["p", agent_pubkey])?);
    if let Some(parent) = parent_root {
        tags.push(parse_tag(&["e", &parent.to_hex(), "", "root"])?);
    }

    let builder = EventBuilder::new(Kind::Custom(KIND_FEEDBACK_NOTE), body).tags(tags);
    let client = runtime.client();
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign feedback note: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish feedback note: {e}")))?;

    let root_id = parent_root
        .map(|id| id.to_hex())
        .unwrap_or_else(|| event.id.to_hex());
    Ok(event_record(&event, &root_id))
}

// --- helpers ---------------------------------------------------------------

fn record_from_root(root: &Event, latest_meta: Option<&Event>) -> FeedbackThreadRecord {
    let title = latest_meta.and_then(|m| {
        first_tag_value(m, "title")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    });
    let summary = latest_meta.and_then(|m| {
        first_tag_value(m, "summary")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    });
    let status_label = latest_meta.and_then(|m| {
        first_tag_value(m, "status-label")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    });
    let root_ts = root.created_at.as_secs();
    let meta_ts = latest_meta.map(|m| m.created_at.as_secs()).unwrap_or(0);
    let last_activity_at = root_ts.max(meta_ts);

    FeedbackThreadRecord {
        root_event_id: root.id.to_hex(),
        author_pubkey: root.pubkey.to_hex(),
        created_at: root_ts,
        last_activity_at,
        title,
        summary,
        status_label,
        preview: trim_preview(&root.content),
    }
}

/// Public shim used by the subscription pump's `build_change` to materialise
/// a delta payload from a streamed event without re-querying ndb.
pub(crate) fn event_record_for_delta(event: &Event, root_event_id: &str) -> FeedbackEventRecord {
    event_record(event, root_event_id)
}

fn event_record(event: &Event, root_event_id: &str) -> FeedbackEventRecord {
    FeedbackEventRecord {
        event_id: event.id.to_hex(),
        root_event_id: root_event_id.to_string(),
        author_pubkey: event.pubkey.to_hex(),
        created_at: event.created_at.as_secs(),
        content: event.content.clone(),
    }
}

fn trim_preview(content: &str) -> String {
    let collapsed: String = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= 140 {
        collapsed
    } else {
        let mut truncated: String = collapsed.chars().take(139).collect();
        truncated.push('…');
        truncated
    }
}

fn has_root_e_marker(event: &Event) -> bool {
    event.tags.iter().any(|tag| {
        let s = tag.as_slice();
        s.first().map(String::as_str) == Some("e")
            && s.get(3).map(String::as_str) == Some("root")
    })
}

fn parse_coordinate(coordinate: &str) -> Result<(u16, String, String), CoreError> {
    let trimmed = coordinate.trim();
    let mut parts = trimmed.splitn(3, ':');
    let kind_str = parts
        .next()
        .ok_or_else(|| CoreError::InvalidInput("coordinate missing kind".into()))?;
    let pubkey = parts
        .next()
        .ok_or_else(|| CoreError::InvalidInput("coordinate missing pubkey".into()))?;
    let d_tag = parts
        .next()
        .ok_or_else(|| CoreError::InvalidInput("coordinate missing d tag".into()))?;
    let kind: u16 = kind_str
        .parse()
        .map_err(|e| CoreError::InvalidInput(format!("coordinate kind not numeric: {e}")))?;
    if pubkey.is_empty() || d_tag.is_empty() {
        return Err(CoreError::InvalidInput(
            "coordinate has empty pubkey or d tag".into(),
        ));
    }
    Ok((kind, pubkey.to_string(), d_tag.to_string()))
}

fn parse_tag(parts: &[&str]) -> Result<Tag, CoreError> {
    Tag::parse(parts.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .map_err(|e| CoreError::Other(format!("build tag: {e}")))
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
    use nostrdb::{Config as NdbConfig, Ndb};
    use tempfile::tempdir;

    const TEST_COORD: &str = "31933:0000000000000000000000000000000000000000000000000000000000000001:demo";

    fn open_ndb() -> (Ndb, tempfile::TempDir) {
        let tmp = tempdir().expect("tempdir");
        let ndb = Ndb::new(
            tmp.path().to_str().unwrap(),
            &NdbConfig::new().set_mapsize(64 * 1024 * 1024),
        )
        .expect("open ndb");
        (ndb, tmp)
    }

    fn process(ndb: &Ndb, event: &Event) {
        let line = format!("[\"EVENT\",\"sub\",{}]", event.as_json());
        ndb.process_event(&line).expect("process event");
    }

    fn sign(keys: &Keys, kind: u16, tags: Vec<Tag>, content: &str, ts: u64) -> Event {
        EventBuilder::new(Kind::Custom(kind), content)
            .tags(tags)
            .custom_created_at(Timestamp::from(ts))
            .sign_with_keys(keys)
            .expect("sign")
    }

    fn tag(parts: &[&str]) -> Tag {
        parse_tag(parts).expect("tag")
    }

    fn flush() {
        std::thread::sleep(std::time::Duration::from_millis(150));
    }

    #[test]
    fn query_threads_filters_by_author_and_coordinate_and_picks_latest_meta() {
        let (ndb, _tmp) = open_ndb();
        let me = Keys::generate();
        let agent = Keys::generate();
        let other = Keys::generate();

        let root = sign(
            &me,
            KIND_FEEDBACK_NOTE,
            vec![
                tag(&["a", TEST_COORD]),
                tag(&["p", &agent.public_key().to_hex()]),
            ],
            "first message",
            1_000,
        );
        // A separate root authored by someone else for the same project — must not surface.
        let other_root = sign(
            &other,
            KIND_FEEDBACK_NOTE,
            vec![
                tag(&["a", TEST_COORD]),
                tag(&["p", &agent.public_key().to_hex()]),
            ],
            "not me",
            1_100,
        );
        // A root authored by the user but for a different project — must not surface.
        let other_project_root = sign(
            &me,
            KIND_FEEDBACK_NOTE,
            vec![
                tag(&[
                    "a",
                    "31933:0000000000000000000000000000000000000000000000000000000000000002:other",
                ]),
                tag(&["p", &agent.public_key().to_hex()]),
            ],
            "wrong project",
            1_050,
        );
        let earlier_meta = sign(
            &agent,
            KIND_FEEDBACK_THREAD_META,
            vec![
                tag(&["a", TEST_COORD]),
                tag(&["e", &root.id.to_hex()]),
                tag(&["title", "Old title"]),
                tag(&["summary", "Old summary"]),
            ],
            "",
            1_500,
        );
        let later_meta = sign(
            &agent,
            KIND_FEEDBACK_THREAD_META,
            vec![
                tag(&["a", TEST_COORD]),
                tag(&["e", &root.id.to_hex()]),
                tag(&["title", "Current title"]),
                tag(&["summary", "Current summary"]),
                tag(&["status-label", "Open"]),
            ],
            "",
            2_000,
        );

        for e in [&root, &other_root, &other_project_root, &earlier_meta, &later_meta] {
            process(&ndb, e);
        }
        flush();

        let threads = query_threads(&ndb, TEST_COORD, &me.public_key().to_hex())
            .expect("query_threads");
        assert_eq!(threads.len(), 1, "only the user's root for this project");
        let t = &threads[0];
        assert_eq!(t.root_event_id, root.id.to_hex());
        assert_eq!(t.title.as_deref(), Some("Current title"));
        assert_eq!(t.summary.as_deref(), Some("Current summary"));
        assert_eq!(t.status_label.as_deref(), Some("Open"));
        assert_eq!(t.last_activity_at, 2_000);
        assert_eq!(t.preview, "first message");
    }

    #[test]
    fn query_threads_drops_replies_so_they_dont_appear_as_their_own_thread() {
        let (ndb, _tmp) = open_ndb();
        let me = Keys::generate();
        let agent = Keys::generate();

        let root = sign(
            &me,
            KIND_FEEDBACK_NOTE,
            vec![
                tag(&["a", TEST_COORD]),
                tag(&["p", &agent.public_key().to_hex()]),
            ],
            "root msg",
            1_000,
        );
        let reply = sign(
            &me,
            KIND_FEEDBACK_NOTE,
            vec![
                tag(&["a", TEST_COORD]),
                tag(&["p", &agent.public_key().to_hex()]),
                tag(&["e", &root.id.to_hex(), "", "root"]),
            ],
            "follow up from me",
            1_500,
        );
        process(&ndb, &root);
        process(&ndb, &reply);
        flush();

        let threads = query_threads(&ndb, TEST_COORD, &me.public_key().to_hex())
            .expect("query_threads");
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].root_event_id, root.id.to_hex());
    }

    #[test]
    fn query_thread_events_returns_root_plus_every_e_tagged_reply_regardless_of_author() {
        let (ndb, _tmp) = open_ndb();
        let me = Keys::generate();
        let agent = Keys::generate();

        let root = sign(
            &me,
            KIND_FEEDBACK_NOTE,
            vec![
                tag(&["a", TEST_COORD]),
                tag(&["p", &agent.public_key().to_hex()]),
            ],
            "root",
            1_000,
        );
        let agent_reply = sign(
            &agent,
            KIND_FEEDBACK_NOTE,
            vec![
                tag(&["a", TEST_COORD]),
                tag(&["e", &root.id.to_hex(), "", "root"]),
            ],
            "agent says hi",
            1_500,
        );
        let user_followup = sign(
            &me,
            KIND_FEEDBACK_NOTE,
            vec![
                tag(&["a", TEST_COORD]),
                tag(&["p", &agent.public_key().to_hex()]),
                tag(&["e", &root.id.to_hex(), "", "root"]),
            ],
            "thanks",
            2_000,
        );
        let unrelated = sign(
            &agent,
            KIND_FEEDBACK_NOTE,
            vec![tag(&["e", &Keys::generate().public_key().to_hex(), "", "root"])],
            "different thread",
            2_500,
        );
        process(&ndb, &root);
        process(&ndb, &agent_reply);
        process(&ndb, &user_followup);
        process(&ndb, &unrelated);
        flush();

        let events = query_thread_events(&ndb, &root.id.to_hex()).expect("query_thread_events");
        let order: Vec<&str> = events.iter().map(|e| e.content.as_str()).collect();
        assert_eq!(order, vec!["root", "agent says hi", "thanks"]);
    }

    #[test]
    fn query_first_agent_pubkey_returns_first_p_tag_of_latest_project_event() {
        let (ndb, _tmp) = open_ndb();
        // We need keys that match the coordinate's pubkey, so derive the
        // coordinate from the actual key pair.
        let project = Keys::generate();
        let coord = format!(
            "{}:{}:{}",
            KIND_PROJECT_DEFINITION,
            project.public_key().to_hex(),
            "demo"
        );
        let agent_a = Keys::generate();
        let agent_b = Keys::generate();

        let project_event = sign(
            &project,
            KIND_PROJECT_DEFINITION,
            vec![
                tag(&["d", "demo"]),
                tag(&["title", "Demo"]),
                tag(&["p", &agent_a.public_key().to_hex()]),
                tag(&["p", &agent_b.public_key().to_hex()]),
            ],
            "",
            1_000,
        );
        process(&ndb, &project_event);
        flush();

        let agent = query_first_agent_pubkey(&ndb, &coord)
            .expect("query")
            .expect("agent present");
        assert_eq!(agent, agent_a.public_key().to_hex());
    }

    #[test]
    fn parse_coordinate_rejects_bad_input() {
        assert!(parse_coordinate("nope").is_err());
        assert!(parse_coordinate("31933::demo").is_err());
        assert!(parse_coordinate("abc:pk:demo").is_err());
        let ok = parse_coordinate("31933:abc:demo").unwrap();
        assert_eq!(ok.0, 31933);
        assert_eq!(ok.1, "abc");
        assert_eq!(ok.2, "demo");
    }

    #[test]
    fn trim_preview_collapses_whitespace_and_truncates() {
        assert_eq!(trim_preview("hello   world"), "hello world");
        let long: String = std::iter::repeat('x').take(200).collect();
        let out = trim_preview(&long);
        assert_eq!(out.chars().count(), 140);
        assert!(out.ends_with('…'));
    }
}
