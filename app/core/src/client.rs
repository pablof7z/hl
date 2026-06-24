//! Top-level UniFFI-exposed object. Swift holds one `HighlighterCore` for
//! the life of the app.
//!
//! State discipline: async methods never hold the `parking_lot` guard across
//! an `.await` point (the guard isn't `Send`). Long-running protocol work
//! happens in `Session` / feature modules, which own their own async state.

use std::{collections::BTreeMap, sync::Arc};

use nostr_sdk::prelude::*;
use parking_lot::RwLock;

use crate::article_reader;
use crate::articles;
use crate::blossom;
use crate::clock::{Clock, SystemClock};
use crate::comments;
use crate::errors::CoreError;
use crate::events::{DataChangeType, Delta, EventCallback};
use crate::feedback;
use crate::follows;
use crate::groups;
use crate::highlights;
use crate::isbn_lookup;
use crate::models::{
    ArticleRecord, ArtifactPreview, ArtifactRecord, BlossomUpload, BookRoute,
    BookmarkSetRecord, CommentRecord, CommentScope, CommunitySummary,
    CurrentUser, DiscussionRecord, FeedbackThreadRecord, HighlightRecord, HighlightSourceKind,
    LoginInputAction, MutationSnapshot, NostrConnectOptions, OnboardingInterest,
    OnboardingInterestProjection, OnboardingInterestSelection, ProfileMetadata,
    ProfileUpdateAction, ProfileUpdateDraft, RelayDiagnostic, SubscriptionStartSnapshot,
};
use crate::nip46::{self, BunkerSigner};
use crate::nostr_runtime::NostrRuntime;
use crate::onboarding;
use crate::podcast_playback;
use crate::podcast_position;
use crate::podcast_transcript::{
    self, PodcastClipComposerInput, PodcastClipComposerProjection, PodcastClipSelection,
    PodcastListeningProjection, PodcastListeningProjectionInput, TranscriptSegment,
};
use crate::profile;
use crate::reads;
use crate::relays::nostr_connect_relay;
use crate::session::{current_user_from_pubkey, Session};
use crate::share_links;
use crate::share_targets;
use crate::subscriptions::{SubscriptionKind, SubscriptionRegistry};
use crate::web_metadata::{self, WebMetadata, WebMetadataStore};

#[derive(uniffi::Object)]
pub struct HighlighterCore {
    inner: Arc<RwLock<Inner>>,
    runtime: Arc<NostrRuntime>,
    /// Shared with every pump task so `set_event_callback` can replace the
    /// callback atomically mid-flight.
    callback_slot: Arc<RwLock<Option<Arc<dyn EventCallback>>>>,
    subscriptions: Arc<SubscriptionRegistry>,
    /// OG/favicon cache shared across all `get_web_metadata` calls. Lives
    /// on the core so concurrent fetches for the same URL coalesce.
    web_metadata: Arc<WebMetadataStore>,
    /// Rust-owned persistent ISBN preview cache. Native shells render these
    /// previews but do not mirror them in platform storage.
    isbn_previews: Arc<isbn_lookup::IsbnPreviewCache>,
    /// Rust-owned durable onboarding completion flag.
    onboarding: Arc<onboarding::OnboardingStore>,
    /// Rust-owned durable podcast playback position.
    podcast_position: Arc<podcast_position::PodcastPositionStore>,
    /// Kernel-owned clock shared by feature modules that need timestamps.
    clock: Arc<dyn Clock>,
}

struct Inner {
    session: Session,
    pending_joins: BTreeMap<String, String>,
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

fn join_room_display_name(room_name: &str) -> String {
    let trimmed = room_name.trim();
    if trimmed.is_empty() {
        "this room".to_string()
    } else {
        trimmed.to_string()
    }
}

fn subscription_start_snapshot(result: Result<u64, CoreError>) -> SubscriptionStartSnapshot {
    match result {
        Ok(handle) => SubscriptionStartSnapshot {
            handle,
            error: String::new(),
        },
        Err(error) => SubscriptionStartSnapshot {
            handle: 0,
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

impl HighlighterCore {
    fn record_nsec_login(&self, nsec: &str) -> Result<(CurrentUser, Option<Keys>), CoreError> {
        // Do the session mutation + keys extraction in a single write-guard
        // scope. Binding both values to locals ensures the guard drops
        // before the subsequent `self.inner.write()` call — without this,
        // Rust keeps the guard alive for the whole expression chain and
        // parking_lot deadlocks on re-entry.
        let mut guard = self.inner.write();
        let user = guard.session.login_nsec(nsec)?;
        let keys = guard.session.keys().cloned();
        Ok((user, keys))
    }

    fn login_nsec_result(&self, nsec: &str) -> Result<CurrentUser, CoreError> {
        let (user, keys) = self.record_nsec_login(nsec)?;

        if let Some(keys) = keys {
            self.runtime.set_signer(keys.clone());
            self.bootstrap_nsec_pubkey(keys.public_key());
        }

        Ok(user)
    }

    async fn login_nsec_result_async(&self, nsec: &str) -> Result<CurrentUser, CoreError> {
        let (user, keys) = self.record_nsec_login(nsec)?;

        if let Some(keys) = keys {
            self.runtime.set_signer_async(keys.clone()).await;
            self.bootstrap_nsec_pubkey(keys.public_key());
        }

        Ok(user)
    }

    fn bootstrap_nsec_pubkey(&self, pubkey: PublicKey) {
        // First-pass: apply whatever's in cache so subscriptions have a
        // pool to talk to immediately. The bootstrap below races to
        // fetch the user's actual NIP-65 from the network and re-apply
        // — without it, a fresh install with cold cache stays on
        // seed_defaults forever.
        self.runtime.spawn_apply_user_relay_config(pubkey.to_hex());
        let user_relay_config_id = self.runtime.spawn_user_relay_config_bootstrap(pubkey);
        let sub_id = self.runtime.spawn_membership_subscription(pubkey);
        let contacts_id = self.runtime.spawn_contacts_subscription(pubkey);
        // Eagerly fetch 39000 metadata for any groups already in the
        // nostrdb cache. Without this, the first `getJoinedCommunities`
        // call on a warm cache would return summaries with name=id because
        // the stage-2 metadata sub would only be installed after the pump
        // sees a live membership delta.
        let cached_ids =
            crate::subscriptions::collect_cached_group_ids(self.runtime.ndb(), &pubkey);
        if !cached_ids.is_empty() {
            self.runtime
                .spawn_group_metadata_subscription(cached_ids.into_iter().collect());
        }
        // Best-effort outbox bootstrap: fetch follows' kind:10002 so the
        // home-feed planner has data to work with. Empty on first login
        // (no kind:3 cached yet) — `subscribe_following_*` will re-arm
        // this whenever it's called, picking up follows discovered since.
        //
        // Also kick off a NIP-77 negentropy sync against purplepag.es
        // for the social trio (kind:0/3/10002) of the same set. Live
        // subscriptions catch incremental updates; negentropy sync is
        // the cheap cold-start path that closes the "no kind:10002
        // cached" gap so the planner stops dumping authors into the
        // fallback shard.
        let cached_follows = current_followed_pubkeys(self.runtime.ndb(), &pubkey);
        self.runtime
            .spawn_negentropy_sync_for_follows(cached_follows.clone());
        let follows_nip65_id = self
            .runtime
            .spawn_follows_relay_lists_subscription(cached_follows);

        let mut guard = self.inner.write();
        guard.session.set_membership_subscription(sub_id);
        guard.session.set_contacts_subscription(contacts_id);
        guard
            .session
            .set_user_relay_config_subscription(user_relay_config_id);
        if let Some(id) = follows_nip65_id {
            guard.session.set_follows_nip65_subscription(id);
        }
    }

    async fn pair_bunker_result(&self, uri: &str) -> Result<CurrentUser, CoreError> {
        let normalized = normalize_bunker_uri(uri);
        if normalized.is_empty() {
            return Err(CoreError::InvalidInput("empty bunker URI".into()));
        }

        let client = self.runtime.client().clone();
        let (signer, user_pubkey) =
            BunkerSigner::pair_with_clock(client, &normalized, self.clock.clone()).await?;
        let user = current_user_from_pubkey(&user_pubkey)?;

        let signer = Arc::new(signer);
        self.runtime.set_signer_async((*signer).clone()).await;
        self.runtime
            .spawn_apply_user_relay_config(user_pubkey.to_hex());

        let sub_id = self.runtime.spawn_membership_subscription(user_pubkey);
        let contacts_id = self.runtime.spawn_contacts_subscription(user_pubkey);
        let cached_ids =
            crate::subscriptions::collect_cached_group_ids(self.runtime.ndb(), &user_pubkey);
        if !cached_ids.is_empty() {
            self.runtime
                .spawn_group_metadata_subscription(cached_ids.into_iter().collect());
        }
        {
            let mut guard = self.inner.write();
            guard.session.set_bunker(signer, user.clone());
            guard.session.set_membership_subscription(sub_id);
            guard.session.set_contacts_subscription(contacts_id);
        }

        let cb = { self.callback_slot.read().clone() };
        if let Some(cb) = cb {
            cb.on_data_changed(Delta {
                subscription_id: 0,
                change: DataChangeType::SignerConnected { user: user.clone() },
            });
        }

        Ok(user)
    }

    async fn start_nostr_connect_with_options(
        &self,
        options: NostrConnectOptions,
        callback: &str,
    ) -> Result<String, CoreError> {
        // Local ephemeral keypair. The remote signer uses this pubkey to
        // address its messages to us over the relay; after pair completion
        // the user's pubkey comes from the remote signer via GetPublicKey.
        let local_keys = Keys::generate();
        let secret = nip46::random_secret();

        let pairing_relay = nostr_connect_relay();
        let uri = nip46::build_nostr_connect_uri(
            local_keys.public_key(),
            pairing_relay,
            &options.name,
            &options.url,
            &options.image,
            &options.perms,
            &secret,
        )?;

        // Ensure the NIP-46 relay is part of the pool before we start
        // listening for the inbound `connect` request. `add_relay` is a
        // no-op if the relay is already known, but we can't rely on the
        // initial pool reconcile having completed yet.
        let client = self.runtime.client().clone();
        if let Err(e) = client.add_relay(pairing_relay).await {
            tracing::warn!(relay = %pairing_relay, error = %e, "add_relay");
        }
        client.connect().await;

        // Spawn a background task that waits for the remote signer to
        // connect and then installs the resulting BunkerSigner. The task
        // must own: the client, callback slot, Session slot, and local keys.
        let inner = self.inner.clone();
        let runtime = self.runtime.clone();
        let callback_slot = self.callback_slot.clone();
        let clock = self.clock.clone();
        self.runtime.runtime_handle().spawn(async move {
            let result = BunkerSigner::await_inbound_with_clock(
                client.clone(),
                local_keys,
                Some(secret),
                clock,
            )
            .await;
            match result {
                Ok((signer, user_pubkey)) => {
                    let user = match current_user_from_pubkey(&user_pubkey) {
                        Ok(u) => u,
                        Err(e) => {
                            tracing::warn!(error = %e, "npub encode after bunker pair");
                            return;
                        }
                    };
                    let signer = Arc::new(signer);
                    // We're inside NostrRuntime's tokio runtime here
                    // (spawned via `runtime_handle().spawn`). The sync
                    // `runtime.set_signer` wrapper uses `block_on`, which
                    // panics when called from that same runtime, so talk to
                    // the client directly instead.
                    client.set_signer((*signer).clone()).await;
                    runtime.spawn_apply_user_relay_config(user_pubkey.to_hex());
                    let sub_id = runtime.spawn_membership_subscription(user_pubkey);
                    let contacts_id = runtime.spawn_contacts_subscription(user_pubkey);
                    {
                        let mut guard = inner.write();
                        guard.session.set_bunker(signer, user.clone());
                        guard.session.set_membership_subscription(sub_id);
                        guard.session.set_contacts_subscription(contacts_id);
                    }
                    let cb = { callback_slot.read().clone() };
                    if let Some(cb) = cb {
                        cb.on_data_changed(Delta {
                            subscription_id: 0,
                            change: DataChangeType::SignerConnected { user },
                        });
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "nostrconnect inbound pairing failed");
                }
            }
        });

        Ok(nip46::append_callback_to_uri(&uri, callback))
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

    // -- Auth (sync) --

    pub fn classify_login_input(&self, input: String) -> LoginInputAction {
        crate::session::classify_login_input(&input)
    }

    pub fn project_public_key_display(
        &self,
        input: crate::session::PublicKeyDisplayProjectionInput,
    ) -> crate::session::PublicKeyDisplayProjection {
        crate::session::public_key_display_projection(input)
    }

    pub fn project_secret_key_display(
        &self,
        input: crate::session::SecretKeyDisplayProjectionInput,
    ) -> crate::session::SecretKeyDisplayProjection {
        crate::session::secret_key_display_projection(input)
    }

    pub fn project_session_storage_write(
        &self,
        input: crate::session::SessionStorageWriteInput,
    ) -> crate::session::SessionStorageWriteSnapshot {
        crate::session::session_storage_write_snapshot(input)
    }

    pub fn current_secret_key_settings_snapshot(
        &self,
        is_revealed: bool,
    ) -> crate::session::SecretKeySettingsSnapshot {
        let nsec = self.inner.read().session.nsec_bech32();
        crate::session::secret_key_settings_snapshot(nsec, is_revealed)
    }

    pub fn project_relative_time_label(
        &self,
        input: crate::time_labels::RelativeTimeLabelInput,
    ) -> crate::time_labels::RelativeTimeLabelProjection {
        crate::time_labels::relative_time_label_projection(input, self.clock.now_unix_seconds())
    }

    pub fn login_nsec(&self, nsec: String) -> crate::session::AuthSessionSnapshot {
        crate::session::auth_session_snapshot_with_persistence(
            self.login_nsec_result(&nsec),
            Some(nsec),
            None,
        )
    }

    pub fn generate_account(&self) -> crate::session::AccountGenerationSnapshot {
        let result: Result<crate::models::GeneratedAccount, CoreError> = (|| {
            let keys = Keys::generate();
            let nsec = keys
                .secret_key()
                .to_bech32()
                .map_err(|e| CoreError::Other(format!("nsec encoding failed: {e}")))?;
            let user = self.login_nsec_result(&nsec)?;
            Ok(crate::models::GeneratedAccount { user, nsec })
        })();
        crate::session::account_generation_snapshot(result)
    }

    pub fn logout(&self) {
        self.subscriptions.clear(&self.runtime);
        {
            let mut guard = self.inner.write();
            if let Some(sub_id) = guard.session.take_membership_subscription() {
                self.runtime.drop_subscription(sub_id);
            }
            if let Some(sub_id) = guard.session.take_contacts_subscription() {
                self.runtime.drop_subscription(sub_id);
            }
            if let Some(sub_id) = guard.session.take_discovery_subscription() {
                self.runtime.drop_subscription(sub_id);
            }
            if let Some(sub_id) = guard.session.take_curation_subscription() {
                self.runtime.drop_subscription(sub_id);
            }
            if let Some(sub_id) = guard.session.take_friends_memberships_subscription() {
                self.runtime.drop_subscription(sub_id);
            }
            if let Some(sub_id) = guard.session.take_follows_nip65_subscription() {
                self.runtime.drop_subscription(sub_id);
            }
            if let Some(sub_id) = guard.session.take_user_relay_config_subscription() {
                self.runtime.drop_subscription(sub_id);
            }
            guard.pending_joins.clear();
            guard.session.logout();
        }
        self.runtime.unset_signer();
    }

    pub fn current_user(&self) -> Option<CurrentUser> {
        self.inner.read().session.current_user()
    }

    pub fn is_onboarding_complete(&self) -> bool {
        self.onboarding.is_complete()
    }

    pub fn set_onboarding_complete(&self, complete: bool) -> MutationSnapshot {
        mutation_snapshot(self.onboarding.set_complete(complete))
    }

    pub fn get_onboarding_interests(&self) -> Vec<OnboardingInterest> {
        onboarding::interest_catalog()
    }

    pub fn get_onboarding_interest_selection(
        &self,
        selected_ids: Vec<String>,
    ) -> OnboardingInterestSelection {
        onboarding::interest_selection(selected_ids)
    }

    pub fn get_onboarding_interest_projection(
        &self,
        selected_ids: Vec<String>,
    ) -> OnboardingInterestProjection {
        onboarding::interest_projection(selected_ids)
    }

    pub fn toggle_onboarding_interest_selection(
        &self,
        selected_ids: Vec<String>,
        interest_id: String,
    ) -> Vec<String> {
        onboarding::toggle_interest_selection(selected_ids, interest_id)
    }

    pub async fn complete_onboarding_interests(
        &self,
        selected_ids: Vec<String>,
    ) -> MutationSnapshot {
        let result: Result<(), CoreError> = async {
            let selection = onboarding::interest_selection(selected_ids);
            if !selection.can_continue {
                return Err(CoreError::InvalidInput(
                    "choose at least three interests".into(),
                ));
            }
            let follower = {
                let guard = self.inner.read();
                guard
                    .session
                    .current_user()
                    .ok_or(CoreError::NotAuthenticated)?
                    .pubkey
            };
            follows::publish_follow_additions(&self.runtime, &follower, &selection.follow_pubkeys)
                .await?;
            self.onboarding.set_complete(true)?;
            Ok(())
        }
        .await;
        mutation_snapshot(result)
    }

    pub fn project_network_wifi_only_preference_apply(
        &self,
        input: crate::relays::NetworkWifiOnlyPreferenceApplyInput,
    ) -> crate::relays::NetworkWifiOnlyPreferenceApplyProjection {
        crate::relays::network_wifi_only_preference_apply_projection(input)
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

    pub fn project_podcast_playback_session_apply(
        &self,
        input: podcast_playback::PodcastPlaybackSessionApplyInput,
    ) -> podcast_playback::PodcastPlaybackSessionApplyProjection {
        podcast_playback::session_apply_projection(input)
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

    pub fn project_podcast_playback_seek(
        &self,
        input: podcast_playback::PodcastPlaybackSeekInput,
    ) -> podcast_playback::PodcastPlaybackSeekProjection {
        podcast_playback::seek_projection(input)
    }

    pub fn project_podcast_playback_tick(
        &self,
        input: podcast_playback::PodcastPlaybackTickInput,
    ) -> podcast_playback::PodcastPlaybackTickProjection {
        podcast_playback::tick_projection(input)
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

    pub async fn load_podcast_transcript(
        &self,
        url: String,
    ) -> podcast_transcript::PodcastTranscriptLoadSnapshot {
        podcast_transcript::transcript_load_snapshot(
            podcast_transcript::fetch_transcript(&url).await,
        )
    }

    pub fn project_podcast_transcript_load_apply(
        &self,
        input: podcast_transcript::PodcastTranscriptLoadApplyInput,
    ) -> podcast_transcript::PodcastTranscriptLoadApplyProjection {
        podcast_transcript::transcript_load_apply_projection(input)
    }

    pub fn get_podcast_clip_composer_projection(
        &self,
        input: PodcastClipComposerInput,
    ) -> PodcastClipComposerProjection {
        podcast_transcript::clip_composer_projection(input)
    }

    pub fn get_podcast_listening_projection(
        &self,
        input: PodcastListeningProjectionInput,
    ) -> PodcastListeningProjection {
        podcast_transcript::listening_projection(input)
    }

    pub fn get_podcast_now_playing_projection(
        &self,
        input: podcast_transcript::PodcastNowPlayingProjectionInput,
    ) -> podcast_transcript::PodcastNowPlayingProjection {
        podcast_transcript::now_playing_projection(input)
    }

    pub fn project_waveform_cache_key(
        &self,
        input: crate::waveform::WaveformCacheKeyProjectionInput,
    ) -> crate::waveform::WaveformCacheKeyProjection {
        crate::waveform::cache_key_projection(input)
    }

    pub fn plan_waveform_peaks(
        &self,
        input: crate::waveform::WaveformPeaksPlanInput,
    ) -> crate::waveform::WaveformPeaksPlan {
        crate::waveform::peaks_plan(input)
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

    pub fn clear_podcast_clip_selection(&self) -> PodcastClipSelection {
        podcast_transcript::clear_clip_selection()
    }

    pub fn mark_podcast_clip_in(
        &self,
        selection: PodcastClipSelection,
        current_time: f64,
    ) -> PodcastClipSelection {
        podcast_transcript::mark_clip_in(&selection, current_time)
    }

    pub fn mark_podcast_clip_out(
        &self,
        selection: PodcastClipSelection,
        current_time: f64,
    ) -> PodcastClipSelection {
        podcast_transcript::mark_clip_out(&selection, current_time)
    }

    pub fn extend_podcast_clip_to_segment(
        &self,
        selection: PodcastClipSelection,
        segment: TranscriptSegment,
    ) -> PodcastClipSelection {
        podcast_transcript::extend_clip_to_segment(&selection, &segment)
    }

    pub fn set_podcast_clip_start(
        &self,
        selection: PodcastClipSelection,
        value: f64,
    ) -> PodcastClipSelection {
        podcast_transcript::set_clip_start(&selection, value)
    }

    pub fn set_podcast_clip_end(
        &self,
        selection: PodcastClipSelection,
        value: f64,
        duration_seconds: f64,
    ) -> PodcastClipSelection {
        podcast_transcript::set_clip_end(&selection, value, duration_seconds)
    }

    pub async fn download_podcast_artwork(&self, url: String) -> Option<Vec<u8>> {
        podcast_transcript::download_artwork(&url).await.ok()
    }

    pub fn share_extension_communities_snapshot(
        &self,
        communities: Vec<CommunitySummary>,
    ) -> Vec<u8> {
        crate::share_extension::communities_snapshot_json(communities)
    }

    pub fn project_share_queue_drain(
        &self,
        input: crate::share_extension::ShareQueueDrainProjectionInput,
    ) -> crate::share_extension::ShareQueueDrainProjection {
        crate::share_extension::share_queue_drain_projection(input)
    }

    // -- Auth (async) --
    // Async auth flows delegate without holding the parking_lot guard across
    // await. The session module is responsible for thread-safe internal state.

    pub async fn start_default_nostr_connect(
        &self,
        callback: String,
    ) -> nip46::NostrConnectStartSnapshot {
        let result = self
            .start_nostr_connect_with_options(NostrConnectOptions::default(), &callback)
            .await;
        nip46::start_snapshot(result)
    }

    pub async fn restore_session_snapshot(
        &self,
        nsec: Option<String>,
        bunker_uri: Option<String>,
    ) -> crate::session::AuthSessionRestoreSnapshot {
        let mut clear_nsec = false;
        let mut clear_bunker_uri = false;
        let mut last_error = None;

        if let Some(saved_nsec) = nsec
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            match self.login_nsec_result_async(saved_nsec).await {
                Ok(user) => {
                    return crate::session::auth_session_restore_snapshot(
                        Ok(Some(user)),
                        clear_nsec,
                        clear_bunker_uri,
                    );
                }
                Err(error) => {
                    clear_nsec = true;
                    last_error = Some(error);
                }
            }
        }

        if let Some(saved_bunker_uri) = bunker_uri
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            match self.pair_bunker_result(saved_bunker_uri).await {
                Ok(user) => {
                    return crate::session::auth_session_restore_snapshot(
                        Ok(Some(user)),
                        clear_nsec,
                        clear_bunker_uri,
                    );
                }
                Err(error) => {
                    clear_bunker_uri = true;
                    last_error = Some(error);
                }
            }
        }

        match last_error {
            Some(error) => crate::session::auth_session_restore_snapshot(
                Err(error),
                clear_nsec,
                clear_bunker_uri,
            ),
            None => crate::session::auth_session_restore_snapshot(
                Ok(None),
                clear_nsec,
                clear_bunker_uri,
            ),
        }
    }

    pub async fn pair_bunker(&self, uri: String) -> crate::session::AuthSessionSnapshot {
        let result = self.pair_bunker_result(&uri).await;
        crate::session::auth_session_snapshot_with_persistence(result, None, Some(uri))
    }

    // -- Subscriptions --

    pub fn set_event_callback(&self, callback: Arc<dyn EventCallback>) {
        *self.callback_slot.write() = Some(callback.clone());

        // One-shot app-scope seed: if a user is already logged in, broadcast
        // `SignerConnected` so any freshly-registered Swift store bootstraps
        // its `currentUser` without racing a Swift-side cache read.
        let seed_user = self.inner.read().session.current_user();
        if let Some(user) = seed_user {
            callback.on_data_changed(Delta {
                subscription_id: 0,
                change: DataChangeType::SignerConnected { user },
            });
        }
    }

    /// Consume a pending join when a matching NIP-29 membership delta arrives.
    /// Swift routes the delta; Rust owns whether it was pending and what toast
    /// should be shown.
    pub fn confirm_pending_join(&self, group_id: String) {
        if let Some(room_name) = self.remove_pending_join(&group_id) {
            self.emit_app_toast(format!("You're in {room_name} ✓"));
        }
    }

    pub fn project_view_subscription_start(
        &self,
        input: crate::models::ViewSubscriptionStartProjectionInput,
    ) -> crate::models::ViewSubscriptionStartProjection {
        crate::models::view_subscription_start_projection(input)
    }

    pub fn project_app_subscription_start(
        &self,
        input: crate::models::AppSubscriptionStartProjectionInput,
    ) -> crate::models::AppSubscriptionStartProjection {
        crate::models::app_subscription_start_projection(input)
    }

    /// App-scope subscription for the joined-communities view. Returns a
    /// handle; fires CommunityUpserted / MembershipChanged deltas tagged
    /// with that handle. Re-uses the relay sub installed at login; this
    /// call is about setting up the nostrdb notification pump.
    pub async fn subscribe_joined_communities(&self) -> SubscriptionStartSnapshot {
        subscription_start_snapshot((|| {
            let user_pubkey = self.require_user_pubkey()?;
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::JoinedCommunities { user_pubkey },
            )
        })())
    }

    /// Per-room view-scope subscription. Returns a handle; fires
    /// ArtifactUpserted / HighlightUpserted / HighlightShared for this
    /// specific group.
    pub async fn subscribe_room(&self, group_id: String) -> SubscriptionStartSnapshot {
        subscription_start_snapshot((|| {
            if group_id.trim().is_empty() {
                return Err(CoreError::InvalidInput("group_id must not be empty".into()));
            }
            self.subscriptions
                .register(&self.runtime, SubscriptionKind::Room { group_id })
        })())
    }

    /// Per-room Discussions view-scope subscription. Returns a handle; fires
    /// `DiscussionUpserted` deltas for kind:11 threads in this group that
    /// carry the `t=discussion` marker.
    pub async fn subscribe_room_discussions(&self, group_id: String) -> SubscriptionStartSnapshot {
        subscription_start_snapshot((|| {
            if group_id.trim().is_empty() {
                return Err(CoreError::InvalidInput("group_id must not be empty".into()));
            }
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::RoomDiscussions { group_id },
            )
        })())
    }

    /// Per-room Chat view-scope subscription. Returns a handle; fires
    /// `ChatMessageUpserted` deltas for kind:9 messages tagged
    /// `#h=<group_id>`.
    pub async fn subscribe_room_chat(&self, group_id: String) -> SubscriptionStartSnapshot {
        subscription_start_snapshot((|| {
            if group_id.trim().is_empty() {
                return Err(CoreError::InvalidInput("group_id must not be empty".into()));
            }
            self.subscriptions
                .register(&self.runtime, SubscriptionKind::RoomChat { group_id })
        })())
    }

    /// Vault view-scope subscription for the current user's own highlights.
    pub async fn subscribe_vault(&self) -> SubscriptionStartSnapshot {
        subscription_start_snapshot((|| {
            let user_pubkey = self.require_user_pubkey()?;
            self.subscriptions
                .register(&self.runtime, SubscriptionKind::Vault { user_pubkey })
        })())
    }

    /// Profile view-scope subscription. Fires `UserProfileUpdated` deltas
    /// when any event relevant to `pubkey_hex`'s profile arrives. Install on
    /// profile view appearance; `unsubscribe(handle)` on disappearance.
    pub async fn subscribe_user_profile(&self, pubkey_hex: String) -> SubscriptionStartSnapshot {
        subscription_start_snapshot((|| {
            let pubkey = PublicKey::from_hex(pubkey_hex.trim())
                .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;
            self.subscriptions
                .register(&self.runtime, SubscriptionKind::UserProfile { pubkey })
        })())
    }

    /// Following Reads view-scope subscription. Snapshots the user's current
    /// follow list, then listens for: (a) new articles authored by a follow,
    /// (b) interactions by a follow against any kind:30023 content. Fires
    /// `FollowingReadsUpdated` deltas; the Swift store re-queries the feed.
    /// Install on tab appearance; `unsubscribe(handle)` on disappearance.
    pub async fn subscribe_following_reads(&self) -> SubscriptionStartSnapshot {
        subscription_start_snapshot((|| {
            let user_pubkey = self.require_user_pubkey()?;
            let follow_hex_strings =
                follows::query_follows(self.runtime.ndb(), &user_pubkey.to_hex())?;
            let follows_pks: Vec<PublicKey> = follow_hex_strings
                .iter()
                .filter_map(|s| PublicKey::from_hex(s.trim()).ok())
                .collect();
            self.refresh_follows_nip65_subscription(&follows_pks);
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::FollowingReads {
                    follows: follows_pks,
                },
            )
        })())
    }

    /// Highlights home-feed view-scope subscription. Snapshots the user's
    /// current follow list (plus self — nobody lists themselves in kind:3
    /// but we want our own highlights in the home feed) and joined-group
    /// ids, then listens for kind:9802 events authored by anyone in that
    /// set or tagged into any joined room.
    pub async fn subscribe_following_highlights(&self) -> SubscriptionStartSnapshot {
        subscription_start_snapshot((|| {
            let user_pubkey = self.require_user_pubkey()?;
            let follow_hex_strings =
                follows::query_follows(self.runtime.ndb(), &user_pubkey.to_hex())?;
            let mut follows_pks: Vec<PublicKey> = follow_hex_strings
                .iter()
                .filter_map(|s| PublicKey::from_hex(s.trim()).ok())
                .collect();
            if !follows_pks.contains(&user_pubkey) {
                follows_pks.push(user_pubkey);
            }
            self.refresh_follows_nip65_subscription(&follows_pks);
            let joined = groups::query_joined_communities_from_ndb(
                self.runtime.ndb(),
                &user_pubkey.to_hex(),
            )?;
            let group_ids: Vec<String> = joined.into_iter().map(|c| c.id).collect();
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::FollowingHighlights {
                    follows: follows_pks,
                    group_ids,
                },
            )
        })())
    }

    /// Article-reader view-scope subscription. Fires `ArticleUpdated` deltas
    /// whenever the article's replaceable body supersedes OR a new kind:9802
    /// highlighting this article's `a`-tag arrives. Install on reader view
    /// appearance; `unsubscribe(handle)` on disappearance.
    pub async fn subscribe_article(
        &self,
        pubkey_hex: String,
        d_tag: String,
    ) -> SubscriptionStartSnapshot {
        subscription_start_snapshot((|| {
            let pubkey_hex = pubkey_hex.trim();
            let d_tag = d_tag.trim();
            if pubkey_hex.is_empty() || d_tag.is_empty() {
                return Err(CoreError::InvalidInput(
                    "pubkey_hex and d_tag must not be empty".into(),
                ));
            }
            let author = PublicKey::from_hex(pubkey_hex)
                .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;
            let address = format!("30023:{}:{}", pubkey_hex, d_tag);
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::Article {
                    author,
                    d_tag: d_tag.to_string(),
                    address,
                },
            )
        })())
    }

    /// Feedback-threads subscription for the shake-to-share surface. Fires
    /// `FeedbackThreadsUpdated` deltas whenever a kind:1 root authored by
    /// the current user (with the project `a` tag) or any kind:513 metadata
    /// for the same project arrives. Swift re-queries on each.
    pub async fn subscribe_feedback_threads(
        &self,
        coordinate: String,
    ) -> SubscriptionStartSnapshot {
        subscription_start_snapshot((|| {
            let coordinate = coordinate.trim();
            if coordinate.is_empty() {
                return Err(CoreError::InvalidInput(
                    "coordinate must not be empty".into(),
                ));
            }
            let user_pubkey = self.require_user_pubkey()?;
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::FeedbackThreads {
                    coordinate: coordinate.to_string(),
                    current_user_pubkey: user_pubkey,
                },
            )
        })())
    }

    /// Per-thread feedback subscription. Fires `FeedbackThreadUpdated` deltas
    /// for every kind:1 `e`-tagged to the root (regardless of author).
    pub async fn subscribe_feedback_thread(
        &self,
        root_event_id: String,
    ) -> SubscriptionStartSnapshot {
        subscription_start_snapshot((|| {
            let root_event_id = root_event_id.trim();
            if root_event_id.is_empty() {
                return Err(CoreError::InvalidInput(
                    "root_event_id must not be empty".into(),
                ));
            }
            let root = EventId::from_hex(root_event_id)
                .map_err(|e| CoreError::InvalidInput(format!("invalid event id: {e}")))?;
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::FeedbackThread {
                    root_event_id: root,
                },
            )
        })())
    }

    /// Drop a subscription by handle. Idempotent.
    pub fn unsubscribe(&self, handle: u64) {
        self.subscriptions.remove(&self.runtime, handle);
    }

    // -- Reads --

    pub async fn get_joined_communities(&self) -> groups::JoinedCommunitiesSnapshot {
        groups::joined_communities_snapshot((|| {
            let Some(user) = self.inner.read().session.current_user() else {
                return Err(CoreError::NotAuthenticated);
            };
            groups::query_joined_communities_from_ndb(self.runtime.ndb(), &user.pubkey)
        })())
    }

    pub fn project_joined_communities_snapshot_apply(
        &self,
        input: groups::JoinedCommunitiesSnapshotApplyInput,
    ) -> groups::JoinedCommunitiesSnapshotApplyProjection {
        groups::joined_communities_snapshot_apply_projection(input)
    }

    pub async fn get_relay_hosted_rooms_snapshot(
        &self,
        url: String,
    ) -> crate::relays::RelayHostedRoomsSnapshot {
        let result = (|| {
            let Some(user) = self.inner.read().session.current_user() else {
                return Err(CoreError::NotAuthenticated);
            };
            groups::query_joined_room_names_for_relay_from_ndb(
                self.runtime.ndb(),
                &user.pubkey,
                &url,
            )
        })();
        crate::relays::relay_hosted_rooms_snapshot(result)
    }

    pub fn project_relay_hosted_rooms_apply(
        &self,
        input: crate::relays::RelayHostedRoomsApplyInput,
    ) -> crate::relays::RelayHostedRoomsApplyProjection {
        crate::relays::relay_hosted_rooms_apply_projection(input)
    }

    /// Classify a highlight source for native icon/label rendering. Rust owns
    /// the source/reference interpretation; native shells only render the enum.
    pub fn get_highlight_source_kind(
        &self,
        preview_source: String,
        external_reference: String,
        artifact_address: String,
        source_url: String,
    ) -> HighlightSourceKind {
        highlights::source_kind(
            &preview_source,
            &external_reference,
            &artifact_address,
            &source_url,
        )
    }

    pub fn project_reading_feed_card(
        &self,
        input: reads::ReadingFeedCardProjectionInput,
    ) -> reads::ReadingFeedCardProjection {
        reads::reading_feed_card_projection(input)
    }

    pub fn project_highlight_group_card(
        &self,
        input: highlights::HighlightGroupCardProjectionInput,
    ) -> highlights::HighlightGroupCardProjection {
        highlights::highlight_group_card_projection(input)
    }

    pub fn project_highlight_resource_header(
        &self,
        input: highlights::HighlightResourceHeaderProjectionInput,
    ) -> highlights::HighlightResourceHeaderProjection {
        highlights::highlight_resource_header_projection(input)
    }

    pub fn project_highlight_detail_resource(
        &self,
        input: highlights::HighlightDetailResourceProjectionInput,
    ) -> highlights::HighlightDetailResourceProjection {
        highlights::highlight_detail_resource_projection(input)
    }

    pub fn project_highlight_feed_content(
        &self,
        input: highlights::HighlightFeedContentProjectionInput,
    ) -> highlights::HighlightFeedContentProjection {
        highlights::highlight_feed_content_projection(input)
    }

    pub fn project_highlight_detail_content(
        &self,
        input: highlights::HighlightDetailContentProjectionInput,
    ) -> highlights::HighlightDetailContentProjection {
        highlights::highlight_detail_content_projection(input)
    }

    /// Project selected article-reader text. Native shells own text-range
    /// extraction; Rust owns quote/context normalization.
    pub fn project_article_reader_selection(
        &self,
        input: highlights::ArticleReaderSelectionProjectionInput,
    ) -> highlights::ArticleReaderSelectionProjection {
        highlights::article_reader_selection_projection(input)
    }

    /// Project article-reader highlight publish state. Rust owns note
    /// normalization and success/failure toast semantics.
    pub fn project_article_highlight_publish(
        &self,
        input: highlights::ArticleHighlightPublishProjectionInput,
    ) -> highlights::ArticleHighlightPublishProjection {
        highlights::article_highlight_publish_projection(input)
    }

    // -- Profile reads (per-pubkey, no auth required) --

    pub async fn get_user_profile(&self, pubkey_hex: String) -> Option<ProfileMetadata> {
        profile::query_profile_from_ndb(self.runtime.ndb(), pubkey_hex.trim())
            .ok()
            .flatten()
    }

    /// Profile/avatar presentation projection. Rust owns profile-name
    /// precedence, pubkey fallback, and avatar URL selection; native shells
    /// render the resulting values without reimplementing business rules.
    pub fn project_profile_display(
        &self,
        input: profile::ProfileDisplayProjectionInput,
    ) -> profile::ProfileDisplayProjection {
        profile::profile_display_projection(input)
    }

    /// Profile/avatar presentation projection for bylines that include an
    /// artifact-provided author label.
    pub fn project_profile_display_with_label(
        &self,
        input: profile::ProfileDisplayWithLabelProjectionInput,
    ) -> profile::ProfileDisplayProjection {
        profile::profile_display_with_label_projection(input)
    }

    /// Compact profile handle projection for social proof surfaces. Rust owns
    /// handle precedence and pubkey fallback length; native shells render it.
    pub fn project_profile_handle(
        &self,
        input: profile::ProfileDisplayProjectionInput,
    ) -> profile::ProfileDisplayProjection {
        profile::profile_handle_projection(input)
    }

    /// Profile header identity projection. Rust owns display fallbacks and
    /// NIP-05 label normalization; native shells render the returned fields.
    pub fn project_profile_identity(
        &self,
        input: profile::ProfileIdentityProjectionInput,
    ) -> profile::ProfileIdentityProjection {
        profile::profile_identity_projection(input)
    }

    /// Profile relationship projection. Rust owns own-profile detection and
    /// follow-action visibility; native shells render and execute taps only.
    pub fn project_profile_relationship(
        &self,
        input: profile::ProfileRelationshipProjectionInput,
    ) -> profile::ProfileRelationshipProjection {
        profile::profile_relationship_projection(input)
    }

    /// Profile follow-tap projection. Rust owns whether a tap may start,
    /// the optimistic button state, and the exact mutation the shell executes.
    pub fn project_profile_follow_action(
        &self,
        relationship: profile::ProfileRelationshipProjection,
        input: profile::ProfileFollowActionInput,
    ) -> profile::ProfileFollowActionProjection {
        profile::profile_follow_action_projection(relationship, input)
    }

    /// Profile edit-form projection. Rust owns draft normalization and save
    /// eligibility; native shells bind controls to the returned projection.
    pub fn project_profile_update(
        &self,
        input: profile::ProfileUpdateProjectionInput,
    ) -> profile::ProfileUpdateProjection {
        profile::profile_update_projection(input)
    }

    pub fn project_profile_image_upload_result(
        &self,
        input: profile::ProfileImageUploadResultInput,
    ) -> profile::ProfileImageUploadResultProjection {
        profile::profile_image_upload_result_projection(input)
    }

    pub fn project_profile_update_result(
        &self,
        input: profile::ProfileUpdateResultInput,
    ) -> profile::ProfileUpdateResultProjection {
        profile::profile_update_result_projection(input)
    }

    /// Publish a new kind:0 metadata event for the current user. Preserves
    /// any unknown JSON fields the user had set via other clients —
    /// only the canonical fields the edit form drives get overwritten.
    /// Empty strings clear the corresponding field. Returns the parsed
    /// metadata so the caller's UI can swap to the new state without
    /// waiting for the relay echo.
    pub async fn update_profile(
        &self,
        draft: ProfileUpdateDraft,
    ) -> profile::ProfileUpdateSnapshot {
        let result: Result<ProfileMetadata, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            profile::publish_profile(&self.runtime, &draft).await
        }
        .await;
        profile::profile_update_snapshot(result)
    }

    /// Full article-reader read model. Rust owns article/profile/highlight
    /// cache reads, the highlight limit, and partial-failure fallback.
    pub async fn get_article_reader_snapshot(
        &self,
        pubkey_hex: String,
        d_tag: String,
    ) -> article_reader::ArticleReaderSnapshot {
        article_reader::query_article_reader_snapshot(
            self.runtime.ndb(),
            pubkey_hex.trim(),
            d_tag.trim(),
        )
    }

    pub fn project_article_reader_snapshot(
        &self,
        input: article_reader::ArticleReaderSnapshotApplyInput,
    ) -> article_reader::ArticleReaderSnapshotProjection {
        article_reader::article_reader_snapshot_projection(input)
    }

    pub fn project_article_reader_publish_result(
        &self,
        input: article_reader::ArticleReaderPublishResultInput,
    ) -> article_reader::ArticleReaderPublishResultProjection {
        article_reader::article_reader_publish_result_projection(input)
    }

    /// Read a single NIP-23 article by its full NIP-33 address
    /// (`30023:<pubkey>:<d>`) from nostrdb.
    pub async fn get_article_by_address(&self, address: String) -> Option<ArticleRecord> {
        articles::query_article_by_address(self.runtime.ndb(), address.trim())
            .ok()
            .flatten()
    }

    /// Return the author pubkey from a valid NIP-23 article address
    /// (`30023:<pubkey>:<d>`).
    pub async fn get_article_address_author(&self, address: String) -> Option<String> {
        articles::article_author_from_address(address.trim())
    }

    pub fn project_article_reader_header(
        &self,
        input: articles::ArticleReaderHeaderProjectionInput,
    ) -> articles::ArticleReaderHeaderProjection {
        articles::article_reader_header_projection(input)
    }

    pub fn project_article_profile_card(
        &self,
        input: articles::ArticleProfileCardProjectionInput,
    ) -> articles::ArticleProfileCardProjection {
        articles::article_profile_card_projection(input)
    }

    pub fn project_share_article_target(
        &self,
        input: share_targets::ShareArticleTargetProjectionInput,
    ) -> share_targets::ShareArtifactTargetProjection {
        share_targets::article_target_projection(input)
    }

    pub fn article_share_url(&self, address: String) -> share_links::ArticleShareUrlSnapshot {
        share_links::article_share_url_snapshot(address)
    }

    pub fn project_share_artifact_target(
        &self,
        input: share_targets::ShareArtifactTargetProjectionInput,
    ) -> share_targets::ShareArtifactTargetProjection {
        share_targets::artifact_target_projection(input)
    }

    pub fn project_share_web_reader_target(
        &self,
        input: share_targets::ShareWebReaderTargetProjectionInput,
    ) -> share_targets::ShareArtifactTargetProjection {
        share_targets::web_reader_target_projection(input)
    }

    pub async fn build_web_reader_share_target(
        &self,
        url: String,
    ) -> share_targets::ShareWebReaderTargetSnapshot {
        share_targets::web_reader_target_snapshot(crate::artifacts::build_preview(&url), &url)
    }

    pub fn project_share_highlight_target(
        &self,
        input: share_targets::ShareHighlightTargetProjectionInput,
    ) -> share_targets::ShareHighlightTargetProjection {
        share_targets::highlight_target_projection(input)
    }

    pub fn project_share_highlight_article_target(
        &self,
        input: share_targets::ShareHighlightArticleTargetProjectionInput,
    ) -> Option<share_targets::ShareArtifactTargetProjection> {
        share_targets::highlight_article_target_projection(input)
    }

    pub fn project_share_to_community_publish_result(
        &self,
        input: share_targets::ShareToCommunityPublishResultInput,
    ) -> share_targets::ShareToCommunityPublishResultProjection {
        share_targets::share_to_community_publish_result_projection(input)
    }

    pub fn project_community_row(
        &self,
        input: groups::CommunityRowProjectionInput,
    ) -> groups::CommunityRowProjection {
        groups::community_row_projection(input)
    }

    /// Resolve a book catalog id into the canonical ISBN route used by native
    /// book screens. Accepts raw ISBNs and `isbn:<digits>` values.
    pub fn get_book_route(&self, catalog_id: String) -> Option<BookRoute> {
        highlights::book_route_for_catalog(catalog_id.trim())
    }

    /// Resolve a highlight's book reference from its external reference or
    /// artifact address. Rust owns the precedence and canonical catalog id.
    pub fn get_highlight_book_route(
        &self,
        external_reference: String,
        artifact_address: String,
    ) -> Option<BookRoute> {
        highlights::book_route_for_highlight(external_reference.trim(), artifact_address.trim())
    }

    /// Screen-shaped snapshot for the native book detail route. Rust owns
    /// catalog-id canonicalization, ISBN route state, and passage lookup.
    pub async fn get_book_detail_snapshot(
        &self,
        catalog_id: String,
        limit: u32,
    ) -> crate::book_detail::BookDetailSnapshot {
        let Some(route) = highlights::book_route_for_catalog(catalog_id.trim()) else {
            return crate::book_detail::BookDetailSnapshot::empty();
        };
        match highlights::query_for_book_catalog(self.runtime.ndb(), &route.catalog_id, limit) {
            Ok(highlights) => crate::book_detail::snapshot(route, highlights),
            Err(error) => crate::book_detail::error_snapshot(Some(route), error),
        }
    }

    pub fn project_book_detail_snapshot_apply(
        &self,
        input: crate::book_detail::BookDetailSnapshotApplyInput,
    ) -> crate::book_detail::BookDetailSnapshotApplyProjection {
        crate::book_detail::snapshot_apply_projection(input)
    }

    /// Classify a subscription event kind into the exact profile slice that
    /// native shells should refresh.
    pub fn get_profile_update_action(&self, kind: u32) -> ProfileUpdateAction {
        crate::events::profile_update_action(kind)
    }

    /// Project a NIP-23 article address into the NIP-22 root scope used by
    /// comment reads/writes.
    pub fn get_article_comment_scope(&self, address: String) -> comments::CommentScopeSnapshot {
        comments::scope_snapshot(comments::article_scope(&address))
    }

    /// Project a NIP-84 highlight event id into the NIP-22 root scope used by
    /// comment reads/writes.
    pub fn get_highlight_comment_scope(
        &self,
        event_id_hex: String,
    ) -> comments::CommentScopeSnapshot {
        comments::scope_snapshot(comments::highlight_scope(&event_id_hex))
    }

    /// Project a kind:11 discussion event id into the NIP-22 root scope used
    /// by comment reads/writes.
    pub fn get_discussion_comment_scope(
        &self,
        event_id_hex: String,
    ) -> comments::CommentScopeSnapshot {
        comments::scope_snapshot(comments::discussion_scope(&event_id_hex))
    }

    /// Project a web URL into the external NIP-22 root scope used by comment
    /// reads/writes.
    pub fn get_web_comment_scope(&self, url: String) -> comments::CommentScopeSnapshot {
        comments::scope_snapshot(comments::web_scope(&url))
    }

    /// Project an artifact preview into a NIP-22 root scope using the
    /// preview's Rust-owned protocol reference fields.
    pub fn get_artifact_comment_scope(
        &self,
        preview: ArtifactPreview,
    ) -> comments::CommentScopeSnapshot {
        comments::scope_snapshot(comments::scope_from_preview(&preview))
    }

    pub fn project_discussion_attachment(
        &self,
        input: crate::discussions::DiscussionAttachmentProjectionInput,
    ) -> crate::discussions::DiscussionAttachmentProjection {
        crate::discussions::attachment_projection(input)
    }

    /// Discussion composer projection. Rust owns draft normalization and
    /// publish eligibility; native shells render the composer affordance.
    pub fn project_discussion_composer(
        &self,
        input: crate::discussions::DiscussionComposerProjectionInput,
    ) -> crate::discussions::DiscussionComposerProjection {
        crate::discussions::composer_projection(input)
    }

    pub fn project_discussion_publish_result(
        &self,
        input: crate::discussions::DiscussionPublishResultInput,
    ) -> crate::discussions::DiscussionPublishResultProjection {
        crate::discussions::publish_result_projection(input)
    }

    /// Comment composer projection. Rust owns draft normalization and submit
    /// eligibility; native shells render the composer affordance.
    pub fn project_comment_composer(
        &self,
        input: comments::CommentComposerProjectionInput,
    ) -> comments::CommentComposerProjection {
        comments::comment_composer_projection(input)
    }

    pub fn project_comment_publish_result(
        &self,
        input: comments::CommentPublishResultInput,
    ) -> comments::CommentPublishResultProjection {
        comments::comment_publish_result_projection(input)
    }

    pub fn project_comment_snapshot_apply(
        &self,
        input: comments::CommentSnapshotApplyInput,
    ) -> comments::CommentSnapshotApplyProjection {
        comments::comment_snapshot_apply_projection(input)
    }

    pub fn project_comment_inline_thread_snapshot_apply(
        &self,
        input: comments::CommentInlineThreadSnapshotApplyInput,
    ) -> comments::CommentInlineThreadSnapshotApplyProjection {
        comments::comment_inline_thread_snapshot_apply_projection(input)
    }

    /// Project a comment thread screen. Rust owns focused-node lookup,
    /// visible child selection, and thread chrome labels.
    pub fn project_comment_thread_view(
        &self,
        input: comments::CommentThreadViewProjectionInput,
    ) -> comments::CommentThreadViewProjection {
        comments::comment_thread_view_projection(input)
    }

    /// Project per-comment reply chrome. Rust owns child counts, preview
    /// choice, "more replies" copy, and author-reply matching.
    pub fn project_comment_node_chrome(
        &self,
        input: comments::CommentNodeChromeProjectionInput,
    ) -> comments::CommentNodeChromeProjection {
        comments::comment_node_chrome_projection(input)
    }

    /// Project the comments toolbar badge. Rust owns count formatting and
    /// accessibility copy.
    pub fn project_comment_toolbar(
        &self,
        input: comments::CommentToolbarProjectionInput,
    ) -> comments::CommentToolbarProjection {
        comments::comment_toolbar_projection(input)
    }

    /// Project comment row reaction/bookmark chrome.
    pub fn project_comment_action_chrome(
        &self,
        input: comments::CommentActionChromeProjectionInput,
    ) -> comments::CommentActionChromeProjection {
        comments::comment_action_chrome_projection(input)
    }

    /// Full comments sheet snapshot for a Rust-owned NIP-22 scope. Rust owns
    /// record query, tree build, reaction summary, and bookmark membership.
    pub async fn get_comment_thread_snapshot(
        &self,
        scope: CommentScope,
        limit: u32,
    ) -> comments::CommentThreadSnapshot {
        comments::comment_thread_snapshot(
            self.runtime.ndb(),
            &scope,
            limit,
            self.current_user_pubkey_hex().as_deref(),
        )
    }

    /// Publish a NIP-22 comment and return the refreshed comments sheet
    /// snapshot. Rust owns optimistic insertion and tree/interaction rebuild.
    pub async fn publish_comment_for_scope_snapshot(
        &self,
        scope: CommentScope,
        parent_event_id: Option<String>,
        content: String,
        limit: u32,
    ) -> comments::CommentPublishSnapshot {
        let current_user = self.current_user_pubkey_hex();
        let base_snapshot = comments::comment_thread_snapshot(
            self.runtime.ndb(),
            &scope,
            limit,
            current_user.as_deref(),
        );
        if let Err(error) = self.require_user_pubkey() {
            return comments::CommentPublishSnapshot {
                snapshot: base_snapshot,
                error: error.to_string(),
            };
        }
        let result: Result<CommentRecord, CoreError> = async {
            let parent = parent_event_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty());
            comments::publish_comment_for_scope(&self.runtime, &scope, parent, content.trim()).await
        }
        .await;
        match result {
            Ok(record) => comments::CommentPublishSnapshot {
                snapshot: comments::comment_thread_snapshot_with_comment(
                    self.runtime.ndb(),
                    base_snapshot,
                    &record,
                    &scope.root_tag_value,
                    current_user.as_deref(),
                ),
                error: String::new(),
            },
            Err(error) => comments::CommentPublishSnapshot {
                snapshot: base_snapshot,
                error: error.to_string(),
            },
        }
    }

    // -- Reactions (NIP-25 kind:7) ---------------------------------------

    /// Toggle the current user's like on a visible NIP-22 comment and return
    /// the updated interaction snapshot for the current screen records.
    pub async fn toggle_comment_like_snapshot(
        &self,
        records: Vec<CommentRecord>,
        event_id: String,
        author_pubkey_hex: String,
    ) -> comments::CommentInteractionMutationSnapshot {
        let current_user = self.current_user_pubkey_hex();
        let base = comments::comment_interaction_snapshot(
            self.runtime.ndb(),
            &records,
            current_user.as_deref(),
        );
        match self
            .toggle_comment_like_state(&event_id, &author_pubkey_hex)
            .await
        {
            Ok(is_liked) => comments::CommentInteractionMutationSnapshot {
                interactions: comments::comment_interaction_snapshot_with_like_state(
                    base, &event_id, is_liked,
                ),
                error: String::new(),
            },
            Err(error) => comments::CommentInteractionMutationSnapshot {
                interactions: base,
                error: error.to_string(),
            },
        }
    }

    /// Toggle the current user's bookmark on a visible NIP-22 comment and
    /// return the updated interaction snapshot for the current screen records.
    pub async fn toggle_comment_bookmark_snapshot(
        &self,
        records: Vec<CommentRecord>,
        event_id_hex: String,
    ) -> comments::CommentInteractionMutationSnapshot {
        let current_user = self.current_user_pubkey_hex();
        let base = comments::comment_interaction_snapshot(
            self.runtime.ndb(),
            &records,
            current_user.as_deref(),
        );
        match self.toggle_event_bookmark_state(&event_id_hex).await {
            Ok(is_bookmarked) => comments::CommentInteractionMutationSnapshot {
                interactions: comments::comment_interaction_snapshot_with_bookmark_state(
                    base,
                    &event_id_hex,
                    is_bookmarked,
                ),
                error: String::new(),
            },
            Err(error) => comments::CommentInteractionMutationSnapshot {
                interactions: base,
                error: error.to_string(),
            },
        }
    }

    /// Publish the Rust-projected profile follow mutation and return the
    /// post-action screen state. Rust owns rollback on error; the shell only
    /// applies the returned snapshot.
    pub async fn apply_profile_follow_mutation(
        &self,
        input: profile::ProfileFollowMutationInput,
    ) -> profile::ProfileFollowMutationSnapshot {
        let target_pubkey = input.target_pubkey.clone();
        let requested_follow_state = input.requested_follow_state;
        let result: Result<(), CoreError> = async {
            let follower = {
                let guard = self.inner.read();
                guard
                    .session
                    .current_user()
                    .ok_or(CoreError::NotAuthenticated)?
                    .pubkey
            };
            follows::publish_follow_toggle(
                &self.runtime,
                &follower,
                target_pubkey.trim(),
                requested_follow_state,
            )
            .await?;
            Ok(())
        }
        .await;
        profile::profile_follow_mutation_snapshot(input, result)
    }

    pub fn project_profile_follow_mutation_apply(
        &self,
        input: profile::ProfileFollowMutationApplyInput,
    ) -> profile::ProfileFollowMutationApplyProjection {
        profile::profile_follow_mutation_apply_projection(input)
    }

    /// Screen-shaped snapshot for the capture book picker. Rust owns recent
    /// book lookup, local artifact search, query normalization, and error
    /// semantics; native shells render rows and transient loading affordances.
    pub async fn get_book_picker_snapshot(
        &self,
        query: String,
        recent_limit: u32,
        search_limit: u32,
    ) -> isbn_lookup::BookPickerSnapshot {
        let projection = isbn_lookup::book_picker_query_projection(
            isbn_lookup::BookPickerQueryProjectionInput { query },
        );
        let user_hex = self.current_user_pubkey_hex().unwrap_or_default();
        let mut error = String::new();
        let recents = match crate::recent_books::query_recent_books(
            self.runtime.ndb(),
            &user_hex,
            recent_limit,
        ) {
            Ok(records) => records,
            Err(err) => {
                error = err.to_string();
                Vec::new()
            }
        };
        let search_results = if projection.has_query && search_limit > 0 {
            match crate::artifacts::search_cached(
                self.runtime.ndb(),
                &projection.search_query,
                search_limit,
            ) {
                Ok(records) => records,
                Err(err) => {
                    if !error.is_empty() {
                        error.push('\n');
                    }
                    error.push_str(&err.to_string());
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };
        isbn_lookup::book_picker_snapshot(projection, recents, search_results, error)
    }

    // -- Search: across local nostrdb (all four surfaces) + NIP-50 relay ---

    /// Project native search field state. Rust owns query trimming and whether
    /// a search should run.
    pub fn project_search_query(
        &self,
        input: crate::search::SearchQueryProjectionInput,
    ) -> crate::search::SearchQueryProjection {
        crate::search::search_query_projection(input)
    }

    pub fn project_search_schedule(
        &self,
        input: crate::search::SearchScheduleInput,
    ) -> crate::search::SearchScheduleProjection {
        crate::search::search_schedule_projection(input)
    }

    pub fn project_search_results_apply(
        &self,
        input: crate::search::SearchResultsApplyInput,
    ) -> crate::search::SearchResultsApplyProjection {
        crate::search::search_results_apply_projection(input)
    }

    pub fn project_search_relay_refresh(
        &self,
        input: crate::search::SearchRelayRefreshInput,
    ) -> crate::search::SearchRelayRefreshProjection {
        crate::search::search_relay_refresh_projection(input)
    }

    pub fn project_search_relay_start_result(
        &self,
        input: crate::search::SearchRelayStartResultInput,
    ) -> crate::search::SearchRelayStartResultProjection {
        crate::search::search_relay_start_result_projection(input)
    }

    pub fn project_search_relay_update(
        &self,
        input: crate::search::SearchRelayUpdateInput,
    ) -> crate::search::SearchRelayUpdateProjection {
        crate::search::search_relay_update_projection(input)
    }

    pub fn project_search_relay_articles_apply(
        &self,
        input: crate::search::SearchRelayArticlesApplyInput,
    ) -> crate::search::SearchRelayArticlesApplyProjection {
        crate::search::search_relay_articles_apply_projection(input)
    }

    /// Project suggested search chips. Rust owns room/fallback ordering,
    /// trimming, dedupe, and cap policy.
    pub fn project_search_suggestions(
        &self,
        input: crate::search::SearchSuggestionsProjectionInput,
    ) -> crate::search::SearchSuggestionsProjection {
        crate::search::search_suggestions_projection(input)
    }

    /// Project a search highlight row. Rust owns navigation route and
    /// page-image URL eligibility.
    pub fn project_search_highlight_row(
        &self,
        input: crate::search::SearchHighlightRowProjectionInput,
    ) -> crate::search::SearchHighlightRowProjection {
        crate::search::search_highlight_row_projection(input)
    }

    /// Project a community search row. Rust owns display copy and optional
    /// metadata labels; native shells render the row layout.
    pub fn project_search_community_row(
        &self,
        input: crate::search::SearchCommunityRowProjectionInput,
    ) -> crate::search::SearchCommunityRowProjection {
        crate::search::search_community_row_projection(input)
    }

    /// Project matched text spans for search result rendering. Rust owns query
    /// trimming and case-insensitive matching; native shells apply styling.
    pub fn project_search_text_matches(
        &self,
        input: crate::search::SearchTextMatchesProjectionInput,
    ) -> crate::search::SearchTextMatchesProjection {
        crate::search::search_text_matches_projection(input)
    }

    /// Local search snapshot for the main search screen. Rust owns section
    /// limits and per-section cache-error fallback; native shells render the
    /// returned buckets.
    pub async fn get_search_results_snapshot(
        &self,
        query: String,
    ) -> crate::search::SearchResultsSnapshot {
        crate::search::search_results_snapshot(self.runtime.ndb(), &query)
    }

    /// Article-only refresh for relay search deltas. Used after NIP-50 events
    /// ingest into nostrdb so the native shell can repaint the Articles bucket
    /// without re-running unrelated sections.
    pub async fn get_search_article_results_snapshot(
        &self,
        query: String,
    ) -> crate::search::SearchArticleResultsSnapshot {
        crate::search::search_article_results_snapshot(self.runtime.ndb(), &query)
    }

    /// Open a NIP-50 relay subscription for kind:30023 against the user's
    /// search relays. Returns a handle; the pump fires
    /// `SearchArticlesUpdated { query }` deltas as matching events ingest,
    /// and the Swift store responds by re-reading Rust's article search
    /// snapshot to merge the new events into its Articles bucket.
    pub async fn subscribe_article_search(&self, query: String) -> SubscriptionStartSnapshot {
        let result: Result<u64, CoreError> = async {
            let trimmed = query.trim().to_string();
            if trimmed.is_empty() {
                return Err(CoreError::InvalidInput(
                    "search query must not be empty".into(),
                ));
            }
            let user_hex = self
                .inner
                .read()
                .session
                .current_user()
                .map(|u| u.pubkey)
                .unwrap_or_default();
            let relays = crate::search::query_search_relays(self.runtime.ndb(), &user_hex)?;
            if relays.is_empty() {
                return Err(CoreError::InvalidInput("no search relays resolved".into()));
            }
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::SearchArticles {
                    query: trimmed,
                    relays,
                },
            )
        }
        .await;
        subscription_start_snapshot(result)
    }

    // -- Bookmarks (NIP-51 kind:10003) -----------------------------------

    /// Project native article bookmark state. Rust owns address trimming,
    /// membership, and the optimistic post-toggle set.
    pub fn project_article_bookmark_state(
        &self,
        input: crate::bookmarks::ArticleBookmarkStateProjectionInput,
    ) -> crate::bookmarks::ArticleBookmarkStateProjection {
        crate::bookmarks::article_bookmark_state_projection(input)
    }

    /// Project article bookmark affordance copy and SF Symbols.
    pub fn project_article_bookmark_chrome(
        &self,
        input: crate::bookmarks::ArticleBookmarkChromeProjectionInput,
    ) -> crate::bookmarks::ArticleBookmarkChromeProjection {
        crate::bookmarks::article_bookmark_chrome_projection(input)
    }

    /// Return the current article bookmark snapshot. Rust owns the nostrdb
    /// query and error semantics; native shells render and cache the returned
    /// address set.
    pub async fn get_article_bookmarks_snapshot(
        &self,
    ) -> crate::bookmarks::ArticleBookmarksSnapshot {
        let user_hex = self
            .inner
            .read()
            .session
            .current_user()
            .map(|u| u.pubkey)
            .unwrap_or_default();
        match crate::bookmarks::query_bookmarks(self.runtime.ndb(), &user_hex) {
            Ok(list) => crate::bookmarks::article_bookmarks_snapshot(list.addresses, ""),
            Err(error) => crate::bookmarks::article_bookmarks_snapshot(Vec::new(), error),
        }
    }

    /// Toggle `address` in the user's kind:10003 list and return the
    /// post-toggle article bookmark snapshot. Rust owns the read-modify-write;
    /// native shells do not inspect a bool mutation outcome.
    pub async fn toggle_article_bookmark_snapshot(
        &self,
        address: String,
    ) -> crate::bookmarks::ArticleBookmarksSnapshot {
        let result: Result<Vec<String>, CoreError> = async {
            let user_hex = self
                .inner
                .read()
                .session
                .current_user()
                .map(|u| u.pubkey)
                .ok_or(CoreError::NotInitialized)?;
            crate::bookmarks::toggle_article_bookmark_addresses(&self.runtime, &user_hex, &address)
                .await
        }
        .await;
        match result {
            Ok(addresses) => crate::bookmarks::article_bookmarks_snapshot(addresses, ""),
            Err(error) => crate::bookmarks::article_bookmarks_snapshot(Vec::new(), error),
        }
    }

    pub fn project_article_bookmarks_snapshot_apply(
        &self,
        input: crate::bookmarks::ArticleBookmarksSnapshotApplyInput,
    ) -> crate::bookmarks::ArticleBookmarksSnapshotApplyProjection {
        crate::bookmarks::article_bookmarks_snapshot_apply_projection(input)
    }

    /// Open a live subscription on the current user's kind:10003 bookmark
    /// events. Deltas land on the app-scope bus (`BookmarksUpdated`); the
    /// Swift bookmarks store re-queries on each.
    pub async fn subscribe_bookmarks(&self) -> SubscriptionStartSnapshot {
        subscription_start_snapshot((|| {
            let user_hex = self
                .inner
                .read()
                .session
                .current_user()
                .map(|u| u.pubkey)
                .ok_or(CoreError::NotInitialized)?;
            let pk = PublicKey::from_hex(&user_hex)
                .map_err(|e| CoreError::InvalidInput(format!("invalid user pubkey: {e}")))?;
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::Bookmarks { user_pubkey: pk },
            )
        })())
    }

    // -- NIP-51 Bookmark sets (kind:30003) / Curation sets (kind:30004) -----

    /// Full bookmark library read model for the current user. Rust owns
    /// bookmark address resolution, set/web/explore section reads, and
    /// per-section cache failure fallback.
    pub async fn get_bookmark_library_snapshot(&self) -> crate::lists::BookmarkLibrarySnapshot {
        let user_hex = self
            .inner
            .read()
            .session
            .current_user()
            .map(|u| u.pubkey)
            .unwrap_or_default();
        crate::lists::query_bookmark_library_snapshot(self.runtime.ndb(), &user_hex)
    }

    /// Screen-shaped read model for bookmark/curation set detail. Rust owns
    /// title fallback, article row resolution, and empty-state policy.
    pub async fn get_bookmark_set_detail_snapshot(
        &self,
        record: BookmarkSetRecord,
    ) -> crate::lists::BookmarkSetDetailSnapshot {
        crate::lists::query_bookmark_set_detail_snapshot(self.runtime.ndb(), record)
    }

    /// Screen-shaped snapshot for the bookmark menu's collection picker. Rust
    /// owns current-user lookup, set ordering, title fallback, and membership.
    pub async fn get_curation_menu_snapshot(
        &self,
        address: String,
    ) -> crate::lists::CurationMenuSnapshot {
        let result = {
            let user_hex = self.current_user_pubkey_hex().unwrap_or_default();
            self.curation_menu_snapshot_for_user(&user_hex, &address)
        };
        match result {
            Ok(snapshot) => snapshot,
            Err(error) => crate::lists::curation_menu_error_snapshot(error),
        }
    }

    pub fn project_curation_menu_snapshot_apply(
        &self,
        input: crate::lists::CurationMenuSnapshotApplyInput,
    ) -> crate::lists::CurationMenuSnapshotApplyProjection {
        crate::lists::curation_menu_snapshot_apply_projection(input)
    }

    pub fn project_bookmarked_article_row(
        &self,
        input: crate::lists::BookmarkedArticleRowProjectionInput,
    ) -> crate::lists::BookmarkedArticleRowProjection {
        crate::lists::bookmarked_article_row_projection(input)
    }

    pub fn project_bookmark_library(
        &self,
        input: crate::lists::BookmarkLibraryProjectionInput,
    ) -> crate::lists::BookmarkLibraryProjection {
        crate::lists::bookmark_library_projection(input)
    }

    pub fn project_bookmark_set_row(
        &self,
        input: crate::lists::BookmarkSetRowProjectionInput,
    ) -> crate::lists::BookmarkSetRowProjection {
        crate::lists::bookmark_set_row_projection(input)
    }

    /// Project create-collection sheet state. Rust owns title normalization
    /// and create eligibility; native shells render the returned state.
    pub fn project_curation_set_create(
        &self,
        input: crate::lists::CurationSetCreateProjectionInput,
    ) -> crate::lists::CurationSetCreateProjection {
        crate::lists::curation_set_create_projection(input)
    }

    pub fn project_web_bookmark_row(
        &self,
        input: crate::lists::WebBookmarkRowProjectionInput,
    ) -> crate::lists::WebBookmarkRowProjection {
        crate::lists::web_bookmark_row_projection(input)
    }

    /// Open a live subscription for the current user's kind:30003/30004 sets.
    /// Delivers `BookmarkSetsUpdated` (view-scoped) on each delta.
    pub async fn subscribe_bookmark_sets(&self) -> SubscriptionStartSnapshot {
        subscription_start_snapshot((|| {
            let user_hex = self
                .inner
                .read()
                .session
                .current_user()
                .map(|u| u.pubkey)
                .ok_or(CoreError::NotInitialized)?;
            let pk = PublicKey::from_hex(&user_hex)
                .map_err(|e| CoreError::InvalidInput(format!("invalid user pubkey: {e}")))?;
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::BookmarkSets { user_pubkey: pk },
            )
        })())
    }

    /// Open a live subscription for kind:30004 sets from followed authors.
    /// Delivers `FollowingCurationSetsUpdated` (view-scoped) on each delta.
    pub async fn subscribe_following_curation_sets(&self) -> SubscriptionStartSnapshot {
        subscription_start_snapshot((|| {
            let user_hex = self
                .inner
                .read()
                .session
                .current_user()
                .map(|u| u.pubkey)
                .ok_or(CoreError::NotInitialized)?;
            let follow_hexes = crate::follows::query_follows(self.runtime.ndb(), &user_hex)?;
            let follows: Vec<PublicKey> = follow_hexes
                .iter()
                .filter_map(|h| PublicKey::from_hex(h).ok())
                .collect();
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::FollowingCurationSets { follows },
            )
        })())
    }

    /// Open a live subscription for the current user's NIP-B0 kind:39701 events.
    /// Delivers `WebBookmarksUpdated` (view-scoped) on each delta.
    pub async fn subscribe_web_bookmarks(&self) -> SubscriptionStartSnapshot {
        subscription_start_snapshot((|| {
            let user_hex = self
                .inner
                .read()
                .session
                .current_user()
                .map(|u| u.pubkey)
                .ok_or(CoreError::NotInitialized)?;
            let pk = PublicKey::from_hex(&user_hex)
                .map_err(|e| CoreError::InvalidInput(format!("invalid user pubkey: {e}")))?;
            self.subscriptions.register(
                &self.runtime,
                SubscriptionKind::WebBookmarks { user_pubkey: pk },
            )
        })())
    }

    pub async fn lookup_isbn(&self, isbn: String) -> isbn_lookup::IsbnPreviewLookupSnapshot {
        isbn_lookup::lookup_snapshot(self.isbn_previews.lookup(&isbn).await)
    }

    /// Normalize user-entered or scanned ISBN input. Native shells use this
    /// only to enable/route capture UI; Rust remains the source of truth for
    /// ISBN validity and canonical ISBN-13 conversion.
    pub fn normalize_isbn_input(&self, raw: String) -> Option<String> {
        isbn_lookup::normalize_isbn(&raw).ok()
    }

    pub fn project_book_picker_query(
        &self,
        input: isbn_lookup::BookPickerQueryProjectionInput,
    ) -> isbn_lookup::BookPickerQueryProjection {
        isbn_lookup::book_picker_query_projection(input)
    }

    pub fn project_isbn_manual_preview(
        &self,
        input: isbn_lookup::IsbnManualPreviewProjectionInput,
    ) -> isbn_lookup::IsbnManualPreviewProjection {
        isbn_lookup::manual_preview_projection(input)
    }

    pub fn project_isbn_preview_request(
        &self,
        input: isbn_lookup::IsbnPreviewRequestProjectionInput,
    ) -> isbn_lookup::IsbnPreviewRequestProjection {
        isbn_lookup::isbn_preview_request_projection(input)
    }

    pub fn project_isbn_preview_lookup_apply(
        &self,
        input: isbn_lookup::IsbnPreviewLookupApplyInput,
    ) -> isbn_lookup::IsbnPreviewLookupApplyProjection {
        isbn_lookup::lookup_apply_projection(input)
    }

    /// Resolve an ISBN against the bounded recent-book projection already
    /// rendered by the native picker. Rust owns the canonical ISBN reference
    /// matching; native shells only decide how to present the selected record.
    pub fn find_existing_book_for_isbn(
        &self,
        isbn: String,
        recents: Vec<ArtifactRecord>,
    ) -> Option<ArtifactRecord> {
        isbn_lookup::existing_record_for_isbn(&isbn, &recents)
    }

    /// Build the edited ISBN book preview after scan/manual entry. Rust owns
    /// ISBN normalization and the NIP-73 reference fields; native supplies
    /// only the user's edited title/author and optional lookup metadata.
    pub fn build_edited_book_preview(
        &self,
        isbn: String,
        base_preview: Option<ArtifactPreview>,
        title: String,
        author: String,
    ) -> isbn_lookup::EditedBookPreviewProjection {
        isbn_lookup::edited_book_preview_projection(isbn.trim(), base_preview, &title, &author)
    }

    pub fn project_capture_book_display(
        &self,
        input: crate::capture::CaptureBookDisplayProjectionInput,
    ) -> crate::capture::CaptureBookDisplayProjection {
        crate::capture::book_display_projection(input)
    }

    pub fn project_capture_community_selection(
        &self,
        input: crate::capture::CaptureCommunitySelectionProjectionInput,
    ) -> crate::capture::CaptureCommunitySelectionProjection {
        crate::capture::community_selection_projection(input)
    }

    pub fn project_capture_stash(
        &self,
        input: crate::capture::CaptureStashProjectionInput,
    ) -> crate::capture::CaptureStashProjection {
        crate::capture::stash_projection(input)
    }

    pub fn project_capture_publish(
        &self,
        input: crate::capture::CapturePublishProjectionInput,
    ) -> crate::capture::CapturePublishProjection {
        crate::capture::publish_projection(input)
    }

    pub fn project_capture_publish_result(
        &self,
        input: crate::capture::CapturePublishResultProjectionInput,
    ) -> crate::capture::CapturePublishResultProjection {
        crate::capture::publish_result_projection(input)
    }

    pub fn project_capture_upload(
        &self,
        input: crate::capture::CaptureUploadProjectionInput,
    ) -> crate::capture::CaptureUploadProjection {
        crate::capture::upload_projection(input)
    }

    pub async fn publish_capture(
        &self,
        input: crate::capture::CapturePublishInput,
    ) -> crate::capture::CapturePublishSnapshot {
        let result: Result<String, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            let crate::capture::CapturePublishInput {
                image,
                quote,
                context,
                note,
                existing_artifact,
                pending_preview,
                target_group_id,
            } = input;

            if existing_artifact.is_some() && pending_preview.is_some() {
                return Err(CoreError::InvalidInput(
                    "Capture publish received both existing and pending artifacts.".into(),
                ));
            }

            let target_group_id = target_group_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let has_artifact = existing_artifact.is_some() || pending_preview.is_some();
            let highlight_projection = crate::capture::highlight_draft_projection(
                crate::capture::CaptureHighlightDraftInput {
                    quote,
                    context,
                    note: note.clone(),
                    image: image.clone(),
                },
            );

            if highlight_projection.has_highlight && has_artifact {
                let artifact = match (existing_artifact, pending_preview) {
                    (Some(record), None) => record,
                    (None, Some(preview)) => {
                        if let Some(group_id) = target_group_id.as_deref() {
                            crate::artifacts::publish(&self.runtime, preview, group_id, None)
                                .await?
                        } else {
                            crate::artifacts::unpublished_record(preview)
                        }
                    }
                    _ => {
                        return Err(CoreError::InvalidInput(
                            "Capture highlight requires one artifact selection.".into(),
                        ))
                    }
                };
                let draft = highlight_projection.draft.ok_or_else(|| {
                    CoreError::InvalidInput("Capture highlight draft was empty.".into())
                })?;

                if let Some(group_id) = target_group_id.as_deref() {
                    let records = crate::highlights::publish_and_share(
                        &self.runtime,
                        artifact,
                        vec![draft],
                        group_id,
                    )
                    .await?;
                    Ok(one_highlight_record(records)?.event_id)
                } else {
                    Ok(crate::highlights::publish(&self.runtime, draft, artifact)
                        .await?
                        .event_id)
                }
            } else {
                let artifact = match (existing_artifact, pending_preview) {
                    (Some(record), None) => Some(record),
                    (None, Some(preview)) => {
                        if let Some(group_id) = target_group_id.as_deref() {
                            Some(
                                crate::artifacts::publish(&self.runtime, preview, group_id, None)
                                    .await?,
                            )
                        } else {
                            Some(crate::artifacts::unpublished_record(preview))
                        }
                    }
                    (None, None) => None,
                    (Some(_), Some(_)) => unreachable!("conflicting capture artifact checked"),
                };
                let draft =
                    crate::capture::picture_draft(crate::capture::CapturePictureDraftInput {
                        image,
                        note,
                        artifact,
                        target_group_id,
                    });
                Ok(crate::pictures::publish_picture(&self.runtime, draft)
                    .await?
                    .event_id)
            }
        }
        .await;
        crate::capture::publish_snapshot(result)
    }

    pub fn reconstruct_ocr_markdown(&self, lines: Vec<crate::ocr::OcrLine>) -> String {
        crate::ocr::reconstruct_markdown(&lines)
    }

    pub fn detect_ocr_active_page(
        &self,
        lines: Vec<crate::ocr::OcrLine>,
    ) -> Option<crate::ocr::OcrPageDetection> {
        crate::ocr::detect_active_page(&lines)
    }

    pub fn crop_ocr_lines(
        &self,
        lines: Vec<crate::ocr::OcrLine>,
        page_rect: crate::ocr::OcrRect,
    ) -> Vec<crate::ocr::OcrLine> {
        crate::ocr::crop_lines(&lines, page_rect)
    }

    pub fn default_highlight_crop_box(
        &self,
        highlight_boxes: Vec<crate::ocr::OcrRect>,
        image_width: f64,
        image_height: f64,
        margin_fraction: f64,
    ) -> Option<crate::ocr::OcrRect> {
        crate::ocr::default_highlight_crop_box(
            &highlight_boxes,
            image_width,
            image_height,
            margin_fraction,
        )
    }

    pub fn sanitize_highlight_crop_box(
        &self,
        crop_box: crate::ocr::OcrRect,
        fallback: Option<crate::ocr::OcrRect>,
    ) -> crate::ocr::OcrRect {
        crate::ocr::sanitize_highlight_crop_box(crop_box, fallback)
    }

    pub fn selectable_ocr_words(
        &self,
        lines: Vec<crate::ocr::OcrLine>,
    ) -> Vec<crate::ocr::OcrWord> {
        crate::ocr::selectable_words(&lines)
    }

    pub fn join_ocr_quote(&self, words: Vec<crate::ocr::OcrWord>) -> String {
        crate::ocr::join_quote(&words)
    }

    pub fn ocr_alt_text(&self, markdown: String) -> String {
        crate::ocr::alt_text_from_markdown(&markdown)
    }

    // #21: `publish_share_queue_item` DELETED — the iOS Share Extension drain
    // now publishes via the kernel `hl.share.drain_queue` action
    // (kernel sole writer; no bespoke per-item publish). The queue-state path
    // (`share_extension::*` projections) is retained and still consumed.

    /// Project native web metadata request state. Rust owns URL validity,
    /// canonical fetch URL, and mirror cache keys.
    pub fn project_web_metadata_request(
        &self,
        input: web_metadata::WebMetadataRequestProjectionInput,
    ) -> web_metadata::WebMetadataRequestProjection {
        web_metadata::web_metadata_request_projection(input)
    }

    /// Fetch OpenGraph + favicon metadata for a web URL. Backed by a
    /// JSON-on-disk cache (7-day positive TTL, 1-hour negative TTL) and
    /// in-flight coalescing — concurrent calls for the same URL share one
    /// HTTP request. Returns `CoreError::NotFound` when the page 404s,
    /// `CoreError::Network` on transport failure.
    pub async fn get_web_metadata(&self, url: String) -> Option<WebMetadata> {
        web_metadata::get_or_fetch(self.web_metadata.clone(), &url)
            .await
            .ok()
    }

    pub async fn get_room_discussion_snapshot(
        &self,
        group_id: String,
    ) -> crate::discussions::RoomDiscussionSnapshot {
        crate::discussions::query_room_discussion_snapshot(self.runtime.ndb(), &group_id)
    }

    /// Lightweight cache projection for whether a room has any chat activity.
    pub async fn get_chat_presence_snapshot(
        &self,
        group_id: String,
    ) -> crate::chat::ChatPresenceSnapshot {
        crate::chat::query_chat_presence_snapshot(self.runtime.ndb(), &group_id)
    }

    /// Bounded room-chat read model. Rust owns page sizing, has-more policy,
    /// row grouping, and reply-target projection; native shells render rows.
    pub async fn get_chat_snapshot(
        &self,
        group_id: String,
        page_count: u32,
    ) -> crate::chat::ChatSnapshot {
        crate::chat::query_chat_snapshot(self.runtime.ndb(), &group_id, page_count)
    }

    pub fn project_chat_load_more(
        &self,
        input: crate::chat::ChatLoadMoreProjectionInput,
    ) -> crate::chat::ChatLoadMoreProjection {
        crate::chat::chat_load_more_projection(input)
    }

    pub fn project_chat_activity_reload(
        &self,
        input: crate::chat::ChatActivityReloadProjectionInput,
    ) -> crate::chat::ChatActivityReloadProjection {
        crate::chat::chat_activity_reload_projection(input)
    }

    // -- Feedback (shake-to-share) --

    /// Threads scoped to `coordinate` authored by the current user. Rust owns
    /// error collapse and returns an empty snapshot when logged out.
    pub async fn get_feedback_threads_snapshot(
        &self,
        coordinate: String,
    ) -> feedback::FeedbackThreadsSnapshot {
        self.feedback_threads_snapshot_for_current_user(&coordinate)
    }

    /// Bounded open-thread read model. Rust owns oldest-first ordering and
    /// message-group header derivation; native shells render rows.
    pub async fn get_feedback_thread_snapshot(
        &self,
        root_event_id: String,
    ) -> feedback::FeedbackThreadSnapshot {
        feedback::query_thread_snapshot(self.runtime.ndb(), &root_event_id)
    }

    /// Feedback composer projection shared by new-thread and reply surfaces.
    /// Rust owns submit trimming and send eligibility so each platform shell
    /// renders the same enabled/disabled state.
    pub fn project_feedback_composer(
        &self,
        input: feedback::FeedbackComposerProjectionInput,
    ) -> feedback::FeedbackComposerProjection {
        feedback::feedback_composer_projection(input)
    }

    /// Feedback publish result projection shared by root-thread and reply
    /// surfaces. Rust owns success/error classification; native shells apply
    /// the corresponding view transition.
    pub fn project_feedback_publish_result(
        &self,
        input: feedback::FeedbackPublishResultInput,
    ) -> feedback::FeedbackPublishResultProjection {
        feedback::feedback_publish_result_projection(input)
    }

    /// Feedback snapshot apply projection shared by list and thread stores.
    /// Rust owns success/error classification; native shells decide whether
    /// the current lifecycle should surface or ignore the projected error.
    pub fn project_feedback_snapshot_apply(
        &self,
        input: feedback::FeedbackSnapshotApplyInput,
    ) -> feedback::FeedbackSnapshotApplyProjection {
        feedback::feedback_snapshot_apply_projection(input)
    }

    /// Feedback thread row/detail presentation projection. Rust owns title,
    /// preview, summary, and status fallback rules; native shells keep
    /// localized relative-time formatting and rendering.
    pub fn project_feedback_thread_presentation(
        &self,
        thread: FeedbackThreadRecord,
    ) -> feedback::FeedbackThreadPresentationProjection {
        feedback::feedback_thread_presentation(thread)
    }

    /// Feedback message bubble presentation projection. Rust owns current-user
    /// classification, header grouping, and profile fallback semantics; native
    /// shells keep markdown and time rendering.
    pub fn project_feedback_message_presentation(
        &self,
        input: feedback::FeedbackMessagePresentationInput,
    ) -> feedback::FeedbackMessagePresentationProjection {
        feedback::feedback_message_presentation(input)
    }

    // -- Writes --

    // #21: `publish_artifact` DELETED — share-to-room artifact (kind:11) now
    // publishes via the kernel `hl.share.artifact_to_room` action (kernel sole
    // writer). The pure builder `artifacts::build_share_event` is retained as
    // the parity oracle only.

    pub async fn publish_discussion(
        &self,
        group_id: String,
        title: String,
        body: String,
        attachment: Option<ArtifactPreview>,
    ) -> crate::discussions::DiscussionPublishSnapshot {
        let result: Result<DiscussionRecord, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            crate::discussions::publish(
                &self.runtime,
                &group_id,
                &title,
                &body,
                attachment,
                self.clock.as_ref(),
            )
            .await
        }
        .await;
        crate::discussions::publish_snapshot(result)
    }

    pub async fn publish_discussion_from_composer(
        &self,
        input: crate::discussions::DiscussionComposerPublishInput,
    ) -> crate::discussions::DiscussionPublishSnapshot {
        let result: Result<DiscussionRecord, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            crate::discussions::publish_from_composer(&self.runtime, input, self.clock.as_ref())
                .await
        }
        .await;
        crate::discussions::publish_snapshot(result)
    }

    /// Chat composer projection. Rust owns draft normalization and send
    /// eligibility; native shells render the composer affordance.
    pub fn project_chat_composer(
        &self,
        input: crate::chat::ChatComposerProjectionInput,
    ) -> crate::chat::ChatComposerProjection {
        crate::chat::chat_composer_projection(input)
    }

    pub fn project_chat_publish_result(
        &self,
        input: crate::chat::ChatPublishResultInput,
    ) -> crate::chat::ChatPublishResultProjection {
        crate::chat::chat_publish_result_projection(input)
    }

    /// Publish a NIP-29 kind:9 chat message and return the refreshed bounded
    /// chat snapshot. Rust owns the optimistic merge of the signed record so
    /// native shells never fabricate chat rows.
    pub async fn publish_chat_message_snapshot(
        &self,
        group_id: String,
        content: String,
        reply_to_event_id: Option<String>,
        page_count: u32,
    ) -> crate::chat::ChatPublishSnapshot {
        if let Err(error) = self.require_user_pubkey() {
            return crate::chat::ChatPublishSnapshot {
                snapshot: crate::chat::query_chat_snapshot(
                    self.runtime.ndb(),
                    &group_id,
                    page_count,
                ),
                error: error.to_string(),
            };
        }
        crate::chat::publish_chat_message_snapshot(
            &self.runtime,
            self.runtime.ndb(),
            &group_id,
            &content,
            reply_to_event_id.as_deref(),
            page_count,
        )
        .await
    }

    /// Publish a feedback root note and return the refreshed bounded thread
    /// snapshot. Rust resolves the optional project agent `p` tag and owns the
    /// optimistic insertion of the signed root before relay echo.
    pub async fn publish_feedback_root_note_snapshot(
        &self,
        coordinate: String,
        body: String,
    ) -> feedback::FeedbackRootPublishSnapshot {
        let base_snapshot = self.feedback_threads_snapshot_for_current_user(&coordinate);
        if let Err(error) = self.require_user_pubkey() {
            return feedback::FeedbackRootPublishSnapshot {
                snapshot: base_snapshot,
                error: error.to_string(),
            };
        }

        let agent_pubkey = self.feedback_agent_pubkey_for(&coordinate);
        match feedback::publish_note(
            &self.runtime,
            &coordinate,
            agent_pubkey.as_deref(),
            None,
            &body,
        )
        .await
        {
            Ok(record) => feedback::FeedbackRootPublishSnapshot {
                snapshot: feedback::threads_snapshot_with_root(base_snapshot, &record),
                error: String::new(),
            },
            Err(error) => feedback::FeedbackRootPublishSnapshot {
                snapshot: base_snapshot,
                error: error.to_string(),
            },
        }
    }

    /// Publish a feedback reply into an existing root thread and return the
    /// refreshed bounded thread snapshot. Rust owns the NIP-10 root marker and
    /// optimistic merge of the signed reply.
    pub async fn publish_feedback_thread_reply_snapshot(
        &self,
        coordinate: String,
        parent_event_id: String,
        body: String,
    ) -> feedback::FeedbackReplyPublishSnapshot {
        let base_snapshot = feedback::query_thread_snapshot(self.runtime.ndb(), &parent_event_id);
        if let Err(error) = self.require_user_pubkey() {
            return feedback::FeedbackReplyPublishSnapshot {
                snapshot: base_snapshot,
                error: error.to_string(),
            };
        }

        let agent_pubkey = self.feedback_agent_pubkey_for(&coordinate);
        match feedback::publish_note(
            &self.runtime,
            &coordinate,
            agent_pubkey.as_deref(),
            Some(parent_event_id.as_str()),
            &body,
        )
        .await
        {
            Ok(record) => feedback::FeedbackReplyPublishSnapshot {
                snapshot: feedback::thread_snapshot_with_event(base_snapshot, &record),
                error: String::new(),
            },
            Err(error) => feedback::FeedbackReplyPublishSnapshot {
                snapshot: base_snapshot,
                error: error.to_string(),
            },
        }
    }

    /// Publish and share one podcast clip highlight. Rust owns clip draft
    /// construction, NIP-29 repost publication, and single-record outcome
    /// collapse for native player controls.
    pub async fn publish_podcast_clip_highlight(
        &self,
        input: podcast_transcript::PodcastClipPublishInput,
    ) -> podcast_transcript::PodcastClipPublishSnapshot {
        let result: Result<HighlightRecord, CoreError> = async {
            let _ = self.require_user_pubkey()?;
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

    pub fn project_podcast_clip_publish_result(
        &self,
        input: podcast_transcript::PodcastClipPublishResultInput,
    ) -> podcast_transcript::PodcastClipPublishResultProjection {
        podcast_transcript::clip_publish_result_projection(input)
    }

    /// Publish a podcast clip from the composer sheet. Rust owns draft
    /// construction and whether the clip is solo-published or also reposted
    /// into a NIP-29 room.
    pub async fn publish_podcast_composer_clip(
        &self,
        input: podcast_transcript::PodcastClipComposerPublishInput,
    ) -> podcast_transcript::PodcastClipPublishSnapshot {
        let result: Result<HighlightRecord, CoreError> = async {
            let _ = self.require_user_pubkey()?;
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

    // #21: `share_highlight_to_room` DELETED — share-to-room highlight repost
    // (kind:16) now publishes via the kernel `hl.share.highlight_to_room`
    // action (kernel sole writer). The pure builder
    // `highlights::build_repost_event` is retained as the parity oracle only.

    /// Publish a solo NIP-84 highlight from an article reader selection and
    /// return the refreshed reader snapshot. Rust owns article artifact
    /// derivation, optimistic highlight insertion, and duplicate suppression.
    pub async fn publish_article_reader_highlight_snapshot(
        &self,
        pubkey_hex: String,
        d_tag: String,
        article: Option<ArticleRecord>,
        quote: String,
        note: String,
        context: String,
    ) -> article_reader::ArticleReaderHighlightPublishSnapshot {
        let base_snapshot =
            article_reader::query_article_reader_snapshot(self.runtime.ndb(), &pubkey_hex, &d_tag);
        if let Err(error) = self.require_user_pubkey() {
            return article_reader::ArticleReaderHighlightPublishSnapshot {
                snapshot: base_snapshot,
                published_highlight_id: String::new(),
                error: error.to_string(),
            };
        }

        let article_for_publish = article.or_else(|| base_snapshot.article.clone());
        let result: Result<HighlightRecord, CoreError> = async {
            let article = article_for_publish
                .ok_or_else(|| CoreError::InvalidInput("Article not yet loaded.".into()))?;
            let artifact = articles::article_artifact_record(&article);
            let draft = highlights::article_reader_highlight_draft(quote, note, context);
            crate::highlights::publish(&self.runtime, draft, artifact).await
        }
        .await;
        match result {
            Ok(record) => article_reader::ArticleReaderHighlightPublishSnapshot {
                snapshot: article_reader::snapshot_with_published_highlight(base_snapshot, &record),
                published_highlight_id: record.event_id,
                error: String::new(),
            },
            Err(error) => article_reader::ArticleReaderHighlightPublishSnapshot {
                snapshot: base_snapshot,
                published_highlight_id: String::new(),
                error: error.to_string(),
            },
        }
    }

    // -- Rooms explorer (discovery + curation + recommendations) --

    /// Install (if not already installed) a long-lived relay sub for every
    /// kind:39000 metadata event. Call once on explorer appear from iOS.
    /// Idempotent; the sub rides until logout.
    pub async fn start_room_discovery(&self) {
        let already = self.inner.read().session.has_discovery_subscription();
        if already {
            return;
        }
        let sub_id = self.runtime.spawn_all_rooms_subscription();
        self.inner
            .write()
            .session
            .set_discovery_subscription(sub_id);
    }

    /// Install (if not already installed) two relay subs that together
    /// power the "Friends are here" explorer shelf:
    ///
    /// 1. kind:10009 authored by any of the user's follows — NIP-51
    ///    user-owned "simple groups" list (denser, always-public signal).
    /// 2. kind:39001 / 39002 where any follow appears in a `p` tag —
    ///    relay-owned membership fallback for groups whose members haven't
    ///    published a 10009 yet.
    ///
    /// No-op if the user isn't logged in or has no follows cached yet.
    /// Idempotent; both subs ride until logout.
    pub async fn start_friends_rooms_discovery(&self) -> MutationSnapshot {
        mutation_snapshot((|| {
            let (have_memberships, have_groups_list) = {
                let guard = self.inner.read();
                (
                    guard.session.has_friends_memberships_subscription(),
                    guard.session.has_friends_groups_list_subscription(),
                )
            };
            if have_memberships && have_groups_list {
                return Ok(());
            }
            let Some(user) = self.inner.read().session.current_user() else {
                return Ok(());
            };
            let follows_hex = follows::query_follows(self.runtime.ndb(), &user.pubkey)?;
            let follows: Vec<PublicKey> = follows_hex
                .iter()
                .filter_map(|s| PublicKey::from_hex(s.trim()).ok())
                .collect();
            if follows.is_empty() {
                return Ok(());
            }

            if !have_groups_list {
                if let Some(sub_id) = self
                    .runtime
                    .spawn_friends_groups_list_subscription(follows.clone())
                {
                    self.inner
                        .write()
                        .session
                        .set_friends_groups_list_subscription(sub_id);
                }
            }

            if !have_memberships {
                if let Some(sub_id) = self.runtime.spawn_friends_memberships_subscription(follows) {
                    self.inner
                        .write()
                        .session
                        .set_friends_memberships_subscription(sub_id);
                }
            }
            Ok(())
        })())
    }

    /// Publish a NIP-29 kind:9021 join-request for `group_id`. Rust owns the
    /// pending-join state and emits app toast deltas for request sent,
    /// request failure, and later membership confirmation.
    pub async fn request_join_room(
        &self,
        group_id: String,
        room_name: String,
    ) -> groups::JoinRoomRequestSnapshot {
        let group_id = group_id.trim().to_string();
        self.record_pending_join(&group_id, &room_name);
        let result: Result<String, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            groups::publish_join_request(&self.runtime, &group_id).await
        }
        .await;
        match &result {
            Ok(_) if self.has_pending_join(&group_id) => {
                self.emit_app_toast("Join requested".to_string());
            }
            Ok(_) => {}
            Err(error) => {
                self.remove_pending_join(&group_id);
                self.emit_app_toast(error.to_string());
            }
        }
        groups::join_room_request_snapshot(&group_id, result)
    }

    /// Create a brand-new NIP-29 room. Publishes kind:9007 (create-group) and
    /// kind:9002 (edit-metadata) signed by the current user. Returns the
    /// freshly-generated group id on success — the relay's 39000/39001/39002
    /// follow-up events drive the iOS membership stream automatically.
    pub fn project_create_room(
        &self,
        input: groups::CreateRoomProjectionInput,
    ) -> groups::CreateRoomProjection {
        groups::create_room_projection(input)
    }

    pub fn project_create_room_cover_upload_result(
        &self,
        input: groups::CreateRoomCoverUploadResultInput,
    ) -> groups::CreateRoomCoverUploadResultProjection {
        groups::create_room_cover_upload_result_projection(input)
    }

    pub fn project_create_room_publish_result(
        &self,
        input: groups::CreateRoomPublishResultInput,
    ) -> groups::CreateRoomPublishResultProjection {
        groups::create_room_publish_result_projection(input)
    }

    pub fn project_room_avatar(
        &self,
        input: groups::RoomAvatarProjectionInput,
    ) -> groups::RoomAvatarProjection {
        groups::room_avatar_projection(input)
    }

    pub fn project_room_cover_card(
        &self,
        input: groups::RoomCoverCardProjectionInput,
    ) -> groups::RoomCoverCardProjection {
        groups::room_cover_card_projection(input)
    }

    pub async fn create_room(
        &self,
        name: String,
        about: String,
        picture: String,
        visibility: groups::RoomVisibility,
        access: groups::RoomAccess,
    ) -> groups::CreateRoomPublishSnapshot {
        let result: Result<String, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            groups::create_room(
                &self.runtime,
                name.trim(),
                about.trim(),
                picture.trim(),
                visibility,
                access,
            )
            .await
        }
        .await;
        groups::create_room_publish_snapshot(result)
    }

    // #21: `get_room_share_link_snapshot` DELETED — RoomShareCard now mints the
    // invite code + publishes kind:9009 via the kernel `hl.share.mint_invite`
    // action (kernel sole writer; reuses the field-complete nmp
    // `nmp.nip29.create_invite` path). The minted code is read from the
    // `SharePublishSnapshot.invite_codes` and Swift composes the share link.

    pub async fn get_room_invite_snapshot(
        &self,
        input: crate::room_invites::RoomInviteSnapshotInput,
    ) -> crate::room_invites::RoomInviteSnapshot {
        let follows_result = (|| {
            let user_pubkey = self.require_user_pubkey()?;
            crate::follows::query_follows(self.runtime.ndb(), &user_pubkey.to_hex())
        })();
        crate::room_invites::snapshot(input, follows_result)
    }

    pub fn get_room_invite_avatar_projection(
        &self,
        input: crate::room_invites::RoomInviteAvatarProjectionInput,
    ) -> crate::room_invites::RoomInviteAvatarProjection {
        crate::room_invites::avatar_projection(input)
    }

    pub fn project_room_invite_selection(
        &self,
        input: crate::room_invites::RoomInviteSelectionInput,
    ) -> crate::room_invites::RoomInviteSelectionProjection {
        crate::room_invites::project_selection(input)
    }

    pub fn project_room_invite_selection_chrome(
        &self,
        input: crate::room_invites::RoomInviteSelectionChromeInput,
    ) -> crate::room_invites::RoomInviteSelectionChromeProjection {
        crate::room_invites::project_selection_chrome(input)
    }

    /// Pure projection of the post-send result toast/error for the room-invite
    /// screen. The actual kind:9000 put-user writes are owned by the kernel
    /// (`hl.room.add_member` → `nmp.nip29.put_user`), which is fire-and-forget
    /// (D6) — no per-pubkey relay ack flows back. Swift dispatches one
    /// `addRoomMember` per selected pubkey, then calls this with the set of
    /// pubkeys it could NOT dispatch (e.g. invalid hex) to render the toast.
    /// On the happy path `failed_pubkeys` is empty ⇒ all-success toast.
    pub fn project_room_invite_send_result(
        &self,
        selected: Vec<crate::room_invites::RoomInviteCandidate>,
        failed_pubkeys: Vec<String>,
    ) -> crate::room_invites::RoomInviteSendResultProjection {
        crate::room_invites::project_send_result(&selected, &failed_pubkeys)
    }

    /// Classify a NIP-19 entity (`npub1…`, `nprofile1…`, `note1…`,
    /// `nevent1…`, `naddr1…`) into a renderable variant. Strips an
    /// optional `nostr:` URI prefix. Used by the iOS rich-text renderer
    /// to walk event content for inline mentions and event-ref cards.
    pub fn decode_nostr_entity(
        &self,
        input: String,
    ) -> crate::nostr_entities::NostrEntityRefSnapshot {
        crate::nostr_entities::ref_snapshot(crate::nostr_entities::decode_nostr_entity(&input))
    }

    pub fn nostr_entity_fallback_label(
        &self,
        entity: crate::nostr_entities::NostrEntityRef,
    ) -> String {
        crate::nostr_entities::fallback_label(&entity)
    }

    pub fn nostr_entity_inline_render(
        &self,
        entity: crate::nostr_entities::NostrEntityRef,
    ) -> crate::nostr_entities::NostrEntityInlineRender {
        crate::nostr_entities::inline_render(&entity)
    }

    pub fn nostr_entity_identity_key(
        &self,
        entity: crate::nostr_entities::NostrEntityRef,
    ) -> String {
        crate::nostr_entities::identity_key(&entity)
    }

    pub fn project_nostr_entity_article_card(
        &self,
        input: crate::nostr_entities::NostrEntityArticleCardProjectionInput,
    ) -> crate::nostr_entities::NostrEntityArticleCardProjection {
        crate::nostr_entities::article_card_projection(input)
    }

    pub fn tokenize_nostr_content(
        &self,
        content: String,
    ) -> Vec<crate::nostr_entities::NostrContentRun> {
        crate::nostr_entities::tokenize_nostr_content(&content)
    }

    pub fn tokenize_nostr_markdown_inline(
        &self,
        content: String,
    ) -> Vec<crate::nostr_entities::NostrContentRun> {
        crate::nostr_entities::tokenize_nostr_markdown_inline(&content)
    }

    pub fn standalone_nostr_entity(
        &self,
        content: String,
    ) -> Option<crate::nostr_entities::NostrEntityRef> {
        crate::nostr_entities::standalone_nostr_entity(&content)
    }

    pub fn extract_nostr_event_refs(
        &self,
        content: String,
    ) -> Vec<crate::nostr_entities::NostrEntityRef> {
        crate::nostr_entities::extract_event_refs(&content)
    }

    /// Project the public highlight share URL. Rust owns the NIP-19 `nevent`
    /// encoding, relay hint, and beta route format; native shells render the
    /// returned URL only.
    pub fn get_highlight_share_url_snapshot(
        &self,
        event_id_hex: String,
        author_pubkey_hex: String,
    ) -> highlights::HighlightShareUrlSnapshot {
        highlights::highlight_share_url_snapshot(&event_id_hex, &author_pubkey_hex)
    }

    /// Best-effort cache lookup for a [`NostrEntityRef`]. Returns the
    /// resolved event when nostrdb already has it, `None` otherwise.
    /// The caller should pair this with `subscribe_nostr_entity` so a
    /// cold-cache reference warms up over the wire.
    pub async fn resolve_nostr_entity(
        &self,
        entity: crate::nostr_entities::NostrEntityRef,
    ) -> crate::nostr_entities::NostrEntityResolutionSnapshot {
        crate::nostr_entities::resolution_snapshot(crate::nostr_entities::resolve_from_cache(
            self.runtime.ndb(),
            &entity,
        ))
    }

    /// Install a view-scoped subscription for the missing event behind an
    /// entity. Routes to relay hints first (when the bech32 carried any) plus
    /// the indexer pool. Events received are persisted to nostrdb via the
    /// `NdbDatabase` bridge; the subscription pump emits
    /// `NostrEntityResolved` when the target lands.
    pub async fn subscribe_nostr_entity(
        &self,
        entity: crate::nostr_entities::NostrEntityRef,
    ) -> SubscriptionStartSnapshot {
        subscription_start_snapshot((|| {
            let _ = self.require_user_pubkey()?;
            let _ = crate::nostr_entities::relay_filter(&entity)?;
            self.subscriptions
                .register(&self.runtime, SubscriptionKind::NostrEntity { entity })
        })())
    }

    // -- Blossom (BUD-03, kind:10063) --

    /// Project the add-Blossom-server sheet. Rust owns URL normalization,
    /// scheme validity, and duplicate detection.
    pub fn project_blossom_server_entry(
        &self,
        input: blossom::BlossomServerEntryProjectionInput,
    ) -> blossom::BlossomServerEntryProjection {
        blossom::blossom_server_entry_projection(input)
    }

    /// Project Blossom server list edits. Rust owns URL normalization,
    /// duplicate filtering, delete protection, and save eligibility.
    pub fn project_blossom_server_list(
        &self,
        input: blossom::BlossomServerListProjectionInput,
    ) -> blossom::BlossomServerListProjection {
        blossom::blossom_server_list_projection(input)
    }

    /// Return the screen-shaped media settings snapshot. Rust owns error
    /// semantics and server-list normalization; native shells render the list.
    pub async fn get_blossom_server_settings_snapshot(
        &self,
    ) -> blossom::BlossomServerSettingsSnapshot {
        let result = (|| {
            let user = self
                .inner
                .read()
                .session
                .current_user()
                .ok_or(CoreError::NotAuthenticated)?;
            blossom::query_blossom_servers(self.runtime.ndb(), &user.pubkey)
        })();
        blossom::blossom_server_settings_snapshot(result)
    }

    /// Replace the user's Blossom server list with the normalized ordered
    /// settings projection. Rust blocks invalid empty saves and returns the
    /// mutation state instead of a raw event-id outcome.
    pub async fn set_blossom_server_settings(
        &self,
        servers: Vec<String>,
    ) -> blossom::BlossomServerSettingsMutationSnapshot {
        let projection =
            blossom::blossom_server_list_projection(blossom::BlossomServerListProjectionInput {
                servers,
                add_url: None,
                remove_indexes: Vec::new(),
                move_indexes: Vec::new(),
                move_to_index: None,
            });
        let normalized_servers = projection.servers.clone();
        let result: Result<String, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            if !projection.can_save {
                return Err(CoreError::InvalidInput(
                    "at least one blossom server required".into(),
                ));
            }
            blossom::publish_blossom_servers(&self.runtime, projection.servers).await
        }
        .await;
        blossom::blossom_server_settings_mutation_snapshot(normalized_servers, result)
    }

    /// Publish the default Blossom server list only if the user has no cached
    /// kind:10063. Called once after login; no-op when the list already exists.
    pub async fn init_default_blossom_servers(&self) -> MutationSnapshot {
        let result: Result<(), CoreError> = async {
            let user = self
                .inner
                .read()
                .session
                .current_user()
                .ok_or(CoreError::NotAuthenticated)?;
            blossom::init_default_blossom_servers(&self.runtime, &user.pubkey).await
        }
        .await;
        mutation_snapshot(result)
    }

    // -- Capture flow (BUD-01 upload + kind:20 picture publish) --

    /// Upload a photo to the default Blossom server (`blossom.primal.net`)
    /// using BUD-01 auth. The caller (iOS) is responsible for stripping EXIF
    /// metadata and recompressing the image before sending bytes — Rust does
    /// not decode the image. `width`/`height` are stamped onto the returned
    /// descriptor for use in the publishing event's `imeta` tag.
    /// `alt` is the recognized OCR text, or empty if none.
    pub async fn upload_photo(
        &self,
        bytes: Vec<u8>,
        mime: String,
        width: u32,
        height: u32,
        alt: String,
    ) -> blossom::BlossomUploadSnapshot {
        let result: Result<BlossomUpload, CoreError> = async {
            let _ = self.require_user_pubkey()?;
            blossom::upload_blob(
                &self.runtime,
                bytes,
                mime,
                width,
                height,
                alt,
                self.clock.as_ref(),
            )
            .await
        }
        .await;
        blossom::upload_snapshot(result)
    }

    // -- Relay config (NIP-65 read/write + NIP-78 rooms/indexer) --

    pub fn project_network_settings_snapshot_apply(
        &self,
        input: crate::relays::NetworkSettingsSnapshotApplyInput,
    ) -> crate::relays::NetworkSettingsSnapshotApplyProjection {
        crate::relays::network_settings_snapshot_apply_projection(input)
    }

    /// Insert-or-update a single relay. Replaces the row with matching URL or
    /// appends a new one, re-publishes kind:10002 + kind:30078, and reconciles
    /// the live relay pool so the change takes effect immediately.
    pub async fn upsert_relay(
        &self,
        cfg: crate::relays::RelayConfig,
    ) -> crate::relays::NetworkSettingsMutationSnapshot {
        let result: Result<(), CoreError> = async {
            let user = self
                .inner
                .read()
                .session
                .current_user()
                .ok_or(CoreError::NotAuthenticated)?;
            crate::relays::upsert_relay(&self.runtime, &user.pubkey, cfg).await?;
            self.runtime.spawn_apply_user_relay_config(user.pubkey);
            Ok(())
        }
        .await;
        crate::relays::network_settings_mutation_snapshot(result, true, "Couldn't add relay")
    }

    pub fn project_network_settings_mutation_apply(
        &self,
        input: crate::relays::NetworkSettingsMutationApplyInput,
    ) -> crate::relays::NetworkSettingsMutationApplyProjection {
        crate::relays::network_settings_mutation_apply_projection(input)
    }

    /// Remove a relay by URL.
    pub async fn remove_relay(
        &self,
        url: String,
    ) -> crate::relays::NetworkSettingsMutationSnapshot {
        let result: Result<(), CoreError> = async {
            let user = self
                .inner
                .read()
                .session
                .current_user()
                .ok_or(CoreError::NotAuthenticated)?;
            crate::relays::remove_relay(&self.runtime, &user.pubkey, url).await?;
            self.runtime.spawn_apply_user_relay_config(user.pubkey);
            Ok(())
        }
        .await;
        crate::relays::network_settings_mutation_snapshot(result, true, "Couldn't remove relay")
    }

    /// Atomically update a single relay's role flags.
    pub async fn set_relay_roles(
        &self,
        url: String,
        read: bool,
        write: bool,
        rooms: bool,
        indexer: bool,
    ) -> crate::relays::NetworkSettingsMutationSnapshot {
        let result: Result<(), CoreError> = async {
            let user = self
                .inner
                .read()
                .session
                .current_user()
                .ok_or(CoreError::NotAuthenticated)?;
            crate::relays::set_relay_roles(
                &self.runtime,
                &user.pubkey,
                url,
                read,
                write,
                rooms,
                indexer,
            )
            .await?;
            self.runtime.spawn_apply_user_relay_config(user.pubkey);
            Ok(())
        }
        .await;
        crate::relays::network_settings_mutation_snapshot(result, true, "Couldn't update roles")
    }

    // -- Relay telemetry --

    /// Project a live diagnostics payload plus the derived Network Settings
    /// header/auto-connected state for the current configured relay rows.
    pub fn project_network_diagnostics_snapshot(
        &self,
        configured_relays: Vec<crate::relays::RelayConfig>,
        diagnostics: Vec<RelayDiagnostic>,
    ) -> crate::relays::NetworkDiagnosticsSnapshot {
        crate::relays::network_diagnostics_snapshot(configured_relays, diagnostics)
    }

    pub fn project_network_diagnostics_snapshot_apply(
        &self,
        input: crate::relays::NetworkDiagnosticsSnapshotApplyInput,
    ) -> crate::relays::NetworkDiagnosticsSnapshotApplyProjection {
        crate::relays::network_diagnostics_snapshot_apply_projection(input)
    }

    pub fn auto_connected_relay_config(&self, url: String) -> crate::relays::RelayConfig {
        crate::relays::auto_connected_display_config(url)
    }

    pub fn project_relay_row(
        &self,
        input: crate::relays::RelayRowProjectionInput,
    ) -> crate::relays::RelayRowProjection {
        crate::relays::relay_row_projection(input)
    }

    pub fn project_relay_detail(
        &self,
        input: crate::relays::RelayDetailProjectionInput,
    ) -> crate::relays::RelayDetailProjection {
        crate::relays::relay_detail_projection(input)
    }

    pub fn project_relay_remove(
        &self,
        input: crate::relays::RelayRemoveProjectionInput,
    ) -> crate::relays::RelayRemoveProjection {
        crate::relays::relay_remove_projection(input)
    }

    pub fn default_add_relay_config(&self) -> crate::relays::RelayConfig {
        crate::relays::default_add_relay_config()
    }

    pub fn project_add_relay_sheet(
        &self,
        input: crate::relays::AddRelaySheetProjectionInput,
    ) -> crate::relays::AddRelaySheetProjection {
        crate::relays::add_relay_sheet_projection(input)
    }

    pub fn plan_relay_nip11_probes(
        &self,
        input: crate::relays::RelayNip11ProbePlanInput,
    ) -> crate::relays::RelayNip11ProbePlan {
        crate::relays::plan_relay_nip11_probes(input)
    }

    pub fn finish_relay_nip11_probe(
        &self,
        in_flight_urls: Vec<String>,
        url: String,
    ) -> Vec<String> {
        crate::relays::finish_relay_nip11_probe(in_flight_urls, url)
    }

    pub fn toggle_import_relay_selection(
        &self,
        fetched: Vec<crate::relays::RelayConfig>,
        selected_urls: Vec<String>,
        url: String,
    ) -> Vec<String> {
        crate::relays::toggle_import_relay_selection(fetched, selected_urls, url)
    }

    /// Project import-relays source input. Rust owns source trimming and fetch
    /// eligibility; native shells render and execute the fetch action.
    pub fn project_import_relays_source(
        &self,
        input: crate::relays::ImportRelaysSourceProjectionInput,
    ) -> crate::relays::ImportRelaysSourceProjection {
        crate::relays::import_relays_source_projection(input)
    }

    pub fn project_import_relays(
        &self,
        input: crate::relays::ImportRelaysProjectionInput,
    ) -> crate::relays::ImportRelaysProjection {
        crate::relays::import_relays_projection(input)
    }

    pub fn project_import_relays_fetch_apply(
        &self,
        input: crate::relays::ImportRelaysFetchApplyInput,
    ) -> crate::relays::ImportRelaysFetchApplyProjection {
        crate::relays::import_relays_fetch_apply_projection(input)
    }

    /// Handle the Swift side uses to match `RelayStatusChanged` deltas on the
    /// event bus. Relay status changes are app-scoped and ride
    /// `subscription_id == 0`, so this returns `0` unconditionally — the
    /// value is a stable contract, not a unique sub id.
    pub async fn subscribe_relay_status(&self) -> SubscriptionStartSnapshot {
        self.runtime.enable_relay_diagnostics_events().await;
        subscription_start_snapshot(Ok(0))
    }

    /// Nudge the relay pool to attempt a reconnect on every disconnected
    /// relay. `Client::connect` is idempotent — already-connected relays
    /// are unaffected; disconnected / terminated / banned relays get a
    /// fresh WebSocket attempt.
    pub async fn reconnect_all(&self) -> crate::relays::NetworkSettingsMutationSnapshot {
        self.runtime.client().connect().await;
        self.runtime.sync_relay_diagnostics().await;
        crate::relays::network_settings_mutation_snapshot(Ok(()), false, "Couldn't reconnect")
    }

    /// Close every WebSocket in the pool. Used by explicit user/app
    /// reconnect flows; Wi-Fi-only path policy is owned by
    /// `apply_network_path_status`.
    pub async fn disconnect_all(&self) -> crate::relays::NetworkSettingsMutationSnapshot {
        self.runtime.client().disconnect().await;
        self.runtime.sync_relay_diagnostics().await;
        crate::relays::network_settings_mutation_snapshot(Ok(()), false, "Couldn't disconnect")
    }

}

impl HighlighterCore {
    fn emit_app_toast(&self, message: String) {
        if message.trim().is_empty() {
            return;
        }
        let cb = { self.callback_slot.read().clone() };
        if let Some(cb) = cb {
            cb.on_data_changed(Delta {
                subscription_id: 0,
                change: DataChangeType::AppToastRequested { message },
            });
        }
    }

    fn record_pending_join(&self, group_id: &str, room_name: &str) {
        let group_id = group_id.trim();
        if group_id.is_empty() {
            return;
        }
        self.inner
            .write()
            .pending_joins
            .insert(group_id.to_string(), join_room_display_name(room_name));
    }

    fn remove_pending_join(&self, group_id: &str) -> Option<String> {
        self.inner.write().pending_joins.remove(group_id.trim())
    }

    fn has_pending_join(&self, group_id: &str) -> bool {
        self.inner
            .read()
            .pending_joins
            .contains_key(group_id.trim())
    }

    async fn toggle_comment_like_state(
        &self,
        event_id: &str,
        author_pubkey_hex: &str,
    ) -> Result<bool, CoreError> {
        let current_user = self.require_user_pubkey()?;
        let current_user_hex = current_user.to_hex();
        let event_id = event_id.trim();
        let summary = crate::reactions::query_like_summary_for_event(
            self.runtime.ndb(),
            event_id,
            Some(current_user_hex.as_str()),
            128,
        )?;
        if let Some(reaction_event_id) = summary.my_like_event_id {
            crate::reactions::unpublish_reaction(&self.runtime, &reaction_event_id).await?;
            Ok(false)
        } else {
            crate::reactions::publish_comment_like(
                &self.runtime,
                event_id,
                author_pubkey_hex.trim(),
            )
            .await?;
            Ok(true)
        }
    }

    async fn toggle_event_bookmark_state(&self, event_id_hex: &str) -> Result<bool, CoreError> {
        let user_hex = self
            .inner
            .read()
            .session
            .current_user()
            .map(|u| u.pubkey)
            .ok_or(CoreError::NotInitialized)?;
        crate::bookmarks::toggle_event_bookmark(&self.runtime, &user_hex, event_id_hex).await
    }

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
        let subscriptions = Arc::new(SubscriptionRegistry::new(callback_slot.clone()));
        // Register relay diagnostics with the app event bus before handing
        // out the Arc<Self>. The runtime seeds its bounded diagnostics map
        // from the SDK pool and then reacts to relay status notifications.
        runtime.install_diagnostics_callback(callback_slot.clone());
        let web_metadata = Arc::new(WebMetadataStore::open_with_clock(
            runtime.data_dir(),
            clock.clone(),
        ));
        let isbn_previews = Arc::new(isbn_lookup::IsbnPreviewCache::new(runtime.data_dir()));
        let onboarding = Arc::new(onboarding::OnboardingStore::new(runtime.data_dir()));
        let podcast_position = Arc::new(podcast_position::PodcastPositionStore::new_with_clock(
            runtime.data_dir(),
            clock.clone(),
        ));
        Arc::new(Self {
            inner: Arc::new(RwLock::new(Inner {
                session: Session::new(),
                pending_joins: BTreeMap::new(),
            })),
            runtime,
            callback_slot,
            subscriptions,
            web_metadata,
            isbn_previews,
            onboarding,
            podcast_position,
            clock,
        })
    }

    /// Drop the previous follows-NIP-65 sub (if any) and install a new one
    /// covering `follows`. Also fires a fresh purplepag.es negentropy sync
    /// for kind:0/3/10002 — cheap when most events are already cached
    /// (negentropy only ships the deltas) and the right thing to do when
    /// the follow set may have grown since last call. No-op when `follows`
    /// is empty.
    fn refresh_follows_nip65_subscription(&self, follows: &[PublicKey]) {
        if follows.is_empty() {
            return;
        }
        self.runtime
            .spawn_negentropy_sync_for_follows(follows.to_vec());
        let new_id = match self
            .runtime
            .spawn_follows_relay_lists_subscription(follows.to_vec())
        {
            Some(id) => id,
            None => return,
        };
        let prev = {
            let mut guard = self.inner.write();
            let prev = guard.session.take_follows_nip65_subscription();
            guard.session.set_follows_nip65_subscription(new_id);
            prev
        };
        if let Some(prev) = prev {
            self.runtime.drop_subscription(prev);
        }
    }

    fn require_user_pubkey(&self) -> Result<PublicKey, CoreError> {
        let guard = self.inner.read();
        let user = guard
            .session
            .current_user()
            .ok_or(CoreError::NotAuthenticated)?;
        PublicKey::from_hex(&user.pubkey)
            .map_err(|e| CoreError::Other(format!("invalid current user pubkey: {e}")))
    }

    fn current_user_pubkey_hex(&self) -> Option<String> {
        self.inner
            .read()
            .session
            .current_user()
            .map(|user| user.pubkey)
    }

    fn curation_menu_snapshot_for_user(
        &self,
        user_hex: &str,
        address: &str,
    ) -> Result<crate::lists::CurationMenuSnapshot, CoreError> {
        let sets = crate::lists::query_user_sets(
            self.runtime.ndb(),
            user_hex,
            crate::lists::KIND_CURATION_SETS,
        )?;
        Ok(crate::lists::curation_menu_snapshot_for_address(
            sets, address,
        ))
    }

    fn feedback_agent_pubkey_for(&self, coordinate: &str) -> Option<String> {
        feedback::query_first_agent_pubkey(self.runtime.ndb(), coordinate)
            .ok()
            .flatten()
    }

    fn feedback_threads_snapshot_for_current_user(
        &self,
        coordinate: &str,
    ) -> feedback::FeedbackThreadsSnapshot {
        let current_user_pubkey = self
            .inner
            .read()
            .session
            .current_user()
            .map(|user| user.pubkey.clone());
        feedback::query_threads_snapshot(
            self.runtime.ndb(),
            coordinate,
            current_user_pubkey.as_deref(),
        )
    }
}

/// Read the cached kind:3 contact list for `user_pubkey` and return the
/// `p`-tag pubkeys that successfully parse. Used at login to seed the
/// follows-NIP-65 subscription before the user touches a home feed.
fn current_followed_pubkeys(ndb: &nostrdb::Ndb, user_pubkey: &PublicKey) -> Vec<PublicKey> {
    let user_hex = user_pubkey.to_hex();
    let hexes = match crate::follows::query_follows(ndb, &user_hex) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "load cached follows for outbox bootstrap");
            return Vec::new();
        }
    };
    hexes
        .iter()
        .filter_map(|s| PublicKey::from_hex(s.trim()).ok())
        .collect()
}

/// Strip a leading `nostr:` prefix, trim whitespace. Olas does this before
/// handing a URI to `NDKBunkerSigner.bunker(...)`.
pub(crate) fn normalize_bunker_uri(input: &str) -> String {
    let t = input.trim();
    t.strip_prefix("nostr:").unwrap_or(t).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{channel, Sender};
    use std::time::Duration;

    struct ChannelCallback {
        tx: Sender<Delta>,
    }

    impl EventCallback for ChannelCallback {
        fn on_data_changed(&self, delta: Delta) {
            self.tx.send(delta).expect("send delta");
        }
    }

    #[test]
    fn confirm_pending_join_emits_rust_owned_toast_once() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let core = HighlighterCore::new_with_data_dir(tmp.path().join("ndb"));
        let (tx, rx) = channel();
        core.set_event_callback(Arc::new(ChannelCallback { tx }));

        core.record_pending_join("alpha", " Alpha ");
        core.confirm_pending_join("alpha".to_string());

        let delta = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("toast delta");
        match delta.change {
            DataChangeType::AppToastRequested { message } => {
                assert_eq!(message, "You're in Alpha ✓");
            }
            other => panic!("expected toast, got {other:?}"),
        }

        core.confirm_pending_join("alpha".to_string());
        assert!(
            rx.recv_timeout(Duration::from_millis(100)).is_err(),
            "pending join should be consumed once"
        );
    }

    #[test]
    fn secret_key_settings_snapshot_reads_active_nsec_session() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let core = HighlighterCore::new_with_data_dir(tmp.path().join("ndb"));

        let missing = core.current_secret_key_settings_snapshot(false);
        assert!(!missing.has_secret_key);
        assert_eq!(missing.copy_value, None);

        let nsec = Keys::generate().secret_key().to_bech32().unwrap();
        let login = core.login_nsec(nsec.clone());
        assert!(login.is_authenticated);
        assert_eq!(login.persist_nsec, Some(nsec.clone()));
        assert_eq!(login.persist_bunker_uri, None);

        let hidden = core.current_secret_key_settings_snapshot(false);
        assert!(hidden.has_secret_key);
        assert_eq!(hidden.copy_value, Some(nsec.clone()));
        assert_ne!(hidden.display_value, nsec);

        let revealed = core.current_secret_key_settings_snapshot(true);
        assert_eq!(revealed.display_value, nsec);

        core.logout();
        assert!(
            !core
                .current_secret_key_settings_snapshot(true)
                .has_secret_key
        );
    }
}
