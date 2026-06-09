use crate::models::{ArtifactPreview, CommunitySummary};

#[derive(Debug, Clone, uniffi::Record)]
pub struct CaptureBookDisplayProjectionInput {
    pub preview: ArtifactPreview,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CaptureBookDisplayProjection {
    pub display_title: String,
    pub author: Option<String>,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct CaptureCommunitySelectionProjectionInput {
    pub selected_group_id: Option<String>,
    pub joined_communities: Vec<CommunitySummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CaptureCommunitySelectionProjection {
    pub display_name: String,
    pub has_selection: bool,
}

/// Project book labels used by capture selection and search rows. Rust owns
/// semantic fallback and optional metadata presence; native owns layout.
pub fn book_display_projection(
    input: CaptureBookDisplayProjectionInput,
) -> CaptureBookDisplayProjection {
    let preview = input.preview;
    CaptureBookDisplayProjection {
        display_title: if preview.title.is_empty() {
            "Untitled".to_string()
        } else {
            preview.title
        },
        author: non_empty_string(preview.author),
        image_url: non_empty_string(preview.image),
    }
}

/// Project capture destination display. Rust owns the optional target fallback
/// and selected room name/id resolution; native owns row styling.
pub fn community_selection_projection(
    input: CaptureCommunitySelectionProjectionInput,
) -> CaptureCommunitySelectionProjection {
    let selected_group_id = input
        .selected_group_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    let display_name = selected_group_id
        .map(|id| community_name_for_id(id, &input.joined_communities))
        .unwrap_or_else(|| "Optional".to_string());

    CaptureCommunitySelectionProjection {
        display_name,
        has_selection: selected_group_id.is_some(),
    }
}

fn non_empty_string(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Chapter;

    #[test]
    fn book_display_projection_preserves_capture_row_fallbacks() {
        let projection = book_display_projection(CaptureBookDisplayProjectionInput {
            preview: preview("", "", ""),
        });

        assert_eq!(projection.display_title, "Untitled");
        assert_eq!(projection.author, None);
        assert_eq!(projection.image_url, None);

        let projection = book_display_projection(CaptureBookDisplayProjectionInput {
            preview: preview("Book title", "Author", "https://img.example/book.jpg"),
        });

        assert_eq!(projection.display_title, "Book title");
        assert_eq!(projection.author, Some("Author".into()));
        assert_eq!(
            projection.image_url,
            Some("https://img.example/book.jpg".into())
        );
    }

    #[test]
    fn community_selection_projection_preserves_optional_and_room_fallbacks() {
        let projection = community_selection_projection(CaptureCommunitySelectionProjectionInput {
            selected_group_id: None,
            joined_communities: Vec::new(),
        });

        assert_eq!(projection.display_name, "Optional");
        assert!(!projection.has_selection);

        let projection = community_selection_projection(CaptureCommunitySelectionProjectionInput {
            selected_group_id: Some("room".into()),
            joined_communities: vec![community("room", "Room name")],
        });

        assert_eq!(projection.display_name, "Room name");
        assert!(projection.has_selection);

        let projection = community_selection_projection(CaptureCommunitySelectionProjectionInput {
            selected_group_id: Some("missing".into()),
            joined_communities: vec![community("other", "")],
        });

        assert_eq!(projection.display_name, "missing");
        assert!(projection.has_selection);
    }

    fn preview(title: &str, author: &str, image: &str) -> ArtifactPreview {
        ArtifactPreview {
            id: "book".into(),
            url: String::new(),
            title: title.into(),
            author: author.into(),
            image: image.into(),
            description: String::new(),
            source: "book".into(),
            domain: String::new(),
            catalog_id: "isbn:9780593716717".into(),
            catalog_kind: "isbn".into(),
            podcast_guid: String::new(),
            podcast_item_guid: String::new(),
            podcast_show_title: String::new(),
            audio_url: String::new(),
            audio_preview_url: String::new(),
            transcript_url: String::new(),
            feed_url: String::new(),
            published_at: String::new(),
            duration_seconds: None,
            reference_tag_name: "i".into(),
            reference_tag_value: "isbn:9780593716717".into(),
            reference_kind: String::new(),
            highlight_tag_name: "i".into(),
            highlight_tag_value: "isbn:9780593716717".into(),
            highlight_reference_key: "i:isbn:9780593716717".into(),
            chapters: Vec::<Chapter>::new(),
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
}
