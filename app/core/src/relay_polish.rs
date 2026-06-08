//! Features that sit on top of `relays.rs` but are optional / user-initiated:
//! NIP-11 probe, import-from-npub, cache stats. Kept out of `relays.rs` so
//! the core persistence + reconciliation module stays lean.

use std::path::Path;
use std::time::Duration;

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;
use crate::models::{CacheStats, Nip11Document};
use crate::nostr_runtime::NostrRuntime;
use crate::relays::RelayConfig;

const NIP11_PROBE_TIMEOUT: Duration = Duration::from_secs(6);
const IMPORT_FETCH_TIMEOUT: Duration = Duration::from_secs(5);
const KIND_RELAY_LIST: u16 = 10002;

/// Convert a `ws[s]://` URL to the `http[s]://` form used for NIP-11 GET.
fn http_url_for_nip11(relay_url: &str) -> Option<String> {
    let trimmed = relay_url.trim();
    if let Some(rest) = trimmed.strip_prefix("wss://") {
        return Some(format!("https://{rest}"));
    }
    if let Some(rest) = trimmed.strip_prefix("ws://") {
        return Some(format!("http://{rest}"));
    }
    None
}

/// GET the relay's NIP-11 information document. Returns a parsed
/// `Nip11Document` or `CoreError::Network` on any transport/JSON failure.
pub async fn probe_nip11(relay_url: &str) -> Result<Nip11Document, CoreError> {
    let http = http_url_for_nip11(relay_url)
        .ok_or_else(|| CoreError::InvalidInput(format!("unsupported scheme: {relay_url}")))?;
    let client = reqwest::Client::builder()
        .timeout(NIP11_PROBE_TIMEOUT)
        .build()
        .map_err(|e| CoreError::Network(format!("build http client: {e}")))?;
    let resp = client
        .get(&http)
        .header("Accept", "application/nostr+json")
        .send()
        .await
        .map_err(|e| CoreError::Network(format!("nip11 GET: {e}")))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(CoreError::Network(format!("nip11 HTTP {status}")));
    }
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| CoreError::Network(format!("nip11 JSON: {e}")))?;

    let name = json
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let description = json
        .get("description")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let pubkey = json
        .get("pubkey")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let contact = json
        .get("contact")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let software = json
        .get("software")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let version = json
        .get("version")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let supported_nips: Vec<u32> = json
        .get("supported_nips")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|n| n.as_u64())
                .map(|n| n as u32)
                .collect()
        })
        .unwrap_or_default();
    let icon = json
        .get("icon")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Ok(Nip11Document {
        url: relay_url.trim().to_string(),
        name,
        description,
        pubkey,
        contact,
        software,
        version,
        supported_nips,
        icon,
    })
}

/// Fetch another user's kind:10002 via the indexer pool and parse it into a
/// list of `RelayConfig` rows (read/write only — rooms/indexer flags are
/// Highlighter-specific and stay off for imports). Empty `Vec` if nothing
/// cached after the timeout.
pub async fn import_from_npub(
    runtime: &NostrRuntime,
    npub_or_hex: &str,
) -> Result<Vec<RelayConfig>, CoreError> {
    let trimmed = npub_or_hex.trim();
    let pubkey = PublicKey::parse(trimmed)
        .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;

    let urls = runtime.indexer_urls();
    if urls.is_empty() {
        return Err(CoreError::InvalidInput(
            "no indexer relays configured — turn on Indexer for at least one relay first".into(),
        ));
    }

    let filter = Filter::new()
        .kinds([Kind::Custom(KIND_RELAY_LIST)])
        .author(pubkey);
    let events = runtime
        .client()
        .fetch_events_from(urls, filter, IMPORT_FETCH_TIMEOUT)
        .await
        .map_err(|e| CoreError::Relay(format!("fetch kind:10002 for import: {e}")))?;

    let Some(event) = events.first() else {
        return Err(CoreError::Other(
            "No kind:10002 found for this user — they may not have published a relay list yet."
                .into(),
        ));
    };
    let rows = relay_rows_from_kind10002(event);
    if rows.is_empty() {
        return Err(CoreError::Other(
            "This user's kind:10002 did not contain any relay URLs.".into(),
        ));
    }
    Ok(rows)
}

fn relay_rows_from_kind10002(event: &Event) -> Vec<RelayConfig> {
    event
        .tags
        .iter()
        .filter_map(|tag| {
            let slice = tag.as_slice();
            if slice.first().map(String::as_str) != Some("r") {
                return None;
            }
            let url = slice.get(1)?.trim().to_string();
            if url.is_empty() {
                return None;
            }
            let (read, write) = match slice.get(2).map(String::as_str) {
                Some("read") => (true, false),
                Some("write") => (false, true),
                _ => (true, true),
            };
            Some(RelayConfig {
                url,
                read,
                write,
                rooms: false,
                indexer: false,
            })
        })
        .collect()
}

/// Best-effort disk + event-count snapshot. `disk_bytes` sums file sizes in
/// `data_dir`; `event_count_estimate` is the size of a wildcard nostrdb
/// query up to a generous cap. Both are order-of-magnitude figures used
/// only for the Network Settings "Local cache" card.
pub fn cache_stats(ndb: &Ndb, data_dir: &Path) -> Result<CacheStats, CoreError> {
    let disk_bytes = dir_size(data_dir).unwrap_or(0);

    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    // Match every event. The cap is defensive — iOS should never hold more
    // than ~500k events in the local cache; anything above that is treated
    // as "lots".
    let filter = NdbFilter::new().build();
    let count = ndb
        .query(&txn, &[filter], 500_000)
        .map(|results| results.len() as u64)
        .unwrap_or(0);

    Ok(CacheStats {
        disk_bytes,
        event_count_estimate: count,
    })
}

fn dir_size(path: &Path) -> std::io::Result<u64> {
    let mut total: u64 = 0;
    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => return Ok(0),
    };
    for entry in entries.flatten() {
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.is_file() {
            total = total.saturating_add(meta.len());
        } else if meta.is_dir() {
            total = total.saturating_add(dir_size(&entry.path()).unwrap_or(0));
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_url_for_nip11_maps_schemes() {
        assert_eq!(
            http_url_for_nip11("wss://relay.example"),
            Some("https://relay.example".to_string())
        );
        assert_eq!(
            http_url_for_nip11("ws://relay.example"),
            Some("http://relay.example".to_string())
        );
        assert_eq!(http_url_for_nip11("https://relay.example"), None);
    }

    #[test]
    fn relay_rows_from_kind10002_skips_empty_urls() {
        let keys = Keys::generate();
        let event = EventBuilder::new(Kind::Custom(KIND_RELAY_LIST), "")
            .tags(vec![
                Tag::parse(["r", "  "]).expect("empty relay tag"),
                Tag::parse(["r", " wss://read.example ", "read"]).expect("read relay tag"),
                Tag::parse(["r", "wss://write.example", "write"]).expect("write relay tag"),
                Tag::parse(["r", "wss://both.example"]).expect("both relay tag"),
            ])
            .sign_with_keys(&keys)
            .expect("sign relay list");

        let rows = relay_rows_from_kind10002(&event);

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].url, "wss://read.example");
        assert!(rows[0].read);
        assert!(!rows[0].write);
        assert_eq!(rows[1].url, "wss://write.example");
        assert!(!rows[1].read);
        assert!(rows[1].write);
        assert_eq!(rows[2].url, "wss://both.example");
        assert!(rows[2].read);
        assert!(rows[2].write);
    }
}
