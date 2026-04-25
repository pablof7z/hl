//! NIP-51 kind:10003 Bookmark List.
//!
//! A single replaceable bookmark list per user. Bookmarks are public — the
//! `a`-tags live in the event's tag array (the `content` field is reserved
//! for encrypted private bookmarks per NIP-51, which we don't support yet).
//!
//! We store article bookmarks as `["a", "30023:<pubkey>:<d>"]` tags so they
//! round-trip with any other nostr client that understands NIP-51. Other
//! bookmark types (URLs via `r`, notes via `e`, hashtags via `t`) are
//! preserved on read/write even though we don't surface them yet — removing
//! them on every toggle would clobber bookmarks set by the web app or other
//! clients.

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;
use crate::nostr_runtime::NostrRuntime;

pub const KIND_BOOKMARKS: u16 = 10003;

/// Parsed shape of a kind:10003 event. We surface `a`-tag (addressable —
/// articles) and `e`-tag (event — comments, highlights) bookmarks; every
/// other tag (`r`, `t`, …) is preserved verbatim so writes don't destroy
/// bookmarks set by the web app or other clients we don't understand.
#[derive(Debug, Clone, Default)]
pub struct BookmarkList {
    /// Addressable bookmarks, e.g. `"30023:<pubkey>:<d>"`.
    pub addresses: Vec<String>,
    /// Event-id bookmarks (hex). Used for kind:1111 comments and any other
    /// non-replaceable event we want to bookmark.
    pub event_ids: Vec<String>,
    /// Preserved tags we don't interpret — `r`, `t`, anything else.
    /// Written back verbatim on the next publish.
    pub other_tags: Vec<Vec<String>>,
    /// Original event content (NIP-51 reserves this for encrypted private
    /// bookmarks; preserved as an opaque blob so we don't nuke them).
    pub content: String,
}

// -- Public API --------------------------------------------------------------

/// Read the newest cached kind:10003 for `user_hex` and return the set of
/// addressable-event bookmarks it carries. Empty list when none cached.
pub fn query_bookmarks(ndb: &Ndb, user_hex: &str) -> Result<BookmarkList, CoreError> {
    if user_hex.is_empty() {
        return Ok(BookmarkList::default());
    }
    let author = PublicKey::from_hex(user_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid user pubkey: {e}")))?;
    let pk_bytes: [u8; 32] = author.to_bytes();

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let filter = NdbFilter::new()
        .kinds([KIND_BOOKMARKS as u64])
        .authors([&pk_bytes])
        .build();
    let results = ndb
        .query(&txn, &[filter], 8)
        .map_err(|e| CoreError::Cache(format!("query bookmarks: {e}")))?;

    let mut newest: Option<Event> = None;
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else { continue };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else { continue };
        newest = Some(match newest {
            Some(prev) if prev.created_at >= event.created_at => prev,
            _ => event,
        });
    }

    Ok(newest.map(parse_bookmark_event).unwrap_or_default())
}

/// Fast predicate: is `address` currently bookmarked for `user_hex`?
pub fn is_bookmarked(ndb: &Ndb, user_hex: &str, address: &str) -> Result<bool, CoreError> {
    let list = query_bookmarks(ndb, user_hex)?;
    Ok(list.addresses.iter().any(|a| a == address))
}

/// Fast predicate: is `event_hex` currently bookmarked for `user_hex`?
pub fn is_event_bookmarked(ndb: &Ndb, user_hex: &str, event_hex: &str) -> Result<bool, CoreError> {
    let list = query_bookmarks(ndb, user_hex)?;
    Ok(list.event_ids.iter().any(|e| e == event_hex))
}

/// Toggle `address` in the user's kind:10003 bookmark list. Reads the newest
/// cached list, flips membership, re-publishes. Returns the new membership
/// state (`true` = now bookmarked, `false` = removed).
pub async fn toggle_bookmark(
    runtime: &NostrRuntime,
    user_hex: &str,
    address: &str,
) -> Result<bool, CoreError> {
    let address = address.trim();
    if address.is_empty() {
        return Err(CoreError::InvalidInput(
            "bookmark address must not be empty".into(),
        ));
    }

    let mut list = query_bookmarks(runtime.ndb(), user_hex)?;
    let now_bookmarked = match list.addresses.iter().position(|a| a == address) {
        Some(idx) => {
            list.addresses.remove(idx);
            false
        }
        None => {
            list.addresses.push(address.to_string());
            true
        }
    };

    publish_bookmarks(runtime, &list).await?;
    Ok(now_bookmarked)
}

/// Toggle `event_hex` in the user's kind:10003 bookmark list (for comments
/// and other event-id-addressed targets). Reads the newest cached list,
/// flips membership, re-publishes. Returns the new membership state.
pub async fn toggle_event_bookmark(
    runtime: &NostrRuntime,
    user_hex: &str,
    event_hex: &str,
) -> Result<bool, CoreError> {
    let event_hex = event_hex.trim();
    if event_hex.is_empty() {
        return Err(CoreError::InvalidInput(
            "bookmark event id must not be empty".into(),
        ));
    }
    EventId::from_hex(event_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid bookmark event id: {e}")))?;

    let mut list = query_bookmarks(runtime.ndb(), user_hex)?;
    let now_bookmarked = match list.event_ids.iter().position(|e| e == event_hex) {
        Some(idx) => {
            list.event_ids.remove(idx);
            false
        }
        None => {
            list.event_ids.push(event_hex.to_string());
            true
        }
    };

    publish_bookmarks(runtime, &list).await?;
    Ok(now_bookmarked)
}

/// Publish `list` as a kind:10003 event replacing whatever's currently on
/// the relays. Preserves `other_tags` and `content` so bookmarks we don't
/// understand (URLs, notes, encrypted private set) survive.
async fn publish_bookmarks(
    runtime: &NostrRuntime,
    list: &BookmarkList,
) -> Result<String, CoreError> {
    let mut tags: Vec<Tag> = Vec::new();
    for addr in &list.addresses {
        tags.push(
            Tag::parse(vec!["a".to_string(), addr.clone()])
                .map_err(|e| CoreError::Other(format!("build a tag: {e}")))?,
        );
    }
    for ev in &list.event_ids {
        tags.push(
            Tag::parse(vec!["e".to_string(), ev.clone()])
                .map_err(|e| CoreError::Other(format!("build e tag: {e}")))?,
        );
    }
    for raw in &list.other_tags {
        if let Ok(tag) = Tag::parse(raw.clone()) {
            tags.push(tag);
        }
    }

    let builder = EventBuilder::new(Kind::Custom(KIND_BOOKMARKS), list.content.clone()).tags(tags);
    let client = runtime.client();
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign bookmarks: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish bookmarks: {e}")))?;
    Ok(event.id.to_hex())
}

// -- Parsing -----------------------------------------------------------------

fn parse_bookmark_event(event: Event) -> BookmarkList {
    let mut list = BookmarkList {
        addresses: Vec::new(),
        event_ids: Vec::new(),
        other_tags: Vec::new(),
        content: event.content.clone(),
    };
    for tag in event.tags.iter() {
        let s = tag.as_slice();
        match s.first().map(String::as_str) {
            Some("a") => {
                if let Some(v) = s.get(1) {
                    list.addresses.push(v.clone());
                }
            }
            Some("e") => {
                if let Some(v) = s.get(1) {
                    list.event_ids.push(v.clone());
                }
            }
            _ => {
                list.other_tags.push(s.to_vec());
            }
        }
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fresh_ndb() -> (Ndb, TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = nostrdb::Config::new();
        let ndb = Ndb::new(tmp.path().to_str().unwrap(), &cfg).unwrap();
        (ndb, tmp)
    }

    fn process(ndb: &Ndb, event: &Event) {
        let line = format!("[\"EVENT\",\"sub\",{}]", event.as_json());
        ndb.process_event(&line).unwrap();
    }

    #[test]
    fn query_bookmarks_parses_a_tags_and_preserves_unknowns() {
        let (ndb, _tmp) = fresh_ndb();
        let keys = Keys::generate();

        let event = EventBuilder::new(Kind::Custom(KIND_BOOKMARKS), "opaque")
            .tags([
                Tag::parse(vec!["a".to_string(), "30023:aa:essay".to_string()]).unwrap(),
                Tag::parse(vec!["a".to_string(), "30023:bb:letter".to_string()]).unwrap(),
                Tag::parse(vec!["r".to_string(), "https://example.com".to_string()]).unwrap(),
                Tag::parse(vec!["t".to_string(), "attention".to_string()]).unwrap(),
            ])
            .sign_with_keys(&keys)
            .unwrap();
        process(&ndb, &event);
        std::thread::sleep(std::time::Duration::from_millis(50));

        let list = query_bookmarks(&ndb, &keys.public_key().to_hex()).unwrap();
        assert_eq!(list.addresses, vec!["30023:aa:essay", "30023:bb:letter"]);
        assert_eq!(list.other_tags.len(), 2);
        assert_eq!(list.content, "opaque");
    }

    #[test]
    fn query_bookmarks_returns_newest_when_multiple_cached() {
        let (ndb, _tmp) = fresh_ndb();
        let keys = Keys::generate();

        let older = EventBuilder::new(Kind::Custom(KIND_BOOKMARKS), "")
            .tags([Tag::parse(vec!["a".to_string(), "30023:aa:old".to_string()]).unwrap()])
            .custom_created_at(Timestamp::from(1_000u64))
            .sign_with_keys(&keys)
            .unwrap();
        let newer = EventBuilder::new(Kind::Custom(KIND_BOOKMARKS), "")
            .tags([Tag::parse(vec!["a".to_string(), "30023:aa:new".to_string()]).unwrap()])
            .custom_created_at(Timestamp::from(2_000u64))
            .sign_with_keys(&keys)
            .unwrap();

        process(&ndb, &older);
        process(&ndb, &newer);
        std::thread::sleep(std::time::Duration::from_millis(50));

        let list = query_bookmarks(&ndb, &keys.public_key().to_hex()).unwrap();
        assert_eq!(list.addresses, vec!["30023:aa:new"]);
    }

    #[test]
    fn is_bookmarked_matches_exact_address_only() {
        let (ndb, _tmp) = fresh_ndb();
        let keys = Keys::generate();

        let event = EventBuilder::new(Kind::Custom(KIND_BOOKMARKS), "")
            .tags([Tag::parse(vec!["a".to_string(), "30023:aa:essay".to_string()]).unwrap()])
            .sign_with_keys(&keys)
            .unwrap();
        process(&ndb, &event);
        std::thread::sleep(std::time::Duration::from_millis(50));

        let pk = keys.public_key().to_hex();
        assert!(is_bookmarked(&ndb, &pk, "30023:aa:essay").unwrap());
        assert!(!is_bookmarked(&ndb, &pk, "30023:aa:letter").unwrap());
        assert!(!is_bookmarked(&ndb, &pk, "30023:aa:").unwrap());
    }
}
