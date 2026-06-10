//! Rust-owned podcast playback policy.
//!
//! Native shells execute platform audio and media-center capabilities. The
//! core owns reusable playback decisions so iOS and Android choose the same
//! episode URL, resume point, seek bounds, and durable-position cadence.

use crate::errors::CoreError;
use crate::models::{ArtifactRecord, PodcastPositionRecord};

const POSITION_PERSIST_INTERVAL_SECONDS: i64 = 5;

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastPlaybackSessionInput {
    pub artifact: ArtifactRecord,
    pub loaded_share_event_id: Option<String>,
    pub has_loaded_player: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PodcastPlaybackSessionPlan {
    pub audio_url: String,
    pub should_reuse_loaded_player: bool,
    pub should_autoplay: bool,
    pub resume_position_seconds: Option<f64>,
    pub transcript_url: String,
    pub preview_duration_seconds: f64,
    pub error: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastPlaybackPositionInput {
    pub artifact: ArtifactRecord,
    pub position_seconds: f64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastPlaybackSeekInput {
    pub target_seconds: f64,
    pub duration_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PodcastPlaybackSeekProjection {
    pub position_seconds: f64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastPlaybackTickInput {
    pub previous_time_seconds: f64,
    pub current_time_seconds: f64,
    pub is_playing: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PodcastPlaybackTickProjection {
    pub current_time_seconds: f64,
    pub should_update_now_playing: bool,
    pub should_persist_position: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PodcastPlaybackRehydrationSnapshot {
    pub should_apply: bool,
    pub artifact: Option<ArtifactRecord>,
    pub current_time_seconds: f64,
    pub duration_seconds: f64,
    pub is_playing: bool,
}

pub(crate) struct PodcastPositionSaveRequest {
    pub guid: String,
    pub position_seconds: f64,
    pub artifact: ArtifactRecord,
}

pub(crate) fn session_plan(
    input: PodcastPlaybackSessionInput,
    saved_position_seconds: Option<f64>,
) -> PodcastPlaybackSessionPlan {
    let audio_url = first_non_empty([
        input.artifact.preview.audio_url.as_str(),
        input.artifact.preview.audio_preview_url.as_str(),
    ]);
    if audio_url.is_empty() {
        return PodcastPlaybackSessionPlan {
            audio_url,
            should_reuse_loaded_player: false,
            should_autoplay: false,
            resume_position_seconds: None,
            transcript_url: String::new(),
            preview_duration_seconds: 0.0,
            error: "No playable audio URL for this episode.".to_string(),
        };
    }

    let loaded_share_event_id = input
        .loaded_share_event_id
        .as_deref()
        .unwrap_or_default()
        .trim();
    let share_event_id = input.artifact.share_event_id.trim();
    let should_reuse_loaded_player = input.has_loaded_player
        && !share_event_id.is_empty()
        && loaded_share_event_id == share_event_id;

    PodcastPlaybackSessionPlan {
        audio_url,
        should_reuse_loaded_player,
        should_autoplay: true,
        resume_position_seconds: if should_reuse_loaded_player {
            None
        } else {
            saved_position_seconds.filter(|position| position.is_finite() && *position >= 0.0)
        },
        transcript_url: input.artifact.preview.transcript_url.trim().to_string(),
        preview_duration_seconds: input
            .artifact
            .preview
            .duration_seconds
            .filter(|duration| *duration > 0)
            .map(|duration| duration as f64)
            .unwrap_or(0.0),
        error: String::new(),
    }
}

pub(crate) fn position_save_request(
    input: PodcastPlaybackPositionInput,
) -> Result<Option<PodcastPositionSaveRequest>, CoreError> {
    let guid = input.artifact.preview.podcast_item_guid.trim().to_string();
    if guid.is_empty() {
        return Ok(None);
    }
    let position_seconds = normalize_position(input.position_seconds)?;
    Ok(Some(PodcastPositionSaveRequest {
        guid,
        position_seconds,
        artifact: input.artifact,
    }))
}

pub(crate) fn seek_projection(input: PodcastPlaybackSeekInput) -> PodcastPlaybackSeekProjection {
    let target = if input.target_seconds.is_finite() {
        input.target_seconds
    } else {
        0.0
    };
    let duration = if input.duration_seconds.is_finite() && input.duration_seconds > 0.0 {
        input.duration_seconds
    } else {
        0.0
    };
    let bounded = if duration > 0.0 {
        target.min(duration)
    } else {
        target
    };
    PodcastPlaybackSeekProjection {
        position_seconds: bounded.max(0.0),
    }
}

pub(crate) fn tick_projection(input: PodcastPlaybackTickInput) -> PodcastPlaybackTickProjection {
    let previous = normalize_time(input.previous_time_seconds);
    let current = normalize_time(input.current_time_seconds);
    let previous_whole = previous as i64;
    let current_whole = current as i64;
    let should_update_now_playing = current_whole != previous_whole;
    let should_persist_position = should_update_now_playing
        && input.is_playing
        && current_whole > 0
        && current_whole % POSITION_PERSIST_INTERVAL_SECONDS == 0;

    PodcastPlaybackTickProjection {
        current_time_seconds: current,
        should_update_now_playing,
        should_persist_position,
    }
}

pub(crate) fn rehydration_snapshot(
    has_current_artifact: bool,
    record: Option<PodcastPositionRecord>,
) -> PodcastPlaybackRehydrationSnapshot {
    if has_current_artifact {
        return empty_rehydration_snapshot();
    }
    let Some(record) = record else {
        return empty_rehydration_snapshot();
    };

    PodcastPlaybackRehydrationSnapshot {
        should_apply: true,
        duration_seconds: record
            .artifact
            .preview
            .duration_seconds
            .filter(|duration| *duration > 0)
            .map(|duration| duration as f64)
            .unwrap_or(0.0),
        current_time_seconds: normalize_time(record.position_seconds),
        artifact: Some(record.artifact),
        is_playing: false,
    }
}

fn empty_rehydration_snapshot() -> PodcastPlaybackRehydrationSnapshot {
    PodcastPlaybackRehydrationSnapshot {
        should_apply: false,
        artifact: None,
        current_time_seconds: 0.0,
        duration_seconds: 0.0,
        is_playing: false,
    }
}

fn first_non_empty<'a>(values: impl IntoIterator<Item = &'a str>) -> String {
    values
        .into_iter()
        .map(str::trim)
        .find(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn normalize_position(position_seconds: f64) -> Result<f64, CoreError> {
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

fn normalize_time(seconds: f64) -> f64 {
    if seconds.is_finite() && seconds >= 0.0 {
        seconds
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ArtifactPreview, Chapter};

    #[test]
    fn session_plan_prefers_full_audio_url_and_detects_loaded_player_reuse() {
        let artifact = podcast_artifact(
            "share-1",
            "https://cdn.example/full.mp3",
            "https://cdn.example/preview.mp3",
            Some(3600),
        );
        let plan = session_plan(
            PodcastPlaybackSessionInput {
                artifact,
                loaded_share_event_id: Some("share-1".into()),
                has_loaded_player: true,
            },
            Some(42.0),
        );

        assert_eq!(plan.audio_url, "https://cdn.example/full.mp3");
        assert!(plan.should_reuse_loaded_player);
        assert!(plan.should_autoplay);
        assert_eq!(plan.resume_position_seconds, None);
        assert_eq!(plan.preview_duration_seconds, 3600.0);
        assert_eq!(plan.error, "");
    }

    #[test]
    fn session_plan_falls_back_to_preview_audio_and_saved_position() {
        let artifact = podcast_artifact("share-1", "", " https://cdn.example/preview.mp3 ", None);
        let plan = session_plan(
            PodcastPlaybackSessionInput {
                artifact,
                loaded_share_event_id: Some("other".into()),
                has_loaded_player: true,
            },
            Some(42.0),
        );

        assert_eq!(plan.audio_url, "https://cdn.example/preview.mp3");
        assert!(!plan.should_reuse_loaded_player);
        assert_eq!(plan.resume_position_seconds, Some(42.0));
        assert_eq!(plan.preview_duration_seconds, 0.0);
    }

    #[test]
    fn session_plan_reports_missing_audio() {
        let artifact = podcast_artifact("share-1", "", "   ", None);
        let plan = session_plan(
            PodcastPlaybackSessionInput {
                artifact,
                loaded_share_event_id: None,
                has_loaded_player: false,
            },
            Some(42.0),
        );

        assert!(!plan.error.is_empty());
        assert!(!plan.should_autoplay);
        assert!(!plan.should_reuse_loaded_player);
    }

    #[test]
    fn position_save_request_noops_without_guid_and_rejects_invalid_positions() {
        let mut artifact = podcast_artifact("share-1", "https://cdn.example/full.mp3", "", None);
        artifact.preview.podcast_item_guid.clear();
        assert!(position_save_request(PodcastPlaybackPositionInput {
            artifact: artifact.clone(),
            position_seconds: 12.0,
        })
        .unwrap()
        .is_none());

        artifact.preview.podcast_item_guid = "episode-guid".into();
        assert!(position_save_request(PodcastPlaybackPositionInput {
            artifact: artifact.clone(),
            position_seconds: f64::NAN,
        })
        .is_err());
        assert!(position_save_request(PodcastPlaybackPositionInput {
            artifact,
            position_seconds: -1.0,
        })
        .is_err());
    }

    #[test]
    fn seek_projection_clamps_to_duration_and_zero() {
        assert_eq!(
            seek_projection(PodcastPlaybackSeekInput {
                target_seconds: 90.0,
                duration_seconds: 60.0,
            })
            .position_seconds,
            60.0
        );
        assert_eq!(
            seek_projection(PodcastPlaybackSeekInput {
                target_seconds: -5.0,
                duration_seconds: 60.0,
            })
            .position_seconds,
            0.0
        );
    }

    #[test]
    fn tick_projection_updates_once_per_second_and_persists_every_five_seconds() {
        let tick = tick_projection(PodcastPlaybackTickInput {
            previous_time_seconds: 4.9,
            current_time_seconds: 5.0,
            is_playing: true,
        });

        assert_eq!(tick.current_time_seconds, 5.0);
        assert!(tick.should_update_now_playing);
        assert!(tick.should_persist_position);

        let same_second = tick_projection(PodcastPlaybackTickInput {
            previous_time_seconds: 5.1,
            current_time_seconds: 5.8,
            is_playing: true,
        });
        assert!(!same_second.should_update_now_playing);
        assert!(!same_second.should_persist_position);
    }

    #[test]
    fn rehydration_snapshot_projects_saved_record_only_when_empty() {
        let record = PodcastPositionRecord {
            guid: "episode-guid".into(),
            position_seconds: 42.5,
            last_played_at_unix_seconds: 10,
            artifact: podcast_artifact("share-1", "https://cdn.example/full.mp3", "", Some(600)),
        };

        let blocked = rehydration_snapshot(true, Some(record.clone()));
        assert!(!blocked.should_apply);
        assert!(blocked.artifact.is_none());

        let snapshot = rehydration_snapshot(false, Some(record));
        assert!(snapshot.should_apply);
        assert_eq!(snapshot.current_time_seconds, 42.5);
        assert_eq!(snapshot.duration_seconds, 600.0);
        assert!(!snapshot.is_playing);
    }

    fn podcast_artifact(
        share_event_id: &str,
        audio_url: &str,
        audio_preview_url: &str,
        duration_seconds: Option<i64>,
    ) -> ArtifactRecord {
        ArtifactRecord {
            preview: ArtifactPreview {
                id: "podcast-1".into(),
                url: "https://podcast.example/episode".into(),
                title: "Episode".into(),
                author: "Host".into(),
                image: "https://podcast.example/art.jpg".into(),
                description: String::new(),
                source: "podcast".into(),
                domain: "podcast.example".into(),
                catalog_id: "podcast:item:guid:episode-guid".into(),
                catalog_kind: "podcast:item:guid".into(),
                podcast_guid: "feed-guid".into(),
                podcast_item_guid: "episode-guid".into(),
                podcast_show_title: "Show".into(),
                audio_url: audio_url.into(),
                audio_preview_url: audio_preview_url.into(),
                transcript_url: " https://podcast.example/transcript.vtt ".into(),
                feed_url: "https://podcast.example/feed.xml".into(),
                published_at: String::new(),
                duration_seconds,
                reference_tag_name: "i".into(),
                reference_tag_value: "podcast:item:guid:episode-guid".into(),
                reference_kind: "podcast:item:guid".into(),
                highlight_tag_name: "i".into(),
                highlight_tag_value: "podcast:item:guid:episode-guid".into(),
                highlight_reference_key: "i:podcast:item:guid:episode-guid".into(),
                chapters: Vec::<Chapter>::new(),
            },
            group_id: "group".into(),
            share_event_id: share_event_id.into(),
            pubkey: "pubkey".into(),
            created_at: Some(10),
            note: String::new(),
        }
    }
}
