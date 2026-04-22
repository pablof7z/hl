//! NIP-29 room discussions (kind:11 threads marked `['t','discussion']`).
//! Ports `web/src/lib/features/discussions/roomDiscussion.ts`.
//!
//! A discussion shares the same event kind as an artifact share — the
//! distinguisher is the `t=discussion` marker. Consumers must check for that
//! marker before interpreting a kind:11 event; `Room` library pumps skip
//! marked events and the `RoomDiscussions` pump only emits deltas for them.

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;
use crate::models::{ArtifactPreview, DiscussionAttachment, DiscussionRecord};
use crate::nostr_runtime::NostrRuntime;

pub const KIND_DISCUSSION: u16 = 11;
pub const DISCUSSION_MARKER_TAG: &str = "discussion";

/// Query cached discussions for a room from nostrdb. Filters kind:11 by
/// `#h=group_id`, then keeps only events whose `t` tags contain
/// `"discussion"`. Sorted most-recent first.
pub fn query_for_group(
    ndb: &Ndb,
    group_id: &str,
    limit: u32,
) -> Result<Vec<DiscussionRecord>, CoreError> {
    if group_id.trim().is_empty() {
        return Err(CoreError::InvalidInput("group_id must not be empty".into()));
    }

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let filter = NdbFilter::new()
        .kinds([KIND_DISCUSSION as u64])
        .tags([group_id], 'h')
        .build();

    let limit_i: i32 = limit.max(1).try_into().unwrap_or(i32::MAX);
    let results = ndb
        .query(&txn, &[filter], limit_i)
        .map_err(|e| CoreError::Cache(format!("query discussions: {e}")))?;

    let mut records: Vec<DiscussionRecord> = Vec::with_capacity(results.len());
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        if !is_discussion(&event) {
            continue;
        }
        if let Some(record) = record_from_event(&event) {
            records.push(record);
        }
    }

    records.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(records)
}

/// Build + sign + publish a kind:11 discussion thread into a NIP-29 group.
/// Optional `attachment` attaches a previously-built artifact preview so the
/// discussion shows "started from this podcast/article/book" in the UI.
pub async fn publish(
    runtime: &NostrRuntime,
    group_id: &str,
    title: &str,
    body: &str,
    attachment: Option<ArtifactPreview>,
) -> Result<DiscussionRecord, CoreError> {
    if group_id.trim().is_empty() {
        return Err(CoreError::InvalidInput("group_id must not be empty".into()));
    }
    let title = title.trim();
    if title.is_empty() {
        return Err(CoreError::InvalidInput("discussion title required".into()));
    }

    let builder = build_event(group_id, title, body, attachment.as_ref())?;
    let client = runtime.client();
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign discussion: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish discussion: {e}")))?;

    record_from_event(&event).ok_or_else(|| {
        CoreError::Other("signed discussion event failed to parse back".into())
    })
}

pub fn is_discussion(event: &Event) -> bool {
    event.kind.as_u16() == KIND_DISCUSSION
        && event.tags.iter().any(|tag| {
            let s = tag.as_slice();
            s.first().map(String::as_str) == Some("t")
                && s.get(1).map(String::as_str) == Some(DISCUSSION_MARKER_TAG)
        })
}

pub(crate) fn record_from_event(event: &Event) -> Option<DiscussionRecord> {
    if !is_discussion(event) {
        return None;
    }
    let title = first_tag_value(event, "title").unwrap_or("").trim().to_string();
    let group_id = first_tag_value(event, "h")?.to_string();
    let slug = first_tag_value(event, "d")
        .map(str::to_string)
        .unwrap_or_else(|| event.id.to_hex());
    let summary = first_tag_value(event, "summary")
        .unwrap_or("")
        .trim()
        .to_string();

    Some(DiscussionRecord {
        id: slug,
        event_id: event.id.to_hex(),
        group_id,
        pubkey: event.pubkey.to_hex(),
        title: if title.is_empty() {
            "Untitled discussion".into()
        } else {
            title
        },
        body: event.content.clone(),
        summary,
        created_at: Some(event.created_at.as_secs()),
        attachment: read_attachment(event),
    })
}

fn read_attachment(event: &Event) -> Option<DiscussionAttachment> {
    let a = first_tag_value(event, "a").map(str::to_string).unwrap_or_default();
    let e = first_tag_value(event, "e").map(str::to_string).unwrap_or_default();
    let i = first_tag_value(event, "i").map(str::to_string).unwrap_or_default();
    let r = first_tag_value(event, "r").map(str::to_string).unwrap_or_default();
    if a.is_empty() && e.is_empty() && i.is_empty() && r.is_empty() {
        return None;
    }
    let (name, value) = if !a.is_empty() {
        ("a".to_string(), a)
    } else if !e.is_empty() {
        ("e".to_string(), e)
    } else if !i.is_empty() {
        ("i".to_string(), i)
    } else {
        ("r".to_string(), r.clone())
    };
    Some(DiscussionAttachment {
        reference_tag_name: name,
        reference_tag_value: value,
        reference_kind: first_tag_value(event, "k").unwrap_or("").to_string(),
        url: r,
        title: first_tag_value(event, "title").unwrap_or("").to_string(),
        author: first_tag_value(event, "author").unwrap_or("").to_string(),
        image: first_tag_value(event, "image").unwrap_or("").to_string(),
        summary: first_tag_value(event, "summary").unwrap_or("").to_string(),
    })
}

fn build_event(
    group_id: &str,
    title: &str,
    body: &str,
    attachment: Option<&ArtifactPreview>,
) -> Result<EventBuilder, CoreError> {
    let slug = slug_from_title(title);

    let mut tags: Vec<Tag> = Vec::new();
    tags.push(parse_tag(&["h", group_id])?);
    tags.push(parse_tag(&["d", &slug])?);
    tags.push(parse_tag(&["t", DISCUSSION_MARKER_TAG])?);
    tags.push(parse_tag(&["title", title])?);

    if let Some(preview) = attachment {
        match preview.reference_tag_name.as_str() {
            "i" => {
                if !preview.url.is_empty() {
                    tags.push(parse_tag(&["i", &preview.reference_tag_value, &preview.url])?);
                } else {
                    tags.push(parse_tag(&["i", &preview.reference_tag_value])?);
                }
                if !preview.reference_kind.is_empty() {
                    tags.push(parse_tag(&["k", &preview.reference_kind])?);
                }
            }
            "a" | "e" => {
                tags.push(parse_tag(&[
                    preview.reference_tag_name.as_str(),
                    &preview.reference_tag_value,
                ])?);
            }
            _ => {}
        }
        if !preview.url.is_empty() {
            tags.push(parse_tag(&["r", &preview.url])?);
        }
        if !preview.author.is_empty() {
            tags.push(parse_tag(&["author", &preview.author])?);
        }
        if !preview.image.is_empty() {
            tags.push(parse_tag(&["image", &preview.image])?);
        }
        if !preview.description.is_empty() {
            tags.push(parse_tag(&["summary", &preview.description])?);
        }
    }

    Ok(EventBuilder::new(Kind::Custom(KIND_DISCUSSION), body.trim()).tags(tags))
}

fn slug_from_title(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut last_dash = false;
    for ch in title.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    if out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        // Never emit an empty `d` tag — fall back to timestamp-ish ms.
        format!("d{}", nostr_sdk::Timestamp::now().as_secs())
    } else {
        out
    }
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

    fn sign(keys: &Keys, tags: Vec<Tag>, content: &str) -> Event {
        EventBuilder::new(Kind::Custom(KIND_DISCUSSION), content)
            .tags(tags)
            .sign_with_keys(keys)
            .expect("sign")
    }

    #[test]
    fn is_discussion_requires_t_marker() {
        let keys = Keys::generate();
        let with_marker = sign(
            &keys,
            vec![
                parse_tag(&["h", "room-a"]).unwrap(),
                parse_tag(&["t", "discussion"]).unwrap(),
            ],
            "hi",
        );
        let without = sign(
            &keys,
            vec![parse_tag(&["h", "room-a"]).unwrap()],
            "hi",
        );
        assert!(is_discussion(&with_marker));
        assert!(!is_discussion(&without));
    }

    #[test]
    fn record_from_event_reads_title_body_and_slug() {
        let keys = Keys::generate();
        let e = sign(
            &keys,
            vec![
                parse_tag(&["h", "room-a"]).unwrap(),
                parse_tag(&["t", "discussion"]).unwrap(),
                parse_tag(&["title", "Thoughts on ep 42"]).unwrap(),
                parse_tag(&["d", "thoughts-on-ep-42"]).unwrap(),
                parse_tag(&["summary", "short"]).unwrap(),
            ],
            "body goes here",
        );
        let rec = record_from_event(&e).expect("discussion record");
        assert_eq!(rec.id, "thoughts-on-ep-42");
        assert_eq!(rec.group_id, "room-a");
        assert_eq!(rec.title, "Thoughts on ep 42");
        assert_eq!(rec.body, "body goes here");
        assert_eq!(rec.summary, "short");
        assert!(rec.attachment.is_none());
    }

    #[test]
    fn record_from_event_reads_attachment_ref() {
        let keys = Keys::generate();
        let e = sign(
            &keys,
            vec![
                parse_tag(&["h", "room-a"]).unwrap(),
                parse_tag(&["t", "discussion"]).unwrap(),
                parse_tag(&["title", "Check this"]).unwrap(),
                parse_tag(&["i", "podcast:guid:abc", "https://example.com/ep"]).unwrap(),
                parse_tag(&["k", "30054"]).unwrap(),
                parse_tag(&["r", "https://example.com/ep"]).unwrap(),
            ],
            "look",
        );
        let rec = record_from_event(&e).expect("discussion record");
        let att = rec.attachment.expect("attachment present");
        assert_eq!(att.reference_tag_name, "i");
        assert_eq!(att.reference_tag_value, "podcast:guid:abc");
        assert_eq!(att.reference_kind, "30054");
        assert_eq!(att.url, "https://example.com/ep");
    }

    #[test]
    fn slug_from_title_is_sane() {
        assert_eq!(slug_from_title("Hello World!"), "hello-world");
        assert_eq!(slug_from_title("  Many   spaces "), "many-spaces");
        assert_eq!(slug_from_title("emoji 🎧 in title"), "emoji-in-title");
    }

    #[test]
    fn build_event_emits_required_tags() {
        let builder = build_event("room-a", "Title", "body", None).expect("build");
        let keys = Keys::generate();
        let e = builder.sign_with_keys(&keys).expect("sign");
        let has = |name: &str, val: &str| {
            e.tags.iter().any(|t| {
                let s = t.as_slice();
                s.first().map(String::as_str) == Some(name)
                    && s.get(1).map(String::as_str) == Some(val)
            })
        };
        assert!(has("h", "room-a"));
        assert!(has("t", "discussion"));
        assert!(has("title", "Title"));
        assert!(e
            .tags
            .iter()
            .any(|t| t.as_slice().first().map(String::as_str) == Some("d")));
    }
}
