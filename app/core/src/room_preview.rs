use crate::models::ArtifactRecord;

const ROOM_PREVIEW_ARTIFACT_LIMIT: usize = 8;

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomPreviewArtifactsProjectionInput {
    pub artifacts: Vec<ArtifactRecord>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomPreviewArtifactRowProjection {
    pub artifact: ArtifactRecord,
    pub title: String,
    pub subtitle: Option<String>,
    pub shows_divider: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomPreviewArtifactsProjection {
    pub rows: Vec<RoomPreviewArtifactRowProjection>,
}

/// Projection for the room preview sheet's "Recent" rows. Rust owns the row
/// cap, divider placement, and title/subtitle fallback policy.
pub fn room_preview_artifacts_projection(
    input: RoomPreviewArtifactsProjectionInput,
) -> RoomPreviewArtifactsProjection {
    let visible: Vec<ArtifactRecord> = input
        .artifacts
        .into_iter()
        .take(ROOM_PREVIEW_ARTIFACT_LIMIT)
        .collect();
    let last_index = visible.len().saturating_sub(1);
    let rows = visible
        .into_iter()
        .enumerate()
        .map(|(index, artifact)| {
            let title = row_title(&artifact);
            let subtitle = row_subtitle(&artifact);
            RoomPreviewArtifactRowProjection {
                artifact,
                title,
                subtitle,
                shows_divider: index < last_index,
            }
        })
        .collect();

    RoomPreviewArtifactsProjection { rows }
}

fn row_title(artifact: &ArtifactRecord) -> String {
    let trimmed = artifact.preview.title.trim();
    if trimmed.is_empty() {
        "Untitled".to_string()
    } else {
        trimmed.to_string()
    }
}

fn row_subtitle(artifact: &ArtifactRecord) -> Option<String> {
    if !artifact.preview.author.is_empty() {
        Some(artifact.preview.author.clone())
    } else if !artifact.preview.domain.is_empty() {
        Some(artifact.preview.domain.clone())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ArtifactPreview;

    #[test]
    fn room_preview_artifacts_caps_rows_and_marks_dividers() {
        let artifacts = (0..9)
            .map(|index| artifact(&format!("id-{index}"), "Title", "", "example.com"))
            .collect();

        let projection =
            room_preview_artifacts_projection(RoomPreviewArtifactsProjectionInput { artifacts });

        assert_eq!(projection.rows.len(), 8);
        assert_eq!(
            projection
                .rows
                .iter()
                .map(|row| row.shows_divider)
                .collect::<Vec<_>>(),
            vec![true, true, true, true, true, true, true, false]
        );
    }

    #[test]
    fn room_preview_artifact_rows_preserve_title_and_subtitle_fallbacks() {
        let projection = room_preview_artifacts_projection(RoomPreviewArtifactsProjectionInput {
            artifacts: vec![
                artifact("a", "  ", "Author", "example.com"),
                artifact("b", "  Trimmed title  ", "", "example.org"),
                artifact("c", "No subtitle", "", ""),
            ],
        });

        assert_eq!(projection.rows[0].title, "Untitled");
        assert_eq!(projection.rows[0].subtitle, Some("Author".into()));
        assert_eq!(projection.rows[1].title, "Trimmed title");
        assert_eq!(projection.rows[1].subtitle, Some("example.org".into()));
        assert_eq!(projection.rows[2].subtitle, None);
    }

    fn artifact(id: &str, title: &str, author: &str, domain: &str) -> ArtifactRecord {
        ArtifactRecord {
            preview: ArtifactPreview {
                id: id.into(),
                url: String::new(),
                title: title.into(),
                author: author.into(),
                image: String::new(),
                description: String::new(),
                source: String::new(),
                domain: domain.into(),
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
            },
            group_id: String::new(),
            share_event_id: id.into(),
            pubkey: String::new(),
            created_at: None,
            note: String::new(),
        }
    }
}
