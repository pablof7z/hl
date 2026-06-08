//! NIP-22 comments (kind:1111). A comment carries two scopes of
//! reference tags: UPPERCASE for the root (the artifact being commented
//! on) and lowercase for the direct parent (the comment above it in the
//! thread). Top-level comments set parent == root.

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;
use crate::models::{ArtifactPreview, CommentRecord, CommentScope};
use crate::nostr_runtime::NostrRuntime;

/// kind:1111 — NIP-22 comment.
pub const KIND_NIP22_COMMENT: u16 = 1111;
const KIND_NIP23_ARTICLE: u16 = 30023;

/// Project a NIP-23 address into the NIP-22 root scope used for comment
/// reads/writes. The shell passes addresses; Rust owns the tag/kind mapping.
pub fn article_scope(address: &str) -> Result<CommentScope, CoreError> {
    let value = address.trim();
    if value.is_empty() {
        return Err(CoreError::InvalidInput(
            "article comment address must not be empty".into(),
        ));
    }
    Ok(scope(
        'A',
        value,
        address_kind(value).unwrap_or(KIND_NIP23_ARTICLE),
    ))
}

/// Project an event id into a NIP-22 event root scope.
pub fn event_scope(event_id_hex: &str, kind: u16) -> Result<CommentScope, CoreError> {
    let value = event_id_hex.trim();
    if value.is_empty() {
        return Err(CoreError::InvalidInput(
            "event comment id must not be empty".into(),
        ));
    }
    Ok(scope('E', value, kind))
}

/// Project an external identifier into a NIP-22 external root scope. The
/// identifier is preserved exactly after trimming because existing data uses
/// both NIP-73 ids (`isbn:…`, `podcast:item:guid:…`) and raw URLs.
pub fn external_scope(identifier: &str, kind: u16) -> Result<CommentScope, CoreError> {
    let value = identifier.trim();
    if value.is_empty() {
        return Err(CoreError::InvalidInput(
            "external comment identifier must not be empty".into(),
        ));
    }
    Ok(scope('I', value, kind))
}

/// Project an artifact preview's protocol reference fields into the NIP-22
/// scope used by comment surfaces. Native shells must not duplicate this
/// `a/e/i` mapping or the default kind policy.
pub fn scope_from_preview(preview: &ArtifactPreview) -> Result<CommentScope, CoreError> {
    let value = preview.reference_tag_value.trim();
    if value.is_empty() {
        return Err(CoreError::InvalidInput(
            "artifact comment reference must not be empty".into(),
        ));
    }
    let parsed_kind = preview
        .reference_kind
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|k| *k > 0);
    match preview
        .reference_tag_name
        .trim()
        .chars()
        .next()
        .map(|c| c.to_ascii_lowercase())
    {
        Some('a') => Ok(scope(
            'A',
            value,
            parsed_kind
                .or_else(|| address_kind(value))
                .unwrap_or(KIND_NIP23_ARTICLE),
        )),
        Some('e') => Ok(scope('E', value, parsed_kind.unwrap_or(0))),
        Some('i') => Ok(scope('I', value, parsed_kind.unwrap_or(0))),
        Some(other) => Err(CoreError::InvalidInput(format!(
            "unsupported comment reference tag: {other}"
        ))),
        None => Err(CoreError::InvalidInput(
            "artifact comment reference tag must not be empty".into(),
        )),
    }
}

fn scope(tag_name: char, tag_value: &str, kind: u16) -> CommentScope {
    CommentScope {
        root_tag_name: tag_name.to_string(),
        root_tag_value: tag_value.to_string(),
        root_kind: kind,
    }
}

fn address_kind(value: &str) -> Option<u16> {
    value
        .split(':')
        .next()?
        .parse::<u16>()
        .ok()
        .filter(|k| *k > 0)
}

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
    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

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
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        if let Some(rec) = record_from_event(&event) {
            records.push(rec);
        }
    }
    records.sort_by(|a, b| b.created_at.unwrap_or(0).cmp(&a.created_at.unwrap_or(0)));
    records.truncate(limit as usize);
    Ok(records)
}

pub fn query_for_scope(
    ndb: &Ndb,
    scope: &CommentScope,
    limit: u32,
) -> Result<Vec<CommentRecord>, CoreError> {
    let Some(ch) = scope.root_tag_name.trim().chars().next() else {
        return Ok(Vec::new());
    };
    query_for_reference(ndb, ch, &scope.root_tag_value, limit)
}

fn record_from_event(event: &Event) -> Option<CommentRecord> {
    if event.kind.as_u16() != KIND_NIP22_COMMENT {
        return None;
    }

    // Root scope — one of uppercase A/E/I. Whichever appears first wins;
    // NIP-22 allows multiple for redundancy but typically only one applies.
    let (root_tag_name, root_tag_value) =
        first_scope_tag(event, &["A", "E", "I"]).unwrap_or((String::new(), String::new()));

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
        let Some(name) = s.first().map(String::as_str) else {
            continue;
        };
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

/// Publish a NIP-22 kind:1111 comment scoped to any artifact root and,
/// optionally, replying to a specific parent comment.
///
/// - `root_tag_name` selects the uppercase scope tag: `'A'` for an
///   addressable artifact (`30023:<pubkey>:<d>`), `'E'` for an event id
///   (e.g. a kind:9802 highlight), `'I'` for external content
///   (`url:…`, `podcast:item:guid:…`, `isbn:…`). Case is normalised.
/// - `root_tag_value` is the corresponding scope value.
/// - `root_kind` is the kind of the root event (used for the uppercase `K`
///   tag). For purely external roots with no host kind, pass `0`.
/// - `parent_event_id` is `None` for top-level comments (parent mirrors
///   root) and `Some(comment_id)` for replies (parent = that kind:1111
///   comment via lowercase `e` + `k=1111`).
///
/// Returns the new `CommentRecord` so callers can optimistically update
/// their cache without waiting for a relay round-trip.
pub async fn publish_comment(
    runtime: &NostrRuntime,
    root_tag_name: char,
    root_tag_value: &str,
    root_kind: u16,
    parent_event_id: Option<&str>,
    content: &str,
) -> Result<CommentRecord, CoreError> {
    let content = content.trim();
    if content.is_empty() {
        return Err(CoreError::InvalidInput(
            "comment body must not be empty".into(),
        ));
    }
    let root_value = root_tag_value.trim();
    if root_value.is_empty() {
        return Err(CoreError::InvalidInput(
            "root tag value must not be empty".into(),
        ));
    }
    let upper = root_tag_name.to_ascii_uppercase();
    let lower = root_tag_name.to_ascii_lowercase();
    if !matches!(upper, 'A' | 'E' | 'I') {
        return Err(CoreError::InvalidInput(format!(
            "root tag must be A/E/I, got {root_tag_name}"
        )));
    }
    if upper == 'E' {
        EventId::from_hex(root_value)
            .map_err(|e| CoreError::InvalidInput(format!("invalid root event id: {e}")))?;
    }

    let mut tags: Vec<Tag> = Vec::with_capacity(4);

    // Uppercase root scope.
    tags.push(
        Tag::parse(vec![upper.to_string(), root_value.to_string()])
            .map_err(|e| CoreError::Other(format!("build {upper} tag: {e}")))?,
    );
    tags.push(
        Tag::parse(vec!["K".to_string(), root_kind.to_string()])
            .map_err(|e| CoreError::Other(format!("build K tag: {e}")))?,
    );

    // Lowercase parent scope. Top-level mirrors root; replies reference
    // the parent comment as a kind:1111 event.
    let (parent_name, parent_value, parent_kind) = match parent_event_id {
        Some(pid) => {
            let pid = pid.trim();
            EventId::from_hex(pid)
                .map_err(|e| CoreError::InvalidInput(format!("invalid parent event id: {e}")))?;
            ('e', pid.to_string(), KIND_NIP22_COMMENT)
        }
        None => (lower, root_value.to_string(), root_kind),
    };
    tags.push(
        Tag::parse(vec![parent_name.to_string(), parent_value.clone()])
            .map_err(|e| CoreError::Other(format!("build {parent_name} tag: {e}")))?,
    );
    tags.push(
        Tag::parse(vec!["k".to_string(), parent_kind.to_string()])
            .map_err(|e| CoreError::Other(format!("build k tag: {e}")))?,
    );

    let builder = EventBuilder::new(Kind::Custom(KIND_NIP22_COMMENT), content).tags(tags);

    let client = runtime.client();
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign comment: {e}")))?;

    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish comment: {e}")))?;

    Ok(CommentRecord {
        event_id: event.id.to_hex(),
        pubkey: event.pubkey.to_hex(),
        body: event.content.clone(),
        root_tag_name: upper.to_string(),
        root_tag_value: root_value.to_string(),
        parent_tag_name: parent_name.to_string(),
        parent_tag_value: parent_value,
        root_kind: root_kind.to_string(),
        created_at: Some(event.created_at.as_secs()),
    })
}

pub async fn publish_comment_for_scope(
    runtime: &NostrRuntime,
    scope: &CommentScope,
    parent_event_id: Option<&str>,
    content: &str,
) -> Result<CommentRecord, CoreError> {
    let Some(root_tag_name) = scope.root_tag_name.trim().chars().next() else {
        return Err(CoreError::InvalidInput("root tag must not be empty".into()));
    };
    publish_comment(
        runtime,
        root_tag_name,
        &scope.root_tag_value,
        scope.root_kind,
        parent_event_id,
        content,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn article_scope_uses_address_kind() {
        let scope = article_scope("30023:pubkey:essay").expect("scope");
        assert_eq!(scope.root_tag_name, "A");
        assert_eq!(scope.root_tag_value, "30023:pubkey:essay");
        assert_eq!(scope.root_kind, 30023);
    }

    #[test]
    fn event_scope_preserves_event_kind() {
        let scope = event_scope("abc123", 9802).expect("scope");
        assert_eq!(scope.root_tag_name, "E");
        assert_eq!(scope.root_tag_value, "abc123");
        assert_eq!(scope.root_kind, 9802);
    }

    #[test]
    fn external_scope_preserves_raw_identifier() {
        let scope = external_scope(" https://example.com/post ", 0).expect("scope");
        assert_eq!(scope.root_tag_name, "I");
        assert_eq!(scope.root_tag_value, "https://example.com/post");
        assert_eq!(scope.root_kind, 0);
    }

    #[test]
    fn scope_from_preview_maps_article_reference() {
        let preview = crate::articles::article_artifact_preview_from_address("30023:pk:essay")
            .expect("preview");
        let scope = scope_from_preview(&preview).expect("scope");
        assert_eq!(scope.root_tag_name, "A");
        assert_eq!(scope.root_tag_value, "30023:pk:essay");
        assert_eq!(scope.root_kind, 30023);
    }

    #[test]
    fn scope_from_preview_maps_external_web_kind_to_zero() {
        let preview = crate::artifacts::build_preview("https://example.com/post").expect("preview");
        let scope = scope_from_preview(&preview).expect("scope");
        assert_eq!(scope.root_tag_name, "I");
        assert_eq!(scope.root_tag_value, "https://example.com/post");
        assert_eq!(scope.root_kind, 0);
    }

    #[test]
    fn scope_from_preview_rejects_non_comment_reference_tag() {
        let mut preview = crate::articles::article_artifact_preview_from_address("30023:pk:essay")
            .expect("preview");
        preview.reference_tag_name = "r".into();
        let err = scope_from_preview(&preview).unwrap_err().to_string();
        assert!(err.contains("unsupported comment reference tag"));
    }
}
