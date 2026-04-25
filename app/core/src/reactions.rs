//! NIP-25 reactions (kind:7). Used here to "like" any event — most
//! commonly a kind:1111 NIP-22 comment, but generic over any target.
//!
//! A reaction event:
//!   kind: 7
//!   content: "+" (like) | "-" (dislike) | unicode emoji
//!   tags: ["e", <target_event_id>], ["p", <target_author_pubkey>],
//!         ["k", <target_kind>]
//!
//! v1 only surfaces likes ("+"). Emoji reactions are a v2 layer.

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;
use crate::nostr_runtime::NostrRuntime;

pub const KIND_REACTION: u16 = 7;
pub const LIKE_CONTENT: &str = "+";

/// One row of cached reaction data — what the UI needs to render
/// "12 likes · I liked this".
#[derive(Debug, Clone, uniffi::Record)]
pub struct ReactionRecord {
    pub event_id: String,
    pub pubkey: String,
    pub target_event_id: String,
    pub content: String,
    pub created_at: Option<u64>,
}

/// All cached reactions on `target_event_id`, newest first. Counts and
/// "did the current user react" predicates are computed from this list
/// in the caller.
pub fn query_reactions_for_event(
    ndb: &Ndb,
    target_event_id: &str,
    limit: u32,
) -> Result<Vec<ReactionRecord>, CoreError> {
    let target = target_event_id.trim();
    if target.is_empty() {
        return Ok(Vec::new());
    }

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let cap = limit.max(64) as i32;
    let filter = NdbFilter::new()
        .kinds([KIND_REACTION as u64])
        .tags([target], 'e')
        .build();
    let results = ndb
        .query(&txn, &[filter], cap)
        .map_err(|e| CoreError::Cache(format!("query reactions: {e}")))?;

    let mut records: Vec<ReactionRecord> = Vec::with_capacity(results.len());
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else { continue };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else { continue };
        if event.kind.as_u16() != KIND_REACTION {
            continue;
        }
        let Some(target_id) = first_e_tag(&event) else { continue };
        records.push(ReactionRecord {
            event_id: event.id.to_hex(),
            pubkey: event.pubkey.to_hex(),
            target_event_id: target_id,
            content: event.content.clone(),
            created_at: Some(event.created_at.as_secs()),
        });
    }
    records.sort_by(|a, b| b.created_at.unwrap_or(0).cmp(&a.created_at.unwrap_or(0)));
    records.truncate(limit as usize);
    Ok(records)
}

/// Publish a kind:7 reaction targeting `event_hex` authored by
/// `author_pubkey_hex` of `target_kind`. `content` is the reaction body
/// — pass `"+"` for a plain like.
pub async fn publish_reaction(
    runtime: &NostrRuntime,
    event_hex: &str,
    author_pubkey_hex: &str,
    target_kind: u16,
    content: &str,
) -> Result<ReactionRecord, CoreError> {
    let event_hex = event_hex.trim();
    let target = EventId::from_hex(event_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid target event id: {e}")))?;
    let author = PublicKey::from_hex(author_pubkey_hex.trim())
        .map_err(|e| CoreError::InvalidInput(format!("invalid target author pubkey: {e}")))?;
    let content = content.trim();
    if content.is_empty() {
        return Err(CoreError::InvalidInput("reaction content must not be empty".into()));
    }

    let tags = vec![
        Tag::parse(vec!["e".to_string(), target.to_hex()])
            .map_err(|e| CoreError::Other(format!("build e tag: {e}")))?,
        Tag::parse(vec!["p".to_string(), author.to_hex()])
            .map_err(|e| CoreError::Other(format!("build p tag: {e}")))?,
        Tag::parse(vec!["k".to_string(), target_kind.to_string()])
            .map_err(|e| CoreError::Other(format!("build k tag: {e}")))?,
    ];

    let builder = EventBuilder::new(Kind::Custom(KIND_REACTION), content).tags(tags);
    let client = runtime.client();
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign reaction: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish reaction: {e}")))?;

    Ok(ReactionRecord {
        event_id: event.id.to_hex(),
        pubkey: event.pubkey.to_hex(),
        target_event_id: target.to_hex(),
        content: event.content.clone(),
        created_at: Some(event.created_at.as_secs()),
    })
}

/// Publish a NIP-25 deletion (kind:5) for the user's own kind:7 reaction.
/// Returns the deletion event id. Relays that honour NIP-09 will drop the
/// original reaction; clients that re-cache the deletion will hide it.
pub async fn unpublish_reaction(
    runtime: &NostrRuntime,
    reaction_event_id: &str,
) -> Result<String, CoreError> {
    let target = EventId::from_hex(reaction_event_id.trim())
        .map_err(|e| CoreError::InvalidInput(format!("invalid reaction event id: {e}")))?;

    let tag = Tag::parse(vec!["e".to_string(), target.to_hex()])
        .map_err(|e| CoreError::Other(format!("build e tag: {e}")))?;
    let builder = EventBuilder::new(Kind::EventDeletion, "").tags(vec![tag]);
    let client = runtime.client();
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign deletion: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish deletion: {e}")))?;

    Ok(event.id.to_hex())
}

fn first_e_tag(event: &Event) -> Option<String> {
    for tag in event.tags.iter() {
        let slice = tag.as_slice();
        if slice.first().map(String::as_str) == Some("e") {
            if let Some(v) = slice.get(1) {
                if !v.is_empty() {
                    return Some(v.clone());
                }
            }
        }
    }
    None
}
