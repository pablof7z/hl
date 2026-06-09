//! Blossom server list (BUD-03, kind:10063) read + publish, and BUD-01 PUT
//! upload.
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

use crate::clock::Clock;
use crate::errors::CoreError;
use crate::models::BlossomUpload;
use crate::nostr_runtime::NostrRuntime;

const KIND_BLOSSOM_SERVERS: u16 = 10063;
/// BUD-01 authorization event kind for Blossom uploads/deletes/listings.
const KIND_BLOSSOM_AUTH: u16 = 24242;
pub const DEFAULT_SERVER: &str = "https://blossom.primal.net";
/// Auth events expire 5 minutes after signing. The server enforces this.
const AUTH_EXPIRATION_SECS: u64 = 300;

/// Native add-server sheet input. Rust owns URL normalization and duplicate
/// checks against the visible ordered server list.
#[derive(Debug, Clone, uniffi::Record)]
pub struct BlossomServerEntryProjectionInput {
    pub url: String,
    pub existing_servers: Vec<String>,
}

/// Native add-server sheet projection. Rust owns validity and add eligibility.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BlossomServerEntryProjection {
    pub submit_url: String,
    pub is_valid: bool,
    pub is_duplicate: bool,
    pub can_add: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BlossomServerListProjectionInput {
    pub servers: Vec<String>,
    pub add_url: Option<String>,
    pub remove_indexes: Vec<u64>,
    pub move_indexes: Vec<u64>,
    pub move_to_index: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BlossomServerListProjection {
    pub servers: Vec<String>,
    pub can_save: bool,
}

// -- Reads --

/// Return the newest kind:10063 event for `user_hex` from nostrdb.
fn latest_server_list(ndb: &Ndb, user_hex: &str) -> Result<Option<Event>, CoreError> {
    if user_hex.is_empty() {
        return Ok(None);
    }
    let author = PublicKey::from_hex(user_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid user pubkey: {e}")))?;

    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

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

/// Project the add-server sheet for Blossom media settings. Native shells
/// render the returned flags and pass `submit_url` to the add action.
pub fn blossom_server_entry_projection(
    input: BlossomServerEntryProjectionInput,
) -> BlossomServerEntryProjection {
    let submit_url = input.url.trim().to_string();
    let is_valid = submit_url.starts_with("https://") || submit_url.starts_with("http://");
    let is_duplicate = input
        .existing_servers
        .iter()
        .any(|server| server.trim() == submit_url);
    BlossomServerEntryProjection {
        can_add: is_valid && !is_duplicate,
        submit_url,
        is_valid,
        is_duplicate,
    }
}

/// Project edits to the ordered Blossom server list. Native shells own the
/// platform list control; Rust owns URL normalization, duplicate filtering,
/// delete-last protection, and save eligibility.
pub fn blossom_server_list_projection(
    input: BlossomServerListProjectionInput,
) -> BlossomServerListProjection {
    let mut servers = normalize_server_list(input.servers);

    if let Some(url) = input.add_url {
        let entry = blossom_server_entry_projection(BlossomServerEntryProjectionInput {
            url,
            existing_servers: servers.clone(),
        });
        if entry.can_add {
            servers.push(entry.submit_url);
        }
    }

    let mut remove_indexes: Vec<usize> = input
        .remove_indexes
        .into_iter()
        .filter_map(|index| usize::try_from(index).ok())
        .filter(|index| *index < servers.len())
        .collect();
    remove_indexes.sort_unstable();
    remove_indexes.dedup();
    if !remove_indexes.is_empty() && servers.len() > remove_indexes.len() {
        for index in remove_indexes.into_iter().rev() {
            if index < servers.len() {
                servers.remove(index);
            }
        }
    }

    if let Some(to_index) = input.move_to_index {
        move_servers(&mut servers, input.move_indexes, to_index);
    }

    BlossomServerListProjection {
        can_save: !servers.is_empty(),
        servers,
    }
}

fn normalize_server_list(servers: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for server in servers {
        let trimmed = server.trim();
        if !trimmed.is_empty() && !out.iter().any(|existing| existing == trimmed) {
            out.push(trimmed.to_string());
        }
    }
    out
}

fn move_servers(servers: &mut Vec<String>, indexes: Vec<u64>, to_index: u64) {
    if servers.len() < 2 {
        return;
    }

    let mut indexes: Vec<usize> = indexes
        .into_iter()
        .filter_map(|index| usize::try_from(index).ok())
        .filter(|index| *index < servers.len())
        .collect();
    indexes.sort_unstable();
    indexes.dedup();
    if indexes.is_empty() || indexes.len() >= servers.len() {
        return;
    }

    let mut moved = Vec::with_capacity(indexes.len());
    for index in indexes.iter().rev() {
        moved.push(servers.remove(*index));
    }
    moved.reverse();

    let raw_to_index = usize::try_from(to_index)
        .ok()
        .unwrap_or(usize::MAX)
        .min(servers.len() + moved.len());
    let indexes_before_target = indexes
        .iter()
        .filter(|index| **index < raw_to_index)
        .count();
    let insertion_index = raw_to_index
        .saturating_sub(indexes_before_target)
        .min(servers.len());

    for (offset, server) in moved.into_iter().enumerate() {
        servers.insert(insertion_index + offset, server);
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
    clock: &dyn Clock,
) -> Result<Event, CoreError> {
    let tags = bud01_upload_auth_tags(sha256_hex_value, clock.now_unix_seconds())?;
    let builder = EventBuilder::new(Kind::Custom(KIND_BLOSSOM_AUTH), note).tags(tags);
    runtime
        .client()
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign blossom upload auth: {e}")))
}

fn bud01_upload_auth_tags(
    sha256_hex_value: &str,
    now_unix_seconds: u64,
) -> Result<Vec<Tag>, CoreError> {
    let expiration = now_unix_seconds + AUTH_EXPIRATION_SECS;
    Ok(vec![
        parse_tag(&["t", "upload"])?,
        parse_tag(&["x", sha256_hex_value])?,
        parse_tag(&["expiration", &expiration.to_string()])?,
    ])
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
    clock: &dyn Clock,
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
    let auth = sign_bud01_upload_auth(runtime, &sha, "Upload book photo", clock).await?;
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
            Tag::parse(vec![
                "server".to_string(),
                "https://blossom.primal.net".to_string(),
            ])
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
    fn blossom_server_entry_projection_trims_validates_and_rejects_duplicates() {
        let projection = blossom_server_entry_projection(BlossomServerEntryProjectionInput {
            url: "  https://media.example.com  ".into(),
            existing_servers: vec!["https://blossom.primal.net".into()],
        });
        let invalid = blossom_server_entry_projection(BlossomServerEntryProjectionInput {
            url: "wss://relay.example.com".into(),
            existing_servers: Vec::new(),
        });
        let duplicate = blossom_server_entry_projection(BlossomServerEntryProjectionInput {
            url: " https://blossom.primal.net ".into(),
            existing_servers: vec!["https://blossom.primal.net".into()],
        });

        assert_eq!(projection.submit_url, "https://media.example.com");
        assert!(projection.is_valid);
        assert!(!projection.is_duplicate);
        assert!(projection.can_add);
        assert!(!invalid.is_valid);
        assert!(!invalid.can_add);
        assert!(duplicate.is_duplicate);
        assert!(!duplicate.can_add);
    }

    #[test]
    fn blossom_server_list_projection_adds_deletes_and_protects_last_server() {
        let added = blossom_server_list_projection(BlossomServerListProjectionInput {
            servers: vec![" https://blossom.primal.net ".into()],
            add_url: Some(" https://media.example.com ".into()),
            remove_indexes: Vec::new(),
            move_indexes: Vec::new(),
            move_to_index: None,
        });
        assert_eq!(
            added.servers,
            vec!["https://blossom.primal.net", "https://media.example.com"]
        );
        assert!(added.can_save);

        let duplicate = blossom_server_list_projection(BlossomServerListProjectionInput {
            servers: added.servers.clone(),
            add_url: Some("https://media.example.com".into()),
            remove_indexes: Vec::new(),
            move_indexes: Vec::new(),
            move_to_index: None,
        });
        assert_eq!(duplicate.servers, added.servers);

        let removed = blossom_server_list_projection(BlossomServerListProjectionInput {
            servers: duplicate.servers,
            add_url: None,
            remove_indexes: vec![0],
            move_indexes: Vec::new(),
            move_to_index: None,
        });
        assert_eq!(removed.servers, vec!["https://media.example.com"]);

        let protected = blossom_server_list_projection(BlossomServerListProjectionInput {
            servers: removed.servers.clone(),
            add_url: None,
            remove_indexes: vec![0, 99],
            move_indexes: Vec::new(),
            move_to_index: None,
        });
        assert_eq!(protected.servers, removed.servers);
        assert!(protected.can_save);

        let malformed = blossom_server_list_projection(BlossomServerListProjectionInput {
            servers: vec![
                "https://one.example.com".into(),
                "https://two.example.com".into(),
            ],
            add_url: None,
            remove_indexes: vec![0, 99],
            move_indexes: Vec::new(),
            move_to_index: None,
        });
        assert_eq!(malformed.servers, vec!["https://two.example.com"]);
    }

    #[test]
    fn blossom_server_list_projection_reorders_like_platform_list() {
        let moved_down = blossom_server_list_projection(BlossomServerListProjectionInput {
            servers: vec!["a".into(), "b".into(), "c".into(), "d".into()],
            add_url: None,
            remove_indexes: Vec::new(),
            move_indexes: vec![1],
            move_to_index: Some(3),
        });
        assert_eq!(moved_down.servers, vec!["a", "c", "b", "d"]);

        let moved_to_end = blossom_server_list_projection(BlossomServerListProjectionInput {
            servers: vec!["a".into(), "b".into(), "c".into(), "d".into()],
            add_url: None,
            remove_indexes: Vec::new(),
            move_indexes: vec![1],
            move_to_index: Some(4),
        });
        assert_eq!(moved_to_end.servers, vec!["a", "c", "d", "b"]);

        let moved_up = blossom_server_list_projection(BlossomServerListProjectionInput {
            servers: vec!["a".into(), "b".into(), "c".into(), "d".into()],
            add_url: None,
            remove_indexes: Vec::new(),
            move_indexes: vec![2],
            move_to_index: Some(0),
        });
        assert_eq!(moved_up.servers, vec!["c", "a", "b", "d"]);

        let moved_multiple = blossom_server_list_projection(BlossomServerListProjectionInput {
            servers: vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()],
            add_url: None,
            remove_indexes: Vec::new(),
            move_indexes: vec![1, 3],
            move_to_index: Some(5),
        });
        assert_eq!(moved_multiple.servers, vec!["a", "c", "e", "b", "d"]);
    }

    #[test]
    fn sha256_hex_is_lowercase_64_chars() {
        let h = sha256_hex(b"hello");
        assert_eq!(h.len(), 64);
        assert!(h
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
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
        let tags = bud01_upload_auth_tags(&sha, 1_000).expect("auth tags");
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
