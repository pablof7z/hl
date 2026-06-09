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
use std::collections::BTreeSet;

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

/// Native article bookmark state projection. Rust owns canonical address
/// trimming, current membership, and the optimistic post-toggle set.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ArticleBookmarkStateProjection {
    pub canonical_address: String,
    pub can_toggle: bool,
    pub is_bookmarked: bool,
    pub optimistic_addresses: Vec<String>,
}

/// Native article bookmark state input.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ArticleBookmarkStateProjectionInput {
    pub addresses: Vec<String>,
    pub address: String,
}

/// Native event bookmark state projection. Used for comment bookmarks and
/// other event-id-addressed NIP-51 entries.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct EventBookmarkStateProjection {
    pub canonical_event_id_hex: String,
    pub can_apply: bool,
    pub is_bookmarked: bool,
    pub optimistic_event_ids: Vec<String>,
}

/// Native event bookmark state input. `desired_member == None` toggles;
/// `Some(true/false)` projects an authoritative member/non-member state.
#[derive(Debug, Clone, uniffi::Record)]
pub struct EventBookmarkStateProjectionInput {
    pub event_ids: Vec<String>,
    pub event_id_hex: String,
    pub desired_member: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ArticleBookmarkChromeProjectionInput {
    pub is_bookmarked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ArticleBookmarkChromeProjection {
    pub toolbar_system_image: String,
    pub uses_accent_color: bool,
    pub accessibility_label: String,
    pub swipe_title: String,
    pub menu_title: String,
    pub action_system_image: String,
}

// -- Public API --------------------------------------------------------------

pub fn article_bookmark_state_projection(
    input: ArticleBookmarkStateProjectionInput,
) -> ArticleBookmarkStateProjection {
    let canonical_address = input.address.trim().to_string();
    let mut addresses = input
        .addresses
        .into_iter()
        .map(|address| address.trim().to_string())
        .filter(|address| !address.is_empty())
        .collect::<BTreeSet<_>>();
    let can_toggle = !canonical_address.is_empty();
    let is_bookmarked = can_toggle && addresses.contains(&canonical_address);
    if can_toggle {
        if is_bookmarked {
            addresses.remove(&canonical_address);
        } else {
            addresses.insert(canonical_address.clone());
        }
    }
    ArticleBookmarkStateProjection {
        canonical_address,
        can_toggle,
        is_bookmarked,
        optimistic_addresses: addresses.into_iter().collect(),
    }
}

pub fn event_bookmark_state_projection(
    input: EventBookmarkStateProjectionInput,
) -> EventBookmarkStateProjection {
    let mut event_ids = input
        .event_ids
        .into_iter()
        .filter_map(|event_id| canonical_event_id_hex(&event_id))
        .collect::<BTreeSet<_>>();
    let Some(canonical_event_id_hex) = canonical_event_id_hex(&input.event_id_hex) else {
        return EventBookmarkStateProjection {
            canonical_event_id_hex: String::new(),
            can_apply: false,
            is_bookmarked: false,
            optimistic_event_ids: event_ids.into_iter().collect(),
        };
    };
    let is_bookmarked = event_ids.contains(&canonical_event_id_hex);
    match input.desired_member {
        Some(true) => {
            event_ids.insert(canonical_event_id_hex.clone());
        }
        Some(false) => {
            event_ids.remove(&canonical_event_id_hex);
        }
        None if is_bookmarked => {
            event_ids.remove(&canonical_event_id_hex);
        }
        None => {
            event_ids.insert(canonical_event_id_hex.clone());
        }
    }
    EventBookmarkStateProjection {
        canonical_event_id_hex,
        can_apply: true,
        is_bookmarked,
        optimistic_event_ids: event_ids.into_iter().collect(),
    }
}

pub fn article_bookmark_chrome_projection(
    input: ArticleBookmarkChromeProjectionInput,
) -> ArticleBookmarkChromeProjection {
    if input.is_bookmarked {
        ArticleBookmarkChromeProjection {
            toolbar_system_image: "bookmark.fill".into(),
            uses_accent_color: true,
            accessibility_label: "Remove bookmark".into(),
            swipe_title: "Remove".into(),
            menu_title: "Remove bookmark".into(),
            action_system_image: "bookmark.slash".into(),
        }
    } else {
        ArticleBookmarkChromeProjection {
            toolbar_system_image: "bookmark".into(),
            uses_accent_color: false,
            accessibility_label: "Bookmark article".into(),
            swipe_title: "Bookmark".into(),
            menu_title: "Bookmark".into(),
            action_system_image: "bookmark".into(),
        }
    }
}

fn canonical_event_id_hex(value: &str) -> Option<String> {
    EventId::from_hex(value.trim()).ok().map(|id| id.to_hex())
}

/// Read the newest cached kind:10003 for `user_hex` and return the set of
/// addressable-event bookmarks it carries. Empty list when none cached.
pub fn query_bookmarks(ndb: &Ndb, user_hex: &str) -> Result<BookmarkList, CoreError> {
    if user_hex.is_empty() {
        return Ok(BookmarkList::default());
    }
    let author = PublicKey::from_hex(user_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid user pubkey: {e}")))?;
    let pk_bytes: [u8; 32] = author.to_bytes();

    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let filter = NdbFilter::new()
        .kinds([KIND_BOOKMARKS as u64])
        .authors([&pk_bytes])
        .build();
    let results = ndb
        .query(&txn, &[filter], 8)
        .map_err(|e| CoreError::Cache(format!("query bookmarks: {e}")))?;

    let mut newest: Option<Event> = None;
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
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

    #[test]
    fn article_bookmark_state_projection_trims_dedupes_and_toggles() {
        let added = article_bookmark_state_projection(ArticleBookmarkStateProjectionInput {
            addresses: vec![
                "30023:aa:essay".into(),
                " 30023:aa:essay ".into(),
                " ".into(),
            ],
            address: " 30023:bb:letter\n".into(),
        });
        let removed = article_bookmark_state_projection(ArticleBookmarkStateProjectionInput {
            addresses: vec!["30023:aa:essay".into(), "30023:bb:letter".into()],
            address: "30023:aa:essay".into(),
        });
        let blank = article_bookmark_state_projection(ArticleBookmarkStateProjectionInput {
            addresses: vec!["30023:aa:essay".into()],
            address: " \n ".into(),
        });

        assert_eq!(added.canonical_address, "30023:bb:letter");
        assert!(added.can_toggle);
        assert!(!added.is_bookmarked);
        assert_eq!(
            added.optimistic_addresses,
            vec!["30023:aa:essay", "30023:bb:letter"]
        );

        assert!(removed.is_bookmarked);
        assert_eq!(removed.optimistic_addresses, vec!["30023:bb:letter"]);

        assert_eq!(blank.canonical_address, "");
        assert!(!blank.can_toggle);
        assert!(!blank.is_bookmarked);
        assert_eq!(blank.optimistic_addresses, vec!["30023:aa:essay"]);
    }

    #[test]
    fn article_bookmark_chrome_projection_matches_bookmark_state() {
        let unbookmarked =
            article_bookmark_chrome_projection(ArticleBookmarkChromeProjectionInput {
                is_bookmarked: false,
            });
        assert_eq!(
            unbookmarked,
            ArticleBookmarkChromeProjection {
                toolbar_system_image: "bookmark".into(),
                uses_accent_color: false,
                accessibility_label: "Bookmark article".into(),
                swipe_title: "Bookmark".into(),
                menu_title: "Bookmark".into(),
                action_system_image: "bookmark".into(),
            }
        );

        let bookmarked = article_bookmark_chrome_projection(ArticleBookmarkChromeProjectionInput {
            is_bookmarked: true,
        });
        assert_eq!(
            bookmarked,
            ArticleBookmarkChromeProjection {
                toolbar_system_image: "bookmark.fill".into(),
                uses_accent_color: true,
                accessibility_label: "Remove bookmark".into(),
                swipe_title: "Remove".into(),
                menu_title: "Remove bookmark".into(),
                action_system_image: "bookmark.slash".into(),
            }
        );
    }

    #[test]
    fn event_bookmark_state_projection_toggles_and_applies_authoritative_state() {
        let event_a = "a".repeat(64);
        let event_b = "b".repeat(64);
        let added = event_bookmark_state_projection(EventBookmarkStateProjectionInput {
            event_ids: vec![format!(" {event_a} "), event_a.to_uppercase()],
            event_id_hex: format!(" {event_b}\n"),
            desired_member: None,
        });
        let removed = event_bookmark_state_projection(EventBookmarkStateProjectionInput {
            event_ids: vec![event_a.clone(), event_b.clone()],
            event_id_hex: event_a.clone(),
            desired_member: None,
        });
        let forced_member = event_bookmark_state_projection(EventBookmarkStateProjectionInput {
            event_ids: vec![event_a.clone()],
            event_id_hex: event_b.clone(),
            desired_member: Some(true),
        });
        let forced_non_member =
            event_bookmark_state_projection(EventBookmarkStateProjectionInput {
                event_ids: vec![event_a.clone(), event_b.clone()],
                event_id_hex: event_b.clone(),
                desired_member: Some(false),
            });
        let invalid = event_bookmark_state_projection(EventBookmarkStateProjectionInput {
            event_ids: vec![event_a.clone()],
            event_id_hex: "not an event".into(),
            desired_member: None,
        });

        assert_eq!(added.canonical_event_id_hex, event_b);
        assert!(added.can_apply);
        assert!(!added.is_bookmarked);
        assert_eq!(
            added.optimistic_event_ids,
            vec![event_a.clone(), event_b.clone()]
        );

        assert!(removed.is_bookmarked);
        assert_eq!(removed.optimistic_event_ids, vec![event_b.clone()]);
        assert_eq!(
            forced_member.optimistic_event_ids,
            vec![event_a.clone(), event_b.clone()]
        );
        assert_eq!(
            forced_non_member.optimistic_event_ids,
            vec![event_a.clone()]
        );

        assert_eq!(invalid.canonical_event_id_hex, "");
        assert!(!invalid.can_apply);
        assert_eq!(invalid.optimistic_event_ids, vec![event_a]);
    }
}
