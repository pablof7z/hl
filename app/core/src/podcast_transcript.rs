//! Podcast transcript loading/parsing and podcast media byte fetches.
//!
//! Native shells own audio playback and platform media surfaces. The shared
//! core owns reusable transcript interpretation and bounded HTTP loading so
//! iOS and Android render the same segment model.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::OnceLock;
use std::time::Duration;

use ::url::Url;
use futures::StreamExt;
use regex::Regex;

use crate::errors::CoreError;
use crate::models::{ArtifactRecord, CommunitySummary, HighlightDraft, HighlightRecord};

const MAX_TRANSCRIPT_BYTES: usize = 8 * 1024 * 1024;
const MAX_ARTWORK_BYTES: usize = 10 * 1024 * 1024;
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct TranscriptSegment {
    pub id: String,
    pub start: f64,
    pub end: f64,
    pub speaker: String,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PodcastTranscriptAvailability {
    Loading,
    Available,
    Unavailable,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastTranscriptLoadSnapshot {
    pub segments: Vec<TranscriptSegment>,
    pub availability: PodcastTranscriptAvailability,
    pub error: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastTranscriptLoadApplyInput {
    pub snapshot: PodcastTranscriptLoadSnapshot,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastTranscriptLoadApplyProjection {
    pub segments: Vec<TranscriptSegment>,
    pub availability: PodcastTranscriptAvailability,
    pub should_log_error: bool,
    pub log_message: String,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PodcastClipSelection {
    pub clip_start_seconds: Option<f64>,
    pub clip_end_seconds: Option<f64>,
    pub speaker: String,
    pub selected_segment_ids: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastClipComposerProjection {
    pub matching_segments: Vec<TranscriptSegment>,
    pub excerpt: String,
    pub speaker: String,
    pub duration_seconds: f64,
    pub clip_start_label: String,
    pub clip_end_label: String,
    pub duration_label: String,
    pub subtitle_label: String,
    pub time_only_message: String,
    pub has_transcript: bool,
    pub can_publish: bool,
    pub community_name: String,
    pub community_display_name: String,
    pub has_community: bool,
    pub selected_segment_ids: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastClipComposerInput {
    pub segments: Vec<TranscriptSegment>,
    pub transcript_available: bool,
    pub clip_start_seconds: f64,
    pub clip_end_seconds: f64,
    pub duration_seconds: f64,
    pub selected_group_id: Option<String>,
    pub joined_communities: Vec<CommunitySummary>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastClipPublishInput {
    pub artifact: ArtifactRecord,
    pub target_group_id: String,
    pub note: String,
    pub segments: Vec<TranscriptSegment>,
    pub selected_segment_ids: Vec<String>,
    pub clip_start_seconds: Option<f64>,
    pub clip_end_seconds: Option<f64>,
    pub clip_speaker: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastClipComposerPublishInput {
    pub artifact: ArtifactRecord,
    pub segments: Vec<TranscriptSegment>,
    pub transcript_available: bool,
    pub context: String,
    pub clip_start_seconds: f64,
    pub clip_end_seconds: f64,
    pub target_group_id: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastClipPublishSnapshot {
    pub highlight: Option<HighlightRecord>,
    pub error: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastClipPublishResultInput {
    pub snapshot: PodcastClipPublishSnapshot,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastClipPublishResultProjection {
    pub did_publish: bool,
    pub error_message: Option<String>,
    pub share_toast: Option<String>,
    pub should_dismiss: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PodcastTimelineRowKind {
    Chapter,
    Clip,
    Transcript,
    WaveformTick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PodcastTimelineRowState {
    Played,
    Active,
    Future,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastTimelineRow {
    pub id: String,
    pub t: f64,
    pub timestamp_label: String,
    pub kind: PodcastTimelineRowKind,
    pub state: PodcastTimelineRowState,
    pub chapter_title: String,
    pub clip: Option<HighlightRecord>,
    pub clip_range_label: String,
    pub transcript_segment: Option<TranscriptSegment>,
    pub waveform_window_seconds: f64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastListeningProjectionInput {
    pub artifact: Option<ArtifactRecord>,
    pub clips: Vec<HighlightRecord>,
    pub transcript_segments: Vec<TranscriptSegment>,
    pub transcript_available: bool,
    pub show_transcript: bool,
    pub show_chapters: bool,
    pub show_clips: bool,
    pub player_duration_seconds: f64,
    pub current_time_seconds: f64,
    pub waveform_tick_window_seconds: f64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastListeningProjection {
    pub show_title: String,
    pub episode_title: String,
    pub image_url: String,
    pub episode_meta: String,
    pub has_chapters: bool,
    pub clip_count: u64,
    pub rows: Vec<PodcastTimelineRow>,
    pub active_row_id: Option<String>,
    pub current_speaker_or_timestamp: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastListeningClipsSnapshot {
    pub clips: Vec<HighlightRecord>,
    pub error: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastNowPlayingProjectionInput {
    pub artifact: ArtifactRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PodcastNowPlayingProjection {
    pub show_title: String,
    pub episode_title: String,
    pub image_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PodcastClipReference {
    pub tag_name: String,
    pub tag_value: String,
    pub limit: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Vtt,
    Srt,
    Json,
    Unknown,
}

pub async fn fetch_transcript(url: &str) -> Result<Vec<TranscriptSegment>, CoreError> {
    let url = validate_http_url(url)?;
    let response = http_client()
        .get(url.clone())
        .header(
            reqwest::header::ACCEPT,
            "text/vtt,application/x-subrip,application/json,text/plain;q=0.8,*/*;q=0.4",
        )
        .send()
        .await
        .map_err(|e| CoreError::Network(format!("transcript fetch: {e}")))?;

    let status = response.status();
    if !status.is_success() {
        return Err(CoreError::Network(format!(
            "transcript fetch failed ({})",
            status.as_u16()
        )));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let bytes = read_limited(response, MAX_TRANSCRIPT_BYTES, "transcript").await?;
    let file_extension = url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .and_then(|last| last.rsplit_once('.').map(|(_, ext)| ext.to_string()));

    Ok(parse_transcript_bytes(
        &bytes,
        content_type.as_deref(),
        file_extension.as_deref(),
    ))
}

pub fn transcript_load_snapshot(
    result: Result<Vec<TranscriptSegment>, CoreError>,
) -> PodcastTranscriptLoadSnapshot {
    match result {
        Ok(segments) => {
            let availability = if segments.is_empty() {
                PodcastTranscriptAvailability::Unavailable
            } else {
                PodcastTranscriptAvailability::Available
            };
            PodcastTranscriptLoadSnapshot {
                segments,
                availability,
                error: String::new(),
            }
        }
        Err(error) => PodcastTranscriptLoadSnapshot {
            segments: Vec::new(),
            availability: PodcastTranscriptAvailability::Unavailable,
            error: error.to_string(),
        },
    }
}

#[uniffi::export]
pub fn transcript_load_apply_projection(
    input: PodcastTranscriptLoadApplyInput,
) -> PodcastTranscriptLoadApplyProjection {
    let error = input.snapshot.error.trim().to_string();
    PodcastTranscriptLoadApplyProjection {
        segments: input.snapshot.segments,
        availability: input.snapshot.availability,
        should_log_error: !error.is_empty(),
        log_message: if error.is_empty() {
            String::new()
        } else {
            format!("transcript load failed: {error}")
        },
    }
}

pub fn clip_publish_snapshot(
    result: Result<HighlightRecord, CoreError>,
) -> PodcastClipPublishSnapshot {
    match result {
        Ok(highlight) => PodcastClipPublishSnapshot {
            highlight: Some(highlight),
            error: String::new(),
        },
        Err(error) => PodcastClipPublishSnapshot {
            highlight: None,
            error: error.to_string(),
        },
    }
}

#[uniffi::export]
pub fn clip_publish_result_projection(
    input: PodcastClipPublishResultInput,
) -> PodcastClipPublishResultProjection {
    let error = input.snapshot.error.trim().to_string();
    if error.is_empty() {
        PodcastClipPublishResultProjection {
            did_publish: true,
            error_message: None,
            share_toast: Some("Clip shared".into()),
            should_dismiss: true,
        }
    } else {
        PodcastClipPublishResultProjection {
            did_publish: false,
            error_message: Some(error),
            share_toast: None,
            should_dismiss: false,
        }
    }
}

pub async fn download_artwork(url: &str) -> Result<Vec<u8>, CoreError> {
    let url = validate_http_url(url)?;
    let response = http_client()
        .get(url)
        .header(reqwest::header::ACCEPT, "image/*,*/*;q=0.3")
        .send()
        .await
        .map_err(|e| CoreError::Network(format!("podcast artwork fetch: {e}")))?;

    let status = response.status();
    if !status.is_success() {
        return Err(CoreError::Network(format!(
            "podcast artwork fetch failed ({})",
            status.as_u16()
        )));
    }

    read_limited(response, MAX_ARTWORK_BYTES, "podcast artwork").await
}

pub fn parse_transcript_bytes(
    bytes: &[u8],
    content_type: Option<&str>,
    file_extension: Option<&str>,
) -> Vec<TranscriptSegment> {
    let Ok(source) = std::str::from_utf8(bytes) else {
        return Vec::new();
    };

    match detect_format(source, content_type, file_extension) {
        Format::Json => parse_json(source),
        Format::Vtt => parse_vtt(source),
        Format::Srt => parse_srt(source),
        Format::Unknown => Vec::new(),
    }
}

/// Build the highlight draft for a podcast clip from the visible transcript
/// selection. Rust owns segment matching, chronological ordering, quote
/// assembly, and the protocol payload passed to highlight publishing.
pub fn clip_highlight_draft(
    segments: &[TranscriptSegment],
    selected_segment_ids: &[String],
    note: String,
    clip_start_seconds: Option<f64>,
    clip_end_seconds: Option<f64>,
    clip_speaker: String,
) -> HighlightDraft {
    let selected_ids: HashSet<&str> = selected_segment_ids
        .iter()
        .map(String::as_str)
        .filter(|id| !id.is_empty())
        .collect();
    let mut selected: Vec<&TranscriptSegment> = segments
        .iter()
        .filter(|segment| selected_ids.contains(segment.id.as_str()))
        .collect();
    selected.sort_by(|a, b| {
        a.start
            .partial_cmp(&b.start)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });

    HighlightDraft {
        quote: selected
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        context: String::new(),
        note,
        clip_start_seconds,
        clip_end_seconds,
        clip_speaker,
        clip_transcript_segment_ids: selected
            .iter()
            .map(|segment| segment.id.clone())
            .collect::<Vec<_>>(),
        image: None,
    }
}

#[uniffi::export]
pub fn clip_composer_projection(input: PodcastClipComposerInput) -> PodcastClipComposerProjection {
    let matching_segments = matching_clip_segments(
        &input.segments,
        input.transcript_available,
        input.clip_start_seconds,
        input.clip_end_seconds,
    );
    let excerpt = matching_segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let speaker = matching_segments
        .iter()
        .find(|segment| !segment.speaker.is_empty())
        .map(|segment| segment.speaker.clone())
        .unwrap_or_default();
    let selected_segment_ids = matching_segments
        .iter()
        .map(|segment| segment.id.clone())
        .collect::<Vec<_>>();
    let selected_group_id = input
        .selected_group_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    let community_name = selected_group_id
        .map(|id| community_name_for_id(id, &input.joined_communities))
        .unwrap_or_default();
    let has_community = selected_group_id.is_some();

    PodcastClipComposerProjection {
        matching_segments,
        excerpt,
        speaker,
        duration_seconds: input.clip_end_seconds - input.clip_start_seconds,
        clip_start_label: format_timestamp(input.clip_start_seconds),
        clip_end_label: format_timestamp(input.clip_end_seconds),
        duration_label: clip_duration_label(input.clip_end_seconds - input.clip_start_seconds),
        subtitle_label: clip_composer_subtitle_label(
            input.clip_end_seconds - input.clip_start_seconds,
            !selected_segment_ids.is_empty(),
        ),
        time_only_message: clip_composer_time_only_message(
            input.clip_end_seconds - input.clip_start_seconds,
        ),
        has_transcript: !selected_segment_ids.is_empty(),
        can_publish: input.clip_start_seconds.is_finite()
            && input.clip_end_seconds.is_finite()
            && input.duration_seconds.is_finite()
            && input.clip_start_seconds >= 0.0
            && input.clip_end_seconds <= input.duration_seconds
            && input.clip_start_seconds + 5.0 <= input.clip_end_seconds,
        community_name: community_name.clone(),
        community_display_name: if has_community {
            community_name
        } else {
            "Personal".to_string()
        },
        has_community,
        selected_segment_ids,
    }
}

pub fn clip_composer_highlight_draft(
    segments: &[TranscriptSegment],
    transcript_available: bool,
    context: String,
    clip_start_seconds: f64,
    clip_end_seconds: f64,
) -> HighlightDraft {
    let matching_segments = matching_clip_segments(
        segments,
        transcript_available,
        clip_start_seconds,
        clip_end_seconds,
    );

    HighlightDraft {
        quote: matching_segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        context,
        note: String::new(),
        clip_start_seconds: Some(clip_start_seconds),
        clip_end_seconds: Some(clip_end_seconds),
        clip_speaker: matching_segments
            .iter()
            .find(|segment| !segment.speaker.is_empty())
            .map(|segment| segment.speaker.clone())
            .unwrap_or_default(),
        clip_transcript_segment_ids: matching_segments
            .iter()
            .map(|segment| segment.id.clone())
            .collect::<Vec<_>>(),
        image: None,
    }
}

#[uniffi::export]
pub fn listening_projection(input: PodcastListeningProjectionInput) -> PodcastListeningProjection {
    let clip_count = input.clips.len() as u64;
    let now_playing = input
        .artifact
        .as_ref()
        .map(now_playing_projection_for_artifact)
        .unwrap_or_else(default_now_playing_projection);
    let has_chapters = input
        .artifact
        .as_ref()
        .map(|artifact| !artifact.preview.chapters.is_empty())
        .unwrap_or(false);
    let episode_meta = episode_meta_label(
        input
            .artifact
            .as_ref()
            .and_then(|artifact| artifact.preview.duration_seconds),
        input.player_duration_seconds,
        clip_count,
    );

    let mut rows = listening_rows(&input);
    let active_row_id = rows
        .iter()
        .rfind(|row| row.t <= input.current_time_seconds)
        .map(|row| row.id.clone());
    for row in &mut rows {
        row.state = if row.t > input.current_time_seconds {
            PodcastTimelineRowState::Future
        } else if active_row_id.as_deref() == Some(row.id.as_str()) {
            PodcastTimelineRowState::Active
        } else {
            PodcastTimelineRowState::Played
        };
    }

    PodcastListeningProjection {
        show_title: now_playing.show_title,
        episode_title: now_playing.episode_title,
        image_url: now_playing.image_url,
        episode_meta,
        has_chapters,
        clip_count,
        rows,
        active_row_id,
        current_speaker_or_timestamp: current_speaker_or_timestamp(
            &input.transcript_segments,
            input.transcript_available,
            input.current_time_seconds,
        ),
    }
}

pub fn listening_clips_snapshot(
    clips: Vec<HighlightRecord>,
    error: impl ToString,
) -> PodcastListeningClipsSnapshot {
    PodcastListeningClipsSnapshot {
        clips,
        error: error.to_string(),
    }
}

/// Project episode metadata for mini-player and system Now Playing surfaces.
/// Rust owns episode title/show fallback parity across platform shells.
#[uniffi::export]
pub fn now_playing_projection(
    input: PodcastNowPlayingProjectionInput,
) -> PodcastNowPlayingProjection {
    now_playing_projection_for_artifact(&input.artifact)
}

fn now_playing_projection_for_artifact(artifact: &ArtifactRecord) -> PodcastNowPlayingProjection {
    PodcastNowPlayingProjection {
        show_title: if artifact.preview.podcast_show_title.is_empty() {
            artifact.preview.author.clone()
        } else {
            artifact.preview.podcast_show_title.clone()
        },
        episode_title: if artifact.preview.title.is_empty() {
            "Untitled episode".to_string()
        } else {
            artifact.preview.title.clone()
        },
        image_url: artifact.preview.image.clone(),
    }
}

fn default_now_playing_projection() -> PodcastNowPlayingProjection {
    PodcastNowPlayingProjection {
        show_title: String::new(),
        episode_title: "Untitled episode".to_string(),
        image_url: String::new(),
    }
}

pub(crate) fn podcast_clip_reference(artifact: &ArtifactRecord) -> PodcastClipReference {
    let guid = artifact.preview.podcast_item_guid.as_str();
    PodcastClipReference {
        tag_name: "i".into(),
        tag_value: if guid.is_empty() {
            artifact.share_event_id.clone()
        } else {
            format!("podcast:item:guid:{guid}")
        },
        limit: 128,
    }
}

fn listening_rows(input: &PodcastListeningProjectionInput) -> Vec<PodcastTimelineRow> {
    let mut rows = Vec::new();
    let window_seconds = if input.waveform_tick_window_seconds.is_finite()
        && input.waveform_tick_window_seconds > 0.0
    {
        input.waveform_tick_window_seconds
    } else {
        30.0
    };

    if input.show_clips {
        let mut clips = input.clips.clone();
        clips.sort_by(|a, b| {
            clip_start(a)
                .partial_cmp(&clip_start(b))
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.event_id.cmp(&b.event_id))
        });
        for clip in clips {
            let t = clip_start(&clip);
            rows.push(PodcastTimelineRow {
                id: format!("clip-{}", clip.event_id),
                t,
                timestamp_label: format_timestamp(t),
                kind: PodcastTimelineRowKind::Clip,
                state: PodcastTimelineRowState::Future,
                chapter_title: String::new(),
                clip_range_label: clip_range_label(&clip),
                clip: Some(clip),
                transcript_segment: None,
                waveform_window_seconds: window_seconds,
            });
        }
    }

    if input.show_chapters {
        if let Some(artifact) = &input.artifact {
            for chapter in &artifact.preview.chapters {
                rows.push(PodcastTimelineRow {
                    id: format!("chapter-{}", swift_double_id(chapter.start_seconds)),
                    t: chapter.start_seconds,
                    timestamp_label: format_timestamp(chapter.start_seconds),
                    kind: PodcastTimelineRowKind::Chapter,
                    state: PodcastTimelineRowState::Future,
                    chapter_title: chapter.title.clone(),
                    clip_range_label: String::new(),
                    clip: None,
                    transcript_segment: None,
                    waveform_window_seconds: window_seconds,
                });
            }
        }
    }

    if input.show_transcript && input.transcript_available {
        for segment in &input.transcript_segments {
            rows.push(PodcastTimelineRow {
                id: format!("transcript-{}", segment.id),
                t: segment.start,
                timestamp_label: format_timestamp(segment.start),
                kind: PodcastTimelineRowKind::Transcript,
                state: PodcastTimelineRowState::Future,
                chapter_title: String::new(),
                clip_range_label: String::new(),
                clip: None,
                transcript_segment: Some(segment.clone()),
                waveform_window_seconds: window_seconds,
            });
        }
    } else {
        let occupied_times = rows.iter().map(|row| row.t).collect::<Vec<_>>();
        let total_duration = if input.player_duration_seconds > 0.0 {
            input.player_duration_seconds
        } else {
            3600.0
        };
        let mut t = 0.0;
        while t < total_duration {
            let has_neighbor = occupied_times
                .iter()
                .any(|occupied| (*occupied - t).abs() < 8.0);
            if !has_neighbor {
                rows.push(PodcastTimelineRow {
                    id: format!("waveform-{}", swift_double_id(t)),
                    t,
                    timestamp_label: format_timestamp(t),
                    kind: PodcastTimelineRowKind::WaveformTick,
                    state: PodcastTimelineRowState::Future,
                    chapter_title: String::new(),
                    clip_range_label: String::new(),
                    clip: None,
                    transcript_segment: None,
                    waveform_window_seconds: window_seconds,
                });
            }
            t += window_seconds;
        }
    }

    rows.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(Ordering::Equal));
    rows
}

fn clip_start(clip: &HighlightRecord) -> f64 {
    clip.clip_start_seconds.unwrap_or(0.0)
}

fn episode_meta_label(
    preview_duration_seconds: Option<i64>,
    player_duration_seconds: f64,
    clip_count: u64,
) -> String {
    let mut parts = Vec::new();
    if let Some(duration) = preview_duration_seconds.filter(|duration| *duration > 0) {
        parts.push(format_duration_minutes(duration));
    } else if player_duration_seconds > 0.0 {
        parts.push(format_duration_minutes(player_duration_seconds as i64));
    }
    if clip_count > 0 {
        parts.push(format!(
            "{} clip{}",
            clip_count,
            if clip_count == 1 { "" } else { "s" }
        ));
    }
    parts.join(" · ")
}

fn format_duration_minutes(total_seconds: i64) -> String {
    let h = total_seconds / 3600;
    let m = (total_seconds % 3600) / 60;
    if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}

fn current_speaker_or_timestamp(
    segments: &[TranscriptSegment],
    transcript_available: bool,
    current_time_seconds: f64,
) -> String {
    if transcript_available {
        if let Some(segment) = segments
            .iter()
            .rfind(|segment| segment.start <= current_time_seconds)
            .filter(|segment| !segment.speaker.is_empty())
        {
            return segment.speaker.clone();
        }
    }
    format_timestamp(current_time_seconds)
}

fn format_timestamp(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "0:00".into();
    }
    let total = seconds as i64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

fn clip_range_label(clip: &HighlightRecord) -> String {
    let start = clip
        .clip_start_seconds
        .and_then(format_optional_clip_timestamp);
    let end = clip
        .clip_end_seconds
        .and_then(format_optional_clip_timestamp);
    match (start, end) {
        (Some(start), Some(end)) => format!("{start}–{end}"),
        (Some(start), None) => start,
        _ => "—".into(),
    }
}

fn clip_duration_label(duration_seconds: f64) -> String {
    let total = duration_seconds as i64;
    let minutes = total / 60;
    let seconds = total % 60;
    if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}

fn clip_composer_subtitle_label(duration_seconds: f64, has_transcript: bool) -> String {
    let suffix = if has_transcript {
        "with transcript"
    } else {
        "time-only clip"
    };
    format!("{} · {suffix}", clip_duration_label(duration_seconds))
}

fn clip_composer_time_only_message(duration_seconds: f64) -> String {
    format!(
        "Time-only clip · {}. Add a note for the room below.",
        clip_duration_label(duration_seconds)
    )
}

fn format_optional_clip_timestamp(seconds: f64) -> Option<String> {
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    Some(format_timestamp(seconds.round()))
}

fn swift_double_id(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        format!("{value}")
    }
}

fn matching_clip_segments(
    segments: &[TranscriptSegment],
    transcript_available: bool,
    clip_start_seconds: f64,
    clip_end_seconds: f64,
) -> Vec<TranscriptSegment> {
    if !transcript_available {
        return Vec::new();
    }

    segments
        .iter()
        .filter(|segment| segment.start < clip_end_seconds && segment.end > clip_start_seconds)
        .cloned()
        .collect()
}

fn community_name_for_id(id: &str, joined_communities: &[CommunitySummary]) -> String {
    joined_communities
        .iter()
        .find(|community| community.id == id)
        .map(|community| {
            if community.name.is_empty() {
                id.to_string()
            } else {
                community.name.clone()
            }
        })
        .unwrap_or_else(|| id.to_string())
}

#[uniffi::export]
pub fn clear_clip_selection() -> PodcastClipSelection {
    PodcastClipSelection {
        clip_start_seconds: None,
        clip_end_seconds: None,
        speaker: String::new(),
        selected_segment_ids: Vec::new(),
    }
}

pub fn mark_clip_in(selection: &PodcastClipSelection, current_time: f64) -> PodcastClipSelection {
    let mut next = selection.clone();
    next.clip_start_seconds = Some(current_time);
    if next
        .clip_end_seconds
        .map(|end| end < current_time)
        .unwrap_or(false)
    {
        next.clip_end_seconds = None;
    }
    next
}

pub fn mark_clip_out(selection: &PodcastClipSelection, current_time: f64) -> PodcastClipSelection {
    let mut next = selection.clone();
    next.clip_end_seconds = Some(current_time);
    if next
        .clip_start_seconds
        .map(|start| start > current_time)
        .unwrap_or(false)
    {
        next.clip_start_seconds = None;
    }
    next
}

pub fn extend_clip_to_segment(
    selection: &PodcastClipSelection,
    segment: &TranscriptSegment,
) -> PodcastClipSelection {
    let mut next = selection.clone();
    next.clip_start_seconds = Some(match next.clip_start_seconds {
        Some(start) => start.min(segment.start),
        None => segment.start,
    });
    next.clip_end_seconds = Some(match next.clip_end_seconds {
        Some(end) => end.max(segment.end),
        None => segment.end,
    });
    if !next.selected_segment_ids.iter().any(|id| id == &segment.id) {
        next.selected_segment_ids.push(segment.id.clone());
    }
    if next.speaker.is_empty() && !segment.speaker.is_empty() {
        next.speaker = segment.speaker.clone();
    }
    next
}

pub fn set_clip_start(selection: &PodcastClipSelection, value: f64) -> PodcastClipSelection {
    let mut next = selection.clone();
    let mut start = value.max(0.0);
    if let Some(end) = next.clip_end_seconds {
        start = start.min((end - 0.05).max(0.0));
    }
    next.clip_start_seconds = Some(start);
    next
}

pub fn set_clip_end(
    selection: &PodcastClipSelection,
    value: f64,
    duration_seconds: f64,
) -> PodcastClipSelection {
    let mut next = selection.clone();
    let mut end = if duration_seconds > 0.0 {
        value.min(duration_seconds)
    } else {
        value
    };
    if let Some(start) = next.clip_start_seconds {
        end = end.max(start + 0.05);
    }
    next.clip_end_seconds = Some(end);
    next
}

/// UniFFI free-function wrappers for clip-selection mutations.
/// The underlying functions take `&PodcastClipSelection`; these take owned
/// values so UniFFI can cross the FFI boundary without reference types.

#[uniffi::export]
pub fn mark_podcast_clip_in(
    selection: PodcastClipSelection,
    current_time: f64,
) -> PodcastClipSelection {
    mark_clip_in(&selection, current_time)
}

#[uniffi::export]
pub fn mark_podcast_clip_out(
    selection: PodcastClipSelection,
    current_time: f64,
) -> PodcastClipSelection {
    mark_clip_out(&selection, current_time)
}

#[uniffi::export]
pub fn extend_podcast_clip_to_segment(
    selection: PodcastClipSelection,
    segment: TranscriptSegment,
) -> PodcastClipSelection {
    extend_clip_to_segment(&selection, &segment)
}

#[uniffi::export]
pub fn set_podcast_clip_start(selection: PodcastClipSelection, value: f64) -> PodcastClipSelection {
    set_clip_start(&selection, value)
}

#[uniffi::export]
pub fn set_podcast_clip_end(
    selection: PodcastClipSelection,
    value: f64,
    duration_seconds: f64,
) -> PodcastClipSelection {
    set_clip_end(&selection, value, duration_seconds)
}

/// Fetch and parse a podcast transcript from `url`, returning a load snapshot.
#[uniffi::export(async_runtime = "tokio")]
pub async fn load_podcast_transcript(url: String) -> PodcastTranscriptLoadSnapshot {
    transcript_load_snapshot(fetch_transcript(&url).await)
}

/// Download raw artwork bytes from `url`. Returns `None` on any error.
#[uniffi::export(async_runtime = "tokio")]
pub async fn download_podcast_artwork(url: String) -> Option<Vec<u8>> {
    download_artwork(&url).await.ok()
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn validate_http_url(raw: &str) -> Result<Url, CoreError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(CoreError::InvalidInput("URL must not be empty".into()));
    }
    let url =
        Url::parse(trimmed).map_err(|e| CoreError::InvalidInput(format!("invalid URL: {e}")))?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        scheme => Err(CoreError::InvalidInput(format!(
            "unsupported URL scheme: {scheme}"
        ))),
    }
}

async fn read_limited(
    response: reqwest::Response,
    max_bytes: usize,
    label: &str,
) -> Result<Vec<u8>, CoreError> {
    if let Some(length) = response.content_length() {
        if length > max_bytes as u64 {
            return Err(CoreError::Network(format!(
                "{label} is too large ({length} bytes)"
            )));
        }
    }

    let mut out = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| CoreError::Network(format!("{label} read: {e}")))?;
        if out.len().saturating_add(chunk.len()) > max_bytes {
            return Err(CoreError::Network(format!(
                "{label} exceeds {} bytes",
                max_bytes
            )));
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

fn detect_format(source: &str, content_type: Option<&str>, file_extension: Option<&str>) -> Format {
    let content_type = content_type.unwrap_or("").to_lowercase();
    let ext = file_extension.unwrap_or("").to_lowercase();

    if content_type.contains("json") || ext == "json" {
        return Format::Json;
    }
    if content_type.contains("vtt") || ext == "vtt" {
        return Format::Vtt;
    }
    if content_type.contains("srt") || ext == "srt" {
        return Format::Srt;
    }

    let sniff = source.chars().take(200).collect::<String>();
    let sniff = sniff.trim();
    if sniff.starts_with("WEBVTT") {
        return Format::Vtt;
    }
    if sniff.starts_with('[') || sniff.starts_with('{') {
        return Format::Json;
    }
    if sniff.contains("-->") {
        return Format::Srt;
    }
    Format::Unknown
}

fn parse_vtt(source: &str) -> Vec<TranscriptSegment> {
    let normalized = source.replace('\r', "");
    let mut body = normalized.as_str();
    if body.starts_with("WEBVTT") {
        body = body.find("\n\n").map(|idx| &body[idx + 2..]).unwrap_or("");
    }

    let mut segments = Vec::new();
    for block in body.split("\n\n") {
        let lines: Vec<String> = block
            .split('\n')
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();
        if lines.len() < 2 {
            continue;
        }
        let Some(time_idx) = lines.iter().position(|line| line.contains("-->")) else {
            continue;
        };
        let Some((start, end)) = split_timecode(&lines[time_idx]) else {
            continue;
        };

        let raw_text = lines[time_idx + 1..].join("\n");
        let cleaned = strip_vtt_tags(&raw_text);
        if cleaned.is_empty() {
            continue;
        }

        let speaker = extract_vtt_speaker(&lines[time_idx + 1..].join(" "))
            .or_else(|| extract_speaker(&cleaned))
            .unwrap_or_default();
        let text = strip_speaker_prefix(&cleaned);

        segments.push(TranscriptSegment {
            id: format!("vtt-{}", segments.len()),
            start,
            end,
            speaker,
            text,
        });
    }
    segments
}

fn parse_srt(source: &str) -> Vec<TranscriptSegment> {
    let normalized = source.replace('\r', "");
    let mut segments = Vec::new();

    for block in normalized.split("\n\n") {
        let lines: Vec<String> = block
            .split('\n')
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();
        if lines.len() < 2 {
            continue;
        }
        let Some(time_idx) = lines.iter().position(|line| line.contains("-->")) else {
            continue;
        };
        let Some((start, end)) = split_timecode(&lines[time_idx]) else {
            continue;
        };

        let seq = lines
            .first()
            .filter(|line| line.parse::<u64>().is_ok())
            .cloned()
            .unwrap_or_else(|| segments.len().to_string());
        let raw_text = lines[time_idx + 1..].join("\n");
        let cleaned = normalize_whitespace(&raw_text);
        if cleaned.is_empty() {
            continue;
        }

        let speaker = extract_speaker(&cleaned).unwrap_or_default();
        let text = strip_speaker_prefix(&cleaned);

        segments.push(TranscriptSegment {
            id: format!("srt-{seq}"),
            start,
            end,
            speaker,
            text,
        });
    }
    segments
}

fn parse_json(source: &str) -> Vec<TranscriptSegment> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return Vec::new();
    };
    find_json_segments(&value)
}

fn find_json_segments(value: &serde_json::Value) -> Vec<TranscriptSegment> {
    match value {
        serde_json::Value::Array(items) => {
            let direct: Vec<TranscriptSegment> = items
                .iter()
                .enumerate()
                .filter_map(|(idx, item)| json_segment(item, idx))
                .collect();
            if !direct.is_empty() {
                return direct;
            }
            for item in items {
                let nested = find_json_segments(item);
                if !nested.is_empty() {
                    return nested;
                }
            }
            Vec::new()
        }
        serde_json::Value::Object(map) => {
            for key in ["segments", "results", "items", "captions", "transcript"] {
                if let Some(sub) = map.get(key) {
                    let nested = find_json_segments(sub);
                    if !nested.is_empty() {
                        return nested;
                    }
                }
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

fn json_segment(value: &serde_json::Value, index: usize) -> Option<TranscriptSegment> {
    let dict = value.as_object()?;
    let text = first_string(dict, &["text", "value", "caption", "body"]);
    if text.is_empty() {
        return None;
    }

    let start = first_f64(dict, &["start", "startTime", "start_time", "offset"]).unwrap_or(0.0);
    let end = first_f64(dict, &["end", "endTime", "end_time"]).unwrap_or(start);
    let speaker = first_string(dict, &["speaker", "speakerName", "speaker_name"]);
    let id = first_string(dict, &["id"]);

    Some(TranscriptSegment {
        id: if id.is_empty() {
            format!("json-{index}")
        } else {
            id
        },
        start,
        end,
        speaker,
        text: normalize_whitespace(&text),
    })
}

fn first_string(map: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> String {
    for key in keys {
        if let Some(value) = map.get(*key).and_then(|value| value.as_str()) {
            if !value.is_empty() {
                return value.to_string();
            }
        }
    }
    String::new()
}

fn first_f64(map: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<f64> {
    for key in keys {
        let Some(value) = map.get(*key) else {
            continue;
        };
        if let Some(n) = value.as_f64() {
            return Some(n);
        }
        if let Some(s) = value.as_str().and_then(|s| s.parse::<f64>().ok()) {
            return Some(s);
        }
    }
    None
}

fn split_timecode(line: &str) -> Option<(f64, f64)> {
    let (start, end) = line.split_once("-->")?;
    Some((parse_timestamp(start.trim())?, parse_timestamp(end.trim())?))
}

fn parse_timestamp(raw: &str) -> Option<f64> {
    static TIMESTAMP_RE: OnceLock<Regex> = OnceLock::new();
    let re = TIMESTAMP_RE
        .get_or_init(|| Regex::new(r"(\d{1,2}):(\d{2})(?::(\d{2}))?(?:[.,](\d{1,3}))?").unwrap());
    let captures = re.captures(raw)?;
    let first = captures.get(1)?.as_str().parse::<f64>().ok()?;
    let second = captures.get(2)?.as_str().parse::<f64>().ok()?;
    let third = captures.get(3).and_then(|m| m.as_str().parse::<f64>().ok());
    let millis = captures
        .get(4)
        .and_then(|m| {
            let mut value = m.as_str().to_string();
            while value.len() < 3 {
                value.push('0');
            }
            value.parse::<f64>().ok()
        })
        .unwrap_or(0.0);

    if let Some(third) = third {
        Some(first * 3600.0 + second * 60.0 + third + millis / 1000.0)
    } else {
        Some(first * 60.0 + second + millis / 1000.0)
    }
}

fn extract_vtt_speaker(raw: &str) -> Option<String> {
    static VTT_SPEAKER_RE: OnceLock<Regex> = OnceLock::new();
    let re = VTT_SPEAKER_RE.get_or_init(|| Regex::new(r"(?i)<v\s+([^>]+)>").unwrap());
    let speaker = re.captures(raw)?.get(1)?.as_str().trim();
    (!speaker.is_empty()).then(|| speaker.to_string())
}

fn strip_vtt_tags(raw: &str) -> String {
    static VTT_VOICE_RE: OnceLock<Regex> = OnceLock::new();
    static TAG_RE: OnceLock<Regex> = OnceLock::new();
    let voice_re = VTT_VOICE_RE.get_or_init(|| Regex::new(r"(?is)<v[^>]*>(.*?)</v>").unwrap());
    let tag_re = TAG_RE.get_or_init(|| Regex::new(r"(?is)<[^>]+>").unwrap());
    let without_voice = voice_re.replace_all(raw, "$1");
    let without_tags = tag_re.replace_all(&without_voice, "");
    normalize_whitespace(&without_tags)
}

fn extract_speaker(raw: &str) -> Option<String> {
    static SPEAKER_RE: OnceLock<Regex> = OnceLock::new();
    let re = SPEAKER_RE.get_or_init(|| {
        Regex::new(r"^([A-Z][a-z]+(?:\s+[A-Z][a-z]+){0,3}|[A-Z]{2,10})\s*:\s+").unwrap()
    });
    let speaker = re.captures(raw)?.get(1)?.as_str().trim();
    (!speaker.is_empty()).then(|| speaker.to_string())
}

fn strip_speaker_prefix(raw: &str) -> String {
    static SPEAKER_RE: OnceLock<Regex> = OnceLock::new();
    let re = SPEAKER_RE.get_or_init(|| {
        Regex::new(r"^([A-Z][a-z]+(?:\s+[A-Z][a-z]+){0,3}|[A-Z]{2,10})\s*:\s+").unwrap()
    });
    re.replace(raw, "").to_string()
}

fn normalize_whitespace(raw: &str) -> String {
    static WHITESPACE_RE: OnceLock<Regex> = OnceLock::new();
    let re = WHITESPACE_RE.get_or_init(|| Regex::new(r"\s+").unwrap());
    re.replace_all(raw, " ")
        .trim_matches(|c: char| c.is_whitespace())
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_load_snapshot_projects_available_segments() {
        let segment = TranscriptSegment {
            id: "s1".to_string(),
            start: 1.0,
            end: 2.0,
            speaker: "Ada".to_string(),
            text: "A useful point.".to_string(),
        };

        let snapshot = transcript_load_snapshot(Ok(vec![segment.clone()]));

        assert_eq!(
            snapshot.availability,
            PodcastTranscriptAvailability::Available
        );
        assert!(snapshot.error.is_empty());
        assert_eq!(snapshot.segments, vec![segment]);
    }

    #[test]
    fn transcript_load_snapshot_projects_empty_and_error_as_unavailable() {
        let empty = transcript_load_snapshot(Ok(Vec::new()));
        assert_eq!(
            empty.availability,
            PodcastTranscriptAvailability::Unavailable
        );
        assert!(empty.segments.is_empty());
        assert!(empty.error.is_empty());

        let failed = transcript_load_snapshot(Err(CoreError::Network("offline".to_string())));
        assert_eq!(
            failed.availability,
            PodcastTranscriptAvailability::Unavailable
        );
        assert!(failed.segments.is_empty());
        assert_eq!(failed.error, "network error: offline");
    }

    #[test]
    fn transcript_load_apply_projection_maps_segments_and_error_logging() {
        let available = transcript_load_snapshot(Ok(vec![segment("s1", 1.0, "hello")]));
        let applied = transcript_load_apply_projection(PodcastTranscriptLoadApplyInput {
            snapshot: available,
        });
        assert_eq!(
            applied.availability,
            PodcastTranscriptAvailability::Available
        );
        assert_eq!(applied.segments.len(), 1);
        assert!(!applied.should_log_error);
        assert!(applied.log_message.is_empty());

        let failed = transcript_load_snapshot(Err(CoreError::Network("offline".to_string())));
        let applied =
            transcript_load_apply_projection(PodcastTranscriptLoadApplyInput { snapshot: failed });
        assert_eq!(
            applied.availability,
            PodcastTranscriptAvailability::Unavailable
        );
        assert!(applied.segments.is_empty());
        assert!(applied.should_log_error);
        assert_eq!(
            applied.log_message,
            "transcript load failed: network error: offline"
        );
    }

    #[test]
    fn clip_publish_snapshot_projects_highlight_or_error_state() {
        let highlight = HighlightRecord {
            event_id: "event123".into(),
            pubkey: "pubkey".into(),
            quote: "clip quote".into(),
            context: "context".into(),
            note: "note".into(),
            artifact_address: "kind:pubkey:d".into(),
            event_reference: "nevent1".into(),
            external_reference: "podcast:item:guid:episode".into(),
            source_url: "https://example.com/episode".into(),
            source_reference_key: "podcast:item:guid:episode".into(),
            clip_start_seconds: Some(12.0),
            clip_end_seconds: Some(18.0),
            clip_speaker: "Host".into(),
            clip_transcript_segment_ids: vec!["s1".into()],
            image_url: "https://example.com/image.jpg".into(),
            created_at: Some(42),
        };

        let ok = clip_publish_snapshot(Ok(highlight));
        assert_eq!(
            ok.highlight
                .as_ref()
                .map(|highlight| highlight.event_id.as_str()),
            Some("event123")
        );
        assert!(ok.error.is_empty());

        let err = clip_publish_snapshot(Err(CoreError::Relay("offline".into())));
        assert!(err.highlight.is_none());
        assert_eq!(err.error, "relay error: offline");
    }

    #[test]
    fn clip_publish_result_projection_maps_ui_effects() {
        let ok = clip_publish_result_projection(PodcastClipPublishResultInput {
            snapshot: PodcastClipPublishSnapshot {
                highlight: None,
                error: String::new(),
            },
        });
        assert!(ok.did_publish);
        assert_eq!(ok.error_message, None);
        assert_eq!(ok.share_toast.as_deref(), Some("Clip shared"));
        assert!(ok.should_dismiss);

        let failed = clip_publish_result_projection(PodcastClipPublishResultInput {
            snapshot: PodcastClipPublishSnapshot {
                highlight: None,
                error: " relay error: offline ".into(),
            },
        });
        assert!(!failed.did_publish);
        assert_eq!(
            failed.error_message.as_deref(),
            Some("relay error: offline")
        );
        assert_eq!(failed.share_toast, None);
        assert!(!failed.should_dismiss);
    }

    #[test]
    fn parses_vtt_with_voice_tags() {
        let bytes = br#"WEBVTT

00:00:01.000 --> 00:00:03.500
<v Alice>Welcome to the show.</v>

00:00:04.000 --> 00:00:05.000
Bob: Thanks.
"#;

        let segments = parse_transcript_bytes(bytes, Some("text/vtt"), None);

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].id, "vtt-0");
        assert_eq!(segments[0].start, 1.0);
        assert_eq!(segments[0].end, 3.5);
        assert_eq!(segments[0].speaker, "Alice");
        assert_eq!(segments[0].text, "Welcome to the show.");
        assert_eq!(segments[1].speaker, "Bob");
        assert_eq!(segments[1].text, "Thanks.");
    }

    #[test]
    fn parses_srt_sequence_ids_and_comma_timestamps() {
        let bytes = br#"12
00:01:02,250 --> 00:01:04,000
HOST: Segment text.
"#;

        let segments = parse_transcript_bytes(bytes, None, Some("srt"));

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].id, "srt-12");
        assert_eq!(segments[0].start, 62.25);
        assert_eq!(segments[0].end, 64.0);
        assert_eq!(segments[0].speaker, "HOST");
        assert_eq!(segments[0].text, "Segment text.");
    }

    #[test]
    fn parses_nested_json_segments() {
        let bytes = br#"{
  "results": [
    {"id": "a", "startTime": "1.5", "end_time": 2, "speakerName": "Ada", "text": "hello\nworld"}
  ]
}"#;

        let segments = parse_transcript_bytes(bytes, Some("application/json"), None);

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].id, "a");
        assert_eq!(segments[0].start, 1.5);
        assert_eq!(segments[0].end, 2.0);
        assert_eq!(segments[0].speaker, "Ada");
        assert_eq!(segments[0].text, "hello world");
    }

    #[test]
    fn clip_highlight_draft_orders_selected_segments_and_builds_quote() {
        let segments = vec![
            segment("later", 20.0, "later line"),
            segment("earlier", 10.0, "earlier line"),
            segment("unused", 5.0, "unused line"),
        ];
        let selected = vec![
            "missing".to_string(),
            "later".to_string(),
            "earlier".to_string(),
        ];

        let draft = clip_highlight_draft(
            &segments,
            &selected,
            "note".into(),
            Some(9.0),
            Some(25.0),
            "Ada".into(),
        );

        assert_eq!(draft.quote, "earlier line later line");
        assert_eq!(draft.context, "");
        assert_eq!(draft.note, "note");
        assert_eq!(draft.clip_start_seconds, Some(9.0));
        assert_eq!(draft.clip_end_seconds, Some(25.0));
        assert_eq!(draft.clip_speaker, "Ada");
        assert_eq!(
            draft.clip_transcript_segment_ids,
            vec!["earlier".to_string(), "later".to_string()]
        );
        assert!(draft.image.is_none());
    }

    #[test]
    fn clip_highlight_draft_allows_empty_selection() {
        let draft = clip_highlight_draft(
            &[segment("a", 1.0, "hello")],
            &[],
            String::new(),
            None,
            None,
            String::new(),
        );

        assert_eq!(draft.quote, "");
        assert!(draft.clip_transcript_segment_ids.is_empty());
    }

    #[test]
    fn mark_clip_in_and_out_clear_reversed_bounds() {
        let selection = PodcastClipSelection {
            clip_start_seconds: Some(30.0),
            clip_end_seconds: Some(10.0),
            speaker: "Ada".into(),
            selected_segment_ids: vec!["a".into()],
        };

        let marked_in = mark_clip_in(&selection, 20.0);
        assert_eq!(marked_in.clip_start_seconds, Some(20.0));
        assert_eq!(marked_in.clip_end_seconds, None);
        assert_eq!(marked_in.speaker, "Ada");
        assert_eq!(marked_in.selected_segment_ids, vec!["a".to_string()]);

        let marked_out = mark_clip_out(&selection, 20.0);
        assert_eq!(marked_out.clip_start_seconds, None);
        assert_eq!(marked_out.clip_end_seconds, Some(20.0));
    }

    #[test]
    fn extend_clip_to_segment_expands_bounds_dedupes_and_adopts_speaker() {
        let selection = PodcastClipSelection {
            clip_start_seconds: Some(10.0),
            clip_end_seconds: Some(20.0),
            speaker: String::new(),
            selected_segment_ids: vec!["existing".into()],
        };
        let segment = TranscriptSegment {
            id: "existing".into(),
            start: 5.0,
            end: 30.0,
            speaker: "Ada".into(),
            text: "hello".into(),
        };

        let out = extend_clip_to_segment(&selection, &segment);

        assert_eq!(out.clip_start_seconds, Some(5.0));
        assert_eq!(out.clip_end_seconds, Some(30.0));
        assert_eq!(out.speaker, "Ada");
        assert_eq!(out.selected_segment_ids, vec!["existing".to_string()]);
    }

    #[test]
    fn set_clip_bounds_enforces_gap_and_duration() {
        let selection = PodcastClipSelection {
            clip_start_seconds: Some(10.0),
            clip_end_seconds: Some(20.0),
            speaker: String::new(),
            selected_segment_ids: Vec::new(),
        };

        let start = set_clip_start(&selection, 30.0);
        assert_eq!(start.clip_start_seconds, Some(19.95));

        let end = set_clip_end(&selection, 30.0, 25.0);
        assert_eq!(end.clip_end_seconds, Some(25.0));

        let end_before_start = set_clip_end(&selection, 5.0, 25.0);
        assert_eq!(end_before_start.clip_end_seconds, Some(10.05));
    }

    #[test]
    fn clear_clip_selection_resets_everything() {
        assert_eq!(
            clear_clip_selection(),
            PodcastClipSelection {
                clip_start_seconds: None,
                clip_end_seconds: None,
                speaker: String::new(),
                selected_segment_ids: Vec::new()
            }
        );
    }

    #[test]
    fn clip_composer_projection_matches_segments_and_room_label() {
        let segments = vec![
            clip_segment("before", 0.0, 4.0, "", "before"),
            clip_segment("a", 4.0, 12.0, "", "alpha"),
            clip_segment("b", 11.0, 20.0, "Ada", "beta"),
            clip_segment("after", 20.0, 25.0, "Grace", "after"),
        ];

        let projection = clip_composer_projection(composer_input(
            segments,
            true,
            10.0,
            18.0,
            60.0,
            Some("room"),
            vec![community("room", "Room name")],
        ));

        assert_eq!(projection.duration_seconds, 8.0);
        assert_eq!(projection.clip_start_label, "0:10");
        assert_eq!(projection.clip_end_label, "0:18");
        assert_eq!(projection.duration_label, "8s");
        assert_eq!(projection.subtitle_label, "8s · with transcript");
        assert_eq!(
            projection.time_only_message,
            "Time-only clip · 8s. Add a note for the room below."
        );
        assert_eq!(projection.excerpt, "alpha beta");
        assert_eq!(projection.speaker, "Ada");
        assert!(projection.has_transcript);
        assert!(projection.can_publish);
        assert_eq!(projection.community_name, "Room name");
        assert_eq!(projection.community_display_name, "Room name");
        assert!(projection.has_community);
        assert_eq!(
            projection.selected_segment_ids,
            vec!["a".to_string(), "b".to_string()]
        );
        assert_eq!(
            projection
                .matching_segments
                .iter()
                .map(|segment| segment.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[test]
    fn clip_composer_projection_preserves_time_only_and_group_fallbacks() {
        let segments = vec![clip_segment("a", 4.0, 12.0, "Ada", "alpha")];

        let projection = clip_composer_projection(composer_input(
            segments.clone(),
            false,
            10.0,
            14.0,
            60.0,
            Some("missing-room"),
            vec![community("other", "")],
        ));

        assert_eq!(projection.excerpt, "");
        assert_eq!(projection.speaker, "");
        assert_eq!(projection.clip_start_label, "0:10");
        assert_eq!(projection.clip_end_label, "0:14");
        assert_eq!(projection.duration_label, "4s");
        assert_eq!(projection.subtitle_label, "4s · time-only clip");
        assert_eq!(
            projection.time_only_message,
            "Time-only clip · 4s. Add a note for the room below."
        );
        assert!(!projection.has_transcript);
        assert!(!projection.can_publish);
        assert_eq!(projection.community_name, "missing-room");
        assert_eq!(projection.community_display_name, "missing-room");
        assert!(projection.has_community);
        assert!(projection.selected_segment_ids.is_empty());

        let personal = clip_composer_projection(composer_input(
            segments,
            true,
            0.0,
            6.0,
            60.0,
            None,
            Vec::new(),
        ));
        assert_eq!(personal.community_name, "");
        assert_eq!(personal.community_display_name, "Personal");
        assert_eq!(personal.duration_label, "6s");
        assert_eq!(personal.subtitle_label, "6s · with transcript");
        assert!(!personal.has_community);
    }

    #[test]
    fn clip_composer_draft_preserves_sheet_context_mapping() {
        let segments = vec![
            clip_segment("a", 4.0, 12.0, "", "alpha"),
            clip_segment("b", 11.0, 20.0, "Ada", "beta"),
        ];

        let draft = clip_composer_highlight_draft(&segments, true, "room note".into(), 10.0, 18.0);

        assert_eq!(draft.quote, "alpha beta");
        assert_eq!(draft.context, "room note");
        assert_eq!(draft.note, "");
        assert_eq!(draft.clip_start_seconds, Some(10.0));
        assert_eq!(draft.clip_end_seconds, Some(18.0));
        assert_eq!(draft.clip_speaker, "Ada");
        assert_eq!(
            draft.clip_transcript_segment_ids,
            vec!["a".to_string(), "b".to_string()]
        );
        assert!(draft.image.is_none());
    }

    #[test]
    fn listening_projection_builds_header_rows_and_active_state() {
        let artifact = podcast_artifact(
            "share-1",
            "Episode title",
            "Author fallback",
            "Show title",
            "guid-1",
            Some(3700),
            vec![
                chapter(0.0, "Start"),
                chapter(60.0, "Next"),
                chapter(90.5, "Fractional"),
            ],
        );
        let clips = vec![
            highlight("clip-a", Some(45.0)),
            highlight("clip-b", Some(15.0)),
        ];
        let segments = vec![
            clip_segment("a", 5.0, 10.0, "Ada", "alpha"),
            clip_segment("b", 50.0, 55.0, "Bob", "beta"),
        ];

        let projection = listening_projection(PodcastListeningProjectionInput {
            artifact: Some(artifact),
            clips,
            transcript_segments: segments,
            transcript_available: true,
            show_transcript: true,
            show_chapters: true,
            show_clips: true,
            player_duration_seconds: 120.0,
            current_time_seconds: 55.0,
            waveform_tick_window_seconds: 30.0,
        });

        assert_eq!(projection.show_title, "Show title");
        assert_eq!(projection.episode_title, "Episode title");
        assert_eq!(projection.image_url, "https://img.example/cover.jpg");
        assert_eq!(projection.episode_meta, "1h 1m · 2 clips");
        assert!(projection.has_chapters);
        assert_eq!(projection.clip_count, 2);
        assert_eq!(projection.current_speaker_or_timestamp, "Bob");
        assert_eq!(projection.active_row_id.as_deref(), Some("transcript-b"));
        assert_eq!(
            projection
                .rows
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "chapter-0.0",
                "transcript-a",
                "clip-clip-b",
                "clip-clip-a",
                "transcript-b",
                "chapter-60.0",
                "chapter-90.5"
            ]
        );
        assert_eq!(projection.rows[0].state, PodcastTimelineRowState::Played);
        assert_eq!(projection.rows[4].state, PodcastTimelineRowState::Active);
        assert_eq!(projection.rows[5].state, PodcastTimelineRowState::Future);
        assert_eq!(projection.rows[0].timestamp_label, "0:00");
        assert_eq!(projection.rows[2].timestamp_label, "0:15");
        assert_eq!(projection.rows[2].clip_range_label, "0:15–0:25");
        assert_eq!(projection.rows[6].timestamp_label, "1:30");
        assert_eq!(projection.rows[6].id, "chapter-90.5");
    }

    #[test]
    fn now_playing_projection_matches_episode_and_show_fallbacks() {
        let artifact = podcast_artifact(
            "share-1",
            "",
            "Author fallback",
            "",
            "guid-1",
            Some(3700),
            Vec::new(),
        );

        let projection = now_playing_projection(PodcastNowPlayingProjectionInput { artifact });

        assert_eq!(projection.episode_title, "Untitled episode");
        assert_eq!(projection.show_title, "Author fallback");
        assert_eq!(projection.image_url, "https://img.example/cover.jpg");

        let artifact = podcast_artifact(
            "share-1",
            "Episode title",
            "Author fallback",
            "Show title",
            "guid-1",
            Some(3700),
            Vec::new(),
        );

        let projection = now_playing_projection(PodcastNowPlayingProjectionInput { artifact });

        assert_eq!(projection.episode_title, "Episode title");
        assert_eq!(projection.show_title, "Show title");
    }

    #[test]
    fn listening_projection_uses_waveform_ticks_away_from_occupied_rows() {
        let artifact = podcast_artifact(
            "share-1",
            "",
            "Author fallback",
            "",
            "",
            None,
            vec![chapter(30.0, "Chapter")],
        );
        let clips = vec![highlight("clip-a", Some(60.0))];

        let projection = listening_projection(PodcastListeningProjectionInput {
            artifact: Some(artifact),
            clips,
            transcript_segments: vec![clip_segment("a", 0.0, 10.0, "Ada", "alpha")],
            transcript_available: true,
            show_transcript: false,
            show_chapters: true,
            show_clips: true,
            player_duration_seconds: 100.0,
            current_time_seconds: 12.0,
            waveform_tick_window_seconds: 30.0,
        });

        assert_eq!(projection.show_title, "Author fallback");
        assert_eq!(projection.episode_title, "Untitled episode");
        assert_eq!(projection.episode_meta, "1m · 1 clip");
        assert_eq!(projection.current_speaker_or_timestamp, "Ada");
        assert_eq!(projection.rows[0].timestamp_label, "0:00");
        assert_eq!(projection.rows[2].clip_range_label, "1:00–1:10");
        assert_eq!(projection.rows[3].timestamp_label, "1:30");
        assert_eq!(
            projection
                .rows
                .iter()
                .map(|row| (row.kind, row.id.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (PodcastTimelineRowKind::WaveformTick, "waveform-0.0"),
                (PodcastTimelineRowKind::Chapter, "chapter-30.0"),
                (PodcastTimelineRowKind::Clip, "clip-clip-a"),
                (PodcastTimelineRowKind::WaveformTick, "waveform-90.0")
            ]
        );
        assert_eq!(projection.active_row_id.as_deref(), Some("waveform-0.0"));
    }

    #[test]
    fn listening_projection_formats_timestamp_when_no_current_speaker() {
        let projection = listening_projection(PodcastListeningProjectionInput {
            artifact: None,
            clips: Vec::new(),
            transcript_segments: vec![clip_segment("a", 5.0, 10.0, "", "alpha")],
            transcript_available: true,
            show_transcript: true,
            show_chapters: true,
            show_clips: true,
            player_duration_seconds: 65.0,
            current_time_seconds: 62.0,
            waveform_tick_window_seconds: 30.0,
        });

        assert_eq!(projection.show_title, "");
        assert_eq!(projection.episode_title, "Untitled episode");
        assert_eq!(projection.episode_meta, "1m");
        assert_eq!(projection.current_speaker_or_timestamp, "1:02");
    }

    #[test]
    fn listening_clips_snapshot_preserves_clips_and_error() {
        let snapshot =
            listening_clips_snapshot(vec![highlight("clip-a", Some(15.0))], "cache unavailable");

        assert_eq!(snapshot.clips.len(), 1);
        assert_eq!(snapshot.clips[0].event_id, "clip-a");
        assert_eq!(snapshot.error, "cache unavailable");
    }

    #[test]
    fn podcast_clip_reference_uses_episode_guid_or_share_event() {
        let with_guid = podcast_artifact("share-1", "Episode", "", "", "guid-1", None, Vec::new());
        assert_eq!(
            podcast_clip_reference(&with_guid),
            PodcastClipReference {
                tag_name: "i".into(),
                tag_value: "podcast:item:guid:guid-1".into(),
                limit: 128,
            }
        );

        let fallback = podcast_artifact("share-2", "Episode", "", "", "", None, Vec::new());
        assert_eq!(
            podcast_clip_reference(&fallback),
            PodcastClipReference {
                tag_name: "i".into(),
                tag_value: "share-2".into(),
                limit: 128,
            }
        );
    }

    #[test]
    fn unknown_or_invalid_transcripts_are_empty() {
        assert!(parse_transcript_bytes(b"not a transcript", None, None).is_empty());
        assert!(parse_transcript_bytes(&[0xff, 0xfe], Some("text/vtt"), None).is_empty());
    }

    fn segment(id: &str, start: f64, text: &str) -> TranscriptSegment {
        TranscriptSegment {
            id: id.into(),
            start,
            end: start + 1.0,
            speaker: String::new(),
            text: text.into(),
        }
    }

    fn clip_segment(
        id: &str,
        start: f64,
        end: f64,
        speaker: &str,
        text: &str,
    ) -> TranscriptSegment {
        TranscriptSegment {
            id: id.into(),
            start,
            end,
            speaker: speaker.into(),
            text: text.into(),
        }
    }

    fn community(id: &str, name: &str) -> CommunitySummary {
        CommunitySummary {
            id: id.into(),
            name: name.into(),
            about: String::new(),
            picture: String::new(),
            access: "open".into(),
            visibility: "public".into(),
            admin_pubkeys: Vec::new(),
            member_count: None,
            relay_url: "wss://relay.example".into(),
            metadata_event_id: String::new(),
            created_at: Some(1),
        }
    }

    fn chapter(start_seconds: f64, title: &str) -> crate::models::Chapter {
        crate::models::Chapter {
            start_seconds,
            title: title.into(),
        }
    }

    fn podcast_artifact(
        share_event_id: &str,
        title: &str,
        author: &str,
        show_title: &str,
        item_guid: &str,
        duration_seconds: Option<i64>,
        chapters: Vec<crate::models::Chapter>,
    ) -> ArtifactRecord {
        ArtifactRecord {
            preview: crate::models::ArtifactPreview {
                id: "preview-id".into(),
                url: "https://podcast.example/episode".into(),
                title: title.into(),
                author: author.into(),
                image: "https://img.example/cover.jpg".into(),
                description: String::new(),
                source: "podcast".into(),
                domain: "podcast.example".into(),
                catalog_id: String::new(),
                catalog_kind: String::new(),
                podcast_guid: String::new(),
                podcast_item_guid: item_guid.into(),
                podcast_show_title: show_title.into(),
                audio_url: "https://podcast.example/audio.mp3".into(),
                audio_preview_url: String::new(),
                transcript_url: String::new(),
                feed_url: String::new(),
                published_at: String::new(),
                duration_seconds,
                reference_tag_name: "i".into(),
                reference_tag_value: String::new(),
                reference_kind: String::new(),
                highlight_tag_name: "i".into(),
                highlight_tag_value: String::new(),
                highlight_reference_key: String::new(),
                chapters,
            },
            group_id: String::new(),
            share_event_id: share_event_id.into(),
            pubkey: "pubkey".into(),
            created_at: Some(1),
            note: String::new(),
        }
    }

    fn highlight(event_id: &str, clip_start_seconds: Option<f64>) -> HighlightRecord {
        HighlightRecord {
            event_id: event_id.into(),
            pubkey: "pubkey".into(),
            quote: "quote".into(),
            context: String::new(),
            note: String::new(),
            artifact_address: String::new(),
            event_reference: String::new(),
            external_reference: String::new(),
            source_url: String::new(),
            source_reference_key: String::new(),
            clip_start_seconds,
            clip_end_seconds: clip_start_seconds.map(|start| start + 10.0),
            clip_speaker: String::new(),
            clip_transcript_segment_ids: Vec::new(),
            image_url: String::new(),
            created_at: Some(1),
        }
    }

    fn composer_input(
        segments: Vec<TranscriptSegment>,
        transcript_available: bool,
        clip_start_seconds: f64,
        clip_end_seconds: f64,
        duration_seconds: f64,
        selected_group_id: Option<&str>,
        joined_communities: Vec<CommunitySummary>,
    ) -> PodcastClipComposerInput {
        PodcastClipComposerInput {
            segments,
            transcript_available,
            clip_start_seconds,
            clip_end_seconds,
            duration_seconds,
            selected_group_id: selected_group_id.map(str::to_string),
            joined_communities,
        }
    }
}
