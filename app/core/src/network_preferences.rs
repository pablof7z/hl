//! Rust-owned network preference state.
//!
//! Platform shells enforce capabilities such as "disconnect on cellular",
//! but durable user preference bits live here for cross-platform parity.

use std::path::{Path, PathBuf};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::errors::CoreError;

const STATE_FILE_NAME: &str = "network-preferences-v1.json";

pub struct NetworkPreferencesStore {
    path: PathBuf,
    state: Mutex<Option<NetworkPreferencesState>>,
}

impl NetworkPreferencesStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join(STATE_FILE_NAME),
            state: Mutex::new(None),
        }
    }

    pub fn wifi_only_enabled(&self) -> bool {
        let mut guard = self.state.lock();
        if guard.is_none() {
            *guard = Some(load_state(&self.path));
        }
        guard
            .as_ref()
            .map(|state| state.wifi_only_enabled)
            .unwrap_or(false)
    }

    pub fn set_wifi_only_enabled(&self, enabled: bool) -> Result<(), CoreError> {
        let mut guard = self.state.lock();
        if guard.is_none() {
            *guard = Some(load_state(&self.path));
        }
        let mut state = guard.clone().unwrap_or_default();
        state.wifi_only_enabled = enabled;
        persist_state(&self.path, &state)?;
        *guard = Some(state);
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct NetworkPreferencesState {
    wifi_only_enabled: bool,
}

fn load_state(path: &Path) -> NetworkPreferencesState {
    match std::fs::read(path) {
        Ok(bytes) => match serde_json::from_slice::<NetworkPreferencesState>(&bytes) {
            Ok(state) => state,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to parse network preferences");
                NetworkPreferencesState::default()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => NetworkPreferencesState::default(),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "failed to read network preferences");
            NetworkPreferencesState::default()
        }
    }
}

fn persist_state(path: &Path, state: &NetworkPreferencesState) -> Result<(), CoreError> {
    let bytes = serde_json::to_vec(state)
        .map_err(|e| CoreError::Cache(format!("encode network preferences: {e}")))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| CoreError::Cache(format!("create network preferences dir: {e}")))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)
        .map_err(|e| CoreError::Cache(format!("write network preferences: {e}")))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| CoreError::Cache(format!("commit network preferences: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_wifi_only_is_off() {
        let dir = tempfile::tempdir().unwrap();
        let store = NetworkPreferencesStore::new(dir.path());

        assert!(!store.wifi_only_enabled());
    }

    #[test]
    fn wifi_only_persists_across_instances() {
        let dir = tempfile::tempdir().unwrap();
        NetworkPreferencesStore::new(dir.path())
            .set_wifi_only_enabled(true)
            .unwrap();

        let restored = NetworkPreferencesStore::new(dir.path());
        assert!(restored.wifi_only_enabled());
    }

    #[test]
    fn wifi_only_can_be_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let store = NetworkPreferencesStore::new(dir.path());
        store.set_wifi_only_enabled(true).unwrap();
        store.set_wifi_only_enabled(false).unwrap();

        let restored = NetworkPreferencesStore::new(dir.path());
        assert!(!restored.wifi_only_enabled());
    }
}
