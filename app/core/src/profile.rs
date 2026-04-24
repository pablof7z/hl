//! NIP-01 kind:0 profile metadata query. The profile view reads from nostrdb
//! first; the relay-side hydrate happens via
//! `SubscriptionKind::UserProfile` so stale cache rows get refreshed while
//! the view is open.

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::errors::CoreError;
use crate::models::ProfileMetadata;
use crate::nostr_runtime::{mirror_social_trio_to_purple, NostrRuntime};

const KIND_METADATA: u16 = 0;

/// Read the newest kind:0 event for `pubkey_hex` out of nostrdb and parse its
/// JSON content into a `ProfileMetadata`. Returns `None` when no metadata is
/// cached yet; the caller can still render a pubkey-only view while the
/// subscription fills in.
pub fn query_profile_from_ndb(
    ndb: &Ndb,
    pubkey_hex: &str,
) -> Result<Option<ProfileMetadata>, CoreError> {
    if pubkey_hex.is_empty() {
        return Ok(None);
    }
    let author = PublicKey::from_hex(pubkey_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let pk_bytes: [u8; 32] = author.to_bytes();
    let filter = NdbFilter::new()
        .kinds([KIND_METADATA as u64])
        .authors([&pk_bytes])
        .build();

    let results = ndb
        .query(&txn, &[filter], 16)
        .map_err(|e| CoreError::Cache(format!("query profile: {e}")))?;

    // Nostrdb may return several kind:0s for this pubkey if relays delivered
    // older revisions. Keep the newest by `created_at`.
    let mut newest: Option<Event> = None;
    for result in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, result.note_key) else {
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

    let Some(event) = newest else {
        return Ok(None);
    };
    Ok(Some(parse_metadata(&event)))
}

/// Pure: parse a kind:0 event into a `ProfileMetadata`. Unknown fields are
/// silently dropped; a completely unparseable body yields a record with only
/// the pubkey populated so the view still has something to render.
pub fn parse_metadata(event: &Event) -> ProfileMetadata {
    let pubkey = event.pubkey.to_hex();
    let created_at = Some(event.created_at.as_secs());
    let raw: RawMetadata = serde_json::from_str(&event.content).unwrap_or_default();

    ProfileMetadata {
        pubkey,
        name: raw.name.unwrap_or_default().trim().to_string(),
        display_name: raw
            .display_name
            .or(raw.displayname)
            .unwrap_or_default()
            .trim()
            .to_string(),
        about: raw.about.unwrap_or_default().trim().to_string(),
        picture: raw.picture.or(raw.image).unwrap_or_default().trim().to_string(),
        banner: raw.banner.unwrap_or_default().trim().to_string(),
        nip05: raw.nip05.unwrap_or_default().trim().to_string(),
        website: raw.website.unwrap_or_default().trim().to_string(),
        lud16: raw.lud16.unwrap_or_default().trim().to_string(),
        created_at,
    }
}

/// JSON shape of the kind:0 content blob. Tolerates both `display_name` (spec)
/// and `displayName` / `displayname` (seen in the wild). Missing fields stay
/// `None`.
#[derive(Debug, Default, Deserialize)]
struct RawMetadata {
    name: Option<String>,
    #[serde(alias = "displayName")]
    display_name: Option<String>,
    displayname: Option<String>,
    about: Option<String>,
    picture: Option<String>,
    image: Option<String>,
    banner: Option<String>,
    nip05: Option<String>,
    website: Option<String>,
    lud16: Option<String>,
}

/// Publish a fresh kind:0 metadata event for the current user. Preserves
/// any unknown fields the user may have set from another client (e.g.
/// `pronouns`, `bot`, `picture_animated`) — we deserialise the existing
/// content as a JSON object and overwrite only the canonical fields the
/// edit form drives. Falls back to a brand-new object if no kind:0 is
/// cached.
///
/// After the standard `send_event` broadcast, mirrors to
/// `PURPLE_PAGES_RELAY` so the canonical social-trio store always has
/// the latest revision (other Nostr clients look there for kind:0).
/// Returns the parsed `ProfileMetadata` so the caller's UI can swap to
/// the new state without waiting for the relay echo.
pub async fn publish_profile(
    runtime: &NostrRuntime,
    name: &str,
    display_name: &str,
    about: &str,
    picture: &str,
    banner: &str,
    nip05: &str,
    website: &str,
    lud16: &str,
) -> Result<ProfileMetadata, CoreError> {
    // Recover the current user's pubkey from the active signer so we can
    // load their existing kind:0 from cache.
    let client = runtime.client();
    let signer = client
        .signer()
        .await
        .map_err(|e| CoreError::Signer(format!("get signer: {e}")))?;
    let user_pubkey = signer
        .get_public_key()
        .await
        .map_err(|e| CoreError::Signer(format!("get pubkey: {e}")))?;

    // Start from any existing JSON so unknown keys round-trip.
    let mut content: Value = match query_raw_metadata_json(runtime.ndb(), &user_pubkey.to_hex())? {
        Some(v) if v.is_object() => v,
        _ => json!({}),
    };
    let obj = content
        .as_object_mut()
        .expect("guaranteed to be a JSON object");

    set_or_clear(obj, "name", name);
    set_or_clear(obj, "display_name", display_name);
    set_or_clear(obj, "about", about);
    set_or_clear(obj, "picture", picture);
    set_or_clear(obj, "banner", banner);
    set_or_clear(obj, "nip05", nip05);
    set_or_clear(obj, "website", website);
    set_or_clear(obj, "lud16", lud16);

    let body = serde_json::to_string(&content)
        .map_err(|e| CoreError::Other(format!("serialise metadata: {e}")))?;

    let builder = EventBuilder::new(Kind::Custom(KIND_METADATA), body);
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign metadata: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish metadata: {e}")))?;
    mirror_social_trio_to_purple(client, &event).await;

    Ok(parse_metadata(&event))
}

/// Set `key` to `value` if non-empty (after trim), otherwise remove the
/// key entirely. Removing rather than writing `""` keeps a cleared field
/// from re-appearing as a stale empty string on clients that just check
/// for key presence.
fn set_or_clear(obj: &mut serde_json::Map<String, Value>, key: &str, value: &str) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        obj.remove(key);
    } else {
        obj.insert(key.to_string(), Value::String(trimmed.to_string()));
    }
}

/// Newest cached kind:0 for `pubkey_hex`, parsed as a JSON value (so the
/// caller can preserve unknown fields). `None` when no kind:0 is cached.
fn query_raw_metadata_json(ndb: &Ndb, pubkey_hex: &str) -> Result<Option<Value>, CoreError> {
    if pubkey_hex.is_empty() {
        return Ok(None);
    }
    let author = PublicKey::from_hex(pubkey_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;
    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let pk_bytes: [u8; 32] = author.to_bytes();
    let filter = NdbFilter::new()
        .kinds([KIND_METADATA as u64])
        .authors([&pk_bytes])
        .build();
    let results = ndb
        .query(&txn, &[filter], 16)
        .map_err(|e| CoreError::Cache(format!("query profile: {e}")))?;
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
    let Some(event) = newest else { return Ok(None) };
    Ok(serde_json::from_str::<Value>(&event.content).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sign_metadata(keys: &Keys, json: &str) -> Event {
        EventBuilder::new(Kind::Custom(KIND_METADATA), json)
            .sign_with_keys(keys)
            .expect("sign")
    }

    #[test]
    fn parses_standard_fields() {
        let keys = Keys::generate();
        let event = sign_metadata(
            &keys,
            r#"{
                "name": "alice",
                "display_name": "Alice Smith",
                "about": " hey ",
                "picture": "https://x/p.png",
                "banner": "https://x/b.png",
                "nip05": "alice@x",
                "website": "https://x",
                "lud16": "alice@x"
            }"#,
        );
        let p = parse_metadata(&event);
        assert_eq!(p.name, "alice");
        assert_eq!(p.display_name, "Alice Smith");
        assert_eq!(p.about, "hey");
        assert_eq!(p.picture, "https://x/p.png");
        assert_eq!(p.banner, "https://x/b.png");
        assert_eq!(p.nip05, "alice@x");
        assert_eq!(p.website, "https://x");
        assert_eq!(p.lud16, "alice@x");
    }

    #[test]
    fn falls_back_from_display_name_alias() {
        let keys = Keys::generate();
        let event = sign_metadata(&keys, r#"{"displayName": "CamelCase"}"#);
        let p = parse_metadata(&event);
        assert_eq!(p.display_name, "CamelCase");
    }

    #[test]
    fn image_substitutes_for_missing_picture() {
        let keys = Keys::generate();
        let event = sign_metadata(&keys, r#"{"image": "https://x/i.png"}"#);
        let p = parse_metadata(&event);
        assert_eq!(p.picture, "https://x/i.png");
    }

    #[test]
    fn unparseable_content_yields_pubkey_only_record() {
        let keys = Keys::generate();
        let event = sign_metadata(&keys, "not json");
        let p = parse_metadata(&event);
        assert_eq!(p.pubkey, keys.public_key().to_hex());
        assert!(p.name.is_empty());
        assert!(p.about.is_empty());
    }
}
