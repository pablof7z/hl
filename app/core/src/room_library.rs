use crate::artifact_detail;
use crate::models::{ArtifactDetailTarget, ArtifactRecord};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum RoomLibraryCardKind {
    Article,
    Book,
    Podcast,
    Generic,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomLibraryCardKindProjectionInput {
    pub artifact: ArtifactRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoomLibraryCardKindProjection {
    pub card_kind: RoomLibraryCardKind,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomLibraryArticleCardProjectionInput {
    pub artifact: ArtifactRecord,
    pub comment_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoomLibraryArticleCardProjection {
    pub display_title: String,
    pub title_is_fallback: bool,
    pub image_url: Option<String>,
    pub article_author_pubkey: Option<String>,
    pub avatar_pubkey: String,
    pub author_profile_pubkey: String,
    pub relative_unix_seconds: Option<u64>,
    pub meta_text: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomLibraryBookCardProjectionInput {
    pub artifact: ArtifactRecord,
    pub comment_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoomLibraryBookCardProjection {
    pub title: String,
    pub title_is_fallback: bool,
    pub author_label: Option<String>,
    pub summary: Option<String>,
    pub image_url: Option<String>,
    pub sharer_pubkey: String,
    pub relative_unix_seconds: Option<u64>,
    pub comment_badge_label: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomLibraryPodcastCardProjectionInput {
    pub artifact: ArtifactRecord,
    pub comment_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoomLibraryPodcastCardProjection {
    pub title: String,
    pub title_is_fallback: bool,
    pub show_label: Option<String>,
    pub duration_label: Option<String>,
    pub image_url: Option<String>,
    pub sharer_pubkey: String,
    pub relative_unix_seconds: Option<u64>,
    pub comment_badge_label: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomLibraryGenericCardProjectionInput {
    pub artifact: ArtifactRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoomLibraryGenericCardProjection {
    pub title: String,
}

/// Select the native room-library card component for an artifact. Rust owns
/// source normalization; native shells only dispatch to the visual component.
pub fn card_kind_projection(
    input: RoomLibraryCardKindProjectionInput,
) -> RoomLibraryCardKindProjection {
    RoomLibraryCardKindProjection {
        card_kind: card_kind_for_artifact(&input.artifact),
    }
}

/// Presentation projection for an article artifact in a room library. Rust
/// owns route-derived article author identity, avatar fallback identity,
/// relative timestamp source, and domain/comment meta bits.
pub fn article_card_projection(
    input: RoomLibraryArticleCardProjectionInput,
) -> RoomLibraryArticleCardProjection {
    let artifact = input.artifact;
    let (display_title, title_is_fallback) = title_or_fallback(&artifact);
    let meta_bits = article_meta_bits(&artifact, input.comment_count);
    let route = artifact_detail::route_for_artifact(&artifact);
    let article_author_pubkey =
        if route.target == ArtifactDetailTarget::Article && !route.article_pubkey.is_empty() {
            Some(route.article_pubkey)
        } else {
            None
        };
    let author_profile_pubkey = article_author_pubkey.clone().unwrap_or_default();
    let avatar_pubkey = article_author_pubkey
        .clone()
        .unwrap_or_else(|| artifact.pubkey.clone());

    RoomLibraryArticleCardProjection {
        display_title,
        title_is_fallback,
        image_url: non_empty_string(&artifact.preview.image),
        article_author_pubkey,
        avatar_pubkey,
        author_profile_pubkey,
        relative_unix_seconds: artifact.created_at.filter(|seconds| *seconds > 0),
        meta_text: joined_meta_text(&meta_bits),
    }
}

/// Presentation projection for a book artifact in a room library. Rust owns
/// display fallbacks and badges; Swift only renders the native layout.
pub fn book_card_projection(
    input: RoomLibraryBookCardProjectionInput,
) -> RoomLibraryBookCardProjection {
    let artifact = input.artifact;
    let (title, title_is_fallback) = title_or_fallback(&artifact);
    let relative_unix_seconds = relative_unix_seconds(&artifact);

    RoomLibraryBookCardProjection {
        title,
        title_is_fallback,
        author_label: non_empty_string(&artifact.preview.author),
        summary: non_empty_string(&artifact.preview.description),
        image_url: non_empty_string(&artifact.preview.image),
        sharer_pubkey: artifact.pubkey,
        relative_unix_seconds,
        comment_badge_label: comment_badge_label(input.comment_count),
    }
}

/// Presentation projection for a podcast artifact in a room library. Rust
/// owns episode/show fallback policy, duration text, and badges.
pub fn podcast_card_projection(
    input: RoomLibraryPodcastCardProjectionInput,
) -> RoomLibraryPodcastCardProjection {
    let artifact = input.artifact;
    let (title, title_is_fallback) = title_or_fallback(&artifact);
    let relative_unix_seconds = relative_unix_seconds(&artifact);
    let show_title = if artifact.preview.podcast_show_title.is_empty() {
        &artifact.preview.author
    } else {
        &artifact.preview.podcast_show_title
    };

    RoomLibraryPodcastCardProjection {
        title,
        title_is_fallback,
        show_label: non_empty_string(show_title),
        duration_label: duration_label(artifact.preview.duration_seconds),
        image_url: non_empty_string(&artifact.preview.image),
        sharer_pubkey: artifact.pubkey,
        relative_unix_seconds,
        comment_badge_label: comment_badge_label(input.comment_count),
    }
}

/// Presentation projection for a fallback artifact row in the room library.
/// Rust owns title fallback parity for source types without a specialized card.
pub fn generic_card_projection(
    input: RoomLibraryGenericCardProjectionInput,
) -> RoomLibraryGenericCardProjection {
    let (title, _) = title_or_fallback(&input.artifact);
    RoomLibraryGenericCardProjection { title }
}

fn article_meta_bits(artifact: &ArtifactRecord, comment_count: u32) -> Vec<String> {
    let mut out = Vec::new();
    if !artifact.preview.domain.is_empty() {
        out.push(artifact.preview.domain.clone());
    }
    if comment_count > 0 {
        out.push(format!(
            "{comment_count} comment{}",
            if comment_count == 1 { "" } else { "s" }
        ));
    }
    out
}

fn joined_meta_text(bits: &[String]) -> Option<String> {
    if bits.is_empty() {
        None
    } else {
        Some(bits.join(" · "))
    }
}

fn title_or_fallback(artifact: &ArtifactRecord) -> (String, bool) {
    if artifact.preview.title.is_empty() {
        ("Untitled".into(), true)
    } else {
        (artifact.preview.title.clone(), false)
    }
}

fn non_empty_string(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn relative_unix_seconds(artifact: &ArtifactRecord) -> Option<u64> {
    artifact.created_at.filter(|seconds| *seconds > 0)
}

fn comment_badge_label(comment_count: u32) -> Option<String> {
    if comment_count > 0 {
        Some(comment_count.to_string())
    } else {
        None
    }
}

fn duration_label(duration_seconds: Option<i64>) -> Option<String> {
    let secs = duration_seconds.filter(|seconds| *seconds > 0)?;
    let hours = secs / 3_600;
    let minutes = (secs % 3_600) / 60;
    if hours > 0 {
        Some(format!("{hours}h {minutes}m"))
    } else {
        Some(format!("{minutes}m"))
    }
}

fn card_kind_for_artifact(artifact: &ArtifactRecord) -> RoomLibraryCardKind {
    match artifact.preview.source.trim().to_ascii_lowercase().as_str() {
        "article" => RoomLibraryCardKind::Article,
        "book" => RoomLibraryCardKind::Book,
        "podcast" => RoomLibraryCardKind::Podcast,
        _ => RoomLibraryCardKind::Generic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ArtifactPreview;

    #[test]
    fn card_kind_projection_normalizes_room_library_sources() {
        for (source, expected) in [
            ("article", RoomLibraryCardKind::Article),
            (" book ", RoomLibraryCardKind::Book),
            ("Podcast", RoomLibraryCardKind::Podcast),
            ("web", RoomLibraryCardKind::Generic),
            ("", RoomLibraryCardKind::Generic),
        ] {
            let projection = card_kind_projection(RoomLibraryCardKindProjectionInput {
                artifact: artifact_record(source),
            });
            assert_eq!(projection.card_kind, expected);
        }
    }

    #[test]
    fn article_card_projection_uses_article_author_and_meta_bits() {
        let mut artifact = artifact_record("article");
        artifact.preview.title = "Article title".into();
        artifact.preview.image = "https://example.com/article.jpg".into();
        artifact.preview.reference_tag_name = "a".into();
        artifact.preview.reference_tag_value = "30023:authorpubkey:essay".into();
        artifact.preview.domain = "example.com".into();
        artifact.created_at = Some(123);

        let projection = article_card_projection(RoomLibraryArticleCardProjectionInput {
            artifact,
            comment_count: 2,
        });

        assert_eq!(projection.display_title, "Article title");
        assert!(!projection.title_is_fallback);
        assert_eq!(
            projection.image_url,
            Some("https://example.com/article.jpg".into())
        );
        assert_eq!(
            projection.article_author_pubkey,
            Some("authorpubkey".into())
        );
        assert_eq!(projection.avatar_pubkey, "authorpubkey");
        assert_eq!(projection.author_profile_pubkey, "authorpubkey");
        assert_eq!(projection.relative_unix_seconds, Some(123));
        assert_eq!(
            projection.meta_text,
            Some("example.com · 2 comments".to_string())
        );
    }

    #[test]
    fn article_card_projection_preserves_fallbacks_without_article_route() {
        let mut artifact = artifact_record("web");
        artifact.preview.title.clear();
        artifact.pubkey = "sharerpubkey".into();
        artifact.created_at = Some(0);

        let projection = article_card_projection(RoomLibraryArticleCardProjectionInput {
            artifact,
            comment_count: 1,
        });

        assert_eq!(projection.display_title, "Untitled");
        assert!(projection.title_is_fallback);
        assert_eq!(projection.image_url, None);
        assert_eq!(projection.article_author_pubkey, None);
        assert_eq!(projection.avatar_pubkey, "sharerpubkey");
        assert_eq!(projection.author_profile_pubkey, "");
        assert_eq!(projection.relative_unix_seconds, None);
        assert_eq!(projection.meta_text, Some("1 comment".to_string()));
    }

    #[test]
    fn book_card_projection_preserves_book_row_policy() {
        let mut artifact = artifact_record("book");
        artifact.preview.title = "The Book".into();
        artifact.preview.author = "Author".into();
        artifact.preview.description = "Summary".into();
        artifact.preview.image = "https://example.com/book.jpg".into();
        artifact.pubkey = "sharerpubkey".into();
        artifact.created_at = Some(456);

        let projection = book_card_projection(RoomLibraryBookCardProjectionInput {
            artifact,
            comment_count: 3,
        });

        assert_eq!(projection.title, "The Book");
        assert!(!projection.title_is_fallback);
        assert_eq!(projection.author_label, Some("Author".into()));
        assert_eq!(projection.summary, Some("Summary".into()));
        assert_eq!(
            projection.image_url,
            Some("https://example.com/book.jpg".into())
        );
        assert_eq!(projection.sharer_pubkey, "sharerpubkey");
        assert_eq!(projection.relative_unix_seconds, Some(456));
        assert_eq!(projection.comment_badge_label, Some("3".into()));
    }

    #[test]
    fn book_card_projection_falls_back_without_optional_bits() {
        let mut artifact = artifact_record("book");
        artifact.preview.title.clear();
        artifact.preview.author.clear();
        artifact.preview.description.clear();
        artifact.preview.image.clear();
        artifact.created_at = Some(0);

        let projection = book_card_projection(RoomLibraryBookCardProjectionInput {
            artifact,
            comment_count: 0,
        });

        assert_eq!(projection.title, "Untitled");
        assert!(projection.title_is_fallback);
        assert_eq!(projection.author_label, None);
        assert_eq!(projection.summary, None);
        assert_eq!(projection.image_url, None);
        assert_eq!(projection.relative_unix_seconds, None);
        assert_eq!(projection.comment_badge_label, None);
    }

    #[test]
    fn podcast_card_projection_preserves_show_duration_and_badges() {
        let mut artifact = artifact_record("podcast");
        artifact.preview.title = "Episode".into();
        artifact.preview.author = "Fallback Show".into();
        artifact.preview.podcast_show_title = "Actual Show".into();
        artifact.preview.duration_seconds = Some(3_900);
        artifact.preview.image = "https://example.com/podcast.jpg".into();
        artifact.pubkey = "podcaster".into();
        artifact.created_at = Some(789);

        let projection = podcast_card_projection(RoomLibraryPodcastCardProjectionInput {
            artifact,
            comment_count: 1,
        });

        assert_eq!(projection.title, "Episode");
        assert!(!projection.title_is_fallback);
        assert_eq!(projection.show_label, Some("Actual Show".into()));
        assert_eq!(projection.duration_label, Some("1h 5m".into()));
        assert_eq!(
            projection.image_url,
            Some("https://example.com/podcast.jpg".into())
        );
        assert_eq!(projection.sharer_pubkey, "podcaster");
        assert_eq!(projection.relative_unix_seconds, Some(789));
        assert_eq!(projection.comment_badge_label, Some("1".into()));
    }

    #[test]
    fn podcast_card_projection_uses_author_show_fallback_and_short_duration() {
        let mut artifact = artifact_record("podcast");
        artifact.preview.title.clear();
        artifact.preview.author = "Author Show".into();
        artifact.preview.podcast_show_title.clear();
        artifact.preview.duration_seconds = Some(59);

        let projection = podcast_card_projection(RoomLibraryPodcastCardProjectionInput {
            artifact,
            comment_count: 0,
        });

        assert_eq!(projection.title, "Untitled");
        assert!(projection.title_is_fallback);
        assert_eq!(projection.show_label, Some("Author Show".into()));
        assert_eq!(projection.duration_label, Some("0m".into()));
        assert_eq!(projection.comment_badge_label, None);
    }

    #[test]
    fn generic_card_projection_preserves_default_row_title_fallback() {
        let mut artifact = artifact_record("web");
        artifact.preview.title.clear();

        let projection =
            generic_card_projection(RoomLibraryGenericCardProjectionInput { artifact });

        assert_eq!(projection.title, "Untitled");

        let mut artifact = artifact_record("paper");
        artifact.preview.title = "Paper".into();
        let projection =
            generic_card_projection(RoomLibraryGenericCardProjectionInput { artifact });

        assert_eq!(projection.title, "Paper");
    }

    fn artifact_record(source: &str) -> ArtifactRecord {
        ArtifactRecord {
            preview: ArtifactPreview {
                id: "artifact".into(),
                url: "https://example.com/read".into(),
                title: "Title".into(),
                author: "Author".into(),
                image: String::new(),
                description: String::new(),
                source: source.into(),
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
            },
            group_id: "group".into(),
            share_event_id: "share".into(),
            pubkey: "sharer".into(),
            created_at: Some(100),
            note: String::new(),
        }
    }
}
