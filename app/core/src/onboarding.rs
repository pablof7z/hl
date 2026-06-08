//! Rust-owned onboarding completion state.
//!
//! Native shells choose the visual route, but the durable "has completed
//! onboarding" flag lives in the shared core so iOS and Android agree.

use std::path::{Path, PathBuf};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::errors::CoreError;

const STATE_FILE_NAME: &str = "onboarding-state-v1.json";

pub struct OnboardingStore {
    path: PathBuf,
    complete: Mutex<Option<bool>>,
}

impl OnboardingStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join(STATE_FILE_NAME),
            complete: Mutex::new(None),
        }
    }

    pub fn is_complete(&self) -> bool {
        let mut guard = self.complete.lock();
        if guard.is_none() {
            *guard = Some(load_state(&self.path));
        }
        guard.unwrap_or(false)
    }

    pub fn set_complete(&self, complete: bool) -> Result<(), CoreError> {
        if complete {
            persist_state(&self.path, true)?;
        } else {
            remove_state(&self.path)?;
        }
        *self.complete.lock() = Some(complete);
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct OnboardingState {
    complete: bool,
}

fn load_state(path: &Path) -> bool {
    match std::fs::read(path) {
        Ok(bytes) => match serde_json::from_slice::<OnboardingState>(&bytes) {
            Ok(state) => state.complete,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to parse onboarding state");
                false
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "failed to read onboarding state");
            false
        }
    }
}

fn persist_state(path: &Path, complete: bool) -> Result<(), CoreError> {
    let bytes = serde_json::to_vec(&OnboardingState { complete })
        .map_err(|e| CoreError::Cache(format!("encode onboarding state: {e}")))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| CoreError::Cache(format!("create onboarding state dir: {e}")))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)
        .map_err(|e| CoreError::Cache(format!("write onboarding state: {e}")))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| CoreError::Cache(format!("commit onboarding state: {e}")))?;
    Ok(())
}

fn remove_state(path: &Path) -> Result<(), CoreError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(CoreError::Cache(format!("clear onboarding state: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_incomplete() {
        let dir = tempfile::tempdir().unwrap();
        let store = OnboardingStore::new(dir.path());

        assert!(!store.is_complete());
    }

    #[test]
    fn completion_persists_across_instances() {
        let dir = tempfile::tempdir().unwrap();
        OnboardingStore::new(dir.path()).set_complete(true).unwrap();

        let restored = OnboardingStore::new(dir.path());
        assert!(restored.is_complete());
    }

    #[test]
    fn reset_clears_persisted_completion() {
        let dir = tempfile::tempdir().unwrap();
        let store = OnboardingStore::new(dir.path());
        store.set_complete(true).unwrap();
        store.set_complete(false).unwrap();

        let restored = OnboardingStore::new(dir.path());
        assert!(!restored.is_complete());
    }
}
