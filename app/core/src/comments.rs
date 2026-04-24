//! NIP-22 comments (kind:1111). A comment carries two scopes of
//! reference tags: UPPERCASE for the root (the artifact being commented
//! on) and lowercase for the direct parent (the comment above it in the
//! thread). Top-level comments set parent == root.

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;
use crate::models::CommentRecord;

/// kind:1111 — NIP-22 comment.
pub const KIND_NIP22_COMMENT: u16 = 1111;

/// Read kind:1111 comments rooted at `tag_value` under a specific
/// uppercase root tag (`'A'` addressable / `'E'` event / `'I'` external
/// content). Newest first.
pub fn query_for_reference(
    ndb: &Ndb,
    tag_name: char,
    tag_value: &str,
    limit: u32,
) -> Result<Vec<CommentRecord>, CoreError> {
    let tag_value = tag_value.trim();
    if tag_value.is_empty() {
        return Ok(Vec::new());
    }
    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let ndb_cap = limit.max(32) as i32;
    let filter = NdbFilter::new()
        .kinds([KIND_NIP22_COMMENT as u64])
        .tags([tag_value], tag_name)
        .build();

    let results = ndb
        .query(&txn, &[filter], ndb_cap)
        .map_err(|e| CoreError::Cache(format!("query comments by reference: {e}")))?;

    let mut records: Vec<CommentRecord> = Vec::with_capacity(results.len());
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else { continue };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else { continue };
        if let Some(rec) = record_from_event(&event) {
            records.push(rec);
        }
    }
    records.sort_by(|a, b| b.created_at.unwrap_or(0).cmp(&a.created_at.unwrap_or(0)));
    records.truncate(limit as usize);
    Ok(records)
}

fn record_from_event(event: &Event) -> Option<CommentRecord> {
    if event.kind.as_u16() != KIND_NIP22_COMMENT {
        return None;
    }

    // Root scope — one of uppercase A/E/I. Whichever appears first wins;
    // NIP-22 allows multiple for redundancy but typically only one applies.
    let (root_tag_name, root_tag_value) = first_scope_tag(event, &["A", "E", "I"])
        .unwrap_or((String::new(), String::new()));

    // Parent scope — lowercase a/e/i. Missing on top-level comments where
    // parent is the root itself; fall back to root in that case so callers
    // can always thread.
    let (parent_tag_name, parent_tag_value) = first_scope_tag(event, &["a", "e", "i"])
        .unwrap_or_else(|| (root_tag_name.clone(), root_tag_value.clone()));

    let root_kind = first_tag_value(event, "K").unwrap_or("").to_string();

    Some(CommentRecord {
        event_id: event.id.to_hex(),
        pubkey: event.pubkey.to_hex(),
        body: event.content.clone(),
        root_tag_name,
        root_tag_value,
        parent_tag_name,
        parent_tag_value,
        root_kind,
        created_at: Some(event.created_at.as_secs()),
    })
}

fn first_scope_tag(event: &Event, names: &[&str]) -> Option<(String, String)> {
    for tag in event.tags.iter() {
        let s = tag.as_slice();
        let Some(name) = s.first().map(String::as_str) else { continue };
        if names.contains(&name) {
            if let Some(value) = s.get(1).map(String::as_str) {
                if !value.is_empty() {
                    return Some((name.to_string(), value.to_string()));
                }
            }
        }
    }
    None
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
