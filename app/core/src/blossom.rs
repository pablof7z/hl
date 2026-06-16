//! Blossom server list (BUD-03, kind:10063) read + publish, NIP-98 auth, and
//! app-model adaptation for NMP-owned Blossom uploads.
//!
//! The kind:10063 "User Server List" is a replaceable event; publishing a new
//! one supersedes the old one on every relay. Tags follow BUD-03: each server
//! is an `["server", "<url>"]` tag. Order is preserved — the first server in
//! the list is the upload default; fallback proceeds in list order.
//!
//! Uploads are dispatched through `nmp.blossom.upload`: Highlighter writes the
//! native-supplied bytes to a local staging file and NMP owns hashing, kind:24242
//! auth, signing, HTTP PUT transport, and action-result reporting.

use std::path::{Path, PathBuf};

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};
use uuid::Uuid;

use crate::errors::CoreError;
use crate::models::BlossomUpload;
use crate::nostr_runtime::NostrRuntime;

const KIND_BLOSSOM_SERVERS: u16 = 10063;
const KIND_NIP98_HTTP_AUTH: u16 = 27235;
pub const DEFAULT_SERVER: &str = "https://blossom.primal.net";
const NMP_BLOSSOM_UPLOAD_NAMESPACE: &str = "nmp.blossom.upload";

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
    runtime.publish_signed_event("blossom-servers-publish", &event)?;
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
/// Optional SHA-256 payload hash for HTTP endpoints that require it.
pub async fn sign_nip98_auth(
    runtime: &NostrRuntime,
    url: &str,
    method: &str,
    payload_hash: Option<&str>,
) -> Result<String, CoreError> {
    let mut tags = vec![parse_tag(&["u", url])?, parse_tag(&["method", method])?];
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

// -- NMP Blossom upload --

/// Stage `bytes`, dispatch `nmp.blossom.upload`, and adapt NMP's action result
/// into the app's `BlossomUpload` descriptor.
///
/// `width`, `height`, and `alt` are stamped onto the returned record but are
/// NOT sent to the server. They remain app metadata used to build the NIP-92
/// `imeta` tag on the publishing event.
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
    let signer_pubkey = runtime
        .active_account_pubkey()
        .ok_or(CoreError::NotAuthenticated)?;
    let mut servers = query_blossom_servers(runtime.ndb(), &signer_pubkey)?;
    if servers.is_empty() {
        servers.push(DEFAULT_SERVER.to_string());
    }

    let staging_path = write_upload_staging_file(runtime.data_dir(), &bytes).await?;
    let file_path = staging_path
        .to_str()
        .ok_or_else(|| CoreError::Cache("upload staging path is not valid UTF-8".into()))?
        .to_string();
    let action = nmp_blossom::UploadInput {
        file_path,
        content_type: Some(mime_clean.to_string()),
        servers,
        signer_pubkey: Some(signer_pubkey),
    };

    let action_result = runtime
        .dispatch_nmp_action_for_result("blossom-upload", NMP_BLOSSOM_UPLOAD_NAMESPACE, &action)
        .await;
    if let Err(e) = tokio::fs::remove_file(&staging_path).await {
        tracing::warn!(
            path = %staging_path.display(),
            error = %e,
            "remove Blossom upload staging file"
        );
    }
    let row = action_result?;
    let result_json = row
        .result
        .as_deref()
        .ok_or_else(|| CoreError::Network("NMP Blossom upload missing descriptor".into()))?;

    upload_from_nmp_result(result_json, mime_clean, size_bytes, width, height, alt)
}

async fn write_upload_staging_file(data_dir: &Path, bytes: &[u8]) -> Result<PathBuf, CoreError> {
    let dir = data_dir.join("nmp-upload-staging");
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| CoreError::Cache(format!("create upload staging dir: {e}")))?;
    let path = dir.join(format!("blossom-{}.blob", Uuid::new_v4()));
    tokio::fs::write(&path, bytes)
        .await
        .map_err(|e| CoreError::Cache(format!("write upload staging file: {e}")))?;
    Ok(path)
}

fn upload_from_nmp_result(
    result_json: &str,
    fallback_mime: &str,
    fallback_size_bytes: u64,
    width: u32,
    height: u32,
    alt: String,
) -> Result<BlossomUpload, CoreError> {
    let descriptor: serde_json::Value = serde_json::from_str(result_json)
        .map_err(|e| CoreError::Network(format!("NMP Blossom descriptor is not JSON: {e}")))?;
    let selected = select_descriptor_value(&descriptor).ok_or_else(|| {
        CoreError::Network("NMP Blossom descriptor missing successful URL".into())
    })?;
    let url = selected
        .get("url")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| CoreError::Network("NMP Blossom descriptor missing `url`".into()))?;
    let sha256_hex = descriptor
        .get("sha256")
        .or_else(|| selected.get("sha256"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| CoreError::Network("NMP Blossom descriptor missing `sha256`".into()))?;
    let size_bytes = descriptor
        .get("size")
        .or_else(|| selected.get("size"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(fallback_size_bytes);
    let mime = descriptor
        .get("type")
        .or_else(|| selected.get("type"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback_mime)
        .to_string();

    Ok(BlossomUpload {
        url,
        sha256_hex,
        mime,
        size_bytes,
        width,
        height,
        alt,
    })
}

fn select_descriptor_value(value: &serde_json::Value) -> Option<&serde_json::Value> {
    if value
        .get("url")
        .and_then(serde_json::Value::as_str)
        .is_some()
    {
        return Some(value);
    }
    value
        .get("servers")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .find(|server| {
            server
                .get("ok")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
                && server
                    .get("url")
                    .and_then(serde_json::Value::as_str)
                    .is_some()
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
    fn nmp_flat_descriptor_maps_to_app_upload() {
        let upload = upload_from_nmp_result(
            r#"{"url":"https://b.example/blob.png","sha256":"abc123","size":42,"type":"image/png","uploaded":1}"#,
            "application/octet-stream",
            5,
            320,
            240,
            "alt text".to_string(),
        )
        .expect("flat descriptor");

        assert_eq!(upload.url, "https://b.example/blob.png");
        assert_eq!(upload.sha256_hex, "abc123");
        assert_eq!(upload.mime, "image/png");
        assert_eq!(upload.size_bytes, 42);
        assert_eq!(upload.width, 320);
        assert_eq!(upload.height, 240);
        assert_eq!(upload.alt, "alt text");
    }

    #[test]
    fn nmp_aggregate_descriptor_uses_first_successful_server() {
        let upload = upload_from_nmp_result(
            r#"{"sha256":"def456","size":99,"type":"image/jpeg","uploaded":2,"servers":[{"server":"https://a.example","ok":false,"error":"nope"},{"server":"https://b.example","ok":true,"url":"https://b.example/blob.jpg"}]}"#,
            "application/octet-stream",
            5,
            0,
            0,
            "".to_string(),
        )
        .expect("aggregate descriptor");

        assert_eq!(upload.url, "https://b.example/blob.jpg");
        assert_eq!(upload.sha256_hex, "def456");
        assert_eq!(upload.mime, "image/jpeg");
        assert_eq!(upload.size_bytes, 99);
    }
}
