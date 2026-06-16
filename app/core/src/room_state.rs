//! Rust-owned helpers for bounded room view state updates.
//!
//! Native shells receive live deltas from the event bridge, but Rust owns the
//! deterministic merge and ordering policy for the screen-shaped collections.

use crate::models::{ArtifactRecord, CommentReferenceBucket, HighlightRecord, HydratedHighlight};
use crate::reference_targets;

pub fn upsert_room_artifact(
    artifacts: &[ArtifactRecord],
    artifact: &ArtifactRecord,
) -> Vec<ArtifactRecord> {
    let mut out = Vec::with_capacity(artifacts.len() + 1);
    let mut replaced = false;
    for existing in artifacts {
        if existing.share_event_id == artifact.share_event_id {
            out.push(artifact.clone());
            replaced = true;
        } else {
            out.push(existing.clone());
        }
    }
    if !replaced {
        out.push(artifact.clone());
    }
    out.sort_by(|a, b| b.created_at.unwrap_or(0).cmp(&a.created_at.unwrap_or(0)));
    out
}

pub fn upsert_room_highlight(
    highlights: &[HydratedHighlight],
    highlight: &HydratedHighlight,
) -> Vec<HydratedHighlight> {
    let mut out = Vec::with_capacity(highlights.len() + 1);
    let mut replaced = false;
    for existing in highlights {
        if existing.highlight.event_id == highlight.highlight.event_id {
            out.push(highlight.clone());
            replaced = true;
        } else {
            out.push(existing.clone());
        }
    }
    if !replaced {
        out.push(highlight.clone());
    }
    out.sort_by(|a, b| {
        b.highlight
            .created_at
            .unwrap_or(0)
            .cmp(&a.highlight.created_at.unwrap_or(0))
    });
    out
}

pub fn upsert_highlight_reference_bucket(
    bucket: &[HighlightRecord],
    highlight: &HighlightRecord,
) -> Vec<HighlightRecord> {
    let mut out = Vec::with_capacity(bucket.len() + 1);
    let mut replaced = false;
    for existing in bucket {
        if existing.event_id == highlight.event_id {
            out.push(highlight.clone());
            replaced = true;
        } else {
            out.push(existing.clone());
        }
    }
    if !replaced {
        out.push(highlight.clone());
    }
    out.sort_by(|a, b| b.created_at.unwrap_or(0).cmp(&a.created_at.unwrap_or(0)));
    out
}

pub fn artifact_comment_count(
    artifact: &ArtifactRecord,
    comments_by_reference: &[CommentReferenceBucket],
) -> u32 {
    let Some(target) = reference_targets::artifact_reference_target(artifact) else {
        return 0;
    };
    if target.comment_key.is_empty() {
        return 0;
    }
    comments_by_reference
        .iter()
        .find(|bucket| bucket.comment_key == target.comment_key)
        .map(|bucket| bucket.comments.len() as u32)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ArtifactPreview;

    #[test]
    fn upsert_room_artifact_replaces_and_orders_newest_first() {
        let older = artifact("older", "share-older", Some(10), "i", "isbn:1");
        let newer = artifact("newer", "share-newer", Some(30), "i", "isbn:2");
        let replacement = artifact("older-updated", "share-older", Some(40), "i", "isbn:1");

        let out = upsert_room_artifact(&[older, newer], &replacement);

        assert_eq!(
            out.iter()
                .map(|artifact| artifact.preview.id.as_str())
                .collect::<Vec<_>>(),
            vec!["older-updated", "newer"]
        );
    }

    #[test]
    fn upsert_room_highlight_replaces_and_orders_newest_first() {
        let older = hydrated(highlight("older", Some(10)));
        let newer = hydrated(highlight("newer", Some(30)));
        let replacement = hydrated(highlight("older", Some(40)));

        let out = upsert_room_highlight(&[older, newer], &replacement);

        assert_eq!(
            out.iter()
                .map(|highlight| highlight.highlight.event_id.as_str())
                .collect::<Vec<_>>(),
            vec!["older", "newer"]
        );
    }

    #[test]
    fn upsert_highlight_reference_bucket_replaces_and_orders_newest_first() {
        let older = highlight("older", Some(10));
        let newer = highlight("newer", Some(30));
        let replacement = highlight("older", Some(40));

        let out = upsert_highlight_reference_bucket(&[older, newer], &replacement);

        assert_eq!(
            out.iter()
                .map(|highlight| highlight.event_id.as_str())
                .collect::<Vec<_>>(),
            vec!["older", "newer"]
        );
    }

    #[test]
    fn artifact_comment_count_uses_rust_reference_target() {
        let artifact = artifact("book", "share-book", Some(10), "i", "isbn:9780735211292");
        let count = artifact_comment_count(
            &artifact,
            &[CommentReferenceBucket {
                comment_key: "I:isbn:9780735211292".into(),
                comments: vec![comment("c1"), comment("c2")],
            }],
        );

        assert_eq!(count, 2);
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

    fn hydrated(highlight: HighlightRecord) -> HydratedHighlight {
        HydratedHighlight {
            highlight,
            artifact: None,
            shared_by_event_id: None,
            shared_by_pubkey: None,
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

    fn comment(event_id: &str) -> crate::models::CommentRecord {
        crate::models::CommentRecord {
            event_id: event_id.into(),
            pubkey: "pubkey".into(),
            body: String::new(),
            root_tag_name: "I".into(),
            root_tag_value: "isbn:9780735211292".into(),
            parent_tag_name: "I".into(),
            parent_tag_value: "isbn:9780735211292".into(),
            root_kind: "0".into(),
            created_at: Some(1),
        }
    }
}
