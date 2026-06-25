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
use crate::models::MutationSnapshot;
use crate::nostr_runtime::NostrRuntime;
use crate::onboarding;
use crate::podcast_playback;
use crate::podcast_position;

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

    pub fn set_event_callback(&self, callback: Arc<dyn EventCallback>) {
        *self.callback_slot.write() = Some(callback);
    }

    /// Fetch another nostr user's relay list (kind:10002 NIP-65) from the
    /// indexer relay pool and return their configured relays. Used by the
    /// "Import from npub" flow. Returns an empty list on parse errors or when
    /// no kind:10002 is found within the 8-second timeout.
    pub async fn fetch_relays_for_pubkey(
        &self,
        pubkey_hex: String,
    ) -> Vec<crate::relays::RelayConfig> {
        let result: Result<Vec<crate::relays::RelayConfig>, crate::errors::CoreError> = async {
            // Accept hex pubkeys and npub1… bech32-encoded pubkeys.
            let pubkey = PublicKey::from_hex(&pubkey_hex)
                .or_else(|_| {
                    Nip19::from_bech32(&pubkey_hex)
                        .ok()
                        .and_then(|decoded| {
                            if let Nip19::Pubkey(pk) = decoded {
                                Some(pk)
                            } else {
                                None
                            }
                        })
                        .ok_or_else(|| nostr_sdk::key::Error::InvalidPublicKey)
                })
                .map_err(|e| crate::errors::CoreError::Other(format!("invalid pubkey: {e}")))?;
            let filter = nostr_sdk::Filter::new()
                .author(pubkey)
                .kind(nostr_sdk::Kind::Custom(10002))
                .limit(1);
            let indexer_urls = self.runtime.indexer_urls();
            let events = self
                .runtime
                .client()
                .fetch_events_from(indexer_urls, filter, std::time::Duration::from_secs(8))
                .await
                .map_err(|e| crate::errors::CoreError::Relay(format!("fetch relays: {e}")))?;
            let rows = events
                .iter()
                .flat_map(crate::relays::parse_nip65_event)
                .map(|(url, read, write)| crate::relays::RelayConfig {
                    url,
                    read,
                    write,
                    rooms: false,
                    indexer: false,
                })
                .collect();
            Ok(rows)
        }
        .await;
        result.unwrap_or_default()
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
}
