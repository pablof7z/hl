//! Podcast playback state domain — Phase 5H.
//!
//! ## Responsibilities
//!
//! * **CAPABILITY** — emits `CapabilityRequest::Audio(AudioOp)` to drive the
//!   native AVPlayer (load/play/pause/seek/stop/waveform).  Native executes;
//!   Rust decides (D7).
//!
//! * **PLAYBACK STATE** — owns the in-kernel state for the currently-loaded
//!   episode: guid, duration, current position, is_playing, clip selection,
//!   waveform cache key.  All transient — not published to nostr.
//!
//! * **RESUME POLICY** — ports `session_plan` / `session_apply_projection` /
//!   `seek_projection` / `tick_projection` from the live `podcast_playback.rs`
//!   bespoke lane.  Live lane is UNTOUCHED.
//!
//! * **POSITION STORE** — ports `PodcastPositionStore` from the live
//!   `podcast_position.rs`.  File: `{data_dir}/podcast-position-v1.json`.
//!   7-day staleness TTL.  DEVICE-LOCAL — NEVER published to nostr
//!   (`hl-app-state-vs-nostr-facts`).
//!
//! * **VIEW** — `ViewId::PodcastListening` /
//!   `ViewSnapshot::PodcastListening(PodcastListeningSnapshot)` carries raw
//!   position fields; Swift formats all display strings (D1).
//!
//! * **BOUNDED PROGRESS CADENCE (D8)** — native reports at most ~1 s between
//!   `AudioResult::Progress` results; the kernel further coalesces via the
//!   injected clock (`tick_projection` — updates only on whole-second boundary).
//!   No polling loop is ever started in the kernel.
//!
//! ## Device-local classification
//!
//! Everything in this domain is device-local.  The ONLY nostr fact produced by
//! the podcast flow is the kind:9802 clip highlight (Phase 5J — not yet built).
//! This domain: resume positions, is_playing, clip selection in-progress,
//! waveform peaks — all stay local.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;

use crate::capabilities::{AudioOp, AudioResult, CapabilityRequest};
use crate::clock::Clock;
use crate::errors::CoreError;
use crate::kernel::app::AppState;
use crate::kernel::domains::capture_draft::new_correlation_id;
use crate::kernel::effect::Effect;
use crate::kernel::models::{ArtifactRecord, PodcastPositionRecord};
use crate::kernel::snapshot::{
    KernelClipPublishPhase, KernelTranscriptAvailability, KernelTranscriptSegment,
    PodcastListeningSnapshot, ViewSnapshot,
};

// ─── Constants ────────────────────────────────────────────────────────────────

/// Position persistence interval — every 5 s of continuous playback.
const POSITION_PERSIST_INTERVAL_SECS: i64 = 5;

/// Staleness TTL for the position store — 7 days.
#[allow(dead_code)]
const POSITION_MAX_AGE_SECS: u64 = 7 * 24 * 60 * 60;

/// State file name within `data_dir`.
pub(crate) const POSITION_FILE_NAME: &str = "podcast-position-v1.json";

// ─── In-kernel playback state ─────────────────────────────────────────────────

/// Playback state kept in `AppState::podcast`.
///
/// Transient — lives only for the current session. All fields are
/// DEVICE-LOCAL (never published to nostr).
#[derive(Debug, Clone, Default)]
pub struct PodcastState {
    /// The episode currently loaded in the native player, if any.
    pub current: Option<LoadedEpisode>,
    // ── Phase 5J additions (append-only) ─────────────────────────────────────
    /// FSM phase for the current clip-publish round-trip.
    ///
    /// DEVICE-LOCAL — only the published kind:9802 is a nostr fact.
    /// Reset to `Idle` when `clip_clear` is dispatched or a new episode loads.
    pub clip_publish_phase: KernelClipPublishPhase,
    /// Correlation id tracked so `apply_action_result_row` in `blossom.rs`
    /// can route the `action_results` verdict to `ClipPublishActionResult`.
    ///
    /// `None` when no publish is in flight.
    pub pending_clip_publish_correlation_id: Option<String>,
}

/// Transcript segment — a time-bounded utterance within an episode.
///
/// Ported from the bespoke `podcast_transcript.rs::TranscriptSegment`.
/// DEVICE-LOCAL — fetched per session, never published to nostr.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptSegment {
    pub id: String,
    pub start: f64,
    pub end: f64,
    pub speaker: String,
    pub text: String,
}

/// Transcript availability state for the current episode.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TranscriptAvailability {
    #[default]
    NotRequested,
    Loading,
    Available,
    Unavailable,
}

/// In-progress clip selection.
///
/// Built by the clip-mark-in/mark-out/extend-segment actions.
/// DEVICE-LOCAL — only the published kind:9802 is a nostr fact (Phase 5J).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClipSelection {
    pub clip_start_seconds: Option<f64>,
    pub clip_end_seconds: Option<f64>,
    pub speaker: String,
    pub selected_segment_ids: Vec<String>,
}

/// State for the currently loaded episode.
#[derive(Debug, Clone)]
pub struct LoadedEpisode {
    /// Podcast item GUID (unique per episode).
    pub guid: String,
    /// The full artifact record (carries audio URL, duration hint, etc.).
    pub artifact: ArtifactRecord,
    /// Duration in seconds, as reported by native after `AudioResult::Loaded`.
    /// `0.0` until the `Loaded` result arrives.
    pub duration_seconds: f64,
    /// Current playback position in seconds.  Updated by `AudioResult::Progress`
    /// (coalesced via `tick_projection` to whole-second boundaries).
    pub position_seconds: f64,
    /// The last position value passed to `tick_projection`, used to detect
    /// whole-second boundaries without keeping wall-clock state.
    pub previous_position_for_tick: f64,
    /// `true` when native reports `is_playing`.
    pub is_playing: bool,
    /// Waveform peaks SHA-256 cache key (URL-keyed, used by Phase 5J waveform).
    pub waveform_cache_key: Option<String>,
    /// Transcript segments fetched for this episode. Empty until
    /// `Effect::FetchTranscript` completes. DEVICE-LOCAL — never published.
    pub transcript_segments: Vec<TranscriptSegment>,
    /// Availability of the transcript for this episode.
    pub transcript_availability: TranscriptAvailability,
    /// In-progress clip selection. `None` when no clip is being assembled.
    pub clip_selection: Option<ClipSelection>,
}

// ─── Position store ───────────────────────────────────────────────────────────

/// Durable resume-position store.
///
/// Ported from the bespoke `podcast_position.rs` (that file is UNTOUCHED).
/// Writes to `{data_dir}/podcast-position-v1.json` atomically via a tmp→rename.
/// 7-day staleness TTL.  NEVER produces a nostr event.
#[allow(dead_code)]
pub(crate) struct PodcastPositionStore {
    path: PathBuf,
    clock: Arc<dyn Clock>,
    /// Inner is `None` until first `current()` call (lazy load).
    record: Mutex<Option<Option<PodcastPositionRecord>>>,
}

#[allow(dead_code)]
impl PodcastPositionStore {
    pub(crate) fn new_with_clock(data_dir: &Path, clock: Arc<dyn Clock>) -> Self {
        Self {
            path: data_dir.join(POSITION_FILE_NAME),
            clock,
            record: Mutex::new(None),
        }
    }

    /// Load (lazily) and return the current position record, applying the TTL.
    pub(crate) fn current(&self) -> Option<PodcastPositionRecord> {
        let mut guard = self.record.lock();
        if guard.is_none() {
            *guard = Some(load_record(&self.path));
        }

        let record = guard.as_ref().and_then(Clone::clone)?;
        let now = self.clock.now_unix_seconds();
        if is_stale(&record, now) {
            if let Err(e) = remove_record(&self.path) {
                tracing::warn!(path = %self.path.display(), error = %e, "failed to remove stale podcast position");
            }
            *guard = Some(None);
            return None;
        }

        Some(record)
    }

    /// Return the saved position for `guid`, or `None`.
    pub(crate) fn position_for_guid(&self, guid: &str) -> Option<f64> {
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

    /// Persist a position update.  Returns `Err` on invalid input or I/O failure.
    pub(crate) fn save(
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
        let position_seconds = validate_position(position_seconds)?;
        let record = PodcastPositionRecord {
            guid,
            position_seconds,
            last_played_at_unix_seconds: self.clock.now_unix_seconds(),
            artifact,
        };

        persist_record(&self.path, &record)?;
        *self.record.lock() = Some(Some(record));
        Ok(())
    }
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
fn remove_record(path: &Path) -> Result<(), CoreError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(CoreError::Cache(format!("clear podcast position: {e}"))),
    }
}

#[allow(dead_code)]
fn is_stale(record: &PodcastPositionRecord, now: u64) -> bool {
    now.saturating_sub(record.last_played_at_unix_seconds) >= POSITION_MAX_AGE_SECS
}

#[allow(dead_code)]
fn validate_position(position_seconds: f64) -> Result<f64, CoreError> {
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

// ─── Action reducers ──────────────────────────────────────────────────────────

/// Reduce `hl.audio.play` — load the player at the saved resume position and
/// emit a `CapabilityRequest::Audio(AudioOp::Load)`.
///
/// Resume position is looked up from `store` (DEVICE-LOCAL — D7: kernel owns
/// policy, native executes).
pub(crate) fn reduce_action_play(
    state: &mut AppState,
    url: String,
    guid: String,
    artifact: ArtifactRecord,
    saved_position: Option<f64>,
) -> Vec<Effect> {
    let url = url.trim().to_string();
    if url.is_empty() {
        tracing::warn!("hl.audio.play: empty url — no-op (D6)");
        return vec![];
    }

    let resume_at = saved_position.filter(|p| p.is_finite() && *p >= 0.0);

    state.podcast.current = Some(LoadedEpisode {
        guid,
        artifact,
        duration_seconds: 0.0,
        position_seconds: resume_at.unwrap_or(0.0),
        previous_position_for_tick: resume_at.unwrap_or(0.0),
        is_playing: false,
        waveform_cache_key: None,
        transcript_segments: Vec::new(),
        transcript_availability: TranscriptAvailability::NotRequested,
        clip_selection: None,
    });

    vec![Effect::EmitCapabilityRequest(CapabilityRequest::Audio(
        AudioOp::Load {
            url,
            resume_at_seconds: resume_at,
        },
    ))]
}

/// Reduce `hl.audio.pause` — emit `AudioOp::Pause`.
pub(crate) fn reduce_action_pause(state: &mut AppState) -> Vec<Effect> {
    if let Some(ep) = &mut state.podcast.current {
        ep.is_playing = false;
    }
    vec![Effect::EmitCapabilityRequest(CapabilityRequest::Audio(
        AudioOp::Pause,
    ))]
}

/// Reduce `hl.audio.resume` — resume playback when the player is already
/// loaded (paused state). Emits `AudioOp::Play` without reloading the episode.
pub(crate) fn reduce_action_resume(state: &mut AppState) -> Vec<Effect> {
    if let Some(ep) = &mut state.podcast.current {
        ep.is_playing = true;
    }
    vec![Effect::EmitCapabilityRequest(CapabilityRequest::Audio(
        AudioOp::Play,
    ))]
}

/// Reduce `hl.audio.seek` — clamp via `seek_projection`, emit `AudioOp::Seek`.
pub(crate) fn reduce_action_seek(state: &mut AppState, seconds: f64) -> Vec<Effect> {
    let duration = state
        .podcast
        .current
        .as_ref()
        .map(|ep| ep.duration_seconds)
        .unwrap_or(0.0);

    let clamped = seek_projection(seconds, duration);

    if let Some(ep) = &mut state.podcast.current {
        ep.position_seconds = clamped;
        ep.previous_position_for_tick = clamped;
    }

    vec![Effect::EmitCapabilityRequest(CapabilityRequest::Audio(
        AudioOp::Seek { seconds: clamped },
    ))]
}

/// Reduce `hl.audio.set_resume` — persist position immediately
/// (e.g. on app-resign-active). No capability request — purely a store write.
///
/// Returns `Effect::SavePodcastPosition` if state allows persistence.
pub(crate) fn reduce_action_set_resume(state: &mut AppState, seconds: f64) -> Vec<Effect> {
    let Some(ep) = state.podcast.current.as_ref() else {
        return vec![];
    };
    if !seconds.is_finite() || seconds < 0.0 {
        return vec![];
    }
    vec![Effect::SavePodcastPosition {
        guid: ep.guid.clone(),
        position_seconds: seconds,
        artifact: Box::new(ep.artifact.clone()),
    }]
}

// ─── Capability result handler ────────────────────────────────────────────────

/// Handle an `AudioResult` arriving via `CapabilityResult::Audio`.
///
/// Returns any effects the kernel needs to emit (position persistence, etc.).
/// Bounded cadence is enforced here: `AudioResult::Progress` is coalesced via
/// `tick_projection` — only whole-second crossings trigger state updates
/// (D8: no per-0.25 s FFI pushes propagate into the kernel's state machine).
pub(crate) fn reduce_capability_audio(state: &mut AppState, result: AudioResult) -> Vec<Effect> {
    match result {
        AudioResult::Progress {
            current_seconds,
            is_playing,
        } => reduce_audio_progress(state, current_seconds, is_playing),
        AudioResult::Loaded { duration_seconds } => {
            if let Some(ep) = &mut state.podcast.current {
                if duration_seconds > 0.0 && duration_seconds.is_finite() {
                    ep.duration_seconds = duration_seconds;
                }
                ep.is_playing = true;
            }
            vec![]
        }
        AudioResult::WaveformPeaks { url: _, buckets: _ } => {
            // Waveform peaks arrive here; Phase 5J will handle caching.
            // For 5H we accept them as a no-op (the snapshot carries no
            // waveform data yet — that's 5J scope).
            vec![]
        }
        AudioResult::Ended => {
            if let Some(ep) = &mut state.podcast.current {
                ep.is_playing = false;
                ep.position_seconds = ep.duration_seconds;
            }
            vec![]
        }
        AudioResult::Error(msg) => {
            tracing::warn!(error = %msg, "podcast audio capability error (D6)");
            if let Some(ep) = &mut state.podcast.current {
                ep.is_playing = false;
            }
            vec![]
        }
    }
}

/// Apply a `Progress` result via `tick_projection`.
///
/// Only emits `SavePodcastPosition` on whole-second boundaries AND when
/// `is_playing` AND the second is a multiple of `POSITION_PERSIST_INTERVAL_SECS`.
/// This is the kernel-side bounded-cadence enforcement (D8).
fn reduce_audio_progress(
    state: &mut AppState,
    current_seconds: f64,
    is_playing: bool,
) -> Vec<Effect> {
    let Some(ep) = state.podcast.current.as_mut() else {
        return vec![];
    };

    let current = normalize_time(current_seconds);
    let previous = ep.previous_position_for_tick;

    let (should_update, should_persist) = tick_projection(previous, current, is_playing);

    ep.previous_position_for_tick = current;
    if should_update {
        ep.position_seconds = current;
        ep.is_playing = is_playing;
    }

    if should_persist {
        let guid = ep.guid.clone();
        let artifact = ep.artifact.clone();
        return vec![Effect::SavePodcastPosition {
            guid,
            position_seconds: current,
            artifact: Box::new(artifact),
        }];
    }

    vec![]
}

// ─── Policy projections (ported from bespoke podcast_playback.rs) ─────────────

/// Clamp a seek target to `[0, duration]`.
pub(crate) fn seek_projection(target_seconds: f64, duration_seconds: f64) -> f64 {
    let target = if target_seconds.is_finite() {
        target_seconds
    } else {
        0.0
    };
    let duration = if duration_seconds.is_finite() && duration_seconds > 0.0 {
        duration_seconds
    } else {
        0.0
    };
    let bounded = if duration > 0.0 {
        target.min(duration)
    } else {
        target
    };
    bounded.max(0.0)
}

/// Compute tick decisions (bounded cadence, D8).
///
/// Returns `(should_update_now_playing, should_persist_position)`.
///
/// - `should_update_now_playing` is `true` when the whole-second part of
///   `current` differs from `previous` (≤ one update per second).
/// - `should_persist_position` is additionally gated on `is_playing` and on
///   `current_whole % POSITION_PERSIST_INTERVAL_SECS == 0`.
///
/// The 0.25 s native `addPeriodicTimeObserver` rate is therefore compressed
/// to at most one kernel state mutation per second (D8 — no polling).
pub(crate) fn tick_projection(
    previous_seconds: f64,
    current_seconds: f64,
    is_playing: bool,
) -> (bool, bool) {
    let prev_whole = previous_seconds as i64;
    let curr_whole = current_seconds as i64;
    let should_update = curr_whole != prev_whole;
    let should_persist = should_update
        && is_playing
        && curr_whole > 0
        && curr_whole % POSITION_PERSIST_INTERVAL_SECS == 0;
    (should_update, should_persist)
}

fn normalize_time(seconds: f64) -> f64 {
    if seconds.is_finite() && seconds >= 0.0 {
        seconds
    } else {
        0.0
    }
}

// ─── Snapshot projection ─────────────────────────────────────────────────────

/// Project `ViewId::PodcastListening` from `AppState::podcast`.
///
/// D1: raw fields only.  Swift formats timestamps, duration labels, progress
/// percentage, and chapter titles.
pub(crate) fn project_podcast_listening_snapshot(state: &AppState) -> Option<ViewSnapshot> {
    let ep = state.podcast.current.as_ref()?;
    Some(ViewSnapshot::PodcastListening(PodcastListeningSnapshot {
        guid: ep.guid.clone(),
        audio_url: ep.artifact.preview.audio_url.trim().to_string(),
        title: ep.artifact.preview.title.clone(),
        author: ep.artifact.preview.author.clone(),
        image_url: ep.artifact.preview.image.clone(),
        duration_seconds: ep.duration_seconds,
        position_seconds: ep.position_seconds,
        is_playing: ep.is_playing,
        clip_start_seconds: ep
            .clip_selection
            .as_ref()
            .and_then(|s| s.clip_start_seconds),
        clip_end_seconds: ep.clip_selection.as_ref().and_then(|s| s.clip_end_seconds),
        transcript_segments: ep
            .transcript_segments
            .iter()
            .map(|s| KernelTranscriptSegment {
                id: s.id.clone(),
                start: s.start,
                end: s.end,
                speaker: s.speaker.clone(),
                text: s.text.clone(),
            })
            .collect(),
        transcript_availability: match ep.transcript_availability {
            TranscriptAvailability::NotRequested => KernelTranscriptAvailability::NotRequested,
            TranscriptAvailability::Loading => KernelTranscriptAvailability::Loading,
            TranscriptAvailability::Available => KernelTranscriptAvailability::Available,
            TranscriptAvailability::Unavailable => KernelTranscriptAvailability::Unavailable,
        },
        clip_speaker: ep
            .clip_selection
            .as_ref()
            .map(|s| s.speaker.clone())
            .unwrap_or_default(),
        clip_selected_segment_ids: ep
            .clip_selection
            .as_ref()
            .map(|s| s.selected_segment_ids.clone())
            .unwrap_or_default(),
        // ── Phase 5J additions (append-only) ─────────────────────────────────
        clip_publish_phase: state.podcast.clip_publish_phase.clone(),
    }))
}

// ─── Phase 5I action reducers ─────────────────────────────────────────────────

/// Reduce `hl.transcript.load` — emit `Effect::FetchTranscript` for the
/// episode's transcript URL. No-op when no episode is loaded or the URL is
/// empty. Sets availability to `Loading`.
pub(crate) fn reduce_action_load_transcript(state: &mut AppState) -> Vec<Effect> {
    let Some(ep) = state.podcast.current.as_mut() else {
        return vec![];
    };
    let url = ep.artifact.preview.transcript_url.trim().to_string();
    if url.is_empty() {
        ep.transcript_availability = TranscriptAvailability::Unavailable;
        return vec![];
    }
    ep.transcript_availability = TranscriptAvailability::Loading;
    ep.transcript_segments.clear();
    vec![Effect::FetchTranscript { url }]
}

/// Handle `KernelEvent::TranscriptReady` — store parsed segments.
pub(crate) fn reduce_event_transcript_ready(
    state: &mut AppState,
    segments: Vec<TranscriptSegment>,
) -> Vec<Effect> {
    let Some(ep) = state.podcast.current.as_mut() else {
        return vec![];
    };
    if segments.is_empty() {
        ep.transcript_availability = TranscriptAvailability::Unavailable;
    } else {
        ep.transcript_availability = TranscriptAvailability::Available;
        ep.transcript_segments = segments;
    }
    vec![]
}

/// Handle `KernelEvent::TranscriptFetchFailed` — mark unavailable. D6.
pub(crate) fn reduce_event_transcript_failed(state: &mut AppState) -> Vec<Effect> {
    if let Some(ep) = state.podcast.current.as_mut() {
        ep.transcript_availability = TranscriptAvailability::Unavailable;
    }
    vec![]
}

/// Reduce `hl.audio.clip_mark_in` — set clip start to `current_time`,
/// clear end if it is now before start.
///
/// Clamps `current_time` to `≥ 0`; non-finite values are a no-op (D6).
pub(crate) fn reduce_action_clip_mark_in(state: &mut AppState, current_time: f64) -> Vec<Effect> {
    if !current_time.is_finite() {
        tracing::warn!("clip_mark_in: non-finite current_time — no-op (D6)");
        return vec![];
    }
    let Some(ep) = state.podcast.current.as_mut() else {
        return vec![];
    };
    let t = current_time.max(0.0);
    let sel = ep.clip_selection.get_or_insert_with(ClipSelection::default);
    sel.clip_start_seconds = Some(t);
    if sel.clip_end_seconds.map(|end| end < t).unwrap_or(false) {
        sel.clip_end_seconds = None;
    }
    vec![]
}

/// Reduce `hl.audio.clip_mark_out` — set clip end, clear start if reversed.
///
/// Clamps `current_time` to `≥ 0`; non-finite values are a no-op (D6).
pub(crate) fn reduce_action_clip_mark_out(state: &mut AppState, current_time: f64) -> Vec<Effect> {
    if !current_time.is_finite() {
        tracing::warn!("clip_mark_out: non-finite current_time — no-op (D6)");
        return vec![];
    }
    let Some(ep) = state.podcast.current.as_mut() else {
        return vec![];
    };
    let t = current_time.max(0.0);
    let sel = ep.clip_selection.get_or_insert_with(ClipSelection::default);
    sel.clip_end_seconds = Some(t);
    if sel
        .clip_start_seconds
        .map(|start| start > t)
        .unwrap_or(false)
    {
        sel.clip_start_seconds = None;
    }
    vec![]
}

/// Reduce `hl.audio.clip_extend_segment` — expand clip bounds to include
/// `segment_id` from the loaded transcript; deduplicate; adopt speaker if
/// the selection has none yet.
pub(crate) fn reduce_action_clip_extend_segment(
    state: &mut AppState,
    segment_id: String,
) -> Vec<Effect> {
    let Some(ep) = state.podcast.current.as_mut() else {
        return vec![];
    };
    let Some(seg) = ep
        .transcript_segments
        .iter()
        .find(|s| s.id == segment_id)
        .cloned()
    else {
        return vec![];
    };
    let sel = ep.clip_selection.get_or_insert_with(ClipSelection::default);
    sel.clip_start_seconds = Some(match sel.clip_start_seconds {
        Some(start) => start.min(seg.start),
        None => seg.start,
    });
    sel.clip_end_seconds = Some(match sel.clip_end_seconds {
        Some(end) => end.max(seg.end),
        None => seg.end,
    });
    if !sel.selected_segment_ids.iter().any(|id| id == &seg.id) {
        sel.selected_segment_ids.push(seg.id.clone());
    }
    if sel.speaker.is_empty() && !seg.speaker.is_empty() {
        sel.speaker = seg.speaker.clone();
    }
    vec![]
}

/// Reduce `hl.audio.clip_set_start` — set clip start, clamped to `≥ 0` and
/// so `start ≤ end − 0.05 s`. Non-finite values are a no-op (D6).
pub(crate) fn reduce_action_clip_set_start(state: &mut AppState, value: f64) -> Vec<Effect> {
    if !value.is_finite() {
        tracing::warn!("clip_set_start: non-finite value — no-op (D6)");
        return vec![];
    }
    let Some(ep) = state.podcast.current.as_mut() else {
        return vec![];
    };
    let sel = ep.clip_selection.get_or_insert_with(ClipSelection::default);
    let mut start = value.max(0.0);
    if let Some(end) = sel.clip_end_seconds {
        start = start.min((end - 0.05).max(0.0));
    }
    sel.clip_start_seconds = Some(start);
    vec![]
}

/// Reduce `hl.audio.clip_set_end` — set clip end, clamped to `[0, duration]`
/// and so `end ≥ start + 0.05 s`. Non-finite values are a no-op (D6).
pub(crate) fn reduce_action_clip_set_end(
    state: &mut AppState,
    value: f64,
    duration_seconds: f64,
) -> Vec<Effect> {
    if !value.is_finite() {
        tracing::warn!("clip_set_end: non-finite value — no-op (D6)");
        return vec![];
    }
    let Some(ep) = state.podcast.current.as_mut() else {
        return vec![];
    };
    let sel = ep.clip_selection.get_or_insert_with(ClipSelection::default);
    // Clamp to [0, duration]; if duration unknown, still floor at 0.
    let mut end = if duration_seconds > 0.0 {
        value.min(duration_seconds)
    } else {
        value
    }
    .max(0.0);
    if let Some(start) = sel.clip_start_seconds {
        end = end.max(start + 0.05);
    }
    sel.clip_end_seconds = Some(end);
    vec![]
}

/// Reduce `hl.audio.clip_clear` — reset the clip selection and clip-publish FSM.
pub(crate) fn reduce_action_clip_clear(state: &mut AppState) -> Vec<Effect> {
    if let Some(ep) = state.podcast.current.as_mut() {
        ep.clip_selection = None;
    }
    // Reset clip-publish FSM so a fresh selection starts from Idle.
    state.podcast.clip_publish_phase = KernelClipPublishPhase::Idle;
    state.podcast.pending_clip_publish_correlation_id = None;
    vec![]
}

// ─── Phase 5J: clip publish ────────────────────────────────────────────────────

/// Build the kind:9802 clip event content from the current clip selection.
///
/// When transcript segments cover the clip, their concatenated text becomes
/// the highlight content. When there are no segments (time-only clip), a
/// fallback content string `"{start:.3}s – {end:.3}s"` is produced using
/// `serde_json::json!` formatting (serde-safe, never `format!` for the JSON
/// template — D-rule).
fn clip_content(clip: &ClipSelection, segments: &[TranscriptSegment]) -> String {
    // Collect selected segments in chronological order (mirrors live clip_highlight_draft).
    let mut selected: Vec<&TranscriptSegment> = segments
        .iter()
        .filter(|s| clip.selected_segment_ids.iter().any(|id| id == &s.id))
        .collect();
    selected.sort_by(|a, b| {
        a.start
            .partial_cmp(&b.start)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });

    let text = selected
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    if !text.trim().is_empty() {
        return text;
    }

    // Time-only fallback — no transcript text available.
    // Format mirrors `build_clip_fallback_quote` in highlights.rs.
    let start = clip.clip_start_seconds.unwrap_or(0.0);
    let end = clip.clip_end_seconds.unwrap_or(0.0);
    format!("{:.3}s \u{2013} {:.3}s", start, end)
}

/// Reduce `hl.podcast.publish_clip` — build a kind:9802 event template from
/// the current `ClipSelection` + the playing episode's `ArtifactRecord`, and
/// emit `Effect::PublishClipWithCorrelation`.
///
/// ## Tag structure (port-exact from live `highlights.rs::build_highlight_event`)
///
/// ```text
/// ["i",        "podcast:item:guid:<guid>"]   ← NIP-73 external reference (mandatory)
/// ["start",    "<start:.3>"]                 ← clip start in seconds (3 d.p.)
/// ["end",      "<end:.3>"]                   ← clip end in seconds (3 d.p.)
/// ["speaker",  "<speaker>"]                  ← omitted if empty
/// ["segment",  "<seg_id>"]                   ← one tag per selected segment id
/// ["comment",  "<note>"]                     ← omitted if note is empty/blank
/// ```
///
/// ## No-op conditions (D6 — all malformed inputs are silent no-ops)
///
/// * No episode loaded (`AppState::podcast.current` is `None`).
/// * `clip_selection` is `None` on the loaded episode.
/// * Either `clip_start_seconds` or `clip_end_seconds` is `None`.
/// * `artifact.preview.podcast_item_guid` is empty (can't build i-tag).
/// * `clip_publish_phase` is already `Publishing` or `Done` (guard against
///   double-tap; caller should reset via `clip_clear` before re-publish).
///
/// ## Correlation id
///
/// A fresh id is minted here (never `None`), stored in
/// `AppState::podcast.pending_clip_publish_correlation_id`, and threaded
/// through to `ActorCommand::PublishRawEvent` via the 5G effect runner so
/// the `action_results` projection can route the verdict.
pub(crate) fn reduce_action_publish_clip(
    state: &mut AppState,
    artifact: ArtifactRecord,
    note: Option<String>,
) -> Vec<Effect> {
    // Guard: only Idle or Error are entry points for a fresh publish.
    match &state.podcast.clip_publish_phase {
        KernelClipPublishPhase::Publishing | KernelClipPublishPhase::Done => {
            tracing::warn!(
                "publish_clip: clip_publish_phase is {:?} — no-op (D6)",
                state.podcast.clip_publish_phase
            );
            return vec![];
        }
        _ => {}
    }

    let Some(ep) = state.podcast.current.as_ref() else {
        tracing::warn!("publish_clip: no episode loaded — no-op (D6)");
        return vec![];
    };

    let Some(clip) = ep.clip_selection.as_ref() else {
        tracing::warn!("publish_clip: no clip selection — no-op (D6)");
        return vec![];
    };

    let (Some(start), Some(end)) = (clip.clip_start_seconds, clip.clip_end_seconds) else {
        tracing::warn!("publish_clip: clip missing start or end — no-op (D6)");
        return vec![];
    };

    // Build NIP-73 i-tag value: "podcast:item:guid:<guid>".
    // Mirrors live `podcast_clip_reference` in podcast_transcript.rs:621-631.
    let guid = artifact.preview.podcast_item_guid.trim();
    if guid.is_empty() {
        tracing::warn!("publish_clip: empty podcast_item_guid — no-op (D6)");
        return vec![];
    }
    let i_tag_value = format!("podcast:item:guid:{guid}");

    // Build event content from selected transcript segments.
    let content = clip_content(clip, &ep.transcript_segments);

    // Build tags with serde_json::json! (D-rule: serde, not format! for JSON).
    let mut tags: Vec<serde_json::Value> = Vec::new();

    // Mandatory NIP-73 external reference tag.
    tags.push(serde_json::json!(["i", i_tag_value]));

    // Clip time tags — always present together (both Some was checked above).
    tags.push(serde_json::json!(["start", format!("{:.3}", start)]));
    tags.push(serde_json::json!(["end", format!("{:.3}", end)]));

    // Speaker tag — omit when empty (mirrors highlights.rs:1623-1629).
    let speaker = clip.speaker.trim();
    if !speaker.is_empty() {
        tags.push(serde_json::json!(["speaker", speaker]));
    }

    // One segment tag per selected transcript segment id (mirrors highlights.rs:1631-1640).
    for seg_id in &clip.selected_segment_ids {
        let seg_id = seg_id.trim();
        if !seg_id.is_empty() {
            tags.push(serde_json::json!(["segment", seg_id]));
        }
    }

    // Optional comment / note tag.
    let note_str = note.as_deref().unwrap_or("").trim().to_string();
    if !note_str.is_empty() {
        tags.push(serde_json::json!(["comment", note_str]));
    }

    let event_template = serde_json::json!({
        "kind": 9802,
        "content": content,
        "tags": tags,
    });

    let json = match serde_json::to_string(&event_template) {
        Ok(s) => s,
        Err(e) => {
            // Serialization failure must not panic (D6).
            tracing::warn!("publish_clip: serde_json::to_string failed: {e} — no-op");
            return vec![];
        }
    };

    // Mint correlation id and store it so apply_action_result_row can route.
    let correlation_id = new_correlation_id();
    state.podcast.pending_clip_publish_correlation_id = Some(correlation_id.clone());
    state.podcast.clip_publish_phase = KernelClipPublishPhase::Publishing;

    vec![Effect::PublishClipWithCorrelation {
        json,
        correlation_id,
    }]
}

/// Handle `KernelEvent::ClipPublishActionResult` — advance FSM to Done or Error.
///
/// Called when the `action_results` projection delivers a verdict for the
/// in-flight clip publish (matched by `pending_clip_publish_correlation_id`
/// in `apply_action_result_row` in `blossom.rs`). Clears the correlation id
/// to prevent stale re-delivery.
pub(crate) fn reduce_event_clip_publish_action_result(
    state: &mut AppState,
    success: bool,
    error: String,
) -> Vec<Effect> {
    // Clear the pending id whether success or failure (D6: stale re-delivery no-op).
    state.podcast.pending_clip_publish_correlation_id = None;

    state.podcast.clip_publish_phase = if success {
        KernelClipPublishPhase::Done
    } else {
        KernelClipPublishPhase::Error {
            message: if error.is_empty() {
                "publish failed".to_string()
            } else {
                error
            },
        }
    };
    vec![]
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::{AppAction, KernelEvent};
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::clock::{Clock as KClock, ManualClock};
    use crate::kernel::effect::Effect;
    use crate::kernel::models::{ArtifactPreview, Chapter};
    use crate::kernel::snapshot::ViewSnapshot;

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn make_state() -> AppState {
        AppState::default()
    }

    fn clock(secs: u64) -> ManualClock {
        let c = ManualClock::default();
        c.set(secs);
        c
    }

    fn step(state: &mut AppState, clk: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = KClock::now_unix_seconds(clk);
        reduce(state, cmd, now)
    }

    fn sample_artifact(audio_url: &str) -> ArtifactRecord {
        ArtifactRecord {
            preview: ArtifactPreview {
                id: "pod-1".into(),
                url: "https://podcast.example/episode".into(),
                title: "Test Episode".into(),
                author: "Host".into(),
                image: "https://podcast.example/art.jpg".into(),
                description: String::new(),
                source: "podcast".into(),
                domain: "podcast.example".into(),
                catalog_id: "podcast:item:guid:ep-guid".into(),
                catalog_kind: "podcast:item:guid".into(),
                podcast_guid: "feed-guid".into(),
                podcast_item_guid: "ep-guid".into(),
                podcast_show_title: "Test Show".into(),
                audio_url: audio_url.into(),
                audio_preview_url: String::new(),
                transcript_url: String::new(),
                feed_url: "https://podcast.example/feed.xml".into(),
                published_at: String::new(),
                duration_seconds: Some(3600),
                reference_tag_name: "i".into(),
                reference_tag_value: "podcast:item:guid:ep-guid".into(),
                reference_kind: "podcast:item:guid".into(),
                highlight_tag_name: "i".into(),
                highlight_tag_value: "podcast:item:guid:ep-guid".into(),
                highlight_reference_key: "i:podcast:item:guid:ep-guid".into(),
                chapters: Vec::<Chapter>::new(),
            },
            group_id: "group".into(),
            share_event_id: "share-1".into(),
            pubkey: "pubkey".into(),
            created_at: Some(10),
            note: String::new(),
        }
    }

    // 5H-T1: play_emits_audio_capability_request
    //
    // AppAction::AudioPlay must emit CapabilityRequest::Audio(AudioOp::Load) and
    // set state.podcast.current.
    #[test]
    fn play_emits_audio_capability_request() {
        let mut state = make_state();
        let clk = clock(1_000);

        let effects = step(
            &mut state,
            &clk,
            Cmd::Action(AppAction::AudioPlay {
                url: "https://cdn.example/ep.mp3".into(),
                guid: "ep-guid".into(),
                artifact_json: serde_json::to_string(&sample_artifact(
                    "https://cdn.example/ep.mp3",
                ))
                .unwrap(),
            }),
        );

        // Must emit exactly one EmitCapabilityRequest::Audio(Load).
        let audio_reqs: Vec<_> = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::EmitCapabilityRequest(crate::capabilities::CapabilityRequest::Audio(
                        AudioOp::Load { .. }
                    ))
                )
            })
            .collect();
        assert_eq!(audio_reqs.len(), 1, "must emit exactly one AudioOp::Load");

        // State must be set.
        assert!(state.podcast.current.is_some());
        let ep = state.podcast.current.as_ref().unwrap();
        assert_eq!(ep.guid, "ep-guid");
        assert!(!ep.is_playing);
    }

    // 5H-T2: play_with_saved_resume_position
    //
    // When the position store has a saved position for the guid, AudioPlay must
    // include resume_at_seconds in the Load op.
    #[test]
    fn play_with_saved_resume_position() {
        let mut state = make_state();
        let clk = clock(1_000);

        // Inject a saved position via KernelEvent.
        step(
            &mut state,
            &clk,
            Cmd::Event(KernelEvent::PodcastPositionLoaded {
                guid: "ep-guid".into(),
                position_seconds: 42.5,
            }),
        );

        let effects = step(
            &mut state,
            &clk,
            Cmd::Action(AppAction::AudioPlay {
                url: "https://cdn.example/ep.mp3".into(),
                guid: "ep-guid".into(),
                artifact_json: serde_json::to_string(&sample_artifact(
                    "https://cdn.example/ep.mp3",
                ))
                .unwrap(),
            }),
        );

        // The Load op must carry resume_at_seconds = 42.5.
        let has_resume = effects.iter().any(|e| {
            matches!(
                e,
                Effect::EmitCapabilityRequest(
                    crate::capabilities::CapabilityRequest::Audio(AudioOp::Load {
                        resume_at_seconds: Some(pos),
                        ..
                    })
                ) if (*pos - 42.5).abs() < f64::EPSILON
            )
        });
        assert!(has_resume, "Load op must carry the saved resume position");
    }

    // 5H-T3: progress_result_updates_position_bounded_cadence
    //
    // Multiple Progress results within the same second must NOT update state
    // (bounded cadence, D8). Only a whole-second crossing triggers an update.
    #[test]
    fn progress_result_updates_position_bounded_cadence() {
        let mut state = make_state();
        let clk = clock(1_000);

        // Load an episode.
        step(
            &mut state,
            &clk,
            Cmd::Action(AppAction::AudioPlay {
                url: "https://cdn.example/ep.mp3".into(),
                guid: "ep-guid".into(),
                artifact_json: serde_json::to_string(&sample_artifact(
                    "https://cdn.example/ep.mp3",
                ))
                .unwrap(),
            }),
        );

        // Simulate native reporting at 0.25 s intervals (sub-second — no state update).
        step(
            &mut state,
            &clk,
            Cmd::Event(KernelEvent::AudioCapabilityResult(AudioResult::Progress {
                current_seconds: 0.25,
                is_playing: true,
            })),
        );
        step(
            &mut state,
            &clk,
            Cmd::Event(KernelEvent::AudioCapabilityResult(AudioResult::Progress {
                current_seconds: 0.50,
                is_playing: true,
            })),
        );
        step(
            &mut state,
            &clk,
            Cmd::Event(KernelEvent::AudioCapabilityResult(AudioResult::Progress {
                current_seconds: 0.75,
                is_playing: true,
            })),
        );

        // position_seconds must still be 0.0 (no whole-second crossing yet).
        let pos = state.podcast.current.as_ref().unwrap().position_seconds;
        assert_eq!(pos, 0.0, "position must not update within same second");

        // Cross the 1 s boundary.
        step(
            &mut state,
            &clk,
            Cmd::Event(KernelEvent::AudioCapabilityResult(AudioResult::Progress {
                current_seconds: 1.0,
                is_playing: true,
            })),
        );
        let pos = state.podcast.current.as_ref().unwrap().position_seconds;
        assert_eq!(pos, 1.0, "position must update at whole-second boundary");
    }

    // 5H-T4: resume_position_persisted_device_local_not_published
    //
    // On a 5 s multiple while playing, Effect::SavePodcastPosition must be
    // emitted and NO publish/nostr effects must appear.
    #[test]
    fn resume_position_persisted_device_local_not_published() {
        let mut state = make_state();
        let clk = clock(1_000);

        step(
            &mut state,
            &clk,
            Cmd::Action(AppAction::AudioPlay {
                url: "https://cdn.example/ep.mp3".into(),
                guid: "ep-guid".into(),
                artifact_json: serde_json::to_string(&sample_artifact(
                    "https://cdn.example/ep.mp3",
                ))
                .unwrap(),
            }),
        );

        // Cross from 4.9 s → 5.0 s while playing → should persist.
        step(
            &mut state,
            &clk,
            Cmd::Event(KernelEvent::AudioCapabilityResult(AudioResult::Progress {
                current_seconds: 4.9,
                is_playing: true,
            })),
        );
        let effects = step(
            &mut state,
            &clk,
            Cmd::Event(KernelEvent::AudioCapabilityResult(AudioResult::Progress {
                current_seconds: 5.0,
                is_playing: true,
            })),
        );

        // Must emit SavePodcastPosition.
        let has_save = effects
            .iter()
            .any(|e| matches!(e, Effect::SavePodcastPosition { .. }));
        assert!(has_save, "must emit SavePodcastPosition at 5 s boundary");

        // Must NOT emit any nostr publish effects.
        let nostr: Vec<_> = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::PublishHighlightEvent { .. }
                        | Effect::DispatchNip29Action { .. }
                        | Effect::DispatchFollowAction { .. }
                        | Effect::DispatchShareToRoom { .. }
                        | Effect::DispatchBookmarkAction { .. }
                        | Effect::DispatchReactAction { .. }
                )
            })
            .collect();
        assert!(
            nostr.is_empty(),
            "resume position MUST NOT trigger nostr publish (hl-app-state-vs-nostr-facts): {nostr:?}"
        );
    }

    // 5H-T5: seek_updates_state
    //
    // AudioSeek must clamp the position and emit AudioOp::Seek.
    #[test]
    fn seek_updates_state() {
        let mut state = make_state();
        let clk = clock(1_000);

        step(
            &mut state,
            &clk,
            Cmd::Action(AppAction::AudioPlay {
                url: "https://cdn.example/ep.mp3".into(),
                guid: "ep-guid".into(),
                artifact_json: serde_json::to_string(&sample_artifact(
                    "https://cdn.example/ep.mp3",
                ))
                .unwrap(),
            }),
        );

        // Simulate loaded duration.
        step(
            &mut state,
            &clk,
            Cmd::Event(KernelEvent::AudioCapabilityResult(AudioResult::Loaded {
                duration_seconds: 60.0,
            })),
        );

        // Seek beyond duration → must clamp to 60.0.
        let effects = step(
            &mut state,
            &clk,
            Cmd::Action(AppAction::AudioSeek { seconds: 90.0 }),
        );

        let clamped = effects.iter().find_map(|e| {
            if let Effect::EmitCapabilityRequest(crate::capabilities::CapabilityRequest::Audio(
                AudioOp::Seek { seconds },
            )) = e
            {
                Some(*seconds)
            } else {
                None
            }
        });
        assert_eq!(clamped, Some(60.0), "seek must be clamped to duration");

        let pos = state.podcast.current.as_ref().unwrap().position_seconds;
        assert_eq!(pos, 60.0, "state position must reflect clamped seek");

        // Seek to negative → must clamp to 0.0.
        let effects = step(
            &mut state,
            &clk,
            Cmd::Action(AppAction::AudioSeek { seconds: -5.0 }),
        );
        let clamped = effects.iter().find_map(|e| {
            if let Effect::EmitCapabilityRequest(crate::capabilities::CapabilityRequest::Audio(
                AudioOp::Seek { seconds },
            )) = e
            {
                Some(*seconds)
            } else {
                None
            }
        });
        assert_eq!(clamped, Some(0.0), "seek must be clamped to 0 on negative");
    }

    // 5H-T6: playback_snapshot_raw_no_timestamp_formatting
    //
    // ViewSnapshot::PodcastListening must contain raw f64 fields; no formatted
    // duration strings or "X:XX" labels (D1).
    #[test]
    fn playback_snapshot_raw_no_timestamp_formatting() {
        let mut state = make_state();
        let clk = clock(1_000);

        step(
            &mut state,
            &clk,
            Cmd::Action(AppAction::AudioPlay {
                url: "https://cdn.example/ep.mp3".into(),
                guid: "ep-guid".into(),
                artifact_json: serde_json::to_string(&sample_artifact(
                    "https://cdn.example/ep.mp3",
                ))
                .unwrap(),
            }),
        );

        step(
            &mut state,
            &clk,
            Cmd::Event(KernelEvent::AudioCapabilityResult(AudioResult::Loaded {
                duration_seconds: 3600.0,
            })),
        );

        let snap = project_podcast_listening_snapshot(&state);
        assert!(snap.is_some(), "snapshot must be Some");

        if let Some(ViewSnapshot::PodcastListening(s)) = snap {
            // Raw f64 position — no "0:00" label.
            assert_eq!(s.position_seconds, 0.0);
            // Raw f64 duration — no "1:00:00" label.
            assert_eq!(s.duration_seconds, 3600.0);
            // Raw guid.
            assert_eq!(s.guid, "ep-guid");
            // is_playing raw bool.
            assert!(s.is_playing);
        } else {
            panic!("expected ViewSnapshot::PodcastListening");
        }
    }

    // 5H-T7: pause_clears_is_playing
    #[test]
    fn pause_clears_is_playing() {
        let mut state = make_state();
        let clk = clock(1_000);

        step(
            &mut state,
            &clk,
            Cmd::Action(AppAction::AudioPlay {
                url: "https://cdn.example/ep.mp3".into(),
                guid: "ep-guid".into(),
                artifact_json: serde_json::to_string(&sample_artifact(
                    "https://cdn.example/ep.mp3",
                ))
                .unwrap(),
            }),
        );
        step(
            &mut state,
            &clk,
            Cmd::Event(KernelEvent::AudioCapabilityResult(AudioResult::Loaded {
                duration_seconds: 60.0,
            })),
        );

        let effects = step(&mut state, &clk, Cmd::Action(AppAction::AudioPause));

        let has_pause = effects.iter().any(|e| {
            matches!(
                e,
                Effect::EmitCapabilityRequest(crate::capabilities::CapabilityRequest::Audio(
                    AudioOp::Pause
                ))
            )
        });
        assert!(has_pause, "pause must emit AudioOp::Pause");
        assert!(!state.podcast.current.as_ref().unwrap().is_playing);
    }

    // 5H-T8: no_polling_loop_d8
    //
    // A sequence of sub-second Progress events must NOT emit SavePodcastPosition
    // or other effects — kernel only acts on whole-second crossings (D8).
    #[test]
    fn no_polling_loop_d8() {
        let mut state = make_state();
        let clk = clock(1_000);

        step(
            &mut state,
            &clk,
            Cmd::Action(AppAction::AudioPlay {
                url: "https://cdn.example/ep.mp3".into(),
                guid: "ep-guid".into(),
                artifact_json: serde_json::to_string(&sample_artifact(
                    "https://cdn.example/ep.mp3",
                ))
                .unwrap(),
            }),
        );

        // Fire sub-second progress events (raw 0.25 s cadence from native).
        let mut total_effects: Vec<Effect> = Vec::new();
        for i in 1..=3u64 {
            let secs = i as f64 * 0.25;
            let mut effs = step(
                &mut state,
                &clk,
                Cmd::Event(KernelEvent::AudioCapabilityResult(AudioResult::Progress {
                    current_seconds: secs,
                    is_playing: true,
                })),
            );
            total_effects.append(&mut effs);
        }

        // None of these sub-second events should emit SavePodcastPosition
        // (no whole-second crossing has occurred).
        let saves: Vec<_> = total_effects
            .iter()
            .filter(|e| matches!(e, Effect::SavePodcastPosition { .. }))
            .collect();
        assert!(
            saves.is_empty(),
            "sub-second Progress events must NOT emit SavePodcastPosition (D8): {saves:?}"
        );
    }

    // 5H-T9: position_store_persists_and_reopens
    //
    // PodcastPositionStore::save writes to disk; a new store at the same path
    // reads it back.
    #[test]
    fn position_store_persists_and_reopens() {
        let dir = tempfile::tempdir().unwrap();

        #[derive(Debug)]
        struct FixedClock(u64);
        impl crate::clock::Clock for FixedClock {
            fn now_unix_seconds(&self) -> u64 {
                self.0
            }
        }

        let store = PodcastPositionStore::new_with_clock(dir.path(), Arc::new(FixedClock(10_000)));
        store
            .save(
                "ep-guid".into(),
                42.5,
                sample_artifact("https://cdn.example/ep.mp3"),
            )
            .unwrap();

        let reopened =
            PodcastPositionStore::new_with_clock(dir.path(), Arc::new(FixedClock(10_001)));
        let record = reopened.current().unwrap();
        assert_eq!(record.guid, "ep-guid");
        assert_eq!(record.position_seconds, 42.5);
        assert_eq!(record.artifact.preview.title, "Test Episode");
    }

    // 5H-T10: stale_position_hidden
    //
    // Records older than 7 days must be discarded (TTL enforcement).
    #[test]
    fn stale_position_hidden() {
        let dir = tempfile::tempdir().unwrap();

        #[derive(Debug)]
        struct FixedClock(u64);
        impl crate::clock::Clock for FixedClock {
            fn now_unix_seconds(&self) -> u64 {
                self.0
            }
        }

        let store = PodcastPositionStore::new_with_clock(dir.path(), Arc::new(FixedClock(1_000)));
        store
            .save(
                "ep-guid".into(),
                42.5,
                sample_artifact("https://cdn.example/ep.mp3"),
            )
            .unwrap();

        // Re-open with now = 1_000 + 7 days + 1 s → stale.
        let stale_now = 1_000 + POSITION_MAX_AGE_SECS + 1;
        let reopened =
            PodcastPositionStore::new_with_clock(dir.path(), Arc::new(FixedClock(stale_now)));
        assert!(
            reopened.current().is_none(),
            "stale position must be discarded"
        );
    }

    // ─── Phase 5J tests ───────────────────────────────────────────────────────

    use crate::kernel::action::AppActionEnvelope;
    use crate::kernel::snapshot::KernelClipPublishPhase;

    fn envelope(ns: &str, json: &str) -> Cmd {
        Cmd::ActionEnvelope(AppActionEnvelope {
            namespace: ns.to_string(),
            json: json.to_string(),
        })
    }

    /// Load an episode and mark a time-only clip.
    fn setup_loaded_episode_with_clip(state: &mut AppState, clk: &ManualClock) {
        step(
            state,
            clk,
            Cmd::Action(AppAction::AudioPlay {
                url: "https://cdn.example/ep.mp3".into(),
                guid: "ep-guid".into(),
                artifact_json: serde_json::to_string(&sample_artifact(
                    "https://cdn.example/ep.mp3",
                ))
                .unwrap(),
            }),
        );
        // Mark clip start at 10 s, end at 40 s.
        step(
            state,
            clk,
            envelope("hl.audio.clip_mark_in", r#"{"current_time":10.0}"#),
        );
        step(
            state,
            clk,
            envelope("hl.audio.clip_mark_out", r#"{"current_time":40.0}"#),
        );
    }

    // 5J-T1: publish_clip_builds_kind9802_with_nip73_itag
    //
    // `hl.podcast.publish_clip` must emit Effect::PublishClipWithCorrelation
    // whose JSON template is kind:9802 containing an ["i", "podcast:item:guid:ep-guid"]
    // tag, plus ["start", "10.000"], ["end", "40.000"] tags.
    #[test]
    fn publish_clip_builds_kind9802_with_nip73_itag() {
        let mut state = make_state();
        let clk = clock(1_000);

        setup_loaded_episode_with_clip(&mut state, &clk);

        let artifact_json =
            serde_json::to_string(&sample_artifact("https://cdn.example/ep.mp3")).unwrap();
        let payload = serde_json::json!({ "artifact_json": artifact_json }).to_string();

        let effects = step(
            &mut state,
            &clk,
            envelope("hl.podcast.publish_clip", &payload),
        );

        let clip_effect = effects
            .iter()
            .find(|e| matches!(e, Effect::PublishClipWithCorrelation { .. }))
            .expect("must emit PublishClipWithCorrelation");

        let (json, _cid) = if let Effect::PublishClipWithCorrelation {
            json,
            correlation_id,
        } = clip_effect
        {
            (json.clone(), correlation_id.clone())
        } else {
            panic!("wrong effect variant");
        };

        let template: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(template["kind"], 9802, "must be kind:9802");

        let tags = template["tags"].as_array().unwrap();

        // NIP-73 i-tag must be present and correct.
        let i_tag = tags.iter().find(|t| t[0] == "i").expect("must have i-tag");
        assert_eq!(
            i_tag[1].as_str().unwrap(),
            "podcast:item:guid:ep-guid",
            "i-tag value must match artifact podcast_item_guid"
        );

        // Start/end tags must be present with 3 decimal places.
        let start_tag = tags
            .iter()
            .find(|t| t[0] == "start")
            .expect("must have start tag");
        assert_eq!(start_tag[1].as_str().unwrap(), "10.000");

        let end_tag = tags
            .iter()
            .find(|t| t[0] == "end")
            .expect("must have end tag");
        assert_eq!(end_tag[1].as_str().unwrap(), "40.000");
    }

    // 5J-T2: publish_clip_uses_correlation_id
    //
    // The correlation_id in the emitted effect must be non-empty and must match
    // the one stored in AppState::podcast.pending_clip_publish_correlation_id.
    #[test]
    fn publish_clip_uses_correlation_id() {
        let mut state = make_state();
        let clk = clock(1_000);

        setup_loaded_episode_with_clip(&mut state, &clk);

        let artifact_json =
            serde_json::to_string(&sample_artifact("https://cdn.example/ep.mp3")).unwrap();
        let payload = serde_json::json!({ "artifact_json": artifact_json }).to_string();

        let effects = step(
            &mut state,
            &clk,
            envelope("hl.podcast.publish_clip", &payload),
        );

        let cid = if let Some(Effect::PublishClipWithCorrelation { correlation_id, .. }) = effects
            .iter()
            .find(|e| matches!(e, Effect::PublishClipWithCorrelation { .. }))
        {
            correlation_id.clone()
        } else {
            panic!("no PublishClipWithCorrelation effect");
        };

        assert!(!cid.is_empty(), "correlation_id must be non-empty");

        // Must be stored in state so apply_action_result_row can route the verdict.
        assert_eq!(
            state.podcast.pending_clip_publish_correlation_id.as_deref(),
            Some(cid.as_str()),
            "pending_clip_publish_correlation_id must match effect correlation_id"
        );

        // Phase must advance to Publishing.
        assert_eq!(
            state.podcast.clip_publish_phase,
            KernelClipPublishPhase::Publishing,
        );
    }

    // 5J-T3: clip_publish_advances_to_done_on_published_result
    //
    // Injecting KernelEvent::ClipPublishActionResult { success:true } must
    // drive clip_publish_phase → Done and clear pending_clip_publish_correlation_id.
    #[test]
    fn clip_publish_advances_to_done_on_published_result() {
        let mut state = make_state();
        let clk = clock(1_000);

        // Plant the correlation_id directly (no live nmp in tests).
        state.podcast.pending_clip_publish_correlation_id = Some("test-cid-done".to_string());
        state.podcast.clip_publish_phase = KernelClipPublishPhase::Publishing;

        step(
            &mut state,
            &clk,
            Cmd::Event(KernelEvent::ClipPublishActionResult {
                success: true,
                error: String::new(),
            }),
        );

        assert_eq!(
            state.podcast.clip_publish_phase,
            KernelClipPublishPhase::Done,
            "FSM must reach Done on success"
        );
        assert!(
            state.podcast.pending_clip_publish_correlation_id.is_none(),
            "correlation_id must be cleared on Done"
        );
    }

    // 5J-T4: clip_publish_error_on_failed_result
    //
    // A failed action result must drive clip_publish_phase → Error{message}.
    #[test]
    fn clip_publish_error_on_failed_result() {
        let mut state = make_state();
        let clk = clock(1_000);

        state.podcast.pending_clip_publish_correlation_id = Some("test-cid-err".to_string());
        state.podcast.clip_publish_phase = KernelClipPublishPhase::Publishing;

        step(
            &mut state,
            &clk,
            Cmd::Event(KernelEvent::ClipPublishActionResult {
                success: false,
                error: "relay rejected event".to_string(),
            }),
        );

        assert!(
            matches!(
                &state.podcast.clip_publish_phase,
                KernelClipPublishPhase::Error { message }
                    if message == "relay rejected event"
            ),
            "FSM must be Error with relay message: {:?}",
            state.podcast.clip_publish_phase
        );
        assert!(state.podcast.pending_clip_publish_correlation_id.is_none());
    }

    // 5J-T5: publish_clip_tag_fidelity_vs_live
    //
    // Verify tag structure matches live `build_highlight_event` in highlights.rs:
    //   - start / end use "{:.3}" (3 decimal places, rounded)
    //   - speaker tag is omitted when empty
    //   - speaker tag is present when non-empty
    //   - segment tags are emitted one per selected segment id
    //   - comment tag present only when note is non-empty
    #[test]
    fn publish_clip_tag_fidelity_vs_live() {
        let mut state = make_state();
        let clk = clock(1_000);

        setup_loaded_episode_with_clip(&mut state, &clk);

        // Add segments and extend clip to include them.
        step(
            &mut state,
            &clk,
            Cmd::Event(KernelEvent::TranscriptReady {
                segments: vec![
                    TranscriptSegment {
                        id: "seg-1".into(),
                        start: 10.0,
                        end: 25.0,
                        speaker: "Alice".into(),
                        text: "Hello world".into(),
                    },
                    TranscriptSegment {
                        id: "seg-2".into(),
                        start: 25.0,
                        end: 40.0,
                        speaker: "Alice".into(),
                        text: "Continued speech".into(),
                    },
                ],
            }),
        );
        step(
            &mut state,
            &clk,
            envelope("hl.audio.clip_extend_segment", r#"{"segment_id":"seg-1"}"#),
        );
        step(
            &mut state,
            &clk,
            envelope("hl.audio.clip_extend_segment", r#"{"segment_id":"seg-2"}"#),
        );

        let artifact_json =
            serde_json::to_string(&sample_artifact("https://cdn.example/ep.mp3")).unwrap();
        let payload =
            serde_json::json!({ "artifact_json": artifact_json, "note": "Great insight" })
                .to_string();

        let effects = step(
            &mut state,
            &clk,
            envelope("hl.podcast.publish_clip", &payload),
        );

        let json = if let Some(Effect::PublishClipWithCorrelation { json, .. }) = effects
            .iter()
            .find(|e| matches!(e, Effect::PublishClipWithCorrelation { .. }))
        {
            json.clone()
        } else {
            panic!("no PublishClipWithCorrelation effect");
        };

        let template: serde_json::Value = serde_json::from_str(&json).unwrap();
        let tags = template["tags"].as_array().unwrap();

        // i-tag.
        let i_tag = tags.iter().find(|t| t[0] == "i").unwrap();
        assert_eq!(i_tag[1].as_str().unwrap(), "podcast:item:guid:ep-guid");

        // start/end with 3 decimal places (live uses format!("{:.3}", start)).
        let start = tags.iter().find(|t| t[0] == "start").unwrap();
        assert_eq!(start[1].as_str().unwrap(), "10.000");
        let end = tags.iter().find(|t| t[0] == "end").unwrap();
        assert_eq!(end[1].as_str().unwrap(), "40.000");

        // speaker tag — present when segments carry a speaker.
        let speaker = tags.iter().find(|t| t[0] == "speaker").unwrap();
        assert_eq!(speaker[1].as_str().unwrap(), "Alice");

        // segment tags — one per selected segment id.
        let segments: Vec<_> = tags
            .iter()
            .filter(|t| t[0] == "segment")
            .map(|t| t[1].as_str().unwrap())
            .collect();
        assert!(
            segments.contains(&"seg-1") && segments.contains(&"seg-2"),
            "both segment ids must appear as segment tags: {segments:?}"
        );

        // comment tag.
        let comment = tags.iter().find(|t| t[0] == "comment").unwrap();
        assert_eq!(comment[1].as_str().unwrap(), "Great insight");
    }

    // 5J-T6: malformed_or_no_selection_noop
    //
    // Dispatching hl.podcast.publish_clip when no episode is loaded OR no
    // clip selection is set must be a silent no-op (D6): no effect emitted,
    // phase stays Idle.
    #[test]
    fn malformed_or_no_selection_noop() {
        let mut state = make_state();
        let clk = clock(1_000);

        let artifact_json =
            serde_json::to_string(&sample_artifact("https://cdn.example/ep.mp3")).unwrap();
        let payload = serde_json::json!({ "artifact_json": artifact_json }).to_string();

        // No episode loaded.
        let effects = step(
            &mut state,
            &clk,
            envelope("hl.podcast.publish_clip", &payload),
        );
        let has_publish = effects
            .iter()
            .any(|e| matches!(e, Effect::PublishClipWithCorrelation { .. }));
        assert!(!has_publish, "must be no-op when no episode loaded");
        assert_eq!(
            state.podcast.clip_publish_phase,
            KernelClipPublishPhase::Idle
        );

        // Episode loaded but no clip selection.
        step(
            &mut state,
            &clk,
            Cmd::Action(AppAction::AudioPlay {
                url: "https://cdn.example/ep.mp3".into(),
                guid: "ep-guid".into(),
                artifact_json: serde_json::to_string(&sample_artifact(
                    "https://cdn.example/ep.mp3",
                ))
                .unwrap(),
            }),
        );

        let effects = step(
            &mut state,
            &clk,
            envelope("hl.podcast.publish_clip", &payload),
        );
        let has_publish = effects
            .iter()
            .any(|e| matches!(e, Effect::PublishClipWithCorrelation { .. }));
        assert!(!has_publish, "must be no-op when no clip selection");
        assert_eq!(
            state.podcast.clip_publish_phase,
            KernelClipPublishPhase::Idle
        );
    }

    // 5J-T7: clip_clear_resets_publish_phase
    //
    // `hl.audio.clip_clear` must reset clip_publish_phase → Idle and
    // clear pending_clip_publish_correlation_id so a fresh clip can start
    // from Idle without the previous publish FSM polluting state.
    #[test]
    fn clip_clear_resets_publish_phase() {
        let mut state = make_state();
        let clk = clock(1_000);

        // Plant a Done phase as if a prior publish completed.
        state.podcast.clip_publish_phase = KernelClipPublishPhase::Done;
        state.podcast.pending_clip_publish_correlation_id = Some("old-cid".to_string());

        step(&mut state, &clk, envelope("hl.audio.clip_clear", "{}"));

        assert_eq!(
            state.podcast.clip_publish_phase,
            KernelClipPublishPhase::Idle,
            "clip_clear must reset phase to Idle"
        );
        assert!(
            state.podcast.pending_clip_publish_correlation_id.is_none(),
            "clip_clear must clear pending_clip_publish_correlation_id"
        );
    }
}
