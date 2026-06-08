//! Rust-owned podcast playback position.
//!
//! Native shells execute audio playback through platform media APIs. The
//! durable "what episode and where was I?" fact lives in the shared core so
//! iOS and Android resume from the same canonical artifact projection.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;

use crate::errors::CoreError;
use crate::models::{ArtifactRecord, PodcastPositionRecord};

const STATE_FILE_NAME: &str = "podcast-position-v1.json";
const MAX_AGE_SECONDS: u64 = 7 * 24 * 60 * 60;

pub struct PodcastPositionStore {
    path: PathBuf,
    record: Mutex<Option<Option<PodcastPositionRecord>>>,
}

impl PodcastPositionStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join(STATE_FILE_NAME),
            record: Mutex::new(None),
        }
    }

    pub fn current(&self) -> Option<PodcastPositionRecord> {
        let mut guard = self.record.lock();
        if guard.is_none() {
            *guard = Some(load_record(&self.path));
        }

        let record = guard.as_ref().and_then(Clone::clone)?;
        let Ok(now) = now_unix_seconds() else {
            tracing::warn!("system clock is before unix epoch while reading podcast position");
            return Some(record);
        };
        if is_stale(&record, now) {
            if let Err(e) = remove_record(&self.path) {
                tracing::warn!(path = %self.path.display(), error = %e, "failed to remove stale podcast position");
            }
            *guard = Some(None);
            return None;
        }

        Some(record)
    }

    pub fn position_for_guid(&self, guid: &str) -> Option<f64> {
        let guid = guid.trim();
        if guid.is_empty() {
            return None;
        }
        self.current().and_then(|record| {
            if record.guid == guid {
                Some(record.position_seconds)
            } else {
                None
            }
        })
    }

    pub fn save(
        &self,
        guid: String,
        position_seconds: f64,
        artifact: ArtifactRecord,
    ) -> Result<(), CoreError> {
        let guid = guid.trim().to_string();
        if guid.is_empty() {
            return Err(CoreError::InvalidInput(
                "podcast guid must not be empty".into(),
            ));
        }
        let position_seconds = normalize_position(position_seconds)?;
        let record = PodcastPositionRecord {
            guid,
            position_seconds,
            last_played_at_unix_seconds: now_unix_seconds()?,
            artifact,
        };

        persist_record(&self.path, &record)?;
        *self.record.lock() = Some(Some(record));
        Ok(())
    }
}

fn load_record(path: &Path) -> Option<PodcastPositionRecord> {
    match std::fs::read(path) {
        Ok(bytes) => match serde_json::from_slice::<PodcastPositionRecord>(&bytes) {
            Ok(record) => Some(record),
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to parse podcast position");
                None
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "failed to read podcast position");
            None
        }
    }
}

fn persist_record(path: &Path, record: &PodcastPositionRecord) -> Result<(), CoreError> {
    let bytes = serde_json::to_vec(record)
        .map_err(|e| CoreError::Cache(format!("encode podcast position: {e}")))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| CoreError::Cache(format!("create podcast position dir: {e}")))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)
        .map_err(|e| CoreError::Cache(format!("write podcast position: {e}")))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| CoreError::Cache(format!("commit podcast position: {e}")))?;
    Ok(())
}

fn remove_record(path: &Path) -> Result<(), CoreError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(CoreError::Cache(format!("clear podcast position: {e}"))),
    }
}

fn normalize_position(position_seconds: f64) -> Result<f64, CoreError> {
    if !position_seconds.is_finite() {
        return Err(CoreError::InvalidInput(
            "podcast position must be finite".into(),
        ));
    }
    if position_seconds < 0.0 {
        return Err(CoreError::InvalidInput(
            "podcast position must not be negative".into(),
        ));
    }
    Ok(position_seconds)
}

fn now_unix_seconds() -> Result<u64, CoreError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|e| CoreError::Other(format!("system clock before unix epoch: {e}")))
}

fn is_stale(record: &PodcastPositionRecord, now: u64) -> bool {
    now.saturating_sub(record.last_played_at_unix_seconds) >= MAX_AGE_SECONDS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ArtifactPreview, Chapter};

    #[test]
    fn default_position_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = PodcastPositionStore::new(dir.path());

        assert!(store.current().is_none());
    }

    #[test]
    fn saved_position_persists_with_artifact_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        PodcastPositionStore::new(dir.path())
            .save("episode-guid".into(), 42.5, sample_artifact())
            .unwrap();

        let restored = PodcastPositionStore::new(dir.path());
        let record = restored.current().unwrap();
        assert_eq!(record.guid, "episode-guid");
        assert_eq!(record.position_seconds, 42.5);
        assert_eq!(record.artifact.preview.title, "Episode title");
    }

    #[test]
    fn position_lookup_requires_matching_guid() {
        let dir = tempfile::tempdir().unwrap();
        let store = PodcastPositionStore::new(dir.path());
        store
            .save("episode-guid".into(), 42.5, sample_artifact())
            .unwrap();

        assert_eq!(store.position_for_guid("episode-guid"), Some(42.5));
        assert_eq!(store.position_for_guid("other-guid"), None);
    }

    #[test]
    fn stale_position_is_hidden_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(STATE_FILE_NAME);
        let record = PodcastPositionRecord {
            guid: "episode-guid".into(),
            position_seconds: 12.0,
            last_played_at_unix_seconds: 1,
            artifact: sample_artifact(),
        };
        std::fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();

        let store = PodcastPositionStore::new(dir.path());

        assert!(store.current().is_none());
        assert!(!path.exists());
    }

    #[test]
    fn invalid_position_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = PodcastPositionStore::new(dir.path());

        assert!(store
            .save("episode-guid".into(), f64::NAN, sample_artifact())
            .is_err());
        assert!(store
            .save("episode-guid".into(), -1.0, sample_artifact())
            .is_err());
    }

    #[test]
    fn empty_guid_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = PodcastPositionStore::new(dir.path());

        assert!(store.save("  ".into(), 1.0, sample_artifact()).is_err());
    }

    fn sample_artifact() -> ArtifactRecord {
        ArtifactRecord {
            preview: ArtifactPreview {
                id: "share-id".into(),
                url: "https://example.com/episode".into(),
                title: "Episode title".into(),
                author: "Host".into(),
                image: "https://example.com/art.jpg".into(),
                description: "Description".into(),
                source: "podcast".into(),
                domain: "example.com".into(),
                catalog_id: "episode-guid".into(),
                catalog_kind: "podcast:item:guid".into(),
                podcast_guid: "feed-guid".into(),
                podcast_item_guid: "episode-guid".into(),
                podcast_show_title: "Show".into(),
                audio_url: "https://example.com/audio.mp3".into(),
                audio_preview_url: String::new(),
                transcript_url: String::new(),
                feed_url: "https://example.com/feed.xml".into(),
                published_at: String::new(),
                duration_seconds: Some(300),
                reference_tag_name: "i".into(),
                reference_tag_value: "podcast:item:guid:episode-guid".into(),
                reference_kind: "podcast:item:guid".into(),
                highlight_tag_name: "i".into(),
                highlight_tag_value: "podcast:item:guid:episode-guid".into(),
                highlight_reference_key: "i:podcast:item:guid:episode-guid".into(),
                chapters: vec![Chapter {
                    start_seconds: 0.0,
                    title: "Intro".into(),
                }],
            },
            group_id: "group".into(),
            share_event_id: "share-id".into(),
            pubkey: "pubkey".into(),
            created_at: Some(10),
            note: "note".into(),
        }
    }
}
