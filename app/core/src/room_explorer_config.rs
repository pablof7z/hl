//! Rust-owned room explorer configuration.
//!
//! The native shells render explorer shelves, but curator policy and the
//! NIP-11 discovery/cache path live here so every platform uses the same
//! featured-room source.

use std::path::{Path, PathBuf};

use nostr_sdk::prelude::PublicKey;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::errors::CoreError;

const CACHE_FILE_NAME: &str = "room-explorer-config-v1.json";
const DEFAULT_CURATOR_PUBKEY_HEX: &str =
    "7e1eabe25256545cfe0c534a99bfa5c6cd224e04b614182a9993feff54196c95";

pub struct RoomExplorerConfigStore {
    path: PathBuf,
    cached_curator: Mutex<Option<Option<String>>>,
}

impl RoomExplorerConfigStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join(CACHE_FILE_NAME),
            cached_curator: Mutex::new(None),
        }
    }

    pub async fn curator_pubkey(&self) -> Result<String, CoreError> {
        let cached = self.cached_curator().await;
        Ok(cached.unwrap_or_else(|| DEFAULT_CURATOR_PUBKEY_HEX.to_string()))
    }

    pub async fn refresh_curator_pubkey(&self) -> Result<String, CoreError> {
        match fetch_curator_pubkey().await {
            Ok(pubkey) => {
                self.persist_curator_pubkey(&pubkey).await?;
                Ok(pubkey)
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to refresh room explorer curator pubkey");
                self.curator_pubkey().await
            }
        }
    }

    async fn cached_curator(&self) -> Option<String> {
        let mut guard = self.cached_curator.lock().await;
        if guard.is_none() {
            *guard = Some(load_config(&self.path).await.and_then(validate_pubkey));
        }
        guard
            .as_ref()
            .expect("room explorer config initialized")
            .clone()
    }

    async fn persist_curator_pubkey(&self, pubkey: &str) -> Result<(), CoreError> {
        let normalized = validate_pubkey(pubkey.to_string())
            .ok_or_else(|| CoreError::InvalidInput("invalid curator pubkey".into()))?;
        let config = RoomExplorerConfig {
            curator_pubkey_hex: normalized.clone(),
        };
        persist_config(&self.path, &config).await?;
        let mut guard = self.cached_curator.lock().await;
        *guard = Some(Some(normalized));
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RoomExplorerConfig {
    curator_pubkey_hex: String,
}

async fn fetch_curator_pubkey() -> Result<String, CoreError> {
    let doc =
        crate::relay_polish::probe_nip11(crate::relays::room_explorer_curator_relay()).await?;
    let pubkey = doc
        .pubkey
        .ok_or_else(|| CoreError::Network("curator relay NIP-11 omitted pubkey".into()))?;
    validate_pubkey(pubkey)
        .ok_or_else(|| CoreError::Network("curator relay NIP-11 pubkey is invalid".into()))
}

async fn load_config(path: &Path) -> Option<String> {
    match tokio::fs::read(path).await {
        Ok(bytes) => match serde_json::from_slice::<RoomExplorerConfig>(&bytes) {
            Ok(config) => Some(config.curator_pubkey_hex),
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to parse room explorer config");
                None
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "failed to read room explorer config");
            None
        }
    }
}

fn validate_pubkey(pubkey: String) -> Option<String> {
    let trimmed = pubkey.trim();
    let parsed = PublicKey::from_hex(trimmed).ok()?;
    Some(parsed.to_hex())
}

async fn persist_config(path: &Path, config: &RoomExplorerConfig) -> Result<(), CoreError> {
    let bytes = serde_json::to_vec(config)
        .map_err(|e| CoreError::Cache(format!("encode room explorer config: {e}")))?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| CoreError::Cache(format!("create room explorer config dir: {e}")))?;
    }
    let tmp = path.with_extension("json.tmp");
    tokio::fs::write(&tmp, bytes)
        .await
        .map_err(|e| CoreError::Cache(format!("write room explorer config: {e}")))?;
    tokio::fs::rename(&tmp, path)
        .await
        .map_err(|e| CoreError::Cache(format!("commit room explorer config: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn curator_pubkey_returns_default_without_cache() {
        let dir = tempfile::tempdir().unwrap();
        let store = RoomExplorerConfigStore::new(dir.path());

        assert_eq!(
            store.curator_pubkey().await.unwrap(),
            DEFAULT_CURATOR_PUBKEY_HEX
        );
    }

    #[tokio::test]
    async fn curator_pubkey_returns_cached_valid_pubkey() {
        let dir = tempfile::tempdir().unwrap();
        let store = RoomExplorerConfigStore::new(dir.path());
        let cached = "0000000000000000000000000000000000000000000000000000000000000001";

        store.persist_curator_pubkey(cached).await.unwrap();

        let restored = RoomExplorerConfigStore::new(dir.path());
        assert_eq!(restored.curator_pubkey().await.unwrap(), cached);
    }

    #[tokio::test]
    async fn invalid_cached_pubkey_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CACHE_FILE_NAME);
        persist_config(
            &path,
            &RoomExplorerConfig {
                curator_pubkey_hex: "not-a-pubkey".into(),
            },
        )
        .await
        .unwrap();

        let store = RoomExplorerConfigStore::new(dir.path());
        assert_eq!(
            store.curator_pubkey().await.unwrap(),
            DEFAULT_CURATOR_PUBKEY_HEX
        );
    }
}
