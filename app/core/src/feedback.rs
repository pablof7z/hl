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
use crate::models::{FeedbackEventRecord, FeedbackThreadRecord, ProfileMetadata};
use crate::nostr_runtime::NostrRuntime;
use crate::relays::feedback_relay;

pub const HIGHLIGHTER_PROJECT_COORDINATE: &str =
    "31933:09d48a1a5dbe13404a729634f1d6ba722d40513468dd713c8ea38ca9b7b6f2c7:highlighter";

pub const KIND_FEEDBACK_NOTE: u16 = 1;
pub const KIND_FEEDBACK_THREAD_META: u16 = 513;
pub const KIND_PROJECT_DEFINITION: u16 = 31933;
pub const FEEDBACK_THREAD_LIMIT: i32 = 256;
pub const FEEDBACK_THREAD_META_LIMIT: i32 = 512;
pub const FEEDBACK_THREAD_EVENT_LIMIT: i32 = 4096;

#[derive(Debug, Clone, uniffi::Record)]
pub struct FeedbackThreadsSnapshot {
    pub threads: Vec<FeedbackThreadRecord>,
    pub error: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FeedbackMessageRowProjection {
    pub event: FeedbackEventRecord,
    pub show_header: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FeedbackThreadSnapshot {
    pub rows: Vec<FeedbackMessageRowProjection>,
    pub error: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FeedbackRootPublishSnapshot {
    pub snapshot: FeedbackThreadsSnapshot,
    pub error: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FeedbackReplyPublishSnapshot {
    pub snapshot: FeedbackThreadSnapshot,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FeedbackComposerProjectionInput {
    pub body: String,
    pub is_publishing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FeedbackComposerProjection {
    pub submit_body: String,
    pub can_send: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FeedbackPublishResultInput {
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FeedbackPublishResultProjection {
    pub did_publish: bool,
    pub error_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FeedbackSnapshotApplyInput {
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FeedbackSnapshotApplyProjection {
    pub should_apply_snapshot: bool,
    pub load_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FeedbackThreadPresentationProjection {
    pub navigation_title: String,
    pub row_title: String,
    pub row_secondary_text: Option<String>,
    pub detail_summary: Option<String>,
    pub status_label: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FeedbackMessagePresentationInput {
    pub event: FeedbackEventRecord,
    pub show_header: bool,
    pub current_user_pubkey: Option<String>,
    pub profile: Option<ProfileMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FeedbackMessagePresentationProjection {
    pub is_from_me: bool,
    pub show_header: bool,
    pub display_name: String,
    pub display_initial: String,
    pub picture_url: String,
}

pub fn feedback_composer_projection(
    input: FeedbackComposerProjectionInput,
) -> FeedbackComposerProjection {
    let submit_body = input.body.trim().to_string();
    FeedbackComposerProjection {
        can_send: !submit_body.is_empty() && !input.is_publishing,
        submit_body,
    }
}

pub fn feedback_publish_result_projection(
    input: FeedbackPublishResultInput,
) -> FeedbackPublishResultProjection {
    let error_message = input.error.trim().to_string();
    FeedbackPublishResultProjection {
        did_publish: error_message.is_empty(),
        error_message,
    }
}

pub fn feedback_snapshot_apply_projection(
    input: FeedbackSnapshotApplyInput,
) -> FeedbackSnapshotApplyProjection {
    let error_message = input.error.trim().to_string();
    let load_error = if error_message.is_empty() {
        None
    } else {
        Some(error_message)
    };
    FeedbackSnapshotApplyProjection {
        should_apply_snapshot: load_error.is_none(),
        load_error,
    }
}

pub fn feedback_message_presentation(
    input: FeedbackMessagePresentationInput,
) -> FeedbackMessagePresentationProjection {
    let is_from_me = input
        .current_user_pubkey
        .as_deref()
        .is_some_and(|pubkey| pubkey == input.event.author_pubkey);
    let display_name = profile_display_name(input.profile.as_ref(), &input.event.author_pubkey);
    let display_initial = display_initial(&display_name);
    let picture_url = input
        .profile
        .as_ref()
        .map(|profile| profile.picture.clone())
        .unwrap_or_default();

    FeedbackMessagePresentationProjection {
        is_from_me,
        show_header: input.show_header,
        display_name,
        display_initial,
        picture_url,
    }
}

pub fn feedback_thread_presentation(
    thread: FeedbackThreadRecord,
) -> FeedbackThreadPresentationProjection {
    let row_title = thread
        .title
        .clone()
        .unwrap_or_else(|| thread.preview.clone());
    let navigation_title = thread.title.clone().unwrap_or_else(|| "Feedback".into());
    let row_secondary_text = renderable_text(thread.summary.clone()).or_else(|| {
        if thread.title.is_some() && !thread.preview.is_empty() {
            Some(thread.preview.clone())
        } else {
            None
        }
    });
    let detail_summary = renderable_text(thread.summary);
    let status_label = renderable_text(thread.status_label);

    FeedbackThreadPresentationProjection {
        navigation_title,
        row_title,
        row_secondary_text,
        detail_summary,
        status_label,
    }
}

pub fn query_threads_snapshot(
    ndb: &Ndb,
    coordinate: &str,
    current_user_pubkey: Option<&str>,
) -> FeedbackThreadsSnapshot {
    let current_user_pubkey = current_user_pubkey.unwrap_or_default().trim();
    if current_user_pubkey.is_empty() {
        return FeedbackThreadsSnapshot {
            threads: Vec::new(),
            error: String::new(),
        };
    }

    match query_threads(ndb, coordinate, current_user_pubkey) {
        Ok(threads) => FeedbackThreadsSnapshot {
            threads,
            error: String::new(),
        },
        Err(error) => FeedbackThreadsSnapshot {
            threads: Vec::new(),
            error: error.to_string(),
        },
    }
}

pub fn query_thread_snapshot(ndb: &Ndb, root_event_id: &str) -> FeedbackThreadSnapshot {
    match query_thread_events(ndb, root_event_id) {
        Ok(events) => snapshot_from_events(events, String::new()),
        Err(error) => FeedbackThreadSnapshot {
            rows: Vec::new(),
            error: error.to_string(),
        },
    }
}

pub fn threads_snapshot_with_root(
    snapshot: FeedbackThreadsSnapshot,
    root_event: &FeedbackEventRecord,
) -> FeedbackThreadsSnapshot {
    FeedbackThreadsSnapshot {
        threads: optimistically_insert_root_thread(&snapshot.threads, root_event),
        error: snapshot.error,
    }
}

pub fn thread_snapshot_with_event(
    snapshot: FeedbackThreadSnapshot,
    event: &FeedbackEventRecord,
) -> FeedbackThreadSnapshot {
    let events: Vec<FeedbackEventRecord> = snapshot.rows.into_iter().map(|row| row.event).collect();
    FeedbackThreadSnapshot {
        rows: rows_for_events(upsert_thread_event(&events, event)),
        error: snapshot.error,
    }
}

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
        return Err(CoreError::InvalidInput(
            "coordinate must not be empty".into(),
        ));
    }
    if current_user_pubkey.is_empty() {
        return Ok(Vec::new());
    }

    let author = PublicKey::from_hex(current_user_pubkey)
        .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;
    let pk_bytes: [u8; 32] = author.to_bytes();

    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

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
        .query(&txn, &[roots_filter], FEEDBACK_THREAD_LIMIT)
        .map_err(|e| CoreError::Cache(format!("query feedback roots: {e}")))?;
    let meta_results = ndb
        .query(&txn, &[meta_filter], FEEDBACK_THREAD_META_LIMIT)
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
        return Err(CoreError::InvalidInput(
            "root_event_id must not be empty".into(),
        ));
    }
    let root_id = EventId::from_hex(root_event_id)
        .map_err(|e| CoreError::InvalidInput(format!("invalid event id: {e}")))?;

    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    // ndb's `e` tag index is unreliable in this codebase (see the `h`-tag
    // note in subscriptions.rs::build_ndb_filters), so the replies filter is
    // kind-only and we post-filter by `e` tag in Rust.
    let root_filter = NdbFilter::new().ids([root_id.as_bytes()]).build();
    let replies_filter = NdbFilter::new().kinds([KIND_FEEDBACK_NOTE as u64]).build();

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
        .query(&txn, &[replies_filter], FEEDBACK_THREAD_EVENT_LIMIT)
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
pub fn query_first_agent_pubkey(ndb: &Ndb, coordinate: &str) -> Result<Option<String>, CoreError> {
    let (kind, pubkey_hex, d_tag) = parse_coordinate(coordinate)?;
    let project_pubkey = PublicKey::from_hex(&pubkey_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid project pubkey: {e}")))?;
    let pk_bytes: [u8; 32] = project_pubkey.to_bytes();

    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

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
/// `a` tag for the project coordinate; a `p` tag is added when an
/// `agent_pubkey` is supplied (`None` is allowed when the project event isn't
/// cached yet — the note still ships, the agent will just discover it via
/// the `a`-tag subscription). When `parent_event_id` is `Some`, an
/// `["e", root, "", "root"]` marker is added so the reply attaches to an
/// existing thread. Published only to the feedback relay.
pub async fn publish_note(
    runtime: &NostrRuntime,
    coordinate: &str,
    agent_pubkey: Option<&str>,
    parent_event_id: Option<&str>,
    body: &str,
) -> Result<FeedbackEventRecord, CoreError> {
    let coordinate = coordinate.trim();
    let body = body.trim();
    if coordinate.is_empty() {
        return Err(CoreError::InvalidInput(
            "coordinate must not be empty".into(),
        ));
    }
    if body.is_empty() {
        return Err(CoreError::InvalidInput(
            "feedback body must not be empty".into(),
        ));
    }
    parse_coordinate(coordinate)?;
    let agent_pubkey = match agent_pubkey.map(str::trim).filter(|s| !s.is_empty()) {
        Some(pk) => {
            PublicKey::from_hex(pk)
                .map_err(|e| CoreError::InvalidInput(format!("invalid agent pubkey: {e}")))?;
            Some(pk.to_string())
        }
        None => None,
    };
    let parent_root = match parent_event_id.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Some(
            EventId::from_hex(s)
                .map_err(|e| CoreError::InvalidInput(format!("invalid parent event id: {e}")))?,
        ),
        None => None,
    };

    let mut tags: Vec<Tag> = Vec::with_capacity(3);
    tags.push(parse_tag(&["a", coordinate])?);
    if let Some(agent) = &agent_pubkey {
        tags.push(parse_tag(&["p", agent])?);
    }
    if let Some(parent) = parent_root {
        tags.push(parse_tag(&["e", &parent.to_hex(), "", "root"])?);
    }

    let builder = EventBuilder::new(Kind::Custom(KIND_FEEDBACK_NOTE), body).tags(tags);
    let client = runtime.client();
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign feedback note: {e}")))?;

    ensure_feedback_relay(client).await;
    let relay = feedback_relay();
    client
        .send_event_to([relay], &event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish feedback note: {e}")))?;

    let root_id = parent_root
        .map(|id| id.to_hex())
        .unwrap_or_else(|| event.id.to_hex());
    Ok(event_record(&event, &root_id))
}

/// Idempotently add + connect the feedback relay to the runtime's relay pool
/// before any feedback publish/subscribe runs. Errors are logged but never
/// propagated — `add_relay` returns `Ok(false)` if the relay is already
/// known, and `connect_relay` is fine to call again on a connected relay.
pub async fn ensure_feedback_relay(client: &Client) {
    let relay = feedback_relay();
    if let Err(e) = client.add_relay(relay).await {
        tracing::warn!(relay = %relay, error = %e, "feedback relay add_relay");
    }
    if let Err(e) = client.connect_relay(relay).await {
        tracing::warn!(relay = %relay, error = %e, "feedback relay connect");
    }
}

/// Insert a freshly-published root event into a thread list before the relay
/// echo is indexed. Rust owns the root-only guard, preview policy, dedupe, and
/// newest-activity ordering.
pub fn optimistically_insert_root_thread(
    threads: &[FeedbackThreadRecord],
    root_event: &FeedbackEventRecord,
) -> Vec<FeedbackThreadRecord> {
    let mut out = threads.to_vec();
    if root_event.root_event_id != root_event.event_id {
        out.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
        return out;
    }
    if !out
        .iter()
        .any(|thread| thread.root_event_id == root_event.event_id)
    {
        out.push(FeedbackThreadRecord {
            root_event_id: root_event.event_id.clone(),
            author_pubkey: root_event.author_pubkey.clone(),
            created_at: root_event.created_at,
            last_activity_at: root_event.created_at,
            title: None,
            summary: None,
            status_label: None,
            preview: trim_preview(&root_event.content),
        });
    }
    out.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    out
}

/// Upsert a streamed feedback-thread event into a bounded view snapshot.
/// Rust owns replacement identity and oldest-first chat ordering.
pub fn upsert_thread_event(
    events: &[FeedbackEventRecord],
    event: &FeedbackEventRecord,
) -> Vec<FeedbackEventRecord> {
    let mut out = Vec::with_capacity(events.len() + 1);
    let mut replaced = false;
    for existing in events {
        if existing.event_id == event.event_id {
            out.push(event.clone());
            replaced = true;
        } else {
            out.push(existing.clone());
        }
    }
    if !replaced {
        out.push(event.clone());
    }
    out.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    out
}

// --- helpers ---------------------------------------------------------------

fn snapshot_from_events(events: Vec<FeedbackEventRecord>, error: String) -> FeedbackThreadSnapshot {
    FeedbackThreadSnapshot {
        rows: rows_for_events(events),
        error,
    }
}

fn rows_for_events(events: Vec<FeedbackEventRecord>) -> Vec<FeedbackMessageRowProjection> {
    events
        .iter()
        .enumerate()
        .map(|(index, event)| {
            let show_header = index == 0
                || events[index - 1].author_pubkey != event.author_pubkey
                || event.created_at > events[index - 1].created_at.saturating_add(300);
            FeedbackMessageRowProjection {
                event: event.clone(),
                show_header,
            }
        })
        .collect()
}

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

fn renderable_text(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}

fn profile_display_name(profile: Option<&ProfileMetadata>, fallback_pubkey: &str) -> String {
    if let Some(profile) = profile {
        if !profile.display_name.is_empty() {
            return profile.display_name.clone();
        }
        if !profile.name.is_empty() {
            return profile.name.clone();
        }
    }
    fallback_pubkey.chars().take(8).collect()
}

fn display_initial(display_name: &str) -> String {
    display_name
        .chars()
        .next()
        .map(|ch| ch.to_uppercase().collect())
        .unwrap_or_default()
}

fn has_root_e_marker(event: &Event) -> bool {
    event.tags.iter().any(|tag| {
        let s = tag.as_slice();
        s.first().map(String::as_str) == Some("e") && s.get(3).map(String::as_str) == Some("root")
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
    use tempfile::{tempdir, TempDir};

    const TEST_COORD: &str =
        "31933:0000000000000000000000000000000000000000000000000000000000000001:demo";

    fn open_ndb() -> (Ndb, TempDir) {
        let tmp = tempdir().expect("tempdir");
        let ndb = Ndb::new(
            tmp.path().to_str().unwrap(),
            &NdbConfig::new().set_mapsize(64 * 1024 * 1024),
        )
        .expect("open ndb");
        (ndb, tmp)
    }

    fn ndb_with_events(events: &[&Event]) -> (Ndb, TempDir) {
        let tmp = tempdir().expect("tempdir");
        let cfg = NdbConfig::new().set_mapsize(64 * 1024 * 1024);
        let db_path = tmp.path().to_str().unwrap().to_owned();
        {
            let ndb = Ndb::new(&db_path, &cfg).expect("open ndb");
            for event in events {
                let line = format!("[\"EVENT\",\"sub\",{}]", event.as_json());
                ndb.process_event(&line).expect("process event");
            }
        }
        let ndb = Ndb::new(&db_path, &cfg).expect("reopen ndb");
        (ndb, tmp)
    }

    fn ndb_with_json_events(json_events: &[&str]) -> (Ndb, TempDir) {
        let tmp = tempdir().expect("tempdir");
        let cfg = NdbConfig::new().set_mapsize(64 * 1024 * 1024);
        let db_path = tmp.path().to_str().unwrap().to_owned();
        {
            let ndb = Ndb::new(&db_path, &cfg).expect("open ndb");
            for json in json_events {
                let line = format!("[\"EVENT\",\"sub\",{}]", json);
                ndb.process_event(&line).expect("process event");
            }
        }
        let ndb = Ndb::new(&db_path, &cfg).expect("reopen ndb");
        (ndb, tmp)
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

    #[test]
    fn query_threads_filters_by_author_and_coordinate_and_picks_latest_meta() {
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

        let (ndb, _tmp) = ndb_with_events(&[
            &root,
            &other_root,
            &other_project_root,
            &earlier_meta,
            &later_meta,
        ]);

        let threads =
            query_threads(&ndb, TEST_COORD, &me.public_key().to_hex()).expect("query_threads");
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
        let (ndb, _tmp) = ndb_with_events(&[&root, &reply]);

        let threads =
            query_threads(&ndb, TEST_COORD, &me.public_key().to_hex()).expect("query_threads");
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].root_event_id, root.id.to_hex());
    }

    #[test]
    fn query_thread_events_returns_root_plus_every_e_tagged_reply_regardless_of_author() {
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
            vec![tag(&[
                "e",
                &Keys::generate().public_key().to_hex(),
                "",
                "root",
            ])],
            "different thread",
            2_500,
        );
        let (ndb, _tmp) = ndb_with_events(&[&root, &agent_reply, &user_followup, &unrelated]);

        let events = query_thread_events(&ndb, &root.id.to_hex()).expect("query_thread_events");
        let order: Vec<&str> = events.iter().map(|e| e.content.as_str()).collect();
        assert_eq!(order, vec!["root", "agent says hi", "thanks"]);
    }

    #[test]
    fn query_thread_snapshot_projects_message_rows() {
        let me = Keys::generate();
        let agent = Keys::generate();

        let root = sign(
            &me,
            KIND_FEEDBACK_NOTE,
            vec![tag(&["a", TEST_COORD])],
            "root",
            1_000,
        );
        let grouped_reply = sign(
            &me,
            KIND_FEEDBACK_NOTE,
            vec![tag(&["e", &root.id.to_hex(), "", "root"])],
            "grouped",
            1_050,
        );
        let agent_reply = sign(
            &agent,
            KIND_FEEDBACK_NOTE,
            vec![tag(&["e", &root.id.to_hex(), "", "root"])],
            "agent",
            1_100,
        );
        let (ndb, _tmp) = ndb_with_events(&[&root, &grouped_reply, &agent_reply]);

        let snapshot = query_thread_snapshot(&ndb, &root.id.to_hex());

        assert!(snapshot.error.is_empty());
        assert_eq!(
            snapshot
                .rows
                .iter()
                .map(|row| row.event.content.as_str())
                .collect::<Vec<_>>(),
            vec!["root", "grouped", "agent"]
        );
        assert!(snapshot.rows[0].show_header);
        assert!(!snapshot.rows[1].show_header);
        assert!(snapshot.rows[2].show_header);
    }

    #[test]
    fn query_snapshot_returns_error_state_for_invalid_inputs() {
        let (ndb, _tmp) = open_ndb();

        let threads = query_threads_snapshot(&ndb, "bad-coordinate", Some("also-bad-pubkey"));
        let events = query_thread_snapshot(&ndb, "not-an-event-id");

        assert!(threads.threads.is_empty());
        assert!(!threads.error.is_empty());
        assert!(events.rows.is_empty());
        assert!(!events.error.is_empty());
    }

    #[test]
    fn query_first_agent_pubkey_returns_first_p_tag_of_latest_project_event() {
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
        let (ndb, _tmp) = ndb_with_events(&[&project_event]);

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
        let long = "x".repeat(200);
        let out = trim_preview(&long);
        assert_eq!(out.chars().count(), 140);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn feedback_composer_projection_trims_submit_body_and_allows_send() {
        let projection = feedback_composer_projection(FeedbackComposerProjectionInput {
            body: "  hello feedback  \n".into(),
            is_publishing: false,
        });

        assert_eq!(projection.submit_body, "hello feedback");
        assert!(projection.can_send);
    }

    #[test]
    fn feedback_composer_projection_blocks_blank_or_publishing_body() {
        let blank = feedback_composer_projection(FeedbackComposerProjectionInput {
            body: "  \n\t ".into(),
            is_publishing: false,
        });
        let publishing = feedback_composer_projection(FeedbackComposerProjectionInput {
            body: "ready".into(),
            is_publishing: true,
        });

        assert_eq!(blank.submit_body, "");
        assert!(!blank.can_send);
        assert_eq!(publishing.submit_body, "ready");
        assert!(!publishing.can_send);
    }

    #[test]
    fn feedback_publish_result_projection_classifies_success_and_error() {
        let success = feedback_publish_result_projection(FeedbackPublishResultInput {
            error: String::new(),
        });
        assert!(success.did_publish);
        assert_eq!(success.error_message, "");

        let failed = feedback_publish_result_projection(FeedbackPublishResultInput {
            error: " publish failed ".into(),
        });
        assert!(!failed.did_publish);
        assert_eq!(failed.error_message, "publish failed");
    }

    #[test]
    fn feedback_snapshot_apply_projection_blocks_error_snapshots() {
        let success = feedback_snapshot_apply_projection(FeedbackSnapshotApplyInput {
            error: String::new(),
        });
        assert!(success.should_apply_snapshot);
        assert_eq!(success.load_error, None);

        let failed = feedback_snapshot_apply_projection(FeedbackSnapshotApplyInput {
            error: " refresh failed ".into(),
        });
        assert!(!failed.should_apply_snapshot);
        assert_eq!(failed.load_error.as_deref(), Some("refresh failed"));
    }

    #[test]
    fn feedback_thread_presentation_uses_title_summary_status_and_detail_title() {
        let projection = feedback_thread_presentation(FeedbackThreadRecord {
            root_event_id: "root".into(),
            author_pubkey: "pubkey".into(),
            created_at: 1,
            last_activity_at: 2,
            title: Some("Title".into()),
            summary: Some("Summary".into()),
            status_label: Some("waiting".into()),
            preview: "Preview".into(),
        });

        assert_eq!(projection.navigation_title, "Title");
        assert_eq!(projection.row_title, "Title");
        assert_eq!(projection.row_secondary_text.as_deref(), Some("Summary"));
        assert_eq!(projection.detail_summary.as_deref(), Some("Summary"));
        assert_eq!(projection.status_label.as_deref(), Some("waiting"));
    }

    #[test]
    fn feedback_thread_presentation_preserves_preview_fallbacks() {
        let no_title = feedback_thread_presentation(FeedbackThreadRecord {
            root_event_id: "root".into(),
            author_pubkey: "pubkey".into(),
            created_at: 1,
            last_activity_at: 2,
            title: None,
            summary: None,
            status_label: Some(String::new()),
            preview: "Preview".into(),
        });
        let titled_without_summary = feedback_thread_presentation(FeedbackThreadRecord {
            root_event_id: "root".into(),
            author_pubkey: "pubkey".into(),
            created_at: 1,
            last_activity_at: 2,
            title: Some("Title".into()),
            summary: Some(String::new()),
            status_label: None,
            preview: "Preview".into(),
        });

        assert_eq!(no_title.navigation_title, "Feedback");
        assert_eq!(no_title.row_title, "Preview");
        assert_eq!(no_title.row_secondary_text, None);
        assert_eq!(no_title.detail_summary, None);
        assert_eq!(no_title.status_label, None);
        assert_eq!(
            titled_without_summary.row_secondary_text.as_deref(),
            Some("Preview")
        );
    }

    #[test]
    fn feedback_message_presentation_marks_current_user_and_profile_name() {
        let event = feedback_event("event", "root", 100, "body");
        let projection = feedback_message_presentation(FeedbackMessagePresentationInput {
            event: FeedbackEventRecord {
                author_pubkey: "me".into(),
                ..event
            },
            show_header: true,
            current_user_pubkey: Some("me".into()),
            profile: Some(profile("alice", "Alice Smith", "https://example.com/a.png")),
        });

        assert!(projection.is_from_me);
        assert!(projection.show_header);
        assert_eq!(projection.display_name, "Alice Smith");
        assert_eq!(projection.display_initial, "A");
        assert_eq!(projection.picture_url, "https://example.com/a.png");
    }

    #[test]
    fn feedback_message_presentation_groups_adjacent_messages() {
        let previous = FeedbackEventRecord {
            author_pubkey: "agent".into(),
            ..feedback_event("previous", "root", 100, "previous")
        };
        let current = FeedbackEventRecord {
            author_pubkey: "agent".into(),
            ..feedback_event("current", "root", 399, "current")
        };
        let later = FeedbackEventRecord {
            author_pubkey: "agent".into(),
            ..feedback_event("later", "root", 401, "later")
        };

        let grouped = feedback_message_presentation(FeedbackMessagePresentationInput {
            event: current.clone(),
            show_header: rows_for_events(vec![previous.clone(), current])[1].show_header,
            current_user_pubkey: Some("me".into()),
            profile: Some(profile("agent-name", "", "")),
        });
        let separated = feedback_message_presentation(FeedbackMessagePresentationInput {
            event: later.clone(),
            show_header: rows_for_events(vec![previous, later])[1].show_header,
            current_user_pubkey: Some("me".into()),
            profile: None,
        });

        assert!(!grouped.is_from_me);
        assert!(!grouped.show_header);
        assert_eq!(grouped.display_name, "agent-name");
        assert_eq!(grouped.display_initial, "A");
        assert!(separated.show_header);
        assert_eq!(separated.display_name, "agent");
        assert_eq!(separated.picture_url, "");
    }

    #[test]
    fn feedback_message_presentation_shows_header_when_author_changes() {
        let previous = FeedbackEventRecord {
            author_pubkey: "agent".into(),
            ..feedback_event("previous", "root", 100, "previous")
        };
        let current = FeedbackEventRecord {
            author_pubkey: "user".into(),
            ..feedback_event("current", "root", 120, "current")
        };

        let projection = feedback_message_presentation(FeedbackMessagePresentationInput {
            event: current.clone(),
            show_header: rows_for_events(vec![previous, current])[1].show_header,
            current_user_pubkey: None,
            profile: None,
        });

        assert!(projection.show_header);
        assert_eq!(projection.display_name, "user");
        assert_eq!(projection.display_initial, "U");
    }

    #[test]
    fn optimistically_insert_root_thread_dedupes_previews_and_sorts() {
        let older = feedback_thread("older", 10);
        let root = feedback_event("new", "new", 30, "hello   world");

        let out = optimistically_insert_root_thread(&[older], &root);

        assert_eq!(
            out.iter()
                .map(|thread| thread.root_event_id.as_str())
                .collect::<Vec<_>>(),
            vec!["new", "older"]
        );
        assert_eq!(out[0].preview, "hello world");

        let duplicate = optimistically_insert_root_thread(&out, &root);
        assert_eq!(
            duplicate
                .iter()
                .filter(|thread| thread.root_event_id == "new")
                .count(),
            1
        );
    }

    #[test]
    fn optimistically_insert_root_thread_ignores_replies() {
        let older = feedback_thread("older", 10);
        let reply = feedback_event("reply", "root", 30, "reply");

        let out = optimistically_insert_root_thread(&[older], &reply);

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].root_event_id, "older");
    }

    #[test]
    fn upsert_thread_event_replaces_and_orders_oldest_first() {
        let older = feedback_event("older", "root", 10, "older");
        let newer = feedback_event("newer", "root", 30, "newer");
        let replacement = feedback_event("newer", "root", 5, "replacement");

        let out = upsert_thread_event(&[older, newer], &replacement);

        assert_eq!(
            out.iter()
                .map(|event| event.event_id.as_str())
                .collect::<Vec<_>>(),
            vec!["newer", "older"]
        );
        assert_eq!(out[0].content, "replacement");
    }

    /// Reproduces the user-reported bug: replies arrive on relay.tenex.chat
    /// (verified via `nak req -k 1 -e <root> wss://relay.tenex.chat`) but
    /// don't render in the iOS chat. Process the EXACT events captured from
    /// the relay and verify `query_thread_events` returns all three.
    #[test]
    fn query_thread_events_returns_replies_from_real_relay_payload() {
        let root_json = r#"{"kind":1,"id":"4ab5db30418354a17fbffbdfd345b22a19dd4ceeb67cb01c08d7ec5c801ca949","pubkey":"fcccc04fd113df1e58740c270733b33b211d1dfe2f730861ac7080125f86503f","created_at":1777553395,"tags":[["a","31933:09d48a1a5dbe13404a729634f1d6ba722d40513468dd713c8ea38ca9b7b6f2c7:highlighter"]],"content":"Sending from outside","sig":"5eb7e2be92a0b46feeb82383c6144083c2ed5b6b5de91964f0e7f0b2f1956de54baee92be5e4010d9926c3d81540052f0699175037c246678f70489ae1f48abe"}"#;
        let user_followup_json = r#"{"kind":1,"id":"58c920d4533c45e1354c861182ada0d8441235356a24630928c07d62452ed6bc","pubkey":"09d48a1a5dbe13404a729634f1d6ba722d40513468dd713c8ea38ca9b7b6f2c7","created_at":1777553426,"tags":[["e","4ab5db30418354a17fbffbdfd345b22a19dd4ceeb67cb01c08d7ec5c801ca949","","root"],["a","31933:09d48a1a5dbe13404a729634f1d6ba722d40513468dd713c8ea38ca9b7b6f2c7:highlighter"],["client","tenex-tui"],["p","4108cd882d5bd7446b4b5cb0688b14694f3d0dbb52bd24f16e1e29ff1636adab"]],"content":"from where did I send this?","sig":"fe430bcf9e064819f942d15f033ed4261ecd7cf1a3cadaecb37b12cb774c55a37ea8c601e30402251dfa48d3b2fb9ff4dbdb56c364c1dbe2cf63b9f816ad0a21"}"#;
        let agent_reply_json = r##"{"kind":1,"id":"7035a5148075421b71eda6f76426c89bc49bce7d3a89a3122e8d859dc1963cd1","pubkey":"4108cd882d5bd7446b4b5cb0688b14694f3d0dbb52bd24f16e1e29ff1636adab","created_at":1777553548,"tags":[["e","4ab5db30418354a17fbffbdfd345b22a19dd4ceeb67cb01c08d7ec5c801ca949","","root"],["e","58c920d4533c45e1354c861182ada0d8441235356a24630928c07d62452ed6bc","","reply"],["p","09d48a1a5dbe13404a729634f1d6ba722d40513468dd713c8ea38ca9b7b6f2c7"],["status","completed"],["llm-prompt-tokens","10711"],["llm-completion-tokens","126"],["llm-total-tokens","10837"],["llm-cached-input-tokens","0"],["a","31933:09d48a1a5dbe13404a729634f1d6ba722d40513468dd713c8ea38ca9b7b6f2c7:highlighter"],["llm-model","openrouter:openai/gpt-4o-mini"],["llm-ral","1"],["branch","main"]],"content":"It looks like the message you sent may have originated from one of the active conversations in the \"Highlighter\" project. Here are the currently active conversations:\n\n1. **Message from Tenex-TUI** [id: 0cbd143a] — last activity 2 minutes ago\n2. **Agent Category Discussion** [id: 28640a67] — last activity 7 minutes ago\n3. **Initial Greeting** [id: d89c7624] — last activity 11 minutes ago\n\nIf you have a specific message in mind, could you clarify which one you are referring to?","sig":"016faba108484dbb1a00a12372d428e6f9980d54a41c5889fd2e0e9fb6296690bb25bb3873e5a38e49ecc3feb05258216114e93ccb4fc2a1232b501516026c5f"}"##;

        let (ndb, _tmp) = ndb_with_json_events(&[root_json, user_followup_json, agent_reply_json]);

        let root_id = "4ab5db30418354a17fbffbdfd345b22a19dd4ceeb67cb01c08d7ec5c801ca949";
        let events = query_thread_events(&ndb, root_id).expect("query");
        let order: Vec<&str> = events.iter().map(|e| e.content.as_str()).collect();
        assert_eq!(
            order,
            vec![
                "Sending from outside",
                "from where did I send this?",
                "It looks like the message you sent may have originated from one of the active conversations in the \"Highlighter\" project. Here are the currently active conversations:\n\n1. **Message from Tenex-TUI** [id: 0cbd143a] — last activity 2 minutes ago\n2. **Agent Category Discussion** [id: 28640a67] — last activity 7 minutes ago\n3. **Initial Greeting** [id: d89c7624] — last activity 11 minutes ago\n\nIf you have a specific message in mind, could you clarify which one you are referring to?"
            ],
            "expected root + both replies"
        );
    }

    /// Live integration test against `wss://relay.tenex.chat`: opens the
    /// EXACT subscription the iOS app opens for a thread (kind:1 + #e:<root>),
    /// waits, and asserts the per-thread query returns all 3 events. Requires
    /// network — run with `cargo test live_subscribe -- --ignored --nocapture`.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn live_subscribe_to_real_thread_lands_replies_in_ndb() {
        let tmp = tempdir().expect("tempdir");
        let ndb = Ndb::new(
            tmp.path().to_str().unwrap(),
            &NdbConfig::new().set_mapsize(64 * 1024 * 1024),
        )
        .expect("open ndb");

        let ndb_database = nostr_ndb::NdbDatabase::from(ndb.clone());
        let client = Client::builder().database(ndb_database).build();
        let relay = feedback_relay();
        client.add_relay(relay).await.expect("add relay");
        client.connect_relay(relay).await.expect("connect");

        let root_hex = "4ab5db30418354a17fbffbdfd345b22a19dd4ceeb67cb01c08d7ec5c801ca949";
        let wait_sub = ndb
            .subscribe(&[NdbFilter::new().kinds([KIND_FEEDBACK_NOTE as u64]).build()])
            .expect("subscribe ndb wait");
        let id = SubscriptionId::generate();
        let filter = Filter::new()
            .kinds([Kind::Custom(KIND_FEEDBACK_NOTE)])
            .custom_tag(SingleLetterTag::lowercase(Alphabet::E), root_hex);
        client
            .subscribe_with_id_to([relay], id, filter, None)
            .await
            .expect("subscribe");

        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            ndb.wait_for_notes(wait_sub, 3),
        )
        .await
        .expect("timed out waiting for feedback notes")
        .expect("feedback notes");

        let txn = Transaction::new(&ndb).expect("txn");
        let all = ndb
            .query(&txn, &[NdbFilter::new().kinds([1u64]).build()], 100)
            .expect("query");
        eprintln!("after live sub: ndb has {} kind:1 events", all.len());
        for r in &all {
            let n = ndb.get_note_by_key(&txn, r.note_key).unwrap();
            eprintln!("  id={} content={:?}", hex::encode(n.id()), n.content());
        }
        assert!(
            all.len() >= 2,
            "expected the relay's two replies to land in ndb, got {}",
            all.len()
        );
    }

    /// Mirrors the iOS app's full path: spin up a real `HighlighterCore`,
    /// call `subscribe_feedback_thread`, wait, then call
    /// `get_feedback_thread_snapshot` — the same Swift-facing functions
    /// `FeedbackThreadStore.start()` calls in order. Verifies the events the
    /// user is missing actually surface end-to-end.
    /// Requires network — run with `--ignored --nocapture`.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn live_full_ios_flow_returns_all_thread_events() {
        let tmp = tempdir().expect("tempdir");
        let core = crate::client::HighlighterCore::new_with_data_dir(tmp.path().to_path_buf());

        let root_hex =
            "4ab5db30418354a17fbffbdfd345b22a19dd4ceeb67cb01c08d7ec5c801ca949".to_string();

        // Step 1 (cache miss expected on a fresh ndb).
        let initial = core.get_feedback_thread_snapshot(root_hex.clone()).await;
        assert!(initial.error.is_empty(), "initial query: {}", initial.error);
        let initial = initial.rows;
        eprintln!("initial cache events: {}", initial.len());

        let wait_sub = core
            .runtime()
            .ndb()
            .subscribe(&[NdbFilter::new().kinds([KIND_FEEDBACK_NOTE as u64]).build()])
            .expect("subscribe ndb wait");

        // Step 2: open the subscription — this is where ensure_feedback_relay
        // adds + connects the relay and the REQ goes out.
        let outcome = core.subscribe_feedback_thread(root_hex.clone()).await;
        assert!(outcome.error.is_empty(), "subscribe: {}", outcome.error);
        let _handle = outcome.handle;

        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            core.runtime().ndb().wait_for_notes(wait_sub, 3),
        )
        .await
        .expect("timed out waiting for feedback notes")
        .expect("feedback notes");

        // Step 3: re-query — by now the subscription should have populated ndb.
        let after = core.get_feedback_thread_snapshot(root_hex.clone()).await;
        assert!(after.error.is_empty(), "after query: {}", after.error);
        let after = after.rows;
        eprintln!("after subscription: {} events", after.len());
        for row in &after {
            let e = &row.event;
            eprintln!("  id={} content={:?}", e.event_id, e.content);
        }
        assert!(
            after.len() >= 3,
            "expected root + both replies, got {}",
            after.len()
        );
    }

    fn feedback_thread(root_event_id: &str, last_activity_at: u64) -> FeedbackThreadRecord {
        FeedbackThreadRecord {
            root_event_id: root_event_id.into(),
            author_pubkey: "pubkey".into(),
            created_at: last_activity_at,
            last_activity_at,
            title: None,
            summary: None,
            status_label: None,
            preview: String::new(),
        }
    }

    fn feedback_event(
        event_id: &str,
        root_event_id: &str,
        created_at: u64,
        content: &str,
    ) -> FeedbackEventRecord {
        FeedbackEventRecord {
            event_id: event_id.into(),
            root_event_id: root_event_id.into(),
            author_pubkey: "pubkey".into(),
            created_at,
            content: content.into(),
        }
    }

    fn profile(name: &str, display_name: &str, picture: &str) -> ProfileMetadata {
        ProfileMetadata {
            pubkey: "profile-pubkey".into(),
            name: name.into(),
            display_name: display_name.into(),
            about: String::new(),
            picture: picture.into(),
            banner: String::new(),
            nip05: String::new(),
            website: String::new(),
            lud16: String::new(),
            created_at: None,
        }
    }
}
