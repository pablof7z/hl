//! Relay config: the set of relays the app connects to, each tagged with the
//! four roles that drive its routing — `read`, `write`, `rooms`, `indexer`.
//!
//! Persistence is split by what each role actually is:
//!
//! - `read` / `write` → **NIP-65 (kind:10002)**. Nostr identity; interops with
//!   any other nostr client. Re-published on every edit.
//! - `rooms` / `indexer` → **NIP-78 app-data (kind:30078)** with
//!   `d = "com.highlighter.relays"`. Highlighter-specific routing, not nostr
//!   identity — doesn't belong in kind:10002.
//!
//! `query_relays` merges both sources on URL. When neither exists yet (first
//! login), `seed_defaults()` fills in a sane starting set.

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};
use serde::{Deserialize, Serialize};

use crate::errors::CoreError;
use crate::nostr_runtime::NostrRuntime;

// -- Well-known relays -------------------------------------------------------

pub const HIGHLIGHTER_RELAY: &str = "wss://relay.highlighter.com";

/// Relays we run NIP-77 negentropy sync against for the cold-start
/// backfill of follows' kind:0/3/10002 (the "social trio"). The premise
/// for using purplepag.es here was wrong — it specialises in those kinds
/// but doesn't currently advertise or implement NIP-77 (its NIP-11
/// supported_nips list omits 77, and `examples/purple_sync_bench.rs`
/// confirms negentropy times out against it). relay.damus.io (strfry)
/// works and, crucially, isn't bound by purple's `max_limit=500` cap on
/// REQ — negentropy returned 1794 events vs REQ's 500 for a 1052-follow
/// query. Keep this list short; sync runs in parallel against each.
pub const NEGENTROPY_SYNC_RELAYS: &[&str] = &["wss://relay.damus.io"];

/// Relay used for outgoing `nostrconnect://` pairing. Matches Olas's choice —
/// Primal's bunker relay is the lowest-friction option because it's what
/// Primal's built-in signer expects.
pub const NOSTR_CONNECT_RELAY: &str = "wss://relay.primal.net";

/// Perms string included in our `nostrconnect://` URI. We request only the
/// kinds Highlighter actually publishes plus encryption for NIP-46 transport.
pub const DEFAULT_NOSTR_CONNECT_PERMS: &str =
    "sign_event:11,sign_event:1111,sign_event:9802,sign_event:16,nip04_encrypt,nip04_decrypt,nip44_encrypt,nip44_decrypt";

// -- Types -------------------------------------------------------------------

/// A single row in the user's relay list, carrying all four roles.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RelayConfig {
    pub url: String,
    pub read: bool,
    pub write: bool,
    pub rooms: bool,
    pub indexer: bool,
}

impl RelayConfig {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            read: false,
            write: false,
            rooms: false,
            indexer: false,
        }
    }

    pub fn read_write(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            read: true,
            write: true,
            rooms: false,
            indexer: false,
        }
    }
}

/// Starting relay set for a brand-new user with no published kind:10002 and
/// no cached NIP-78 app-data yet. Called by `query_relays` as the fallback.
pub fn seed_defaults() -> Vec<RelayConfig> {
    vec![
        RelayConfig {
            url: HIGHLIGHTER_RELAY.to_string(),
            read: true,
            write: true,
            rooms: true,
            indexer: false,
        },
        RelayConfig {
            url: "wss://relay.damus.io".to_string(),
            read: true,
            write: true,
            rooms: false,
            indexer: false,
        },
        RelayConfig {
            url: "wss://purplepag.es".to_string(),
            read: false,
            write: false,
            rooms: false,
            indexer: true,
        },
        RelayConfig {
            url: "wss://relay.primal.net".to_string(),
            read: false,
            write: false,
            rooms: false,
            indexer: true,
        },
    ]
}

// -- NIP-65 (kind:10002) -----------------------------------------------------

const KIND_RELAY_LIST: u16 = 10002;

/// Build the `["r", url, marker?]` tags for the provided rows. Rows with
/// neither `read` nor `write` are skipped — NIP-65 has no concept of a
/// "disabled" relay entry, only "inbox/outbox/both".
fn nip65_tags(rows: &[RelayConfig]) -> Result<Vec<Tag>, CoreError> {
    let mut tags: Vec<Tag> = Vec::new();
    for row in rows {
        let marker = match (row.read, row.write) {
            (true, true) => None,
            (true, false) => Some("read"),
            (false, true) => Some("write"),
            (false, false) => continue,
        };
        let parts: Vec<String> = match marker {
            Some(m) => vec!["r".into(), row.url.clone(), m.into()],
            None => vec!["r".into(), row.url.clone()],
        };
        tags.push(
            Tag::parse(parts).map_err(|e| CoreError::Other(format!("build relay tag: {e}")))?,
        );
    }
    Ok(tags)
}

/// Parse a kind:10002 event into `(url, read, write)` rows.
fn parse_nip65_event(event: &Event) -> Vec<(String, bool, bool)> {
    let mut out: Vec<(String, bool, bool)> = Vec::new();
    for tag in event.tags.iter() {
        let slice = tag.as_slice();
        if slice.first().map(String::as_str) != Some("r") {
            continue;
        }
        let Some(url) = slice.get(1) else { continue };
        let url = url.trim().to_string();
        if url.is_empty() {
            continue;
        }
        let (read, write) = match slice.get(2).map(String::as_str) {
            Some("read") => (true, false),
            Some("write") => (false, true),
            _ => (true, true),
        };
        out.push((url, read, write));
    }
    out
}

/// Newest kind:10002 for `user_hex` cached in nostrdb, or `None`.
fn latest_nip65(ndb: &Ndb, user_hex: &str) -> Result<Option<Event>, CoreError> {
    if user_hex.is_empty() {
        return Ok(None);
    }
    let author = PublicKey::from_hex(user_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid user pubkey: {e}")))?;
    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let pk_bytes: [u8; 32] = author.to_bytes();
    let filter = NdbFilter::new()
        .kinds([KIND_RELAY_LIST as u64])
        .authors([&pk_bytes])
        .build();
    let results = ndb
        .query(&txn, &[filter], 8)
        .map_err(|e| CoreError::Cache(format!("query relay list: {e}")))?;
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

// -- NIP-78 app-data (kind:30078) for rooms/indexer flags --------------------

const KIND_APP_DATA: u16 = 30078;
const APP_DATA_D_TAG: &str = "com.highlighter.relays";

/// Per-row payload stored in the NIP-78 event's JSON content. Flat shape so
/// it round-trips losslessly.
#[derive(Debug, Serialize, Deserialize)]
struct AppDataEntry {
    url: String,
    #[serde(default)]
    rooms: bool,
    #[serde(default)]
    indexer: bool,
}

/// JSON content for the kind:30078 event. Skips rows with neither flag — no
/// point persisting empty entries.
fn app_data_content(rows: &[RelayConfig]) -> String {
    let entries: Vec<AppDataEntry> = rows
        .iter()
        .filter(|r| r.rooms || r.indexer)
        .map(|r| AppDataEntry {
            url: r.url.clone(),
            rooms: r.rooms,
            indexer: r.indexer,
        })
        .collect();
    serde_json::to_string(&entries).unwrap_or_else(|_| "[]".into())
}

fn parse_app_data_event(event: &Event) -> Vec<AppDataEntry> {
    serde_json::from_str::<Vec<AppDataEntry>>(&event.content).unwrap_or_default()
}

fn latest_app_data(ndb: &Ndb, user_hex: &str) -> Result<Option<Event>, CoreError> {
    if user_hex.is_empty() {
        return Ok(None);
    }
    let author = PublicKey::from_hex(user_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid user pubkey: {e}")))?;
    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let pk_bytes: [u8; 32] = author.to_bytes();
    let filter = NdbFilter::new()
        .kinds([KIND_APP_DATA as u64])
        .authors([&pk_bytes])
        .tags([APP_DATA_D_TAG], 'd')
        .build();
    let results = ndb
        .query(&txn, &[filter], 8)
        .map_err(|e| CoreError::Cache(format!("query relay app-data: {e}")))?;
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

// -- Merge + public API ------------------------------------------------------

/// Merge kind:10002 and kind:30078 into the user's effective relay list,
/// deduped by URL. Falls back to `seed_defaults()` when neither event is
/// cached.
pub fn query_relays(ndb: &Ndb, user_hex: &str) -> Result<Vec<RelayConfig>, CoreError> {
    let nip65 = latest_nip65(ndb, user_hex)?
        .as_ref()
        .map(parse_nip65_event)
        .unwrap_or_default();
    let app_data = latest_app_data(ndb, user_hex)?
        .as_ref()
        .map(parse_app_data_event)
        .unwrap_or_default();

    if nip65.is_empty() && app_data.is_empty() {
        return Ok(seed_defaults());
    }

    let mut rows: Vec<RelayConfig> = Vec::new();
    for (url, read, write) in nip65 {
        rows.push(RelayConfig {
            url,
            read,
            write,
            rooms: false,
            indexer: false,
        });
    }
    for entry in app_data {
        if let Some(row) = rows.iter_mut().find(|r| r.url == entry.url) {
            row.rooms = entry.rooms;
            row.indexer = entry.indexer;
        } else {
            rows.push(RelayConfig {
                url: entry.url,
                read: false,
                write: false,
                rooms: entry.rooms,
                indexer: entry.indexer,
            });
        }
    }
    Ok(rows)
}

/// Publish kind:10002 (NIP-65) with the current rows' read/write flags.
pub async fn publish_nip65(
    runtime: &NostrRuntime,
    rows: &[RelayConfig],
) -> Result<String, CoreError> {
    let tags = nip65_tags(rows)?;
    let builder = EventBuilder::new(Kind::Custom(KIND_RELAY_LIST), "").tags(tags);
    let client = runtime.client();
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign relay list: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish relay list: {e}")))?;
    Ok(event.id.to_hex())
}

/// Publish kind:30078 app-data with the current rows' rooms/indexer flags.
pub async fn publish_app_data(
    runtime: &NostrRuntime,
    rows: &[RelayConfig],
) -> Result<String, CoreError> {
    let content = app_data_content(rows);
    let d_tag = Tag::parse(vec!["d".to_string(), APP_DATA_D_TAG.to_string()])
        .map_err(|e| CoreError::Other(format!("build d tag: {e}")))?;
    let builder = EventBuilder::new(Kind::Custom(KIND_APP_DATA), content).tags([d_tag]);
    let client = runtime.client();
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign relay app-data: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish relay app-data: {e}")))?;
    Ok(event.id.to_hex())
}

/// Replace the user's relay list with `rows`. Re-publishes both NIP-65 and
/// NIP-78 so every flag is durable. Validates that every row's URL is a
/// non-empty `ws://` or `wss://` URL with no duplicates.
pub async fn set_relays(
    runtime: &NostrRuntime,
    rows: Vec<RelayConfig>,
) -> Result<(), CoreError> {
    if rows.is_empty() {
        return Err(CoreError::InvalidInput(
            "relay list must not be empty".into(),
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for row in &rows {
        let url = row.url.trim();
        if !(url.starts_with("wss://") || url.starts_with("ws://")) {
            return Err(CoreError::InvalidInput(format!(
                "relay URL must start with ws:// or wss://: {url}"
            )));
        }
        if !seen.insert(url.to_string()) {
            return Err(CoreError::InvalidInput(format!(
                "duplicate relay URL in list: {url}"
            )));
        }
    }
    publish_nip65(runtime, &rows).await?;
    publish_app_data(runtime, &rows).await?;
    Ok(())
}

/// Insert-or-update a single relay. Reads the current list, replaces the row
/// with matching URL (or appends), and re-publishes.
pub async fn upsert_relay(
    runtime: &NostrRuntime,
    user_hex: &str,
    cfg: RelayConfig,
) -> Result<(), CoreError> {
    let mut rows = query_relays(runtime.ndb(), user_hex)?;
    if let Some(existing) = rows.iter_mut().find(|r| r.url == cfg.url) {
        *existing = cfg;
    } else {
        rows.push(cfg);
    }
    set_relays(runtime, rows).await
}

/// Remove a relay by URL. Errors if the URL isn't in the list.
pub async fn remove_relay(
    runtime: &NostrRuntime,
    user_hex: &str,
    url: String,
) -> Result<(), CoreError> {
    let mut rows = query_relays(runtime.ndb(), user_hex)?;
    let before = rows.len();
    rows.retain(|r| r.url != url);
    if rows.len() == before {
        return Err(CoreError::NotFound);
    }
    set_relays(runtime, rows).await
}

/// Atomically update a single relay's role flags without touching its URL.
pub async fn set_relay_roles(
    runtime: &NostrRuntime,
    user_hex: &str,
    url: String,
    read: bool,
    write: bool,
    rooms: bool,
    indexer: bool,
) -> Result<(), CoreError> {
    let mut rows = query_relays(runtime.ndb(), user_hex)?;
    let Some(row) = rows.iter_mut().find(|r| r.url == url) else {
        return Err(CoreError::NotFound);
    };
    row.read = read;
    row.write = write;
    row.rooms = rooms;
    row.indexer = indexer;
    set_relays(runtime, rows).await
}

// -- Tests -------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_rows() -> Vec<RelayConfig> {
        vec![
            RelayConfig {
                url: "wss://relay.highlighter.com".into(),
                read: true,
                write: true,
                rooms: true,
                indexer: false,
            },
            RelayConfig {
                url: "wss://relay.damus.io".into(),
                read: true,
                write: false,
                rooms: false,
                indexer: false,
            },
            RelayConfig {
                url: "wss://purplepag.es".into(),
                read: false,
                write: false,
                rooms: false,
                indexer: true,
            },
        ]
    }

    #[test]
    fn seed_defaults_has_four_rows_with_expected_roles() {
        let seed = seed_defaults();
        assert_eq!(seed.len(), 4);

        let hl = seed.iter().find(|r| r.url.contains("highlighter")).expect("hl");
        assert!(hl.read && hl.write && hl.rooms && !hl.indexer);

        let damus = seed.iter().find(|r| r.url.contains("damus")).expect("damus");
        assert!(damus.read && damus.write && !damus.rooms && !damus.indexer);

        let purple = seed.iter().find(|r| r.url.contains("purplepag")).expect("purple");
        assert!(!purple.read && !purple.write && !purple.rooms && purple.indexer);

        let primal = seed.iter().find(|r| r.url.contains("primal")).expect("primal");
        assert!(!primal.read && !primal.write && !primal.rooms && primal.indexer);
    }

    #[test]
    fn nip65_tags_use_marker_for_asymmetric_rows_and_none_for_both() {
        let tags = nip65_tags(&sample_rows()).expect("build tags");
        let hl = tags
            .iter()
            .find(|t| {
                t.as_slice().get(1).map(String::as_str) == Some("wss://relay.highlighter.com")
            })
            .expect("hl tag");
        assert_eq!(hl.as_slice().len(), 2);

        let damus = tags
            .iter()
            .find(|t| t.as_slice().get(1).map(String::as_str) == Some("wss://relay.damus.io"))
            .expect("damus tag");
        assert_eq!(damus.as_slice().get(2).map(String::as_str), Some("read"));
    }

    #[test]
    fn nip65_tags_skip_rows_with_neither_read_nor_write() {
        let tags = nip65_tags(&sample_rows()).expect("build tags");
        assert!(tags
            .iter()
            .all(|t| t.as_slice().get(1).map(String::as_str) != Some("wss://purplepag.es")));
    }

    #[test]
    fn nip65_roundtrip_preserves_read_write_flags() {
        let keys = Keys::generate();
        let rows = sample_rows();
        let tags = nip65_tags(&rows).expect("build tags");
        let event = EventBuilder::new(Kind::Custom(KIND_RELAY_LIST), "")
            .tags(tags)
            .sign_with_keys(&keys)
            .expect("sign");

        let parsed = parse_nip65_event(&event);
        assert_eq!(parsed.len(), 2);

        let hl = parsed
            .iter()
            .find(|(u, _, _)| u == "wss://relay.highlighter.com")
            .expect("hl");
        assert!(hl.1 && hl.2);

        let damus = parsed
            .iter()
            .find(|(u, _, _)| u == "wss://relay.damus.io")
            .expect("damus");
        assert!(damus.1 && !damus.2);
    }

    #[test]
    fn app_data_content_round_trip_preserves_rooms_and_indexer() {
        let keys = Keys::generate();
        let rows = sample_rows();
        let content = app_data_content(&rows);
        let d_tag = Tag::parse(vec!["d".to_string(), APP_DATA_D_TAG.to_string()])
            .expect("d tag");
        let event = EventBuilder::new(Kind::Custom(KIND_APP_DATA), content)
            .tags([d_tag])
            .sign_with_keys(&keys)
            .expect("sign");

        let entries = parse_app_data_event(&event);
        assert_eq!(entries.len(), 2);

        let hl = entries
            .iter()
            .find(|e| e.url == "wss://relay.highlighter.com")
            .expect("hl entry");
        assert!(hl.rooms && !hl.indexer);

        let purple = entries
            .iter()
            .find(|e| e.url == "wss://purplepag.es")
            .expect("purple entry");
        assert!(!purple.rooms && purple.indexer);
    }

    #[test]
    fn parse_nip65_event_handles_missing_marker_as_both() {
        let keys = Keys::generate();
        let tag = Tag::parse(vec!["r".to_string(), "wss://one.example".to_string()])
            .expect("tag");
        let event = EventBuilder::new(Kind::Custom(KIND_RELAY_LIST), "")
            .tags([tag])
            .sign_with_keys(&keys)
            .expect("sign");

        let parsed = parse_nip65_event(&event);
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].1 && parsed[0].2);
    }

    #[test]
    fn app_data_content_empty_array_when_no_rooms_or_indexer_rows() {
        let rows = vec![RelayConfig::read_write("wss://a.example")];
        assert_eq!(app_data_content(&rows), "[]");
    }
}
