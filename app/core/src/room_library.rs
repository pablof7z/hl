use crate::artifact_detail;
use crate::models::{ArtifactDetailTarget, ArtifactRecord};

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomLibraryArticleCardProjectionInput {
    pub artifact: ArtifactRecord,
    pub comment_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoomLibraryArticleCardProjection {
    pub article_author_pubkey: Option<String>,
    pub avatar_pubkey: String,
    pub author_profile_pubkey: String,
    pub relative_unix_seconds: Option<u64>,
    pub meta_bits: Vec<String>,
}

/// Presentation projection for an article artifact in a room library. Rust
/// owns route-derived article author identity, avatar fallback identity,
/// relative timestamp source, and domain/comment meta bits.
pub fn article_card_projection(
    input: RoomLibraryArticleCardProjectionInput,
) -> RoomLibraryArticleCardProjection {
    let artifact = input.artifact;
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
        article_author_pubkey,
        avatar_pubkey,
        author_profile_pubkey,
        relative_unix_seconds: artifact.created_at.filter(|seconds| *seconds > 0),
        meta_bits: article_meta_bits(&artifact, input.comment_count),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ArtifactPreview;

    #[test]
    fn article_card_projection_uses_article_author_and_meta_bits() {
        let mut artifact = artifact_record("article");
        artifact.preview.reference_tag_name = "a".into();
        artifact.preview.reference_tag_value = "30023:authorpubkey:essay".into();
        artifact.preview.domain = "example.com".into();
        artifact.created_at = Some(123);

        let projection = article_card_projection(RoomLibraryArticleCardProjectionInput {
            artifact,
            comment_count: 2,
        });

        assert_eq!(
            projection.article_author_pubkey,
            Some("authorpubkey".into())
        );
        assert_eq!(projection.avatar_pubkey, "authorpubkey");
        assert_eq!(projection.author_profile_pubkey, "authorpubkey");
        assert_eq!(projection.relative_unix_seconds, Some(123));
        assert_eq!(
            projection.meta_bits,
            vec!["example.com".to_string(), "2 comments".to_string()]
        );
    }

    #[test]
    fn article_card_projection_preserves_fallbacks_without_article_route() {
        let mut artifact = artifact_record("web");
        artifact.pubkey = "sharerpubkey".into();
        artifact.created_at = Some(0);

        let projection = article_card_projection(RoomLibraryArticleCardProjectionInput {
            artifact,
            comment_count: 1,
        });

        assert_eq!(projection.article_author_pubkey, None);
        assert_eq!(projection.avatar_pubkey, "sharerpubkey");
        assert_eq!(projection.author_profile_pubkey, "");
        assert_eq!(projection.relative_unix_seconds, None);
        assert_eq!(projection.meta_bits, vec!["1 comment".to_string()]);
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
