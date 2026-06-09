use crate::models::ArtifactPreview;

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

fn non_empty_string(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
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
}
