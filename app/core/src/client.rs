//! Top-level UniFFI-exposed object. Swift holds one `HighlighterCore` for
//! the life of the app.
//!
//! State discipline: async methods never hold the `parking_lot` guard across
//! an `.await` point (the guard isn't `Send`). Long-running protocol work
//! happens in `Session` / feature modules, which own their own async state.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;

use crate::clock::{Clock, SystemClock};
use crate::errors::CoreError;
use crate::events::EventCallback;
use crate::models::MutationSnapshot;
use crate::onboarding;
use crate::podcast_playback;
use crate::podcast_position;

#[derive(uniffi::Object)]
pub struct HighlighterCore {
    data_dir: PathBuf,
    /// Shared with every pump task so `set_event_callback` can replace the
    /// callback atomically mid-flight.
    callback_slot: Arc<RwLock<Option<Arc<dyn EventCallback>>>>,
    /// Rust-owned durable onboarding completion flag.
    onboarding: Arc<onboarding::OnboardingStore>,
    /// Rust-owned durable podcast playback position.
    podcast_position: Arc<podcast_position::PodcastPositionStore>,
    /// Kernel-owned clock shared by feature modules that need timestamps.
    clock: Arc<dyn Clock>,
}

fn mutation_snapshot(result: Result<(), CoreError>) -> MutationSnapshot {
    match result {
        Ok(()) => MutationSnapshot {
            applied: true,
            error: String::new(),
        },
        Err(error) => MutationSnapshot {
            applied: false,
            error: error.to_string(),
        },
    }
}

#[uniffi::export]
impl HighlighterCore {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        let data_dir = default_data_dir().expect("nostr runtime initialization must succeed");
        Self::assemble(data_dir)
    }

    pub fn plan_podcast_playback_session(
        &self,
        input: podcast_playback::PodcastPlaybackSessionInput,
    ) -> podcast_playback::PodcastPlaybackSessionPlan {
        let guid = input.artifact.preview.podcast_item_guid.trim();
        let saved_position_seconds = if guid.is_empty() {
            None
        } else {
            self.podcast_position.position_for_guid(guid)
        };
        podcast_playback::session_plan(input, saved_position_seconds)
    }

    pub fn record_podcast_playback_position(
        &self,
        input: podcast_playback::PodcastPlaybackPositionInput,
    ) -> MutationSnapshot {
        let result = (|| {
            let Some(request) = podcast_playback::position_save_request(input)? else {
                return Ok(());
            };
            self.podcast_position
                .save(request.guid, request.position_seconds, request.artifact)
        })();
        mutation_snapshot(result)
    }

    pub fn get_podcast_playback_rehydration_snapshot(
        &self,
        has_current_artifact: bool,
    ) -> podcast_playback::PodcastPlaybackRehydrationSnapshot {
        let record = if has_current_artifact {
            None
        } else {
            self.podcast_position.current()
        };
        podcast_playback::rehydration_snapshot(has_current_artifact, record)
    }

    pub fn set_event_callback(&self, callback: Arc<dyn EventCallback>) {
        *self.callback_slot.write() = Some(callback);
    }
}

impl HighlighterCore {
    /// The directory used for durable on-disk stores (onboarding flag, podcast
    /// positions, etc.). Not exposed to Swift.
    #[allow(dead_code)]
    pub(crate) fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Construct with an isolated data directory. Used by tests to avoid
    /// polluting the real application data dir. Not annotated with
    /// `#[uniffi::export]`, so it stays out of the Swift surface.
    #[doc(hidden)]
    pub fn new_with_data_dir(data_dir: std::path::PathBuf) -> Arc<Self> {
        Self::assemble(data_dir)
    }

    fn assemble(data_dir: PathBuf) -> Arc<Self> {
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        Self::assemble_with_clock(data_dir, clock)
    }

    fn assemble_with_clock(data_dir: PathBuf, clock: Arc<dyn Clock>) -> Arc<Self> {
        let callback_slot: Arc<RwLock<Option<Arc<dyn EventCallback>>>> =
            Arc::new(RwLock::new(None));
        let onboarding = Arc::new(onboarding::OnboardingStore::new(&data_dir));
        let podcast_position = Arc::new(podcast_position::PodcastPositionStore::new_with_clock(
            &data_dir,
            clock.clone(),
        ));
        Arc::new(Self {
            data_dir,
            callback_slot,
            onboarding,
            podcast_position,
            clock,
        })
    }
}

/// Resolve the platform-appropriate data directory. On iOS we're inside a
/// sandboxed container; `dirs::data_dir()` resolves to
/// `<app>/Library/Application Support`, the correct location for persistent,
/// non-user-visible data.
fn default_data_dir() -> Result<PathBuf, CoreError> {
    let base = dirs::data_dir()
        .ok_or_else(|| CoreError::Cache("no platform data_dir available".into()))?;
    Ok(base.join("highlighter").join("ndb"))
}
