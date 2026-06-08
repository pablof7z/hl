//! Rust-owned What's New payload and seen-state.
//!
//! Native shells render the sheet and dispatch dismissal. The changelog
//! payload, first-install seeding, sorting, and seen marker live here.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::errors::CoreError;

const STATE_FILE_NAME: &str = "whats-new-state-v1.json";
const BUNDLED_WHATS_NEW_JSON: &str = include_str!("../resources/whats-new.json");

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WhatsNewEntry {
    pub shipped_at_iso: String,
    pub shipped_at_unix_seconds: u64,
    pub lines: Vec<String>,
}

pub struct WhatsNewStore {
    path: PathBuf,
    last_seen: Mutex<Option<Option<u64>>>,
}

impl WhatsNewStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join(STATE_FILE_NAME),
            last_seen: Mutex::new(None),
        }
    }

    pub async fn prepare(&self) -> Result<Vec<WhatsNewEntry>, CoreError> {
        let entries = bundled_entries()?;
        let Some(newest) = entries.first() else {
            return Ok(Vec::new());
        };

        let mut guard = self.last_seen.lock().await;
        if guard.is_none() {
            *guard = Some(load_state(&self.path).await);
        }

        match *guard {
            Some(Some(marker)) => Ok(entries
                .into_iter()
                .filter(|entry| entry.shipped_at_unix_seconds > marker)
                .collect()),
            Some(None) => {
                persist_state(&self.path, newest.shipped_at_unix_seconds).await?;
                *guard = Some(Some(newest.shipped_at_unix_seconds));
                Ok(Vec::new())
            }
            None => unreachable!("whats-new state initialized above"),
        }
    }

    pub async fn mark_seen(&self, shipped_at_unix_seconds: u64) -> Result<(), CoreError> {
        let mut guard = self.last_seen.lock().await;
        if guard.is_none() {
            *guard = Some(load_state(&self.path).await);
        }
        let next = guard
            .as_ref()
            .and_then(|marker| *marker)
            .map(|existing| existing.max(shipped_at_unix_seconds))
            .unwrap_or(shipped_at_unix_seconds);
        persist_state(&self.path, next).await?;
        *guard = Some(Some(next));
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct WhatsNewPayload {
    schema_version: u32,
    entries: Vec<WhatsNewEntryPayload>,
}

#[derive(Debug, Deserialize)]
struct WhatsNewEntryPayload {
    shipped_at: String,
    lines: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct WhatsNewState {
    last_seen_at_unix_seconds: u64,
}

fn bundled_entries() -> Result<Vec<WhatsNewEntry>, CoreError> {
    decode_entries(BUNDLED_WHATS_NEW_JSON)
}

fn decode_entries(json: &str) -> Result<Vec<WhatsNewEntry>, CoreError> {
    let payload: WhatsNewPayload = serde_json::from_str(json)
        .map_err(|e| CoreError::Cache(format!("decode whats-new payload: {e}")))?;
    if payload.schema_version != 1 {
        return Err(CoreError::Cache(format!(
            "unsupported whats-new schema: {}",
            payload.schema_version
        )));
    }

    let mut entries = Vec::new();
    for entry in payload.entries {
        if entry.lines.is_empty() {
            continue;
        }
        let shipped_at_unix_seconds = parse_iso8601_utc(&entry.shipped_at).ok_or_else(|| {
            CoreError::Cache(format!("invalid whats-new timestamp: {}", entry.shipped_at))
        })?;
        entries.push(WhatsNewEntry {
            shipped_at_iso: entry.shipped_at,
            shipped_at_unix_seconds,
            lines: entry.lines,
        });
    }
    entries.sort_by(|a, b| b.shipped_at_unix_seconds.cmp(&a.shipped_at_unix_seconds));
    Ok(entries)
}

async fn load_state(path: &Path) -> Option<u64> {
    match tokio::fs::read(path).await {
        Ok(bytes) => match serde_json::from_slice::<WhatsNewState>(&bytes) {
            Ok(state) => Some(state.last_seen_at_unix_seconds),
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to parse whats-new state");
                None
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "failed to read whats-new state");
            None
        }
    }
}

async fn persist_state(path: &Path, last_seen_at_unix_seconds: u64) -> Result<(), CoreError> {
    let bytes = serde_json::to_vec(&WhatsNewState {
        last_seen_at_unix_seconds,
    })
    .map_err(|e| CoreError::Cache(format!("encode whats-new state: {e}")))?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| CoreError::Cache(format!("create whats-new state dir: {e}")))?;
    }
    let tmp = path.with_extension("json.tmp");
    tokio::fs::write(&tmp, bytes)
        .await
        .map_err(|e| CoreError::Cache(format!("write whats-new state: {e}")))?;
    tokio::fs::rename(&tmp, path)
        .await
        .map_err(|e| CoreError::Cache(format!("commit whats-new state: {e}")))?;
    Ok(())
}

fn parse_iso8601_utc(value: &str) -> Option<u64> {
    if value.len() != 20
        || &value[4..5] != "-"
        || &value[7..8] != "-"
        || &value[10..11] != "T"
        || &value[13..14] != ":"
        || &value[16..17] != ":"
        || &value[19..20] != "Z"
    {
        return None;
    }
    let year: i32 = value[0..4].parse().ok()?;
    let month: u32 = value[5..7].parse().ok()?;
    let day: u32 = value[8..10].parse().ok()?;
    let hour: u32 = value[11..13].parse().ok()?;
    let minute: u32 = value[14..16].parse().ok()?;
    let second: u32 = value[17..19].parse().ok()?;

    if month == 0
        || month > 12
        || day == 0
        || day > days_in_month(year, month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }

    let days = days_from_civil(year, month, day);
    if days < 0 {
        return None;
    }
    Some(days as u64 * 86_400 + hour as u64 * 3_600 + minute as u64 * 60 + second as u64)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = month as i32;
    let day = day as i32;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_payload_decodes_newest_first() {
        let entries = bundled_entries().unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].shipped_at_unix_seconds > entries[1].shipped_at_unix_seconds);
        assert_eq!(entries[0].shipped_at_iso, "2026-05-14T21:45:00Z");
    }

    #[test]
    fn iso8601_parser_matches_known_timestamp() {
        assert_eq!(parse_iso8601_utc("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso8601_utc("2026-05-14T21:45:00Z"), Some(1_778_795_100));
        assert_eq!(parse_iso8601_utc("2026-02-29T00:00:00Z"), None);
    }

    #[tokio::test]
    async fn first_launch_seeds_newest_and_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = WhatsNewStore::new(dir.path());

        assert!(store.prepare().await.unwrap().is_empty());

        let restored = WhatsNewStore::new(dir.path());
        assert!(restored.prepare().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn returns_entries_newer_than_marker() {
        let dir = tempfile::tempdir().unwrap();
        let store = WhatsNewStore::new(dir.path());
        let entries = bundled_entries().unwrap();
        store
            .mark_seen(entries[1].shipped_at_unix_seconds)
            .await
            .unwrap();

        let unseen = store.prepare().await.unwrap();

        assert_eq!(unseen, vec![entries[0].clone()]);
    }

    #[tokio::test]
    async fn mark_seen_never_moves_marker_backwards() {
        let dir = tempfile::tempdir().unwrap();
        let store = WhatsNewStore::new(dir.path());
        let entries = bundled_entries().unwrap();
        store
            .mark_seen(entries[0].shipped_at_unix_seconds)
            .await
            .unwrap();
        store
            .mark_seen(entries[1].shipped_at_unix_seconds)
            .await
            .unwrap();

        assert!(store.prepare().await.unwrap().is_empty());
    }
}
