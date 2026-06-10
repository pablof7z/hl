use crate::blossom::BlossomUploadSnapshot;
use crate::errors::CoreError;
use crate::models::{
    ArtifactPreview, ArtifactRecord, BlossomUpload, CommunitySummary, HighlightDraft, PictureDraft,
};

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

#[derive(Debug, Clone, uniffi::Record)]
pub struct CaptureStashProjectionInput {
    pub quote: String,
    pub context: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CaptureStashProjection {
    pub quote: String,
    pub context: String,
    pub should_stash: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum CapturePublishPhase {
    Idle,
    Processing,
    Reviewing,
    Publishing,
    Done,
    Error,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct CapturePublishProjectionInput {
    pub phase: CapturePublishPhase,
    pub has_upload: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CapturePublishProjection {
    pub can_publish: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct CaptureUploadProjectionInput {
    pub snapshot: BlossomUploadSnapshot,
    pub request_generation: u64,
    pub current_generation: u64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct CaptureUploadProjection {
    pub should_apply: bool,
    pub upload: Option<BlossomUpload>,
    pub upload_error: String,
}

#[derive(Debug, Clone)]
pub struct CaptureHighlightDraftInput {
    pub quote: String,
    pub context: String,
    pub note: String,
    pub image: BlossomUpload,
}

#[derive(Debug, Clone)]
pub struct CaptureHighlightDraftProjection {
    pub draft: Option<HighlightDraft>,
    pub has_highlight: bool,
}

#[derive(Debug, Clone)]
pub struct CapturePictureDraftInput {
    pub image: BlossomUpload,
    pub note: String,
    pub artifact: Option<ArtifactRecord>,
    pub target_group_id: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct CapturePublishInput {
    pub image: BlossomUpload,
    pub quote: String,
    pub context: String,
    pub note: String,
    pub existing_artifact: Option<ArtifactRecord>,
    pub pending_preview: Option<ArtifactPreview>,
    pub target_group_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CapturePublishSnapshot {
    pub event_id: String,
    pub error: String,
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

/// Project a selected OCR passage into a stashed highlight. Rust owns text
/// normalization and blank-selection rejection.
pub fn stash_projection(input: CaptureStashProjectionInput) -> CaptureStashProjection {
    let quote = input.quote.trim().to_string();
    let context = input.context.trim().to_string();
    CaptureStashProjection {
        should_stash: !quote.is_empty(),
        quote,
        context,
    }
}

/// Capture publish button projection. Rust owns the workflow-state predicate;
/// native shells own how that state is rendered.
pub fn publish_projection(input: CapturePublishProjectionInput) -> CapturePublishProjection {
    let phase_allows_publish = matches!(
        input.phase,
        CapturePublishPhase::Processing | CapturePublishPhase::Reviewing
    );
    CapturePublishProjection {
        can_publish: phase_allows_publish && input.has_upload,
    }
}

/// Project the result of a native upload capability. Rust owns stale-result
/// rejection and success/error state; native only executes the upload and
/// applies the returned projection.
pub fn upload_projection(input: CaptureUploadProjectionInput) -> CaptureUploadProjection {
    if input.request_generation != input.current_generation {
        return CaptureUploadProjection {
            should_apply: false,
            upload: None,
            upload_error: String::new(),
        };
    }

    if !input.snapshot.error.is_empty() {
        return CaptureUploadProjection {
            should_apply: true,
            upload: None,
            upload_error: input.snapshot.error,
        };
    }

    CaptureUploadProjection {
        should_apply: true,
        upload: input.snapshot.upload,
        upload_error: String::new(),
    }
}

pub fn publish_snapshot(result: Result<String, CoreError>) -> CapturePublishSnapshot {
    match result {
        Ok(event_id) => CapturePublishSnapshot {
            event_id,
            error: String::new(),
        },
        Err(error) => CapturePublishSnapshot {
            event_id: String::new(),
            error: error.to_string(),
        },
    }
}

/// Build the capture highlight draft. Rust owns note/context/quote
/// normalization and the non-audio clip sentinel fields.
pub fn highlight_draft_projection(
    input: CaptureHighlightDraftInput,
) -> CaptureHighlightDraftProjection {
    let quote = input.quote.trim().to_string();
    if quote.is_empty() {
        return CaptureHighlightDraftProjection {
            draft: None,
            has_highlight: false,
        };
    }

    CaptureHighlightDraftProjection {
        draft: Some(HighlightDraft {
            quote,
            context: input.context.trim().to_string(),
            note: input.note.trim().to_string(),
            clip_start_seconds: None,
            clip_end_seconds: None,
            clip_speaker: String::new(),
            clip_transcript_segment_ids: Vec::new(),
            image: Some(input.image),
        }),
        has_highlight: true,
    }
}

/// Build the capture picture draft. Rust owns note and target-room
/// normalization; native shells provide already-resolved artifact context.
pub fn picture_draft(input: CapturePictureDraftInput) -> PictureDraft {
    PictureDraft {
        image: input.image,
        note: input.note.trim().to_string(),
        artifact: input.artifact,
        target_group_id: input
            .target_group_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string),
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

    #[test]
    fn stash_projection_trims_and_rejects_blank_quote() {
        let projection = stash_projection(CaptureStashProjectionInput {
            quote: "  quote from page  ".into(),
            context: "  paragraph context  ".into(),
        });

        assert_eq!(projection.quote, "quote from page");
        assert_eq!(projection.context, "paragraph context");
        assert!(projection.should_stash);

        let blank = stash_projection(CaptureStashProjectionInput {
            quote: " \n\t ".into(),
            context: "ignored".into(),
        });
        assert!(!blank.should_stash);
        assert_eq!(blank.quote, "");
    }

    #[test]
    fn publish_projection_requires_processing_or_reviewing_upload() {
        let reviewing = publish_projection(CapturePublishProjectionInput {
            phase: CapturePublishPhase::Reviewing,
            has_upload: true,
        });
        let processing = publish_projection(CapturePublishProjectionInput {
            phase: CapturePublishPhase::Processing,
            has_upload: true,
        });
        let no_upload = publish_projection(CapturePublishProjectionInput {
            phase: CapturePublishPhase::Reviewing,
            has_upload: false,
        });
        let publishing = publish_projection(CapturePublishProjectionInput {
            phase: CapturePublishPhase::Publishing,
            has_upload: true,
        });

        assert!(reviewing.can_publish);
        assert!(processing.can_publish);
        assert!(!no_upload.can_publish);
        assert!(!publishing.can_publish);
    }

    #[test]
    fn upload_projection_rejects_stale_results_and_projects_error_or_upload() {
        let stale = upload_projection(CaptureUploadProjectionInput {
            snapshot: crate::blossom::BlossomUploadSnapshot {
                upload: Some(upload()),
                error: String::new(),
            },
            request_generation: 1,
            current_generation: 2,
        });
        assert!(!stale.should_apply);
        assert!(stale.upload.is_none());
        assert!(stale.upload_error.is_empty());

        let failed = upload_projection(CaptureUploadProjectionInput {
            snapshot: crate::blossom::BlossomUploadSnapshot {
                upload: None,
                error: "network down".into(),
            },
            request_generation: 2,
            current_generation: 2,
        });
        assert!(failed.should_apply);
        assert!(failed.upload.is_none());
        assert_eq!(failed.upload_error, "network down");

        let ok = upload_projection(CaptureUploadProjectionInput {
            snapshot: crate::blossom::BlossomUploadSnapshot {
                upload: Some(upload()),
                error: String::new(),
            },
            request_generation: 3,
            current_generation: 3,
        });
        assert!(ok.should_apply);
        assert_eq!(
            ok.upload.as_ref().map(|upload| upload.url.as_str()),
            Some("https://blossom.example/page.jpg")
        );
        assert!(ok.upload_error.is_empty());
    }

    #[test]
    fn publish_snapshot_projects_event_id_and_error_state() {
        let ok = publish_snapshot(Ok("event123".into()));
        assert_eq!(ok.event_id, "event123");
        assert!(ok.error.is_empty());

        let err = publish_snapshot(Err(CoreError::InvalidInput("bad capture".into())));
        assert!(err.event_id.is_empty());
        assert_eq!(err.error, "invalid input: bad capture");
    }

    #[test]
    fn highlight_draft_projection_trims_and_omits_blank_quote() {
        let projection = highlight_draft_projection(CaptureHighlightDraftInput {
            quote: "  quote  ".into(),
            context: "  context  ".into(),
            note: "  note  ".into(),
            image: upload(),
        });
        let draft = projection.draft.expect("highlight draft");

        assert!(projection.has_highlight);
        assert_eq!(draft.quote, "quote");
        assert_eq!(draft.context, "context");
        assert_eq!(draft.note, "note");
        assert!(draft.image.is_some());
        assert_eq!(draft.clip_start_seconds, None);
        assert!(draft.clip_transcript_segment_ids.is_empty());

        let blank = highlight_draft_projection(CaptureHighlightDraftInput {
            quote: " \n\t ".into(),
            context: "context".into(),
            note: "note".into(),
            image: upload(),
        });
        assert!(!blank.has_highlight);
        assert!(blank.draft.is_none());
    }

    #[test]
    fn picture_draft_trims_note_and_target_group() {
        let draft = picture_draft(CapturePictureDraftInput {
            image: upload(),
            note: "  page note  ".into(),
            artifact: None,
            target_group_id: Some("  room-a  ".into()),
        });
        let standalone = picture_draft(CapturePictureDraftInput {
            image: upload(),
            note: String::new(),
            artifact: None,
            target_group_id: Some(" \n\t ".into()),
        });

        assert_eq!(draft.note, "page note");
        assert_eq!(draft.target_group_id, Some("room-a".into()));
        assert_eq!(standalone.target_group_id, None);
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

    fn upload() -> BlossomUpload {
        BlossomUpload {
            url: "https://blossom.example/page.jpg".into(),
            sha256_hex: "abc123".into(),
            mime: "image/jpeg".into(),
            size_bytes: 1024,
            width: 800,
            height: 1200,
            alt: "page text".into(),
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
