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
pub const KIND_COMMENT: u16 = 1111;
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

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ReactionSummary {
    pub like_count: u32,
    pub my_like_event_id: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct CommentLikeStateProjectionInput {
    pub liked_event_ids: Vec<String>,
    pub event_id_hex: String,
    pub like_count: u32,
    pub desired_liked: Option<bool>,
    pub adjust_count: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CommentLikeStateProjection {
    pub canonical_event_id_hex: String,
    pub is_liked: bool,
    pub like_count: u32,
    pub optimistic_liked_event_ids: Vec<String>,
    pub can_apply: bool,
}

/// All cached reactions on `target_event_id`, newest first. Counts and
/// "did the current user react" predicates are computed from this list
/// in the caller.
fn query_reactions_for_event(
    ndb: &Ndb,
    target_event_id: &str,
    limit: u32,
) -> Result<Vec<ReactionRecord>, CoreError> {
    let target = target_event_id.trim();
    if target.is_empty() {
        return Ok(Vec::new());
    }
    let target_event_id = match EventId::from_hex(target) {
        Ok(event_id) => event_id,
        Err(_) => return Ok(Vec::new()),
    };
    let target_bytes = target_event_id.to_bytes();

    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let cap = limit.max(64) as i32;
    let filter = NdbFilter::new()
        .kinds([KIND_REACTION as u64])
        .event(&target_bytes)
        .build();
    let results = ndb
        .query(&txn, &[filter], cap)
        .map_err(|e| CoreError::Cache(format!("query reactions: {e}")))?;

    let mut records: Vec<ReactionRecord> = Vec::with_capacity(results.len());
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        if event.kind.as_u16() != KIND_REACTION {
            continue;
        }
        let Some(target_id) = first_e_tag(&event) else {
            continue;
        };
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

pub fn summarize_likes(
    records: &[ReactionRecord],
    current_user_pubkey: Option<&str>,
) -> ReactionSummary {
    let current_user_pubkey = current_user_pubkey
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut like_count = 0;
    let mut my_like_event_id = None;

    for record in records {
        if record.content != LIKE_CONTENT {
            continue;
        }
        like_count += 1;
        if my_like_event_id.is_none()
            && current_user_pubkey
                .map(|me| record.pubkey == me)
                .unwrap_or(false)
        {
            my_like_event_id = Some(record.event_id.clone());
        }
    }

    ReactionSummary {
        like_count,
        my_like_event_id,
    }
}

pub fn query_like_summary_for_event(
    ndb: &Ndb,
    target_event_id: &str,
    current_user_pubkey: Option<&str>,
    limit: u32,
) -> Result<ReactionSummary, CoreError> {
    let records = query_reactions_for_event(ndb, target_event_id, limit)?;
    Ok(summarize_likes(&records, current_user_pubkey))
}

pub fn comment_like_state_projection(
    input: CommentLikeStateProjectionInput,
) -> CommentLikeStateProjection {
    let Ok(event_id) = EventId::from_hex(input.event_id_hex.trim()) else {
        return CommentLikeStateProjection {
            canonical_event_id_hex: String::new(),
            is_liked: false,
            like_count: input.like_count,
            optimistic_liked_event_ids: dedupe_event_ids(input.liked_event_ids),
            can_apply: false,
        };
    };
    let canonical = event_id.to_hex();
    let mut liked = dedupe_event_ids(input.liked_event_ids);
    let is_liked = liked.contains(&canonical);
    let desired = input.desired_liked.unwrap_or(!is_liked);
    let adjust_count = input.desired_liked.is_none() || input.adjust_count;

    let like_count = if adjust_count {
        match (is_liked, desired) {
            (true, false) => input.like_count.saturating_sub(1),
            (false, true) => input.like_count.saturating_add(1),
            _ => input.like_count,
        }
    } else {
        input.like_count
    };
    if desired {
        if !liked.contains(&canonical) {
            liked.push(canonical.clone());
            liked.sort();
        }
    } else {
        liked.retain(|event_id| event_id != &canonical);
    }

    CommentLikeStateProjection {
        canonical_event_id_hex: canonical,
        is_liked,
        like_count,
        optimistic_liked_event_ids: liked,
        can_apply: true,
    }
}

/// Publish a kind:7 reaction targeting `event_hex` authored by
/// `author_pubkey_hex` of `target_kind`. `content` is the reaction body
/// — pass `"+"` for a plain like.
async fn publish_reaction(
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
        return Err(CoreError::InvalidInput(
            "reaction content must not be empty".into(),
        ));
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

pub async fn publish_comment_like(
    runtime: &NostrRuntime,
    event_hex: &str,
    author_pubkey_hex: &str,
) -> Result<ReactionRecord, CoreError> {
    publish_reaction(
        runtime,
        event_hex,
        author_pubkey_hex,
        KIND_COMMENT,
        LIKE_CONTENT,
    )
    .await
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

fn dedupe_event_ids(event_ids: Vec<String>) -> Vec<String> {
    let mut ids = event_ids
        .into_iter()
        .filter_map(|event_id| EventId::from_hex(event_id.trim()).ok())
        .map(|event_id| event_id.to_hex())
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
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

#[cfg(test)]
mod tests {
    use super::*;

    fn reaction(event_id: &str, pubkey: &str, content: &str) -> ReactionRecord {
        ReactionRecord {
            event_id: event_id.into(),
            pubkey: pubkey.into(),
            target_event_id: "target".into(),
            content: content.into(),
            created_at: None,
        }
    }

    #[test]
    fn summarize_likes_counts_likes_and_current_user_reaction() {
        let records = vec![
            reaction("newer", "alice", LIKE_CONTENT),
            reaction("emoji", "bob", "🔥"),
            reaction("mine", "me", LIKE_CONTENT),
            reaction("dislike", "carol", "-"),
        ];

        assert_eq!(
            summarize_likes(&records, Some("me")),
            ReactionSummary {
                like_count: 2,
                my_like_event_id: Some("mine".into()),
            }
        );
    }

    #[test]
    fn summarize_likes_omits_current_user_when_not_liked() {
        let records = vec![
            reaction("one", "alice", LIKE_CONTENT),
            reaction("two", "bob", LIKE_CONTENT),
        ];

        assert_eq!(
            summarize_likes(&records, Some("me")),
            ReactionSummary {
                like_count: 2,
                my_like_event_id: None,
            }
        );
    }

    #[test]
    fn comment_like_state_projection_toggles_and_applies_authoritative_state() {
        let event_a = EventId::all_zeros().to_hex();
        let event_b = EventId::from_slice(&[1u8; 32]).unwrap().to_hex();

        let added = comment_like_state_projection(CommentLikeStateProjectionInput {
            liked_event_ids: vec![event_b.clone(), event_b.clone()],
            event_id_hex: event_a.clone(),
            like_count: 2,
            desired_liked: None,
            adjust_count: false,
        });
        assert!(added.can_apply);
        assert!(!added.is_liked);
        assert_eq!(added.like_count, 3);
        assert_eq!(
            added.optimistic_liked_event_ids,
            vec![event_a.clone(), event_b.clone()]
        );

        let removed = comment_like_state_projection(CommentLikeStateProjectionInput {
            liked_event_ids: added.optimistic_liked_event_ids,
            event_id_hex: event_a.clone(),
            like_count: added.like_count,
            desired_liked: None,
            adjust_count: false,
        });
        assert!(removed.is_liked);
        assert_eq!(removed.like_count, 2);
        assert_eq!(removed.optimistic_liked_event_ids, vec![event_b.clone()]);

        let confirmed = comment_like_state_projection(CommentLikeStateProjectionInput {
            liked_event_ids: removed.optimistic_liked_event_ids,
            event_id_hex: event_a.clone(),
            like_count: removed.like_count,
            desired_liked: Some(true),
            adjust_count: true,
        });
        assert_eq!(confirmed.like_count, 3);
        assert_eq!(
            confirmed.optimistic_liked_event_ids,
            vec![event_a.clone(), event_b]
        );

        let authoritative = comment_like_state_projection(CommentLikeStateProjectionInput {
            liked_event_ids: Vec::new(),
            event_id_hex: event_a.clone(),
            like_count: 7,
            desired_liked: Some(true),
            adjust_count: false,
        });
        assert_eq!(authoritative.like_count, 7);
        assert_eq!(authoritative.optimistic_liked_event_ids, vec![event_a]);

        let invalid = comment_like_state_projection(CommentLikeStateProjectionInput {
            liked_event_ids: Vec::new(),
            event_id_hex: "not-hex".into(),
            like_count: 4,
            desired_liked: None,
            adjust_count: false,
        });
        assert!(!invalid.can_apply);
        assert_eq!(invalid.like_count, 4);
    }
}
