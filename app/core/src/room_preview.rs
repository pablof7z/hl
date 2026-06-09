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

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum RoomPreviewSecondaryAction {
    None,
    PeekInside,
    OpenFullRoom,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomPreviewActionProjectionInput {
    pub room_access: String,
    pub room_id: String,
    pub joined_room_ids: Vec<String>,
    pub is_expanded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoomPreviewActionProjection {
    pub already_joined: bool,
    pub primary_label: String,
    pub secondary_action: RoomPreviewSecondaryAction,
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

/// Projection for the room preview sheet's action buttons. Rust owns joined
/// membership, access-mode labels, and whether the secondary action peeks or
/// opens the full room.
pub fn room_preview_action_projection(
    input: RoomPreviewActionProjectionInput,
) -> RoomPreviewActionProjection {
    let already_joined = input
        .joined_room_ids
        .iter()
        .any(|id| id.trim() == input.room_id.trim());
    let access = input.room_access.trim();
    let secondary_action = if already_joined || access != "open" {
        RoomPreviewSecondaryAction::None
    } else if input.is_expanded {
        RoomPreviewSecondaryAction::OpenFullRoom
    } else {
        RoomPreviewSecondaryAction::PeekInside
    };

    RoomPreviewActionProjection {
        already_joined,
        primary_label: if already_joined {
            "Open room".into()
        } else if access == "closed" {
            "Request to join".into()
        } else {
            "Join room".into()
        },
        secondary_action,
    }
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

    #[test]
    fn room_preview_action_projects_membership_and_access_labels() {
        let joined = room_preview_action_projection(RoomPreviewActionProjectionInput {
            room_access: "open".into(),
            room_id: "room-a".into(),
            joined_room_ids: vec!["room-a".into()],
            is_expanded: false,
        });
        assert!(joined.already_joined);
        assert_eq!(joined.primary_label, "Open room");
        assert_eq!(joined.secondary_action, RoomPreviewSecondaryAction::None);

        let closed = room_preview_action_projection(RoomPreviewActionProjectionInput {
            room_access: "closed".into(),
            room_id: "room-a".into(),
            joined_room_ids: Vec::new(),
            is_expanded: false,
        });
        assert_eq!(closed.primary_label, "Request to join");
        assert_eq!(closed.secondary_action, RoomPreviewSecondaryAction::None);

        let peek = room_preview_action_projection(RoomPreviewActionProjectionInput {
            room_access: "open".into(),
            room_id: "room-a".into(),
            joined_room_ids: Vec::new(),
            is_expanded: false,
        });
        assert_eq!(peek.primary_label, "Join room");
        assert_eq!(
            peek.secondary_action,
            RoomPreviewSecondaryAction::PeekInside
        );

        let open = room_preview_action_projection(RoomPreviewActionProjectionInput {
            room_access: "open".into(),
            room_id: "room-a".into(),
            joined_room_ids: Vec::new(),
            is_expanded: true,
        });
        assert_eq!(
            open.secondary_action,
            RoomPreviewSecondaryAction::OpenFullRoom
        );
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
