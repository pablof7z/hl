//! NIP-29 group chat (kind:9 messages tagged `["h", group_id]`).
//!
//! Distinct from `discussions.rs` which handles kind:11 threaded discussions
//! marked `["t","discussion"]`. Chat messages are flat conversational
//! events — no title, no thread markers, just content + an optional
//! `["e", <event-id>, "", "reply"]` for inline replies.

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;
use crate::models::ChatMessageRecord;
use crate::nostr_runtime::NostrRuntime;

/// NIP-29 chat message. Content is the message body; the only required tag
/// is `["h", <group_id>]` so the relay routes it to the room. Optional
/// `["e", <reply-target-id>, "", "reply"]` marks the message as a reply.
pub const KIND_CHAT_MESSAGE: u16 = 9;

/// Query cached chat messages for `group_id`. Sorted ascending by
/// `created_at` so the chat view can stream-append at the bottom without
/// re-sorting on each apply. `limit` caps the most recent N events the
/// caller wants to hydrate (the underlying ndb query orders newest-first
/// internally; we re-sort here).
pub fn query_chat_messages(
    ndb: &Ndb,
    group_id: &str,
    limit: u32,
) -> Result<Vec<ChatMessageRecord>, CoreError> {
    if group_id.trim().is_empty() {
        return Err(CoreError::InvalidInput("group_id must not be empty".into()));
    }

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    // Kind-only filter: nostrdb's #h tag index is unreliable across
    // parameterized-replaceable + chat kinds (same caveat as Room/
    // RoomDiscussions). We re-check the `h` tag in Rust below.
    let filter = NdbFilter::new()
        .kinds([KIND_CHAT_MESSAGE as u64])
        .build();

    let limit_i: i32 = limit.max(1).try_into().unwrap_or(i32::MAX);
    let results = ndb
        .query(&txn, &[filter], limit_i)
        .map_err(|e| CoreError::Cache(format!("query chat messages: {e}")))?;

    let mut records: Vec<ChatMessageRecord> = Vec::with_capacity(results.len());
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        let Some(event_group) = first_tag_value(&event, "h") else {
            continue;
        };
        if event_group != group_id {
            continue;
        }
        if let Some(record) = record_from_event(&event) {
            records.push(record);
        }
    }

    records.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Ok(records)
}

/// Build + sign + publish a kind:9 chat message into `group_id`.
/// `reply_to_event_id`, when set, becomes a marked NIP-10-style `e` tag
/// `["e", <id>, "", "reply"]` so other clients render the threading.
pub async fn publish_chat_message(
    runtime: &NostrRuntime,
    group_id: &str,
    content: &str,
    reply_to_event_id: Option<&str>,
) -> Result<ChatMessageRecord, CoreError> {
    let group_id = group_id.trim();
    if group_id.is_empty() {
        return Err(CoreError::InvalidInput("group_id must not be empty".into()));
    }
    let content = content.trim();
    if content.is_empty() {
        return Err(CoreError::InvalidInput("chat message must not be empty".into()));
    }

    let mut tags: Vec<Tag> = Vec::with_capacity(2);
    tags.push(parse_tag(&["h", group_id])?);
    if let Some(reply_to) = reply_to_event_id {
        let reply_to = reply_to.trim();
        if !reply_to.is_empty() {
            tags.push(parse_tag(&["e", reply_to, "", "reply"])?);
        }
    }

    let builder = EventBuilder::new(Kind::Custom(KIND_CHAT_MESSAGE), content).tags(tags);

    let client = runtime.client();
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign chat message: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish chat message: {e}")))?;

    record_from_event(&event)
        .ok_or_else(|| CoreError::Other("signed chat message failed to parse back".into()))
}

pub(crate) fn record_from_event(event: &Event) -> Option<ChatMessageRecord> {
    if event.kind.as_u16() != KIND_CHAT_MESSAGE {
        return None;
    }
    let group_id = first_tag_value(event, "h")?.to_string();
    let reply_to = reply_target(event);

    Some(ChatMessageRecord {
        event_id: event.id.to_hex(),
        group_id,
        author_pubkey: event.pubkey.to_hex(),
        content: event.content.clone(),
        created_at: event.created_at.as_secs(),
        reply_to_event_id: reply_to,
    })
}

/// Return the `e`-tag value carrying the `reply` marker, or — as a
/// fallback for clients that don't mark — the first `e` tag.
fn reply_target(event: &Event) -> Option<String> {
    let mut first_e: Option<String> = None;
    for tag in event.tags.iter() {
        let s = tag.as_slice();
        if s.first().map(String::as_str) != Some("e") {
            continue;
        }
        let Some(value) = s.get(1).map(String::as_str) else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        // Marker is at index 3 per NIP-10.
        let marker = s.get(3).map(String::as_str).unwrap_or("");
        if marker == "reply" || marker == "root" {
            return Some(value.to_string());
        }
        if first_e.is_none() {
            first_e = Some(value.to_string());
        }
    }
    first_e
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

fn parse_tag(parts: &[&str]) -> Result<Tag, CoreError> {
    Tag::parse(parts.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .map_err(|e| CoreError::Other(format!("build tag: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sign(keys: &Keys, tags: Vec<Tag>, content: &str) -> Event {
        EventBuilder::new(Kind::Custom(KIND_CHAT_MESSAGE), content)
            .tags(tags)
            .sign_with_keys(keys)
            .expect("sign")
    }

    #[test]
    fn record_from_event_extracts_core_fields() {
        let keys = Keys::generate();
        let e = sign(
            &keys,
            vec![parse_tag(&["h", "room-a"]).unwrap()],
            "hi everyone",
        );
        let rec = record_from_event(&e).expect("chat record");
        assert_eq!(rec.group_id, "room-a");
        assert_eq!(rec.author_pubkey, keys.public_key().to_hex());
        assert_eq!(rec.content, "hi everyone");
        assert!(rec.reply_to_event_id.is_none());
        assert!(rec.created_at > 0);
    }

    #[test]
    fn record_from_event_surfaces_reply_marker() {
        let keys = Keys::generate();
        let target_id = "0".repeat(64);
        let e = sign(
            &keys,
            vec![
                parse_tag(&["h", "room-a"]).unwrap(),
                parse_tag(&["e", target_id.as_str(), "", "reply"]).unwrap(),
            ],
            "agreed",
        );
        let rec = record_from_event(&e).expect("chat record");
        assert_eq!(rec.reply_to_event_id.as_deref(), Some(target_id.as_str()));
    }

    #[test]
    fn record_from_event_rejects_messages_without_h_tag() {
        let keys = Keys::generate();
        let e = sign(&keys, vec![], "lonely");
        assert!(record_from_event(&e).is_none());
    }

    #[test]
    fn record_from_event_rejects_wrong_kind() {
        let keys = Keys::generate();
        let e = EventBuilder::new(Kind::Custom(11), "not chat")
            .tags(vec![parse_tag(&["h", "room-a"]).unwrap()])
            .sign_with_keys(&keys)
            .expect("sign");
        assert!(record_from_event(&e).is_none());
    }

    #[test]
    fn query_chat_messages_filters_by_group_and_orders_ascending() {
        // Drive an isolated nostrdb directly — same harness shape as
        // discussions/feedback tests.
        let tmp = tempfile::tempdir().expect("tempdir");
        let cfg = nostrdb::Config::new().set_mapsize(32 * 1024 * 1024);
        let ndb = Ndb::new(tmp.path().to_str().unwrap(), &cfg).expect("ndb");

        let keys = Keys::generate();
        let other_keys = Keys::generate();

        let older = EventBuilder::new(Kind::Custom(KIND_CHAT_MESSAGE), "earlier")
            .tags(vec![parse_tag(&["h", "alpha"]).unwrap()])
            .custom_created_at(Timestamp::from(1_000))
            .sign_with_keys(&keys)
            .expect("sign older");
        let newer = EventBuilder::new(Kind::Custom(KIND_CHAT_MESSAGE), "later")
            .tags(vec![parse_tag(&["h", "alpha"]).unwrap()])
            .custom_created_at(Timestamp::from(2_000))
            .sign_with_keys(&other_keys)
            .expect("sign newer");
        let off_topic = EventBuilder::new(Kind::Custom(KIND_CHAT_MESSAGE), "wrong room")
            .tags(vec![parse_tag(&["h", "bravo"]).unwrap()])
            .custom_created_at(Timestamp::from(1_500))
            .sign_with_keys(&keys)
            .expect("sign off-topic");

        for ev in [&older, &newer, &off_topic] {
            let line = format!("[\"EVENT\",\"sub\",{}]", ev.as_json());
            ndb.process_event(&line).expect("ingest");
        }

        // ndb is async-ingested; poll briefly for visibility.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut out: Vec<ChatMessageRecord> = Vec::new();
        while std::time::Instant::now() < deadline {
            out = query_chat_messages(&ndb, "alpha", 32).expect("query");
            if out.len() == 2 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        assert_eq!(out.len(), 2, "expected exactly the two alpha messages");
        assert_eq!(out[0].content, "earlier", "ascending order: oldest first");
        assert_eq!(out[1].content, "later");
        assert!(out.iter().all(|r| r.group_id == "alpha"));
    }

    #[test]
    fn query_chat_messages_rejects_empty_group_id() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cfg = nostrdb::Config::new().set_mapsize(32 * 1024 * 1024);
        let ndb = Ndb::new(tmp.path().to_str().unwrap(), &cfg).expect("ndb");
        let err = query_chat_messages(&ndb, "  ", 16).unwrap_err();
        assert!(matches!(err, CoreError::InvalidInput(_)));
    }
}
