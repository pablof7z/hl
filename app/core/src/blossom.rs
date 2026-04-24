//! Blossom server list (BUD-03, kind:10063) read + publish, NIP-98 auth, and
//! BUD-01 PUT upload.
//!
//! The kind:10063 "User Server List" is a replaceable event; publishing a new
//! one supersedes the old one on every relay. Tags follow BUD-03: each server
//! is an `["server", "<url>"]` tag. Order is preserved — the first server in
//! the list is the upload default; fallback proceeds in list order.
//!
//! Uploads use BUD-01 auth (`kind:24242`, action=upload, x=sha256,
//! expiration=now+300) base64-encoded into an `Authorization: Nostr <b64>`
//! header. The server returns a JSON blob descriptor with the canonical URL.

use base64::{engine::general_purpose::STANDARD, Engine};
use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};
use sha2::{Digest, Sha256};

use crate::errors::CoreError;
use crate::models::BlossomUpload;
use crate::nostr_runtime::NostrRuntime;

const KIND_BLOSSOM_SERVERS: u16 = 10063;
const KIND_NIP98_HTTP_AUTH: u16 = 27235;
/// BUD-01 authorization event kind for Blossom uploads/deletes/listings.
const KIND_BLOSSOM_AUTH: u16 = 24242;
pub const DEFAULT_SERVER: &str = "https://blossom.primal.net";
/// Auth events expire 5 minutes after signing. The server enforces this.
const AUTH_EXPIRATION_SECS: u64 = 300;

// -- Reads --

/// Return the newest kind:10063 event for `user_hex` from nostrdb.
fn latest_server_list(ndb: &Ndb, user_hex: &str) -> Result<Option<Event>, CoreError> {
    if user_hex.is_empty() {
        return Ok(None);
    }
    let author = PublicKey::from_hex(user_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid user pubkey: {e}")))?;

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let pk_bytes: [u8; 32] = author.to_bytes();
    let filter = NdbFilter::new()
        .kinds([KIND_BLOSSOM_SERVERS as u64])
        .authors([&pk_bytes])
        .build();

    let results = ndb
        .query(&txn, &[filter], 8)
        .map_err(|e| CoreError::Cache(format!("query blossom servers: {e}")))?;

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

/// Extract ordered server URLs from `["server", "<url>"]` tags.
fn extract_server_tags(event: &Event) -> Vec<String> {
    let mut servers: Vec<String> = Vec::new();
    for tag in event.tags.iter() {
        let slice = tag.as_slice();
        if slice.first().map(String::as_str) == Some("server") {
            if let Some(url) = slice.get(1) {
                let trimmed = url.trim();
                if !trimmed.is_empty() {
                    servers.push(trimmed.to_string());
                }
            }
        }
    }
    servers
}

/// Return the user's ordered Blossom server list from nostrdb. Empty if none
/// cached yet (e.g. first login before the relay delivers kind:10063).
pub fn query_blossom_servers(ndb: &Ndb, user_hex: &str) -> Result<Vec<String>, CoreError> {
    match latest_server_list(ndb, user_hex)? {
        None => Ok(Vec::new()),
        Some(event) => Ok(extract_server_tags(&event)),
    }
}

// -- Writes --

fn parse_tag(parts: &[&str]) -> Result<Tag, CoreError> {
    Tag::parse(parts.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .map_err(|e| CoreError::Other(format!("build tag: {e}")))
}

/// Publish a new kind:10063 that replaces the user's current server list.
/// `servers` must be non-empty. Order is preserved as-is.
pub async fn publish_blossom_servers(
    runtime: &NostrRuntime,
    servers: Vec<String>,
) -> Result<String, CoreError> {
    if servers.is_empty() {
        return Err(CoreError::InvalidInput(
            "at least one blossom server required".into(),
        ));
    }

    let mut tags: Vec<Tag> = Vec::with_capacity(servers.len());
    for url in &servers {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            tags.push(parse_tag(&["server", trimmed])?);
        }
    }
    if tags.is_empty() {
        return Err(CoreError::InvalidInput("all server URLs were empty".into()));
    }

    let builder = EventBuilder::new(Kind::Custom(KIND_BLOSSOM_SERVERS), "").tags(tags);
    let client = runtime.client();
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign blossom servers: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish blossom servers: {e}")))?;
    Ok(event.id.to_hex())
}

/// Publish the default server list only if no kind:10063 is cached for the
/// user. Called once after login so every user has a working upload target.
/// No-op when the cache already has a list (avoids overwriting user's own
/// servers set from another client).
pub async fn init_default_blossom_servers(
    runtime: &NostrRuntime,
    user_hex: &str,
) -> Result<(), CoreError> {
    let existing = query_blossom_servers(runtime.ndb(), user_hex)?;
    if !existing.is_empty() {
        return Ok(());
    }
    publish_blossom_servers(runtime, vec![DEFAULT_SERVER.to_string()]).await?;
    Ok(())
}

// -- NIP-98 HTTP Auth --

/// Build and sign a kind:27235 NIP-98 HTTP auth event for use as a Blossom
/// upload `Authorization` header. Returns the raw JSON of the signed event;
/// the caller base64-encodes it and prefixes `"Nostr "`.
///
/// `payload_hash`: hex-encoded SHA-256 of the request body (required by
/// BUD-01 for PUT uploads).
pub async fn sign_nip98_auth(
    runtime: &NostrRuntime,
    url: &str,
    method: &str,
    payload_hash: Option<&str>,
) -> Result<String, CoreError> {
    let mut tags = vec![
        parse_tag(&["u", url])?,
        parse_tag(&["method", method])?,
    ];
    if let Some(hash) = payload_hash {
        tags.push(parse_tag(&["payload", hash])?);
    }

    let builder = EventBuilder::new(Kind::Custom(KIND_NIP98_HTTP_AUTH), "").tags(tags);
    let client = runtime.client();
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign nip98 auth: {e}")))?;
    Ok(event.as_json())
}

// -- BUD-01 upload --

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Build + sign a kind:24242 BUD-01 upload authorization event.
async fn sign_bud01_upload_auth(
    runtime: &NostrRuntime,
    sha256_hex_value: &str,
    note: &str,
) -> Result<Event, CoreError> {
    let expiration = Timestamp::now().as_secs() + AUTH_EXPIRATION_SECS;
    let tags = vec![
        parse_tag(&["t", "upload"])?,
        parse_tag(&["x", sha256_hex_value])?,
        parse_tag(&["expiration", &expiration.to_string()])?,
    ];
    let builder = EventBuilder::new(Kind::Custom(KIND_BLOSSOM_AUTH), note).tags(tags);
    runtime
        .client()
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign blossom upload auth: {e}")))
}

/// PUT `bytes` to `<server>/upload` with a BUD-01 `Authorization: Nostr <b64>`
/// header. Returns the parsed `BlossomUpload` descriptor.
///
/// `width`, `height`, and `alt` are stamped onto the returned record but are
/// NOT sent to the server — they're metadata the caller uses to build a
/// NIP-92 `imeta` tag on the publishing event. Pass `0` for unknown
/// dimensions; iOS callers always know dim post-recompression.
pub async fn upload_blob(
    runtime: &NostrRuntime,
    bytes: Vec<u8>,
    mime: String,
    width: u32,
    height: u32,
    alt: String,
) -> Result<BlossomUpload, CoreError> {
    if bytes.is_empty() {
        return Err(CoreError::InvalidInput("upload bytes are empty".into()));
    }
    let mime_clean = mime.trim();
    if mime_clean.is_empty() {
        return Err(CoreError::InvalidInput("mime type is required".into()));
    }

    let size_bytes = bytes.len() as u64;
    let sha = sha256_hex(&bytes);
    let auth = sign_bud01_upload_auth(runtime, &sha, "Upload book photo").await?;
    let auth_b64 = STANDARD.encode(auth.as_json().as_bytes());
    let endpoint = format!("{DEFAULT_SERVER}/upload");

    let client = reqwest::Client::new();
    let response = client
        .put(&endpoint)
        .header("Authorization", format!("Nostr {auth_b64}"))
        .header("Content-Type", mime_clean)
        .body(bytes)
        .send()
        .await
        .map_err(|e| CoreError::Network(format!("blossom PUT: {e}")))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(CoreError::Network(format!(
            "blossom upload failed: {status} {body}"
        )));
    }

    // Server returns a Blob descriptor. We need at least `url`. The rest we
    // already know locally (we just hashed/sized the bytes).
    let descriptor: serde_json::Value = response
        .json()
        .await
        .map_err(|e| CoreError::Network(format!("blossom response not JSON: {e}")))?;
    let url = descriptor
        .get("url")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| CoreError::Network("blossom response missing `url`".into()))?;

    Ok(BlossomUpload {
        url,
        sha256_hex: sha,
        mime: mime_clean.to_string(),
        size_bytes,
        width,
        height,
        alt,
    })
}

// -- Tests --

#[cfg(test)]
mod tests {
    use super::*;

    fn make_server_list_event(keys: &Keys, servers: &[&str], ts: u64) -> Event {
        let tags: Vec<Tag> = servers
            .iter()
            .map(|url| {
                Tag::parse(vec!["server".to_string(), url.to_string()]).expect("parse server tag")
            })
            .collect();
        EventBuilder::new(Kind::Custom(KIND_BLOSSOM_SERVERS), "")
            .tags(tags)
            .custom_created_at(Timestamp::from(ts))
            .sign_with_keys(keys)
            .expect("sign")
    }

    #[test]
    fn extract_server_tags_returns_ordered_urls() {
        let keys = Keys::generate();
        let event = make_server_list_event(
            &keys,
            &[
                "https://blossom.primal.net",
                "https://blossom.band",
                "https://media.nostr.band",
            ],
            1,
        );
        let servers = extract_server_tags(&event);
        assert_eq!(
            servers,
            vec![
                "https://blossom.primal.net",
                "https://blossom.band",
                "https://media.nostr.band",
            ]
        );
    }

    #[test]
    fn extract_server_tags_skips_non_server_tags() {
        let keys = Keys::generate();
        let tags = vec![
            Tag::parse(vec!["t".to_string(), "blossom".to_string()]).unwrap(),
            Tag::parse(vec!["server".to_string(), "https://blossom.primal.net".to_string()])
                .unwrap(),
        ];
        let event = EventBuilder::new(Kind::Custom(KIND_BLOSSOM_SERVERS), "")
            .tags(tags)
            .sign_with_keys(&keys)
            .expect("sign");
        let servers = extract_server_tags(&event);
        assert_eq!(servers, vec!["https://blossom.primal.net"]);
    }

    #[test]
    fn extract_server_tags_empty_event() {
        let keys = Keys::generate();
        let event = EventBuilder::new(Kind::Custom(KIND_BLOSSOM_SERVERS), "")
            .sign_with_keys(&keys)
            .expect("sign");
        assert!(extract_server_tags(&event).is_empty());
    }

    #[test]
    fn sha256_hex_is_lowercase_64_chars() {
        let h = sha256_hex(b"hello");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        // Known vector for "hello".
        assert_eq!(
            h,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn bud01_auth_event_has_required_tags() {
        // Build the event via the same path the upload function uses, but
        // sign locally so we can inspect the result without network IO.
        let keys = Keys::generate();
        let sha = sha256_hex(b"some bytes");
        let expiration = Timestamp::now().as_secs() + AUTH_EXPIRATION_SECS;
        let tags = vec![
            Tag::parse(vec!["t".to_string(), "upload".to_string()]).unwrap(),
            Tag::parse(vec!["x".to_string(), sha.clone()]).unwrap(),
            Tag::parse(vec!["expiration".to_string(), expiration.to_string()]).unwrap(),
        ];
        let event = EventBuilder::new(Kind::Custom(KIND_BLOSSOM_AUTH), "Upload book photo")
            .tags(tags)
            .sign_with_keys(&keys)
            .expect("sign");

        assert_eq!(event.kind, Kind::Custom(24242));
        let tag_pairs: Vec<(String, String)> = event
            .tags
            .iter()
            .filter_map(|t| {
                let s = t.as_slice();
                Some((s.first()?.clone(), s.get(1)?.clone()))
            })
            .collect();
        assert!(tag_pairs.contains(&("t".into(), "upload".into())));
        assert!(tag_pairs.contains(&("x".into(), sha)));
        assert!(tag_pairs
            .iter()
            .any(|(k, v)| k == "expiration" && v.parse::<u64>().is_ok()));
    }
}
