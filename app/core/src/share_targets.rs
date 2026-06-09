use crate::articles;
use crate::errors::CoreError;
use crate::models::{ArticleRecord, ArtifactPreview, ArtifactRecord, HighlightRecord};
use ::url::Url;

#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareArticleTargetProjectionInput {
    pub article: ArticleRecord,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareArtifactTargetProjection {
    pub preview: ArtifactPreview,
    pub display_title: String,
    pub display_subtitle: String,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareArtifactTargetProjectionInput {
    pub artifact: ArtifactRecord,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareWebReaderTargetProjectionInput {
    pub preview: ArtifactPreview,
    pub fallback_url: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareWebReaderTargetSnapshot {
    pub target: Option<ShareArtifactTargetProjection>,
    pub ready: bool,
    pub error_message: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareHighlightTargetProjectionInput {
    pub highlight: HighlightRecord,
    pub relay_hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ShareHighlightTargetProjection {
    pub event_id: String,
    pub author_pubkey_hex: String,
    pub relay_hint: String,
    pub display_title: String,
    pub display_subtitle: String,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareHighlightArticleTargetProjectionInput {
    pub highlight: HighlightRecord,
}

pub fn article_target_projection(
    input: ShareArticleTargetProjectionInput,
) -> ShareArtifactTargetProjection {
    let article = input.article;
    let preview = articles::article_artifact_preview(&article);
    ShareArtifactTargetProjection {
        display_title: title_or_fallback(&article.title),
        display_subtitle: article.summary,
        image_url: non_empty_string(&article.image),
        preview,
    }
}

pub fn artifact_target_projection(
    input: ShareArtifactTargetProjectionInput,
) -> ShareArtifactTargetProjection {
    let preview = input.artifact.preview;
    ShareArtifactTargetProjection {
        display_title: title_or_fallback(&preview.title),
        display_subtitle: preview.description.clone(),
        image_url: non_empty_string(&preview.image),
        preview,
    }
}

pub fn web_reader_target_projection(
    input: ShareWebReaderTargetProjectionInput,
) -> ShareArtifactTargetProjection {
    let preview = input.preview;
    ShareArtifactTargetProjection {
        display_title: if preview.title.is_empty() {
            web_reader_fallback_title(&input.fallback_url)
        } else {
            preview.title.clone()
        },
        display_subtitle: preview.description.clone(),
        image_url: non_empty_string(&preview.image),
        preview,
    }
}

pub fn web_reader_target_snapshot(
    result: Result<ArtifactPreview, CoreError>,
    fallback_url: &str,
) -> ShareWebReaderTargetSnapshot {
    match result {
        Ok(preview) => ShareWebReaderTargetSnapshot {
            target: Some(web_reader_target_projection(
                ShareWebReaderTargetProjectionInput {
                    preview,
                    fallback_url: fallback_url.to_string(),
                },
            )),
            ready: true,
            error_message: String::new(),
        },
        Err(error) => ShareWebReaderTargetSnapshot {
            target: None,
            ready: false,
            error_message: share_preview_error_message(error),
        },
    }
}

pub fn highlight_target_projection(
    input: ShareHighlightTargetProjectionInput,
) -> ShareHighlightTargetProjection {
    let highlight = input.highlight;
    ShareHighlightTargetProjection {
        event_id: highlight.event_id,
        author_pubkey_hex: highlight.pubkey,
        relay_hint: input.relay_hint,
        display_title: if highlight.quote.is_empty() {
            "Highlight".into()
        } else {
            format!("\u{201C}{}\u{201D}", highlight.quote)
        },
        display_subtitle: highlight.note,
        image_url: None,
    }
}

pub fn highlight_article_target_projection(
    input: ShareHighlightArticleTargetProjectionInput,
) -> Option<ShareArtifactTargetProjection> {
    let highlight = input.highlight;
    let address = highlight.artifact_address.trim();
    if address.is_empty() {
        return None;
    }
    let preview = articles::article_artifact_preview_from_address(address)?;
    Some(ShareArtifactTargetProjection {
        preview,
        display_title: "Article".into(),
        display_subtitle: highlight.quote,
        image_url: None,
    })
}

fn title_or_fallback(title: &str) -> String {
    if title.is_empty() {
        "Untitled".into()
    } else {
        title.to_string()
    }
}

fn non_empty_string(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn web_reader_fallback_title(raw_url: &str) -> String {
    Url::parse(raw_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .unwrap_or_else(|| raw_url.to_string())
}

fn share_preview_error_message(error: CoreError) -> String {
    let reason = error.to_string();
    if reason.is_empty() {
        "Couldn't build a preview: Unknown error".into()
    } else {
        format!("Couldn't build a preview: {reason}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ArtifactPreview, Chapter};

    #[test]
    fn article_target_projects_preview_and_display_header() {
        let mut article = article_record();
        article.title = "Essay".into();
        article.summary = "Summary".into();
        article.image = "https://example.com/image.jpg".into();

        let projection = article_target_projection(ShareArticleTargetProjectionInput { article });

        assert_eq!(projection.display_title, "Essay");
        assert_eq!(projection.display_subtitle, "Summary");
        assert_eq!(
            projection.image_url,
            Some("https://example.com/image.jpg".into())
        );
        assert_eq!(projection.preview.title, "Essay");
        assert_eq!(projection.preview.source, "article");
    }

    #[test]
    fn article_target_falls_back_for_empty_title_and_image() {
        let mut article = article_record();
        article.title.clear();
        article.image.clear();

        let projection = article_target_projection(ShareArticleTargetProjectionInput { article });

        assert_eq!(projection.display_title, "Untitled");
        assert_eq!(projection.image_url, None);
    }

    #[test]
    fn artifact_target_projects_existing_preview() {
        let mut artifact = artifact_record();
        artifact.preview.title = "Artifact".into();
        artifact.preview.description = "Description".into();
        artifact.preview.image = "https://example.com/artifact.jpg".into();

        let projection =
            artifact_target_projection(ShareArtifactTargetProjectionInput { artifact });

        assert_eq!(projection.display_title, "Artifact");
        assert_eq!(projection.display_subtitle, "Description");
        assert_eq!(
            projection.image_url,
            Some("https://example.com/artifact.jpg".into())
        );
        assert_eq!(projection.preview.title, "Artifact");
    }

    #[test]
    fn web_reader_target_uses_title_or_url_host_fallback() {
        let mut preview = artifact_record().preview;
        preview.title.clear();
        preview.description = "Description".into();
        preview.image = "https://example.com/image.jpg".into();

        let projection = web_reader_target_projection(ShareWebReaderTargetProjectionInput {
            preview,
            fallback_url: "https://example.com/article".into(),
        });

        assert_eq!(projection.display_title, "example.com");
        assert_eq!(projection.display_subtitle, "Description");
        assert_eq!(
            projection.image_url,
            Some("https://example.com/image.jpg".into())
        );

        let mut preview = projection.preview;
        preview.title = "Page title".into();

        let projection = web_reader_target_projection(ShareWebReaderTargetProjectionInput {
            preview,
            fallback_url: "not a url".into(),
        });

        assert_eq!(projection.display_title, "Page title");
    }

    #[test]
    fn web_reader_target_snapshot_projects_success_and_error_copy() {
        let mut preview = artifact_record().preview;
        preview.title.clear();

        let snapshot = web_reader_target_snapshot(Ok(preview), "https://example.com/post");
        assert!(snapshot.ready);
        assert!(snapshot.error_message.is_empty());
        assert_eq!(
            snapshot
                .target
                .as_ref()
                .map(|target| target.display_title.as_str()),
            Some("example.com")
        );

        let snapshot = web_reader_target_snapshot(
            Err(CoreError::InvalidInput("invalid URL".into())),
            "not a url",
        );
        assert!(!snapshot.ready);
        assert!(snapshot.target.is_none());
        assert_eq!(
            snapshot.error_message,
            "Couldn't build a preview: invalid input: invalid URL"
        );
    }

    #[test]
    fn highlight_target_projects_repost_payload_and_snippet() {
        let mut highlight = highlight_record();
        highlight.quote = "A quoted passage".into();
        highlight.note = "Worth sharing".into();

        let projection = highlight_target_projection(ShareHighlightTargetProjectionInput {
            highlight,
            relay_hint: "wss://relay.example".into(),
        });

        assert_eq!(projection.event_id, "highlight");
        assert_eq!(projection.author_pubkey_hex, "author");
        assert_eq!(projection.relay_hint, "wss://relay.example");
        assert_eq!(projection.display_title, "\u{201C}A quoted passage\u{201D}");
        assert_eq!(projection.display_subtitle, "Worth sharing");
        assert_eq!(projection.image_url, None);
    }

    #[test]
    fn highlight_target_falls_back_without_quote() {
        let mut highlight = highlight_record();
        highlight.quote.clear();

        let projection = highlight_target_projection(ShareHighlightTargetProjectionInput {
            highlight,
            relay_hint: String::new(),
        });

        assert_eq!(projection.display_title, "Highlight");
    }

    #[test]
    fn highlight_article_target_projects_minimal_article_preview() {
        let mut highlight = highlight_record();
        highlight.artifact_address = "30023:author:d-tag".into();
        highlight.quote = "Quote".into();

        let projection =
            highlight_article_target_projection(ShareHighlightArticleTargetProjectionInput {
                highlight,
            })
            .expect("projection");

        assert_eq!(projection.display_title, "Article");
        assert_eq!(projection.display_subtitle, "Quote");
        assert_eq!(projection.image_url, None);
        assert_eq!(projection.preview.reference_tag_name, "a");
        assert_eq!(projection.preview.reference_tag_value, "30023:author:d-tag");
    }

    #[test]
    fn highlight_article_target_rejects_empty_or_invalid_address() {
        let mut highlight = highlight_record();
        highlight.artifact_address.clear();
        assert!(
            highlight_article_target_projection(ShareHighlightArticleTargetProjectionInput {
                highlight: highlight.clone()
            })
            .is_none()
        );

        highlight.artifact_address = "1:author:note".into();
        assert!(
            highlight_article_target_projection(ShareHighlightArticleTargetProjectionInput {
                highlight
            })
            .is_none()
        );
    }

    fn article_record() -> ArticleRecord {
        ArticleRecord {
            event_id: "article-event".into(),
            address: "30023:author:d-tag".into(),
            pubkey: "author".into(),
            identifier: "d-tag".into(),
            title: "Title".into(),
            summary: String::new(),
            image: String::new(),
            content: String::new(),
            hashtags: Vec::new(),
            published_at: Some(1_000),
            created_at: Some(900),
        }
    }

    fn artifact_record() -> ArtifactRecord {
        ArtifactRecord {
            preview: ArtifactPreview {
                id: "artifact".into(),
                url: "https://example.com/read".into(),
                title: "Title".into(),
                author: "Author".into(),
                image: String::new(),
                description: String::new(),
                source: "article".into(),
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
            },
            group_id: "group".into(),
            share_event_id: "share".into(),
            pubkey: "sharer".into(),
            created_at: Some(100),
            note: String::new(),
        }
    }

    fn highlight_record() -> HighlightRecord {
        HighlightRecord {
            event_id: "highlight".into(),
            pubkey: "author".into(),
            quote: "Quote".into(),
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
            created_at: Some(123),
        }
    }
}
