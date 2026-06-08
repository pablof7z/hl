//! Artifact detail routing projections. Platform shells render native screens,
//! but Rust owns the source/reference interpretation so iOS and Android open
//! the same target for the same kind:11 artifact share.

use crate::artifacts::normalize_artifact_url;
use crate::models::{ArtifactDetailRoute, ArtifactDetailTarget, ArtifactPreview, ArtifactRecord};

pub fn route_for_artifact(artifact: &ArtifactRecord) -> ArtifactDetailRoute {
    let preview = &artifact.preview;
    match preview.source.trim().to_ascii_lowercase().as_str() {
        "podcast" => route(ArtifactDetailTarget::Podcast),
        "article" => article_route(preview)
            .or_else(|| web_route(preview))
            .unwrap_or_else(unavailable_route),
        "book" => book_route(preview)
            .or_else(|| web_route(preview))
            .unwrap_or_else(unavailable_route),
        _ => web_route(preview).unwrap_or_else(unavailable_route),
    }
}

fn article_route(preview: &ArtifactPreview) -> Option<ArtifactDetailRoute> {
    let raw = reference_value_for(preview, "a")?;
    let (pubkey, d_tag) = parse_nip23_address(raw)?;
    Some(ArtifactDetailRoute {
        target: ArtifactDetailTarget::Article,
        article_pubkey: pubkey,
        article_d_tag: d_tag,
        book_catalog_id: String::new(),
        url: String::new(),
    })
}

fn book_route(preview: &ArtifactPreview) -> Option<ArtifactDetailRoute> {
    let catalog_id = first_non_empty(&[
        preview.catalog_id.as_str(),
        if preview.reference_tag_name.eq_ignore_ascii_case("i") {
            preview.reference_tag_value.as_str()
        } else {
            ""
        },
        if preview.highlight_tag_name.eq_ignore_ascii_case("i") {
            preview.highlight_tag_value.as_str()
        } else {
            ""
        },
    ]);
    if catalog_id.is_empty() || !is_book_catalog_id(&catalog_id) {
        return None;
    }
    Some(ArtifactDetailRoute {
        target: ArtifactDetailTarget::Book,
        article_pubkey: String::new(),
        article_d_tag: String::new(),
        book_catalog_id: catalog_id,
        url: String::new(),
    })
}

fn web_route(preview: &ArtifactPreview) -> Option<ArtifactDetailRoute> {
    let url = url_for_preview(preview)?;
    Some(ArtifactDetailRoute {
        target: ArtifactDetailTarget::Web,
        article_pubkey: String::new(),
        article_d_tag: String::new(),
        book_catalog_id: String::new(),
        url,
    })
}

fn unavailable_route() -> ArtifactDetailRoute {
    route(ArtifactDetailTarget::Unavailable)
}

fn route(target: ArtifactDetailTarget) -> ArtifactDetailRoute {
    ArtifactDetailRoute {
        target,
        article_pubkey: String::new(),
        article_d_tag: String::new(),
        book_catalog_id: String::new(),
        url: String::new(),
    }
}

fn reference_value_for<'a>(preview: &'a ArtifactPreview, tag_name: &str) -> Option<&'a str> {
    if preview.highlight_tag_name.eq_ignore_ascii_case(tag_name)
        && !preview.highlight_tag_value.trim().is_empty()
    {
        return Some(preview.highlight_tag_value.as_str());
    }
    if preview.reference_tag_name.eq_ignore_ascii_case(tag_name)
        && !preview.reference_tag_value.trim().is_empty()
    {
        return Some(preview.reference_tag_value.as_str());
    }
    None
}

fn parse_nip23_address(raw: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = raw.trim().splitn(3, ':').collect();
    if parts.len() != 3 || parts[0] != "30023" || parts[1].is_empty() || parts[2].is_empty() {
        return None;
    }
    Some((parts[1].to_string(), parts[2].to_string()))
}

fn url_for_preview(preview: &ArtifactPreview) -> Option<String> {
    let candidates = [
        preview.url.as_str(),
        url_reference_value(&preview.reference_tag_name, &preview.reference_tag_value),
        url_reference_value(&preview.highlight_tag_name, &preview.highlight_tag_value),
        preview.audio_url.as_str(),
        preview.audio_preview_url.as_str(),
    ];
    candidates.into_iter().find_map(normalize_reference_url)
}

fn url_reference_value<'a>(tag_name: &str, value: &'a str) -> &'a str {
    match tag_name.trim().to_ascii_lowercase().as_str() {
        "r" | "u" | "i" => value,
        _ => "",
    }
}

fn normalize_reference_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let candidate = trimmed.strip_prefix("url:").unwrap_or(trimmed);
    normalize_artifact_url(candidate)
}

fn is_book_catalog_id(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    normalized.starts_with("isbn:")
        || normalized.starts_with("openlibrary:")
        || normalized.starts_with("goodreads:")
}

fn first_non_empty(values: &[&str]) -> String {
    values
        .iter()
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifacts::{build_preview_with, PreviewInput};

    #[test]
    fn routes_podcast_by_source() {
        let mut preview = preview("podcast");
        preview.audio_url = "https://cdn.example.com/episode.mp3".into();
        let route = route_for_artifact(&record(preview));
        assert_eq!(route.target, ArtifactDetailTarget::Podcast);
    }

    #[test]
    fn routes_article_from_nip23_address() {
        let mut preview = preview("article");
        preview.reference_tag_name = "a".into();
        preview.reference_tag_value = "30023:abcdef:my-article".into();
        let route = route_for_artifact(&record(preview));
        assert_eq!(route.target, ArtifactDetailTarget::Article);
        assert_eq!(route.article_pubkey, "abcdef");
        assert_eq!(route.article_d_tag, "my-article");
    }

    #[test]
    fn routes_article_to_web_when_address_missing_but_url_present() {
        let mut preview = preview("article");
        preview.reference_tag_name = String::new();
        preview.reference_tag_value = String::new();
        let route = route_for_artifact(&record(preview));
        assert_eq!(route.target, ArtifactDetailTarget::Web);
        assert_eq!(route.url, "https://example.com/read");
    }

    #[test]
    fn routes_book_from_isbn_catalog_id() {
        let mut preview = preview("book");
        preview.catalog_id = "isbn:9780593716717".into();
        let route = route_for_artifact(&record(preview));
        assert_eq!(route.target, ArtifactDetailTarget::Book);
        assert_eq!(route.book_catalog_id, "isbn:9780593716717");
    }

    #[test]
    fn routes_url_backed_unknown_sources_to_web() {
        let mut preview = preview("video");
        preview.url = "https://youtube.com/watch?v=abc&utm_source=newsletter".into();
        let route = route_for_artifact(&record(preview));
        assert_eq!(route.target, ArtifactDetailTarget::Web);
        assert_eq!(route.url, "https://youtube.com/watch?v=abc");
    }

    #[test]
    fn routes_url_prefix_references_to_web() {
        let mut preview = preview("web");
        preview.url = String::new();
        preview.reference_tag_name = "i".into();
        preview.reference_tag_value = "url:https://example.com/a#ignored".into();
        let route = route_for_artifact(&record(preview));
        assert_eq!(route.target, ArtifactDetailTarget::Web);
        assert_eq!(route.url, "https://example.com/a");
    }

    #[test]
    fn rejects_unavailable_without_supported_reference() {
        let mut preview = preview("paper");
        preview.url = String::new();
        preview.reference_tag_name = "i".into();
        preview.reference_tag_value = "doi:10.0000/example".into();
        preview.highlight_tag_name = String::new();
        preview.highlight_tag_value = String::new();
        let route = route_for_artifact(&record(preview));
        assert_eq!(route.target, ArtifactDetailTarget::Unavailable);
    }

    fn preview(source: &str) -> ArtifactPreview {
        build_preview_with(PreviewInput {
            url: "https://example.com/read".into(),
            source: Some(source.into()),
            ..Default::default()
        })
        .unwrap()
    }

    fn record(preview: ArtifactPreview) -> ArtifactRecord {
        ArtifactRecord {
            preview,
            group_id: "group".into(),
            share_event_id: "event".into(),
            pubkey: "pubkey".into(),
            created_at: Some(1),
            note: String::new(),
        }
    }
}
