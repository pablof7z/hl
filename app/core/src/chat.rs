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
pub const CHAT_PAGE_SIZE: u32 = 50;
pub const CHAT_MAX_PAGES: u32 = 20;

#[derive(Debug, Clone, uniffi::Record)]
pub struct ChatMessageRowProjection {
    pub message: ChatMessageRecord,
    pub show_header: bool,
    pub reply_to_message: Option<ChatMessageRecord>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ChatSnapshot {
    pub rows: Vec<ChatMessageRowProjection>,
    pub has_more: bool,
    pub page_count: u32,
    pub has_activity: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ChatPresenceSnapshot {
    pub has_activity: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ChatPublishSnapshot {
    pub snapshot: ChatSnapshot,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ChatComposerProjectionInput {
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ChatComposerProjection {
    pub submit_body: String,
    pub can_send: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ChatLoadMoreProjectionInput {
    pub is_loading_more: bool,
    pub has_more: bool,
    pub current_page_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ChatLoadMoreProjection {
    pub should_load: bool,
    pub requested_page_count: u32,
    pub is_loading_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ChatActivityReloadProjectionInput {
    pub activity_event_id: String,
    pub visible_event_ids: Vec<String>,
    pub current_activity_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ChatActivityReloadProjection {
    pub should_mark_activity: bool,
    pub activity_delta: u64,
    pub activity_revision: u64,
}

/// Chat composer projection. Rust owns draft normalization and send
/// eligibility; native shells render the composer affordance.
pub fn chat_composer_projection(input: ChatComposerProjectionInput) -> ChatComposerProjection {
    let submit_body = input.body.trim().to_string();
    ChatComposerProjection {
        can_send: !submit_body.is_empty(),
        submit_body,
    }
}

pub fn chat_load_more_projection(input: ChatLoadMoreProjectionInput) -> ChatLoadMoreProjection {
    let current_page_count = normalized_page_count(input.current_page_count);
    let should_load =
        !input.is_loading_more && input.has_more && current_page_count < CHAT_MAX_PAGES;
    ChatLoadMoreProjection {
        should_load,
        requested_page_count: if should_load {
            current_page_count.saturating_add(1)
        } else {
            current_page_count
        },
        is_loading_more: should_load,
    }
}

pub fn chat_activity_reload_projection(
    input: ChatActivityReloadProjectionInput,
) -> ChatActivityReloadProjection {
    let activity_event_id = input.activity_event_id.trim();
    let should_mark_activity = !activity_event_id.is_empty()
        && !input
            .visible_event_ids
            .iter()
            .any(|event_id| event_id == activity_event_id);
    ChatActivityReloadProjection {
        should_mark_activity,
        activity_delta: if should_mark_activity { 1 } else { 0 },
        activity_revision: if should_mark_activity {
            input.current_activity_revision.saturating_add(1)
        } else {
            input.current_activity_revision
        },
    }
}

pub fn query_chat_presence_snapshot(ndb: &Ndb, group_id: &str) -> ChatPresenceSnapshot {
    let has_activity = match query_chat_messages(ndb, group_id, 1) {
        Ok(messages) => !messages.is_empty(),
        Err(error) => {
            tracing::warn!(error = %error, "chat presence snapshot failed");
            false
        }
    };
    ChatPresenceSnapshot { has_activity }
}

pub fn query_chat_snapshot(ndb: &Ndb, group_id: &str, page_count: u32) -> ChatSnapshot {
    query_chat_snapshot_with_page_size(ndb, group_id, page_count, CHAT_PAGE_SIZE)
}

fn query_chat_snapshot_with_page_size(
    ndb: &Ndb,
    group_id: &str,
    page_count: u32,
    page_size: u32,
) -> ChatSnapshot {
    let page_count = normalized_page_count(page_count);
    let limit = chat_window_limit(page_count, page_size);
    if group_id.trim().is_empty() || limit == 0 {
        return empty_snapshot(page_count);
    }

    let fetch_limit = limit.saturating_add(1);
    let mut messages = match query_chat_messages(ndb, group_id, fetch_limit as u32) {
        Ok(messages) => messages,
        Err(error) => {
            tracing::warn!(error = %error, "chat snapshot failed");
            return empty_snapshot(page_count);
        }
    };

    let has_older = messages.len() > limit && page_count < CHAT_MAX_PAGES;
    if messages.len() > limit {
        let extra = messages.len() - limit;
        messages.drain(0..extra);
    }

    snapshot_from_messages(messages, page_count, has_older)
}

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
    let group_id = group_id.trim();
    if group_id.is_empty() {
        return Err(CoreError::InvalidInput("group_id must not be empty".into()));
    }

    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let filter = NdbFilter::new()
        .kinds([KIND_CHAT_MESSAGE as u64])
        .tags([group_id], 'h')
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

pub async fn publish_chat_message_snapshot(
    runtime: &NostrRuntime,
    ndb: &Ndb,
    group_id: &str,
    content: &str,
    reply_to_event_id: Option<&str>,
    page_count: u32,
) -> ChatPublishSnapshot {
    let base_snapshot = query_chat_snapshot(ndb, group_id, page_count);
    match publish_chat_message(runtime, group_id, content, reply_to_event_id).await {
        Ok(message) => ChatPublishSnapshot {
            snapshot: snapshot_with_message(base_snapshot, message),
            error: String::new(),
        },
        Err(error) => ChatPublishSnapshot {
            snapshot: base_snapshot,
            error: error.to_string(),
        },
    }
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
        return Err(CoreError::InvalidInput(
            "chat message must not be empty".into(),
        ));
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

fn normalized_page_count(page_count: u32) -> u32 {
    page_count.clamp(1, CHAT_MAX_PAGES)
}

fn chat_window_limit(page_count: u32, page_size: u32) -> usize {
    page_count
        .saturating_mul(page_size)
        .try_into()
        .unwrap_or(usize::MAX)
}

fn empty_snapshot(page_count: u32) -> ChatSnapshot {
    ChatSnapshot {
        rows: Vec::new(),
        has_more: false,
        page_count,
        has_activity: false,
    }
}

fn snapshot_from_messages(
    messages: Vec<ChatMessageRecord>,
    page_count: u32,
    has_more: bool,
) -> ChatSnapshot {
    let rows = chat_rows(messages);
    let has_activity = !rows.is_empty();
    ChatSnapshot {
        rows,
        has_more,
        page_count,
        has_activity,
    }
}

fn chat_rows(messages: Vec<ChatMessageRecord>) -> Vec<ChatMessageRowProjection> {
    let by_event_id: std::collections::HashMap<String, ChatMessageRecord> = messages
        .iter()
        .map(|message| (message.event_id.clone(), message.clone()))
        .collect();

    messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            let show_header = index == 0
                || messages[index - 1].author_pubkey != message.author_pubkey
                || message.created_at > messages[index - 1].created_at.saturating_add(300);
            let reply_to_message = message
                .reply_to_event_id
                .as_ref()
                .and_then(|event_id| by_event_id.get(event_id))
                .cloned();
            ChatMessageRowProjection {
                message: message.clone(),
                show_header,
                reply_to_message,
            }
        })
        .collect()
}

fn snapshot_with_message(snapshot: ChatSnapshot, message: ChatMessageRecord) -> ChatSnapshot {
    let limit = chat_window_limit(snapshot.page_count, CHAT_PAGE_SIZE);
    let mut messages: Vec<ChatMessageRecord> = snapshot
        .rows
        .into_iter()
        .map(|row| row.message)
        .filter(|existing| existing.event_id != message.event_id)
        .collect();

    if messages
        .first()
        .is_some_and(|existing| existing.group_id != message.group_id)
    {
        return snapshot_from_messages(messages, snapshot.page_count, snapshot.has_more);
    }

    messages.push(message);
    messages.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    let overflowed = messages.len() > limit;
    if overflowed {
        let extra = messages.len() - limit;
        messages.drain(0..extra);
    }

    snapshot_from_messages(
        messages,
        snapshot.page_count,
        snapshot.has_more || overflowed,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sign(keys: &Keys, tags: Vec<Tag>, content: &str) -> Event {
        EventBuilder::new(Kind::Custom(KIND_CHAT_MESSAGE), content)
            .tags(tags)
            .sign_with_keys(keys)
            .expect("sign")
    }

    fn ndb_with_events(events: &[&Event]) -> (Ndb, TempDir) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cfg = nostrdb::Config::new().set_mapsize(32 * 1024 * 1024);
        let db_path = tmp.path().to_str().unwrap().to_owned();
        {
            let ndb = Ndb::new(&db_path, &cfg).expect("ndb");
            for event in events {
                let line = format!("[\"EVENT\",\"sub\",{}]", event.as_json());
                ndb.process_event(&line).expect("ingest");
            }
        }
        let ndb = Ndb::new(&db_path, &cfg).expect("reopen ndb");
        (ndb, tmp)
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

        let (ndb, _tmp) = ndb_with_events(&[&older, &newer, &off_topic]);
        let out = query_chat_messages(&ndb, "alpha", 32).expect("query");

        assert_eq!(out.len(), 2, "expected exactly the two alpha messages");
        assert_eq!(out[0].content, "earlier", "ascending order: oldest first");
        assert_eq!(out[1].content, "later");
        assert!(out.iter().all(|r| r.group_id == "alpha"));
    }

    #[test]
    fn chat_snapshot_projects_rows_replies_and_page_policy() {
        let keys = Keys::generate();

        let older = EventBuilder::new(Kind::Custom(KIND_CHAT_MESSAGE), "older")
            .tags(vec![parse_tag(&["h", "alpha"]).unwrap()])
            .custom_created_at(Timestamp::from(1_000))
            .sign_with_keys(&keys)
            .expect("sign older");
        let middle = EventBuilder::new(Kind::Custom(KIND_CHAT_MESSAGE), "middle")
            .tags(vec![parse_tag(&["h", "alpha"]).unwrap()])
            .custom_created_at(Timestamp::from(2_000))
            .sign_with_keys(&keys)
            .expect("sign middle");
        let latest = EventBuilder::new(Kind::Custom(KIND_CHAT_MESSAGE), "latest")
            .tags(vec![
                parse_tag(&["h", "alpha"]).unwrap(),
                parse_tag(&["e", middle.id.to_hex().as_str(), "", "reply"]).unwrap(),
            ])
            .custom_created_at(Timestamp::from(2_120))
            .sign_with_keys(&keys)
            .expect("sign latest");

        let (ndb, _tmp) = ndb_with_events(&[&older, &middle, &latest]);
        let snapshot = query_chat_snapshot_with_page_size(&ndb, "alpha", 1, 2);

        assert!(snapshot.has_activity);
        assert!(snapshot.has_more);
        assert_eq!(snapshot.page_count, 1);
        assert_eq!(snapshot.rows.len(), 2);
        assert_eq!(snapshot.rows[0].message.content, "middle");
        assert_eq!(snapshot.rows[1].message.content, "latest");
        assert!(snapshot.rows[0].show_header);
        assert!(!snapshot.rows[1].show_header);
        assert_eq!(
            snapshot.rows[1]
                .reply_to_message
                .as_ref()
                .map(|message| message.content.as_str()),
            Some("middle")
        );
    }

    #[test]
    fn query_chat_messages_rejects_empty_group_id() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cfg = nostrdb::Config::new().set_mapsize(32 * 1024 * 1024);
        let ndb = Ndb::new(tmp.path().to_str().unwrap(), &cfg).expect("ndb");
        let err = query_chat_messages(&ndb, "  ", 16).unwrap_err();
        assert!(matches!(err, CoreError::InvalidInput(_)));
    }

    #[test]
    fn chat_composer_projection_trims_and_blocks_blank_body() {
        let projection = chat_composer_projection(ChatComposerProjectionInput {
            body: "  hello room  ".into(),
        });

        assert_eq!(projection.submit_body, "hello room");
        assert!(projection.can_send);

        let projection = chat_composer_projection(ChatComposerProjectionInput {
            body: " \n\t ".into(),
        });

        assert_eq!(projection.submit_body, "");
        assert!(!projection.can_send);
    }

    #[test]
    fn chat_load_more_projection_blocks_duplicate_or_exhausted_loads() {
        let ready = chat_load_more_projection(ChatLoadMoreProjectionInput {
            is_loading_more: false,
            has_more: true,
            current_page_count: 1,
        });
        assert!(ready.should_load);
        assert_eq!(ready.requested_page_count, 2);
        assert!(ready.is_loading_more);

        let already_loading = chat_load_more_projection(ChatLoadMoreProjectionInput {
            is_loading_more: true,
            has_more: true,
            current_page_count: 2,
        });
        assert!(!already_loading.should_load);
        assert_eq!(already_loading.requested_page_count, 2);
        assert!(!already_loading.is_loading_more);

        let exhausted = chat_load_more_projection(ChatLoadMoreProjectionInput {
            is_loading_more: false,
            has_more: false,
            current_page_count: 2,
        });
        assert!(!exhausted.should_load);
        assert_eq!(exhausted.requested_page_count, 2);
        assert!(!exhausted.is_loading_more);

        let maxed = chat_load_more_projection(ChatLoadMoreProjectionInput {
            is_loading_more: false,
            has_more: true,
            current_page_count: CHAT_MAX_PAGES,
        });
        assert!(!maxed.should_load);
        assert_eq!(maxed.requested_page_count, CHAT_MAX_PAGES);
        assert!(!maxed.is_loading_more);
    }

    #[test]
    fn chat_activity_reload_projection_marks_only_offscreen_activity() {
        let hidden = chat_activity_reload_projection(ChatActivityReloadProjectionInput {
            activity_event_id: "event-3".into(),
            visible_event_ids: vec!["event-1".into(), "event-2".into()],
            current_activity_revision: 4,
        });
        assert!(hidden.should_mark_activity);
        assert_eq!(hidden.activity_delta, 1);
        assert_eq!(hidden.activity_revision, 5);

        let visible = chat_activity_reload_projection(ChatActivityReloadProjectionInput {
            activity_event_id: "event-2".into(),
            visible_event_ids: vec!["event-1".into(), "event-2".into()],
            current_activity_revision: 4,
        });
        assert!(!visible.should_mark_activity);
        assert_eq!(visible.activity_delta, 0);
        assert_eq!(visible.activity_revision, 4);

        let blank = chat_activity_reload_projection(ChatActivityReloadProjectionInput {
            activity_event_id: " ".into(),
            visible_event_ids: vec!["event-1".into()],
            current_activity_revision: 4,
        });
        assert!(!blank.should_mark_activity);
        assert_eq!(blank.activity_revision, 4);
    }
}
