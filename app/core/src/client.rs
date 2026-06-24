//! Top-level UniFFI-exposed object. Swift holds one `HighlighterCore` for
//! the life of the app.
//!
//! State discipline: async methods never hold the `parking_lot` guard across
//! an `.await` point (the guard isn't `Send`). Long-running protocol work
//! happens in `Session` / feature modules, which own their own async state.

use std::sync::Arc;

use nostr_sdk::prelude::*;
use parking_lot::RwLock;

use crate::clock::{Clock, SystemClock};
use crate::errors::CoreError;
use crate::events::EventCallback;
use crate::highlights;
use crate::models::{ArtifactRecord, HighlightRecord, MutationSnapshot};
use crate::nostr_runtime::NostrRuntime;
use crate::onboarding;
use crate::podcast_playback;
use crate::podcast_position;
use crate::podcast_transcript;

#[derive(uniffi::Object)]
pub struct HighlighterCore {
    runtime: Arc<NostrRuntime>,
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

fn one_highlight_record(records: Vec<HighlightRecord>) -> Result<HighlightRecord, CoreError> {
    records
        .into_iter()
        .next()
        .ok_or_else(|| CoreError::Relay("No highlight returned from publish.".into()))
}

#[uniffi::export(async_runtime = "tokio")]
impl HighlighterCore {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        let runtime =
            Arc::new(NostrRuntime::new().expect("nostr runtime initialization must succeed"));
        Self::assemble(runtime)
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

    pub async fn get_podcast_listening_clips_snapshot(
        &self,
        artifact: Option<ArtifactRecord>,
        limit: u32,
    ) -> podcast_transcript::PodcastListeningClipsSnapshot {
        let result = (|| {
            let Some(artifact) = artifact else {
                return Ok(Vec::new());
            };
            let reference = podcast_transcript::podcast_clip_reference(&artifact);
            let Some(tag) = reference.tag_name.trim().chars().next() else {
                return Ok(Vec::new());
            };
            highlights::query_for_reference(
                self.runtime.ndb(),
                tag,
                reference.tag_value.trim(),
                if limit == 0 { reference.limit } else { limit },
            )
        })();
        match result {
            Ok(clips) => podcast_transcript::listening_clips_snapshot(clips, ""),
            Err(error) => podcast_transcript::listening_clips_snapshot(Vec::new(), error),
        }
    }

    pub fn set_event_callback(&self, callback: Arc<dyn EventCallback>) {
        *self.callback_slot.write() = Some(callback);
    }

    pub async fn publish_podcast_clip_highlight(
        &self,
        input: podcast_transcript::PodcastClipPublishInput,
    ) -> podcast_transcript::PodcastClipPublishSnapshot {
        let result: Result<HighlightRecord, CoreError> = async {
            let _ = self.require_user_pubkey().await?;
            let draft = podcast_transcript::clip_highlight_draft(
                &input.segments,
                &input.selected_segment_ids,
                input.note,
                input.clip_start_seconds,
                input.clip_end_seconds,
                input.clip_speaker,
            );
            let records = crate::highlights::publish_and_share(
                &self.runtime,
                input.artifact,
                vec![draft],
                &input.target_group_id,
            )
            .await?;
            one_highlight_record(records)
        }
        .await;
        podcast_transcript::clip_publish_snapshot(result)
    }

    /// Publish a podcast clip from the composer sheet. Rust owns draft
    /// construction and whether the clip is solo-published or also reposted
    /// into a NIP-29 room.
    pub async fn publish_podcast_composer_clip(
        &self,
        input: podcast_transcript::PodcastClipComposerPublishInput,
    ) -> podcast_transcript::PodcastClipPublishSnapshot {
        let result: Result<HighlightRecord, CoreError> = async {
            let _ = self.require_user_pubkey().await?;
            let draft = podcast_transcript::clip_composer_highlight_draft(
                &input.segments,
                input.transcript_available,
                input.context,
                input.clip_start_seconds,
                input.clip_end_seconds,
            );
            let target_group_id = input.target_group_id.unwrap_or_default();
            if target_group_id.trim().is_empty() {
                crate::highlights::publish(&self.runtime, draft, input.artifact).await
            } else {
                let records = crate::highlights::publish_and_share(
                    &self.runtime,
                    input.artifact,
                    vec![draft],
                    &target_group_id,
                )
                .await?;
                one_highlight_record(records)
            }
        }
        .await;
        podcast_transcript::clip_publish_snapshot(result)
    }
}

impl HighlighterCore {
    /// Internal access for feature modules (artifacts, groups, highlights,
    /// recent_books) to the shared Client + Ndb. Not exposed to Swift.
    #[allow(dead_code)]
    pub(crate) fn runtime(&self) -> &NostrRuntime {
        &self.runtime
    }

    /// Construct with an isolated nostrdb path. Used by tests to avoid
    /// polluting the real application data dir. Not annotated with
    /// `#[uniffi::export]`, so it stays out of the Swift surface.
    #[doc(hidden)]
    pub fn new_with_data_dir(data_dir: std::path::PathBuf) -> Arc<Self> {
        let runtime = Arc::new(
            NostrRuntime::with_data_dir(data_dir)
                .expect("nostr runtime initialization must succeed"),
        );
        Self::assemble(runtime)
    }

    fn assemble(runtime: Arc<NostrRuntime>) -> Arc<Self> {
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        Self::assemble_with_clock(runtime, clock)
    }

    fn assemble_with_clock(runtime: Arc<NostrRuntime>, clock: Arc<dyn Clock>) -> Arc<Self> {
        let callback_slot: Arc<RwLock<Option<Arc<dyn EventCallback>>>> =
            Arc::new(RwLock::new(None));
        // Register relay diagnostics with the app event bus before handing
        // out the Arc<Self>. The runtime seeds its bounded diagnostics map
        // from the SDK pool and then reacts to relay status notifications.
        runtime.install_diagnostics_callback(callback_slot.clone());
        let onboarding = Arc::new(onboarding::OnboardingStore::new(runtime.data_dir()));
        let podcast_position = Arc::new(podcast_position::PodcastPositionStore::new_with_clock(
            runtime.data_dir(),
            clock.clone(),
        ));
        Arc::new(Self {
            runtime,
            callback_slot,
            onboarding,
            podcast_position,
            clock,
        })
    }

    async fn require_user_pubkey(&self) -> Result<PublicKey, CoreError> {
        self.runtime
            .client()
            .public_key()
            .await
            .map_err(|_| CoreError::NotAuthenticated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn require_user_pubkey_errors_when_logged_out() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let core = HighlighterCore::new_with_data_dir(tmp.path().join("ndb"));
        assert!(core.require_user_pubkey().await.is_err());
    }
}
