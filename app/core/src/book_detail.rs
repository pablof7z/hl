use crate::models::{BookRoute, HighlightRecord};

#[derive(Debug, Clone, uniffi::Record)]
pub struct BookDetailSnapshot {
    pub route: Option<BookRoute>,
    pub highlights: Vec<HighlightRecord>,
    pub error: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BookDetailSnapshotApplyInput {
    pub route: Option<BookRoute>,
    pub highlights: Vec<HighlightRecord>,
    pub error: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BookDetailSnapshotApplyProjection {
    pub route: Option<BookRoute>,
    pub highlights: Vec<HighlightRecord>,
    pub isbn_preview_request: Option<String>,
}

impl BookDetailSnapshot {
    pub fn empty() -> Self {
        Self {
            route: None,
            highlights: Vec::new(),
            error: String::new(),
        }
    }
}

pub fn snapshot(route: BookRoute, highlights: Vec<HighlightRecord>) -> BookDetailSnapshot {
    BookDetailSnapshot {
        route: Some(route),
        highlights,
        error: String::new(),
    }
}

pub fn error_snapshot(route: Option<BookRoute>, error: impl ToString) -> BookDetailSnapshot {
    BookDetailSnapshot {
        route,
        highlights: Vec::new(),
        error: error.to_string(),
    }
}

pub fn snapshot_apply_projection(
    input: BookDetailSnapshotApplyInput,
) -> BookDetailSnapshotApplyProjection {
    let has_error = !input.error.trim().is_empty();
    let isbn_preview_request = input.route.as_ref().map(|route| route.isbn.clone());
    let highlights = if has_error || input.route.is_none() {
        Vec::new()
    } else {
        input.highlights
    };
    BookDetailSnapshotApplyProjection {
        route: input.route,
        highlights,
        isbn_preview_request,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route() -> BookRoute {
        BookRoute {
            catalog_id: "isbn:9780143127550".to_string(),
            isbn: "9780143127550".to_string(),
        }
    }

    #[test]
    fn empty_snapshot_has_no_route_or_error() {
        let snapshot = BookDetailSnapshot::empty();

        assert!(snapshot.route.is_none());
        assert!(snapshot.highlights.is_empty());
        assert!(snapshot.error.is_empty());
    }

    #[test]
    fn error_snapshot_preserves_route_and_clears_rows() {
        let snapshot = error_snapshot(Some(route()), "cache miss");

        assert_eq!(snapshot.route.unwrap().catalog_id, "isbn:9780143127550");
        assert!(snapshot.highlights.is_empty());
        assert_eq!(snapshot.error, "cache miss");
    }

    #[test]
    fn snapshot_apply_projection_controls_route_preview_and_highlights() {
        let highlight = HighlightRecord {
            event_id: "highlight".into(),
            pubkey: "pubkey".into(),
            quote: "quote".into(),
            context: String::new(),
            note: String::new(),
            artifact_address: "isbn:9780143127550".into(),
            event_reference: String::new(),
            external_reference: String::new(),
            source_url: String::new(),
            source_reference_key: String::new(),
            clip_start_seconds: None,
            clip_end_seconds: None,
            clip_speaker: String::new(),
            clip_transcript_segment_ids: Vec::new(),
            image_url: String::new(),
            created_at: Some(42),
        };

        let success = snapshot_apply_projection(BookDetailSnapshotApplyInput {
            route: Some(route()),
            highlights: vec![highlight.clone()],
            error: String::new(),
        });
        assert_eq!(
            success.route.as_ref().map(|route| route.isbn.as_str()),
            Some("9780143127550")
        );
        assert_eq!(
            success.isbn_preview_request.as_deref(),
            Some("9780143127550")
        );
        assert_eq!(success.highlights.len(), 1);

        let missing_route = snapshot_apply_projection(BookDetailSnapshotApplyInput {
            route: None,
            highlights: vec![highlight.clone()],
            error: String::new(),
        });
        assert!(missing_route.route.is_none());
        assert_eq!(missing_route.isbn_preview_request, None);
        assert!(missing_route.highlights.is_empty());

        let failed = snapshot_apply_projection(BookDetailSnapshotApplyInput {
            route: Some(route()),
            highlights: vec![highlight],
            error: " cache failed ".into(),
        });
        assert!(failed.highlights.is_empty());
        assert_eq!(
            failed.isbn_preview_request.as_deref(),
            Some("9780143127550")
        );
    }
}
