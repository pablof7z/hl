//! Screen-shaped reference lookup targets for room artifact lanes.
//!
//! Native shells fetch and render, but Rust owns which protocol reference
//! identifies an artifact/highlight and which NIP-22 scope comments use.

use crate::comments;
use crate::models::{
    ArtifactRecord, ArtifactReferenceTarget, HighlightRecord, HighlightReferenceTarget,
};

pub fn artifact_reference_target(artifact: &ArtifactRecord) -> Option<ArtifactReferenceTarget> {
    let preview = &artifact.preview;
    let (lowercase_tag, value) = if !preview.reference_tag_name.trim().is_empty()
        && !preview.reference_tag_value.trim().is_empty()
    {
        (
            preview.reference_tag_name.trim().to_ascii_lowercase(),
            preview.reference_tag_value.trim().to_string(),
        )
    } else if !preview.highlight_tag_name.trim().is_empty()
        && !preview.highlight_tag_value.trim().is_empty()
    {
        (
            preview.highlight_tag_name.trim().to_ascii_lowercase(),
            preview.highlight_tag_value.trim().to_string(),
        )
    } else {
        return None;
    };

    let comment_scope = comments::scope_from_preview(preview).ok();
    let comment_key = comment_scope
        .as_ref()
        .map(|scope| format!("{}:{}", scope.root_tag_name, scope.root_tag_value))
        .unwrap_or_default();

    Some(ArtifactReferenceTarget {
        artifact_id: artifact_id(artifact),
        lookup_key: lookup_key(&lowercase_tag, &value),
        lowercase_tag,
        value,
        comment_scope,
        comment_key,
    })
}

pub fn highlight_reference_target(highlight: &HighlightRecord) -> Option<HighlightReferenceTarget> {
    if !highlight.artifact_address.trim().is_empty() {
        return Some(highlight_target("a", highlight.artifact_address.trim()));
    }
    if !highlight.event_reference.trim().is_empty() {
        return Some(highlight_target("e", highlight.event_reference.trim()));
    }
    if !highlight.source_url.trim().is_empty() {
        return Some(highlight_target("r", highlight.source_url.trim()));
    }
    None
}

pub(crate) fn artifact_id(artifact: &ArtifactRecord) -> String {
    let share_event_id = artifact.share_event_id.trim();
    if share_event_id.is_empty() {
        artifact.preview.id.trim().to_string()
    } else {
        share_event_id.to_string()
    }
}

fn highlight_target(lowercase_tag: &str, value: &str) -> HighlightReferenceTarget {
    HighlightReferenceTarget {
        lowercase_tag: lowercase_tag.to_string(),
        value: value.to_string(),
        lookup_key: lookup_key(lowercase_tag, value),
    }
}

fn lookup_key(lowercase_tag: &str, value: &str) -> String {
    format!("{lowercase_tag}:{value}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ArtifactPreview, Chapter};

    #[test]
    fn artifact_reference_target_uses_primary_reference_and_comment_scope() {
        let artifact = artifact_record(ArtifactPreview {
            id: "preview-id".into(),
            reference_tag_name: "I".into(),
            reference_tag_value: "isbn:9780735211292".into(),
            reference_kind: "isbn".into(),
            highlight_tag_name: "r".into(),
            highlight_tag_value: "https://example.test/book".into(),
            ..preview_defaults()
        });

        let target = artifact_reference_target(&artifact).unwrap();

        assert_eq!(target.artifact_id, "share-id");
        assert_eq!(target.lowercase_tag, "i");
        assert_eq!(target.value, "isbn:9780735211292");
        assert_eq!(target.lookup_key, "i:isbn:9780735211292");
        assert_eq!(target.comment_key, "I:isbn:9780735211292");
        assert_eq!(target.comment_scope.unwrap().root_tag_name, "I");
    }

    #[test]
    fn artifact_reference_target_falls_back_to_highlight_reference_without_comment_scope() {
        let mut preview = preview_defaults();
        preview.id = "local-preview".into();
        preview.reference_tag_name.clear();
        preview.reference_tag_value.clear();
        preview.highlight_tag_name = "R".into();
        preview.highlight_tag_value = "https://example.test/audio.mp3".into();
        let mut artifact = artifact_record(preview);
        artifact.share_event_id.clear();

        let target = artifact_reference_target(&artifact).unwrap();

        assert_eq!(target.artifact_id, "local-preview");
        assert_eq!(target.lowercase_tag, "r");
        assert_eq!(target.lookup_key, "r:https://example.test/audio.mp3");
        assert!(target.comment_scope.is_none());
        assert!(target.comment_key.is_empty());
    }

    #[test]
    fn artifact_reference_target_returns_none_without_reference() {
        let mut preview = preview_defaults();
        preview.reference_tag_name.clear();
        preview.reference_tag_value.clear();
        preview.highlight_tag_name.clear();
        preview.highlight_tag_value.clear();

        assert!(artifact_reference_target(&artifact_record(preview)).is_none());
    }

    #[test]
    fn highlight_reference_target_preserves_swift_precedence() {
        let mut highlight = highlight_record();
        highlight.artifact_address = "30023:pubkey:d".into();
        highlight.event_reference = "event-id".into();
        highlight.source_url = "https://example.test".into();

        let target = highlight_reference_target(&highlight).unwrap();

        assert_eq!(target.lowercase_tag, "a");
        assert_eq!(target.value, "30023:pubkey:d");
        assert_eq!(target.lookup_key, "a:30023:pubkey:d");

        highlight.artifact_address.clear();
        let target = highlight_reference_target(&highlight).unwrap();
        assert_eq!(target.lookup_key, "e:event-id");

        highlight.event_reference.clear();
        let target = highlight_reference_target(&highlight).unwrap();
        assert_eq!(target.lookup_key, "r:https://example.test");
    }

    fn artifact_record(preview: ArtifactPreview) -> ArtifactRecord {
        ArtifactRecord {
            preview,
            group_id: "room".into(),
            share_event_id: "share-id".into(),
            pubkey: "pubkey".into(),
            created_at: Some(10),
            note: String::new(),
        }
    }

    fn preview_defaults() -> ArtifactPreview {
        ArtifactPreview {
            id: String::new(),
            url: String::new(),
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
            chapters: Vec::<Chapter>::new(),
        }
    }

    fn highlight_record() -> HighlightRecord {
        HighlightRecord {
            event_id: "highlight-id".into(),
            pubkey: "pubkey".into(),
            quote: "quote".into(),
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
            created_at: Some(1),
        }
    }
}
