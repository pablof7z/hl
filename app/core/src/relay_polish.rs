//! Features that sit on top of `relays.rs` but are optional / user-initiated:
//! import-from-npub and cache stats. Kept out of `relays.rs` so the core
//! persistence + reconciliation module stays lean. (NIP-11 lives entirely in
//! NMP per ADR-0051 — see `HighlighterCore::probe_relay_nip11`.)

use std::path::Path;
use std::time::Duration;

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;
use crate::models::CacheStats;
use crate::nostr_runtime::NostrRuntime;
use crate::relays::RelayConfig;

const IMPORT_FETCH_TIMEOUT: Duration = Duration::from_secs(5);
const KIND_RELAY_LIST: u16 = 10002;

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
    let pk_bytes: [u8; 32] = pubkey.to_bytes();
    let ndb_filter = NdbFilter::new()
        .kinds([KIND_RELAY_LIST as u64])
        .authors([&pk_bytes])
        .build();
    runtime
        .open_nmp_filter_once_and_wait(
            "relay-import/nip65",
            filter,
            urls,
            vec![ndb_filter],
            IMPORT_FETCH_TIMEOUT,
        )
        .await?;

    relay_rows_from_cache(runtime.ndb(), pubkey)
}

fn relay_rows_from_cache(ndb: &Ndb, pubkey: PublicKey) -> Result<Vec<RelayConfig>, CoreError> {
    let pk_bytes: [u8; 32] = pubkey.to_bytes();
    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let filter = NdbFilter::new()
        .kinds([KIND_RELAY_LIST as u64])
        .authors([&pk_bytes])
        .build();
    let mut events: Vec<Event> = ndb
        .query(&txn, &[filter], 16)
        .map_err(|e| CoreError::Cache(format!("query imported kind:10002: {e}")))?
        .into_iter()
        .filter_map(|result| result.note.json().ok())
        .filter_map(|json| Event::from_json(&json).ok())
        .collect();
    events.sort_by_key(|event| std::cmp::Reverse(event.created_at));

    let mut rows: Vec<RelayConfig> = Vec::new();
    // Events is sorted newest first — first one wins per replaceable rules.
    if let Some(event) = events.first() {
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
            rows.push(RelayConfig {
                url,
                read,
                write,
                rooms: false,
                indexer: false,
            });
        }
    }
    Ok(rows)
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

