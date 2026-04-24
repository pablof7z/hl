//! NIP-02 contact list (kind:3) read + mutate. Broadest-interop follow list;
//! every Nostr client reads kind:3, so this is what the iOS profile's
//! Follow/Unfollow button publishes.

use std::collections::HashSet;

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;
use crate::nostr_runtime::NostrRuntime;

const KIND_CONTACTS: u16 = 3;

/// Return the newest kind:3 event authored by `follower_hex` from nostrdb.
/// `None` means the cache has no contact list yet — the caller should treat
/// that as an empty follow set on first use.
fn latest_contact_list(ndb: &Ndb, follower_hex: &str) -> Result<Option<Event>, CoreError> {
    if follower_hex.is_empty() {
        return Ok(None);
    }
    let author = PublicKey::from_hex(follower_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid follower pubkey: {e}")))?;

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let pk_bytes: [u8; 32] = author.to_bytes();
    let filter = NdbFilter::new()
        .kinds([KIND_CONTACTS as u64])
        .authors([&pk_bytes])
        .build();

    let results = ndb
        .query(&txn, &[filter], 8)
        .map_err(|e| CoreError::Cache(format!("query contacts: {e}")))?;

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
    Ok(newest)
}

/// Pubkeys in `follower_hex`'s latest cached contact list. Empty if none cached.
pub fn query_follows(ndb: &Ndb, follower_hex: &str) -> Result<Vec<String>, CoreError> {
    match latest_contact_list(ndb, follower_hex)? {
        None => Ok(Vec::new()),
        Some(event) => Ok(extract_p_tags(&event)),
    }
}

/// Does `follower_hex`'s cached contact list include `target_hex`?
pub fn is_following(
    ndb: &Ndb,
    follower_hex: &str,
    target_hex: &str,
) -> Result<bool, CoreError> {
    if follower_hex.is_empty() || target_hex.is_empty() {
        return Ok(false);
    }
    let follows = query_follows(ndb, follower_hex)?;
    Ok(follows.iter().any(|p| p.eq_ignore_ascii_case(target_hex)))
}

/// Publish a new kind:3 that adds or removes `target_hex` from the follower's
/// contact list. Preserves every other `p` tag and any non-`p` tags on the
/// current list. Signs via the installed Client signer and publishes.
///
/// Idempotent: adding a pubkey that's already followed, or removing one that
/// isn't, succeeds without republishing. Returns the event id that was
/// published (or `None` on a no-op).
pub async fn publish_follow_toggle(
    runtime: &NostrRuntime,
    follower_hex: &str,
    target_hex: &str,
    follow: bool,
) -> Result<Option<String>, CoreError> {
    let target = PublicKey::from_hex(target_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid target pubkey: {e}")))?;
    if follower_hex.eq_ignore_ascii_case(target_hex) {
        return Err(CoreError::InvalidInput("cannot follow yourself".into()));
    }

    let current = latest_contact_list(runtime.ndb(), follower_hex)?;
    let (new_tags, existing_content, changed) = next_contact_tags(current.as_ref(), target, follow);
    if !changed {
        return Ok(None);
    }

    let builder = EventBuilder::new(Kind::Custom(KIND_CONTACTS), existing_content).tags(new_tags);
    let client = runtime.client();
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign contact list: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish contact list: {e}")))?;
    crate::nostr_runtime::mirror_social_trio_to_purple(client, &event).await;
    Ok(Some(event.id.to_hex()))
}

/// Pure: take the current (optional) contact list event, the target, and the
/// desired state, and produce the next tag set + whether anything changed.
///
/// - Preserves every existing tag (p tags and others), only touching the one
///   `p` tag for `target`.
/// - When adding, appends `["p", target_hex]` at the end.
/// - When removing, drops every p-tag whose value matches target.
/// - Content is preserved verbatim so relay list blobs don't get clobbered.
fn next_contact_tags(
    current: Option<&Event>,
    target: PublicKey,
    follow: bool,
) -> (Vec<Tag>, String, bool) {
    let target_lower = target.to_hex().to_ascii_lowercase();
    let mut tags: Vec<Tag> = Vec::new();
    let mut existing_p: HashSet<String> = HashSet::new();
    let mut removed_target = false;
    let mut content = String::new();

    if let Some(event) = current {
        content = event.content.clone();
        for tag in event.tags.iter() {
            let slice = tag.as_slice();
            match slice.first().map(String::as_str) {
                Some("p") => {
                    let Some(v) = slice.get(1) else { continue };
                    let v_lower = v.to_ascii_lowercase();
                    if v_lower == target_lower {
                        if follow {
                            // Already following; keep (dedup into existing_p below).
                        } else {
                            removed_target = true;
                            continue;
                        }
                    }
                    if existing_p.insert(v_lower) {
                        tags.push(tag.clone());
                    }
                }
                _ => tags.push(tag.clone()),
            }
        }
    }

    if follow {
        if existing_p.contains(&target_lower) {
            return (tags, content, false);
        }
        tags.push(Tag::public_key(target));
        return (tags, content, true);
    }

    (tags, content, removed_target)
}

fn extract_p_tags(event: &Event) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for tag in event.tags.iter() {
        let s = tag.as_slice();
        if s.first().map(String::as_str) != Some("p") {
            continue;
        }
        if let Some(pk) = s.get(1) {
            let lower = pk.to_ascii_lowercase();
            if seen.insert(lower) {
                out.push(pk.to_string());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sign_contacts(keys: &Keys, follows: &[PublicKey], content: &str, ts: u64) -> Event {
        let tags: Vec<Tag> = follows.iter().map(|pk| Tag::public_key(*pk)).collect();
        EventBuilder::new(Kind::Custom(KIND_CONTACTS), content)
            .tags(tags)
            .custom_created_at(Timestamp::from(ts))
            .sign_with_keys(keys)
            .expect("sign")
    }

    #[test]
    fn extract_p_tags_dedupes_case_insensitive() {
        let a = Keys::generate();
        let b = Keys::generate();
        let event = EventBuilder::new(Kind::Custom(KIND_CONTACTS), "")
            .tags(vec![
                Tag::public_key(a.public_key()),
                Tag::public_key(b.public_key()),
                Tag::public_key(a.public_key()),
            ])
            .sign_with_keys(&Keys::generate())
            .expect("sign");
        let out = extract_p_tags(&event);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn next_contact_tags_adds_to_empty() {
        let target = Keys::generate();
        let (tags, content, changed) = next_contact_tags(None, target.public_key(), true);
        assert!(changed);
        assert_eq!(content, "");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].as_slice()[0], "p");
        assert_eq!(tags[0].as_slice()[1], target.public_key().to_hex());
    }

    #[test]
    fn next_contact_tags_noop_when_already_following() {
        let me = Keys::generate();
        let target = Keys::generate();
        let existing = sign_contacts(&me, &[target.public_key()], "kept", 1);
        let (tags, content, changed) =
            next_contact_tags(Some(&existing), target.public_key(), true);
        assert!(!changed);
        assert_eq!(tags.len(), 1);
        assert_eq!(content, "kept");
    }

    #[test]
    fn next_contact_tags_removes_target_preserving_others() {
        let me = Keys::generate();
        let other = Keys::generate();
        let target = Keys::generate();
        let existing =
            sign_contacts(&me, &[other.public_key(), target.public_key()], "blob", 1);
        let (tags, content, changed) =
            next_contact_tags(Some(&existing), target.public_key(), false);
        assert!(changed);
        assert_eq!(content, "blob");
        let p_values: Vec<_> = tags
            .iter()
            .filter(|t| t.as_slice().first().map(String::as_str) == Some("p"))
            .map(|t| t.as_slice()[1].clone())
            .collect();
        assert_eq!(p_values, vec![other.public_key().to_hex()]);
    }

    #[test]
    fn next_contact_tags_noop_when_unfollowing_unknown() {
        let me = Keys::generate();
        let target = Keys::generate();
        let existing = sign_contacts(&me, &[], "", 1);
        let (_, _, changed) = next_contact_tags(Some(&existing), target.public_key(), false);
        assert!(!changed);
    }

    #[test]
    fn next_contact_tags_preserves_non_p_tags() {
        let me = Keys::generate();
        let target = Keys::generate();
        // Some clients include `t` / `e` tags on kind:3 for relay hints.
        let event = EventBuilder::new(Kind::Custom(KIND_CONTACTS), "content")
            .tags(vec![
                Tag::parse(vec!["t".to_string(), "community".to_string()]).unwrap(),
                Tag::public_key(me.public_key()),
            ])
            .custom_created_at(Timestamp::from(1))
            .sign_with_keys(&me)
            .expect("sign");
        let (tags, _, _) = next_contact_tags(Some(&event), target.public_key(), true);
        // t tag must survive
        assert!(tags
            .iter()
            .any(|t| t.as_slice().first().map(String::as_str) == Some("t")));
    }
}
