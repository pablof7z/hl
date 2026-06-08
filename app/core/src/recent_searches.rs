//! Rust-owned recent-search ring buffer.
//!
//! Native shells can render and trigger search suggestions, but persistence
//! and de-duplication live here so iOS and Android share the same behavior.

use std::path::{Path, PathBuf};

use tokio::sync::Mutex;

use crate::errors::CoreError;

const CACHE_FILE_NAME: &str = "recent-searches-v1.json";
const MAX_RECENT_SEARCHES: usize = 8;

pub struct RecentSearchesStore {
    path: PathBuf,
    entries: Mutex<Option<Vec<String>>>,
}

impl RecentSearchesStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join(CACHE_FILE_NAME),
            entries: Mutex::new(None),
        }
    }

    pub async fn all(&self) -> Result<Vec<String>, CoreError> {
        let mut guard = self.entries.lock().await;
        if guard.is_none() {
            *guard = Some(load_searches(&self.path).await);
        }
        Ok(guard.as_ref().expect("recent searches initialized").clone())
    }

    pub async fn record(&self, query: &str) -> Result<Vec<String>, CoreError> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return self.all().await;
        }

        let mut guard = self.entries.lock().await;
        if guard.is_none() {
            *guard = Some(load_searches(&self.path).await);
        }

        let entries = guard.as_mut().expect("recent searches initialized");
        let key = case_fold_key(trimmed);
        entries.retain(|existing| case_fold_key(existing) != key);
        entries.insert(0, trimmed.to_string());
        entries.truncate(MAX_RECENT_SEARCHES);
        persist_searches(&self.path, entries).await?;
        Ok(entries.clone())
    }

    pub async fn clear(&self) -> Result<Vec<String>, CoreError> {
        let mut guard = self.entries.lock().await;
        *guard = Some(Vec::new());
        match tokio::fs::remove_file(&self.path).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(CoreError::Cache(format!("clear recent searches: {e}"))),
        }
        Ok(Vec::new())
    }
}

async fn load_searches(path: &Path) -> Vec<String> {
    match tokio::fs::read(path).await {
        Ok(bytes) => match serde_json::from_slice::<Vec<String>>(&bytes) {
            Ok(searches) => normalize_loaded(searches),
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to parse recent searches");
                Vec::new()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "failed to read recent searches");
            Vec::new()
        }
    }
}

fn normalize_loaded(searches: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for search in searches {
        let trimmed = search.trim();
        let key = case_fold_key(trimmed);
        if trimmed.is_empty() || out.iter().any(|existing| case_fold_key(existing) == key) {
            continue;
        }
        out.push(trimmed.to_string());
        if out.len() == MAX_RECENT_SEARCHES {
            break;
        }
    }
    out
}

fn case_fold_key(value: &str) -> String {
    value.to_lowercase()
}

async fn persist_searches(path: &Path, searches: &[String]) -> Result<(), CoreError> {
    let bytes = serde_json::to_vec(searches)
        .map_err(|e| CoreError::Cache(format!("encode recent searches: {e}")))?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| CoreError::Cache(format!("create recent searches dir: {e}")))?;
    }
    let tmp = path.with_extension("json.tmp");
    tokio::fs::write(&tmp, bytes)
        .await
        .map_err(|e| CoreError::Cache(format!("write recent searches: {e}")))?;
    tokio::fs::rename(&tmp, path)
        .await
        .map_err(|e| CoreError::Cache(format!("commit recent searches: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn record_trims_dedupes_and_caps_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let store = RecentSearchesStore::new(dir.path());

        for query in [
            "  Bitcoin  ",
            "Attention",
            "Dostoevsky",
            "Borges",
            "Philosophy",
            "Svelte",
            "Rust",
            "Nostr",
            "Relay",
            "bitcoin",
        ] {
            store.record(query).await.unwrap();
        }

        assert_eq!(
            store.all().await.unwrap(),
            vec![
                "bitcoin",
                "Relay",
                "Nostr",
                "Rust",
                "Svelte",
                "Philosophy",
                "Borges",
                "Dostoevsky"
            ]
        );
    }

    #[tokio::test]
    async fn persists_across_store_instances() {
        let dir = tempfile::tempdir().unwrap();
        RecentSearchesStore::new(dir.path())
            .record("Borges")
            .await
            .unwrap();

        let restored = RecentSearchesStore::new(dir.path());
        assert_eq!(restored.all().await.unwrap(), vec!["Borges"]);
    }

    #[tokio::test]
    async fn clear_removes_persisted_history() {
        let dir = tempfile::tempdir().unwrap();
        let store = RecentSearchesStore::new(dir.path());
        store.record("Borges").await.unwrap();
        assert_eq!(store.clear().await.unwrap(), Vec::<String>::new());

        let restored = RecentSearchesStore::new(dir.path());
        assert!(restored.all().await.unwrap().is_empty());
    }
}
