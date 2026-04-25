//! NIP-68 picture events (`kind:20`) — the OCR-fallback path for the
//! capture flow. When the user can't or doesn't want to extract a quote
//! from a captured book photo, the image still gets published, attached
//! via a NIP-92 `imeta` tag, and lands in the target NIP-29 community
//! directly (kind:20 carries the `h` tag inline; no kind:16 wrapper).

use nostr_sdk::prelude::*;

use crate::errors::CoreError;
use crate::highlights::build_imeta_tag;
use crate::models::{ArtifactRecord, BlossomUpload, PictureDraft, PictureRecord};
use crate::nostr_runtime::NostrRuntime;

const KIND_PICTURE: u16 = 20;

/// Publish a NIP-68 `kind:20` picture into a NIP-29 group. The image must
/// already be on Blossom (see `blossom::upload_blob`).
pub async fn publish_picture(
    runtime: &NostrRuntime,
    draft: PictureDraft,
) -> Result<PictureRecord, CoreError> {
    if draft.image.url.trim().is_empty() {
        return Err(CoreError::InvalidInput("image must have a url".into()));
    }
    let group_id = draft
        .target_group_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let builder = build_picture_event(
        group_id,
        &draft.image,
        draft.artifact.as_ref(),
        &draft.note,
    )?;
    let client = runtime.client();
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign picture: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish picture: {e}")))?;

    Ok(PictureRecord {
        event_id: event.id.to_hex(),
        pubkey: event.pubkey.to_hex(),
        group_id: group_id.unwrap_or_default().to_string(),
        note: draft.note.trim().to_string(),
        image_url: draft.image.url.clone(),
        image_sha256: draft.image.sha256_hex.clone(),
        artifact_reference_key: draft
            .artifact
            .as_ref()
            .map(artifact_reference_key)
            .unwrap_or_default(),
        created_at: Some(event.created_at.as_secs()),
    })
}

/// Pure builder for the `kind:20` event. Unit-testable.
fn build_picture_event(
    group_id: Option<&str>,
    image: &BlossomUpload,
    artifact: Option<&ArtifactRecord>,
    note: &str,
) -> Result<EventBuilder, CoreError> {
    let mut tags: Vec<Tag> = Vec::new();
    if let Some(gid) = group_id {
        tags.push(parse_tag(&["h", gid])?);
    }
    tags.push(build_imeta_tag(image)?);

    if let Some(artifact) = artifact {
        let ref_name = artifact.preview.highlight_tag_name.trim();
        let ref_value = artifact.preview.highlight_tag_value.trim();
        if !ref_name.is_empty() && !ref_value.is_empty() {
            tags.push(parse_tag(&[ref_name, ref_value])?);
        }
    }

    Ok(EventBuilder::new(Kind::Custom(KIND_PICTURE), note.trim()).tags(tags))
}

fn artifact_reference_key(artifact: &ArtifactRecord) -> String {
    let name = artifact.preview.highlight_tag_name.trim();
    let value = artifact.preview.highlight_tag_value.trim();
    if name.is_empty() || value.is_empty() {
        String::new()
    } else {
        format!("{name}:{value}")
    }
}

fn parse_tag(parts: &[&str]) -> Result<Tag, CoreError> {
    Tag::parse(parts.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .map_err(|e| CoreError::Other(format!("build tag: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ArtifactPreview, ArtifactRecord};

    fn sample_image() -> BlossomUpload {
        BlossomUpload {
            url: "https://blossom.primal.net/abc.jpg".into(),
            sha256_hex: "abc123".into(),
            mime: "image/jpeg".into(),
            size_bytes: 4096,
            width: 1024,
            height: 768,
            alt: "page text".into(),
        }
    }

    fn sample_artifact() -> ArtifactRecord {
        ArtifactRecord {
            preview: ArtifactPreview {
                id: "id1".into(),
                url: "https://example.com/book".into(),
                title: "Book".into(),
                author: "Author".into(),
                image: String::new(),
                description: String::new(),
                source: "book".into(),
                domain: "example.com".into(),
                catalog_id: "isbn:9781234567890".into(),
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
                reference_tag_value: "isbn:9781234567890".into(),
                reference_kind: "isbn".into(),
                highlight_tag_name: "i".into(),
                highlight_tag_value: "isbn:9781234567890".into(),
                highlight_reference_key: "i:isbn:9781234567890".into(),
                chapters: Vec::new(),
            },
            group_id: "group-a".into(),
            share_event_id: "share-1".into(),
            pubkey: "f".repeat(64),
            created_at: Some(1_700_000_000),
            note: String::new(),
        }
    }

    fn tag_pairs(builder: &EventBuilder) -> Vec<Vec<String>> {
        let keys = Keys::generate();
        let event = builder
            .clone()
            .sign_with_keys(&keys)
            .expect("sign for inspection");
        event.tags.iter().map(|t| t.as_slice().to_vec()).collect()
    }

    #[test]
    fn picture_event_kind_is_20() {
        let builder = build_picture_event(Some("group-a"), &sample_image(), None, "")
            .expect("build picture event");
        let event = builder.sign_with_keys(&Keys::generate()).expect("sign");
        assert_eq!(event.kind, Kind::Custom(20));
    }

    #[test]
    fn picture_event_has_h_and_imeta() {
        let builder = build_picture_event(Some("group-a"), &sample_image(), None, "hello")
            .expect("build");
        let tags = tag_pairs(&builder);

        assert!(
            tags.iter()
                .any(|t| t.first().map(String::as_str) == Some("h")
                    && t.get(1).map(String::as_str) == Some("group-a")),
            "h tag missing or wrong: {tags:?}"
        );
        let imeta = tags
            .iter()
            .find(|t| t.first().map(String::as_str) == Some("imeta"))
            .expect("imeta tag present");
        let parts: Vec<&str> = imeta.iter().skip(1).map(String::as_str).collect();
        assert!(parts.contains(&"url https://blossom.primal.net/abc.jpg"));
        assert!(parts.contains(&"m image/jpeg"));
        assert!(parts.contains(&"x abc123"));
        assert!(parts.contains(&"dim 1024x768"));
    }

    #[test]
    fn picture_event_includes_artifact_reference_when_provided() {
        let artifact = sample_artifact();
        let builder = build_picture_event(Some("group-a"), &sample_image(), Some(&artifact), "")
            .expect("build");
        let tags = tag_pairs(&builder);
        assert!(
            tags.iter().any(|t| t.first().map(String::as_str) == Some("i")
                && t.get(1).map(String::as_str) == Some("isbn:9781234567890")),
            "artifact ref tag missing: {tags:?}"
        );
    }

    #[test]
    fn picture_event_omits_artifact_reference_when_none() {
        let builder = build_picture_event(Some("group-a"), &sample_image(), None, "")
            .expect("build");
        let tags = tag_pairs(&builder);
        assert!(
            !tags
                .iter()
                .any(|t| matches!(
                    t.first().map(String::as_str),
                    Some("i") | Some("a") | Some("e") | Some("r")
                )),
            "no artifact ref tag expected: {tags:?}"
        );
    }

    #[test]
    fn picture_event_content_is_trimmed_note() {
        let builder = build_picture_event(Some("group-a"), &sample_image(), None, "  hi  ")
            .expect("build");
        let event = builder.sign_with_keys(&Keys::generate()).expect("sign");
        assert_eq!(event.content, "hi");
    }
}
