//! Rust-owned community home lane assembly.
//!
//! The native view supplies bounded, already-fetched screen data. Rust owns
//! artifact/highlight matching, comment bucket lookup, de-duplication, and
//! activity ordering so iOS and Android render the same lanes.

use std::collections::{HashMap, HashSet};

use crate::models::{
    ArtifactRecord, CommentRecord, CommentReferenceBucket, HighlightRecord,
    HighlightReferenceBucket, HydratedHighlight, RoomLane,
};
use crate::reference_targets;

pub fn build_visible_room_lanes(
    artifacts: &[ArtifactRecord],
    highlights: &[HydratedHighlight],
    highlights_by_reference: &[HighlightReferenceBucket],
    comments_by_reference: &[CommentReferenceBucket],
) -> Vec<RoomLane> {
    let highlight_buckets: HashMap<&str, &[HighlightRecord]> = highlights_by_reference
        .iter()
        .map(|bucket| (bucket.lookup_key.as_str(), bucket.highlights.as_slice()))
        .collect();
    let comment_buckets: HashMap<&str, &[CommentRecord]> = comments_by_reference
        .iter()
        .map(|bucket| (bucket.comment_key.as_str(), bucket.comments.as_slice()))
        .collect();

    let mut lanes: Vec<RoomLaneWithActivity> = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        let artifact_id = reference_targets::artifact_id(artifact);
        let target = reference_targets::artifact_reference_target(artifact);
        let mut lane_highlights: Vec<HydratedHighlight> = Vec::new();
        let mut seen_highlight_ids: HashSet<String> = HashSet::new();
        let mut lane_comments: Vec<CommentRecord> = Vec::new();

        if let Some(target) = &target {
            if let Some(records) = highlight_buckets.get(target.lookup_key.as_str()) {
                for record in *records {
                    seen_highlight_ids.insert(record.event_id.clone());
                    lane_highlights.push(HydratedHighlight {
                        highlight: record.clone(),
                        artifact: Some(artifact.clone()),
                        shared_by_event_id: None,
                        shared_by_pubkey: None,
                    });
                }
            }

            if !target.comment_key.is_empty() {
                if let Some(records) = comment_buckets.get(target.comment_key.as_str()) {
                    lane_comments.extend(records.iter().cloned());
                }
            }
        }

        for highlight in highlights
            .iter()
            .filter(|highlight| highlight_matches_artifact(highlight, artifact))
        {
            if seen_highlight_ids.insert(highlight.highlight.event_id.clone()) {
                lane_highlights.push(highlight.clone());
            }
        }

        lane_highlights.sort_by(|a, b| {
            b.highlight
                .created_at
                .unwrap_or(0)
                .cmp(&a.highlight.created_at.unwrap_or(0))
        });
        lane_comments.sort_by(|a, b| b.created_at.unwrap_or(0).cmp(&a.created_at.unwrap_or(0)));

        if lane_highlights.is_empty() && lane_comments.is_empty() {
            continue;
        }

        let latest_activity = latest_activity(artifact, &lane_highlights, &lane_comments);
        lanes.push(RoomLaneWithActivity {
            lane: RoomLane {
                id: artifact_id,
                artifact: artifact.clone(),
                highlights: lane_highlights,
                comments: lane_comments,
            },
            latest_activity,
        });
    }

    lanes.sort_by(|a, b| b.latest_activity.cmp(&a.latest_activity));
    lanes.into_iter().map(|entry| entry.lane).collect()
}

fn highlight_matches_artifact(highlight: &HydratedHighlight, artifact: &ArtifactRecord) -> bool {
    let highlight = &highlight.highlight;
    let preview = &artifact.preview;

    if !preview.reference_tag_name.is_empty() && !preview.reference_tag_value.is_empty() {
        let artifact_key = format!(
            "{}:{}",
            preview.reference_tag_name, preview.reference_tag_value
        );
        if !highlight.source_reference_key.is_empty()
            && highlight.source_reference_key == artifact_key
        {
            return true;
        }
    }

    if !highlight.artifact_address.is_empty() {
        if highlight.artifact_address == preview.reference_tag_value {
            return true;
        }
        if highlight.artifact_address == preview.highlight_tag_value {
            return true;
        }
    }

    if !highlight.external_reference.is_empty() {
        if highlight.external_reference == preview.reference_tag_value {
            return true;
        }
        if highlight.external_reference == preview.highlight_tag_value {
            return true;
        }
        if !preview.podcast_item_guid.is_empty()
            && highlight.external_reference
                == format!("podcast:item:guid:{}", preview.podcast_item_guid)
        {
            return true;
        }
    }

    if !highlight.event_reference.is_empty() {
        if highlight.event_reference == preview.reference_tag_value {
            return true;
        }
        if highlight.event_reference == artifact.share_event_id {
            return true;
        }
    }

    if !highlight.source_url.is_empty() {
        if highlight.source_url == preview.url {
            return true;
        }
        if !preview.audio_url.is_empty() && highlight.source_url == preview.audio_url {
            return true;
        }
    }

    false
}

fn latest_activity(
    artifact: &ArtifactRecord,
    highlights: &[HydratedHighlight],
    comments: &[CommentRecord],
) -> u64 {
    let mut timestamp = 0;
    if let Some(highlight_ts) = highlights
        .iter()
        .filter_map(|highlight| highlight.highlight.created_at)
        .max()
    {
        timestamp = timestamp.max(highlight_ts);
    }
    if let Some(comment_ts) = comments
        .iter()
        .filter_map(|comment| comment.created_at)
        .max()
    {
        timestamp = timestamp.max(comment_ts);
    }
    if timestamp > 0 {
        timestamp
    } else {
        artifact.created_at.unwrap_or(0)
    }
}

struct RoomLaneWithActivity {
    lane: RoomLane,
    latest_activity: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ArtifactPreview;

    #[test]
    fn builds_visible_lanes_from_reference_buckets_and_comments() {
        let first = artifact("first", "share-first", Some(10), "i", "isbn:9780735211292");
        let second = artifact(
            "second",
            "share-second",
            Some(20),
            "i",
            "isbn:9781593278281",
        );
        let first_highlight = highlight("h1", Some(30));
        let second_comment = comment("c1", Some(40));

        let lanes = build_visible_room_lanes(
            &[first.clone(), second.clone()],
            &[],
            &[HighlightReferenceBucket {
                lookup_key: "i:isbn:9780735211292".into(),
                highlights: vec![first_highlight.clone()],
            }],
            &[CommentReferenceBucket {
                comment_key: "I:isbn:9781593278281".into(),
                comments: vec![second_comment.clone()],
            }],
        );

        assert_eq!(lanes.len(), 2);
        assert_eq!(lanes[0].id, "share-second");
        assert!(lanes[0].highlights.is_empty());
        assert_eq!(lanes[0].comments[0].event_id, "c1");
        assert_eq!(lanes[1].id, "share-first");
        assert_eq!(lanes[1].highlights[0].highlight.event_id, "h1");
        assert_eq!(
            lanes[1].highlights[0].artifact.as_ref().unwrap().id(),
            "share-first"
        );
    }

    #[test]
    fn fallback_group_highlights_match_and_dedupe_by_event_id() {
        let artifact = artifact("book", "share-book", Some(10), "i", "isbn:9780735211292");
        let bucket_highlight = highlight("h1", Some(5));
        let mut fallback = hydrated(highlight("h1", Some(50)));
        fallback.highlight.source_reference_key = "i:isbn:9780735211292".into();
        let mut other = hydrated(highlight("h2", Some(60)));
        other.highlight.source_reference_key = "i:isbn:9780735211292".into();

        let lanes = build_visible_room_lanes(
            &[artifact],
            &[fallback, other],
            &[HighlightReferenceBucket {
                lookup_key: "i:isbn:9780735211292".into(),
                highlights: vec![bucket_highlight],
            }],
            &[],
        );

        assert_eq!(lanes.len(), 1);
        assert_eq!(
            lanes[0]
                .highlights
                .iter()
                .map(|h| h.highlight.event_id.as_str())
                .collect::<Vec<_>>(),
            vec!["h2", "h1"]
        );
    }

    #[test]
    fn fallback_matching_preserves_podcast_guid_and_url_rules() {
        let mut podcast = artifact(
            "podcast",
            "share-podcast",
            Some(1),
            "i",
            "podcast:guid:show",
        );
        podcast.preview.podcast_item_guid = "episode-guid".into();
        podcast.preview.audio_url = "https://cdn.example.test/audio.mp3".into();

        let mut by_guid = hydrated(highlight("guid", Some(2)));
        by_guid.highlight.external_reference = "podcast:item:guid:episode-guid".into();
        let mut by_audio = hydrated(highlight("audio", Some(3)));
        by_audio.highlight.source_url = "https://cdn.example.test/audio.mp3".into();

        let lanes = build_visible_room_lanes(&[podcast], &[by_guid, by_audio], &[], &[]);

        assert_eq!(lanes.len(), 1);
        assert_eq!(lanes[0].highlights.len(), 2);
    }

    #[test]
    fn drops_dormant_lanes() {
        let lanes = build_visible_room_lanes(
            &[artifact("dormant", "share", Some(1), "i", "x")],
            &[],
            &[],
            &[],
        );

        assert!(lanes.is_empty());
    }

    trait ArtifactRecordTestExt {
        fn id(&self) -> &str;
    }

    impl ArtifactRecordTestExt for ArtifactRecord {
        fn id(&self) -> &str {
            &self.share_event_id
        }
    }

    fn artifact(
        id: &str,
        share_event_id: &str,
        created_at: Option<u64>,
        reference_tag_name: &str,
        reference_tag_value: &str,
    ) -> ArtifactRecord {
        let mut preview = preview(id);
        preview.reference_tag_name = reference_tag_name.into();
        preview.reference_tag_value = reference_tag_value.into();
        preview.reference_kind = reference_tag_value
            .split(':')
            .next()
            .unwrap_or_default()
            .into();
        preview.highlight_tag_name = reference_tag_name.into();
        preview.highlight_tag_value = reference_tag_value.into();
        ArtifactRecord {
            preview,
            group_id: "room".into(),
            share_event_id: share_event_id.into(),
            pubkey: "pubkey".into(),
            created_at,
            note: String::new(),
        }
    }

    fn preview(id: &str) -> ArtifactPreview {
        ArtifactPreview {
            id: id.into(),
            url: "https://example.test/article".into(),
            title: String::new(),
            author: String::new(),
            image: String::new(),
            description: String::new(),
            source: "web".into(),
            domain: String::new(),
            catalog_id: String::new(),
            catalog_kind: String::new(),
            podcast_guid: String::new(),
            podcast_item_guid: String::new(),
            podcast_show_title: String::new(),
            audio_url: String::new(),
            audio_preview_url: String::new(),
            transcript_url: String::new(),
            feed_url: String::new(),
            published_at: String::new(),
            duration_seconds: None,
            reference_tag_name: String::new(),
            reference_tag_value: String::new(),
            reference_kind: String::new(),
            highlight_tag_name: String::new(),
            highlight_tag_value: String::new(),
            highlight_reference_key: String::new(),
            chapters: Vec::new(),
        }
    }

    fn highlight(event_id: &str, created_at: Option<u64>) -> HighlightRecord {
        HighlightRecord {
            event_id: event_id.into(),
            pubkey: "pubkey".into(),
            quote: String::new(),
            context: String::new(),
            note: String::new(),
            artifact_address: String::new(),
            event_reference: String::new(),
            external_reference: String::new(),
            source_url: String::new(),
            source_reference_key: String::new(),
            clip_start_seconds: None,
            clip_end_seconds: None,
            clip_speaker: String::new(),
            clip_transcript_segment_ids: Vec::new(),
            image_url: String::new(),
            created_at,
        }
    }

    fn hydrated(highlight: HighlightRecord) -> HydratedHighlight {
        HydratedHighlight {
            highlight,
            artifact: None,
            shared_by_event_id: None,
            shared_by_pubkey: None,
        }
    }

    fn comment(event_id: &str, created_at: Option<u64>) -> CommentRecord {
        CommentRecord {
            event_id: event_id.into(),
            pubkey: "pubkey".into(),
            body: String::new(),
            root_tag_name: "I".into(),
            root_tag_value: "isbn:9781593278281".into(),
            parent_tag_name: "I".into(),
            parent_tag_value: "isbn:9781593278281".into(),
            root_kind: "0".into(),
            created_at,
        }
    }
}
