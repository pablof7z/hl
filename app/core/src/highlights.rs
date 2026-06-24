//! NIP-84 highlights (kind:9802) + cross-community shares (kind:16). Ports
//! `web/src/lib/ndk/highlights.ts`.

use std::collections::{BTreeMap, HashSet};

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;
use crate::models::{
    ArtifactRecord, BlossomUpload, BookRoute, HighlightDraft, HighlightRecord, HighlightSourceKind,
    HydratedHighlight, ProfileMetadata,
};
use crate::nostr_runtime::NostrRuntime;
use crate::profile::{
    profile_display_projection, profile_display_with_label_projection, ProfileDisplayFallback,
    ProfileDisplayProjectionInput, ProfileDisplayWithLabelProjectionInput,
};
use crate::relays::highlighter_relay;
use ::url::Url;

/// NIP-84 highlight event.
const KIND_HIGHLIGHT: u16 = 9802;
/// NIP-18 generic repost used to share a highlight into a NIP-29 community.
const KIND_GENERIC_REPOST: u16 = 16;

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlightGroupHighlighterProfile {
    pub pubkey: String,
    pub profile: Option<ProfileMetadata>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlightGroupCardProjectionInput {
    pub items: Vec<HydratedHighlight>,
    pub highlighter_profiles: Vec<HighlightGroupHighlighterProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HighlightGroupHighlighterProjection {
    pub pubkey: String,
    pub display_name: String,
    pub display_initial: String,
    pub picture_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HighlightGroupLabelSegment {
    pub text: String,
    pub emphasized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HighlightGroupCardProjection {
    pub show_highlighters_strip: bool,
    pub visible_highlighters: Vec<HighlightGroupHighlighterProjection>,
    pub overflow_count: u32,
    pub highlighters_label_segments: Vec<HighlightGroupLabelSegment>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlightResourceAuthorProfile {
    pub pubkey: String,
    pub profile: Option<ProfileMetadata>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlightResourceHeaderProjectionInput {
    pub lead: HydratedHighlight,
    pub source_article: Option<crate::models::ArticleRecord>,
    pub source_article_author_pubkey: String,
    pub article_author_profiles: Vec<HighlightResourceAuthorProfile>,
    pub book_preview: Option<crate::models::ArtifactPreview>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HighlightResourceHeaderProjection {
    pub source_kind: HighlightSourceKind,
    pub icon_system_name: String,
    pub title: String,
    pub author_or_domain: String,
    pub time_label: Option<String>,
    pub cover_url: Option<String>,
    pub book_isbn: Option<String>,
    pub article_address: Option<String>,
    pub article_author_pubkey: String,
    pub web_metadata_url: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlightDetailResourceProjectionInput {
    pub item: HydratedHighlight,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HighlightDetailResourceProjection {
    pub source_kind: HighlightSourceKind,
    pub kind_label: String,
    pub icon_system_name: String,
    pub title: String,
    pub author: String,
    pub cover_url: Option<String>,
    pub article_route: Option<crate::models::ArticleReaderRoute>,
    pub book_catalog_id: Option<String>,
    pub web_url: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlightFeedContentProjectionInput {
    pub highlight: HighlightRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HighlightFeedContentProjection {
    pub quote_text: String,
    pub note_text: Option<String>,
    pub page_image_url: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct HighlightDetailContentProjectionInput {
    pub highlight: HighlightRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HighlightDetailContentProjection {
    pub quote_text: String,
    pub note_text: Option<String>,
    pub page_image_url: Option<String>,
    pub share_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HighlightShareUrlSnapshot {
    pub share_url: Option<String>,
    pub ready: bool,
    pub error_message: String,
}

/// Native article-reader highlight publish input. `error` is empty before
/// publish and carries the Rust outcome error after publish.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ArticleHighlightPublishProjectionInput {
    pub note: String,
    pub error: String,
}

/// Native article-reader highlight publish projection. Rust owns the canonical
/// note body and the user-visible toast copy.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ArticleHighlightPublishProjection {
    pub submit_note: String,
    pub toast_message: String,
    pub is_success: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ArticleReaderSelectionProjectionInput {
    pub quote: String,
    pub context: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ArticleReaderSelectionProjection {
    pub quote: String,
    pub context: String,
    pub has_quote: bool,
}

/// Presentation projection for a grouped highlight card. Rust owns unique
/// highlighter order, strip visibility, avatar cap, overflow count, and the
/// social byline text sequence; native shells only style the segments.
pub fn highlight_group_card_projection(
    input: HighlightGroupCardProjectionInput,
) -> HighlightGroupCardProjection {
    let unique_pubkeys = unique_highlighter_pubkeys(&input.items);
    let show_highlighters_strip = input.items.len() >= 2 && unique_pubkeys.len() >= 2;
    let highlighters: Vec<HighlightGroupHighlighterProjection> = unique_pubkeys
        .iter()
        .map(|pubkey| highlighter_projection(pubkey, &input.highlighter_profiles))
        .collect();

    if !show_highlighters_strip {
        return HighlightGroupCardProjection {
            show_highlighters_strip,
            visible_highlighters: Vec::new(),
            overflow_count: 0,
            highlighters_label_segments: Vec::new(),
        };
    }

    HighlightGroupCardProjection {
        show_highlighters_strip,
        visible_highlighters: highlighters.iter().take(3).cloned().collect(),
        overflow_count: highlighters.len().saturating_sub(3) as u32,
        highlighters_label_segments: if show_highlighters_strip {
            highlighters_label_segments(&highlighters)
        } else {
            Vec::new()
        },
    }
}

fn unique_highlighter_pubkeys(items: &[HydratedHighlight]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        if seen.insert(item.highlight.pubkey.clone()) {
            out.push(item.highlight.pubkey.clone());
        }
    }
    out
}

fn highlighter_projection(
    pubkey: &str,
    profiles: &[HighlightGroupHighlighterProfile],
) -> HighlightGroupHighlighterProjection {
    let profile = profiles
        .iter()
        .find(|snapshot| snapshot.pubkey == pubkey)
        .and_then(|snapshot| snapshot.profile.clone());
    let display = profile_display_projection(ProfileDisplayProjectionInput {
        pubkey: pubkey.to_string(),
        profile,
        fallback: ProfileDisplayFallback::Pubkey10,
    });
    HighlightGroupHighlighterProjection {
        pubkey: pubkey.to_string(),
        display_name: display.display_name,
        display_initial: display.display_initial,
        picture_url: display.picture_url,
    }
}

fn highlighters_label_segments(
    highlighters: &[HighlightGroupHighlighterProjection],
) -> Vec<HighlightGroupLabelSegment> {
    let mut out = vec![plain_label_segment("Highlighted by ")];
    match highlighters.len() {
        0 => {}
        1 => out.push(emphasized_label_segment(&highlighters[0].display_name)),
        2 => {
            out.push(emphasized_label_segment(&highlighters[0].display_name));
            out.push(plain_label_segment(" and "));
            out.push(emphasized_label_segment(&highlighters[1].display_name));
        }
        _ => {
            out.push(emphasized_label_segment(&highlighters[0].display_name));
            out.push(plain_label_segment(", "));
            out.push(emphasized_label_segment(&highlighters[1].display_name));
            out.push(plain_label_segment(" and "));
            out.push(emphasized_label_segment(&format!(
                "{} others",
                highlighters.len() - 2
            )));
        }
    }
    out
}

fn plain_label_segment(text: &str) -> HighlightGroupLabelSegment {
    HighlightGroupLabelSegment {
        text: text.to_string(),
        emphasized: false,
    }
}

fn emphasized_label_segment(text: &str) -> HighlightGroupLabelSegment {
    HighlightGroupLabelSegment {
        text: text.to_string(),
        emphasized: true,
    }
}

/// Classify the source behind a highlight for native rendering. Rust owns this
/// source/reference interpretation so all platform shells show the same icon
/// and label for the same highlight.
pub fn source_kind(
    preview_source: &str,
    external_reference: &str,
    artifact_address: &str,
    source_url: &str,
) -> HighlightSourceKind {
    match preview_source.trim().to_ascii_lowercase().as_str() {
        "article" => return HighlightSourceKind::Article,
        "web" => return HighlightSourceKind::Web,
        "podcast" => return HighlightSourceKind::Podcast,
        "book" => return HighlightSourceKind::Book,
        "video" => return HighlightSourceKind::Video,
        "paper" => return HighlightSourceKind::Paper,
        "" => {}
        _ => return HighlightSourceKind::Unknown,
    }

    if is_isbn_reference(external_reference) {
        return HighlightSourceKind::Book;
    }
    if is_nip23_article_address(artifact_address) {
        return HighlightSourceKind::Article;
    }
    if is_isbn_reference(artifact_address) {
        return HighlightSourceKind::Book;
    }
    if !source_url.trim().is_empty() {
        return HighlightSourceKind::Web;
    }
    HighlightSourceKind::Unknown
}

/// Project the resource header for a grouped highlight card. Rust owns source
/// interpretation, title/author fallback precedence, lookup keys for async
/// enrichment, read-time/duration labels, and cover URL selection. Native
/// shells render the returned fields without branching on artifact semantics.
pub fn highlight_resource_header_projection(
    input: HighlightResourceHeaderProjectionInput,
) -> HighlightResourceHeaderProjection {
    let lead = input.lead;
    let preview = lead.artifact.as_ref().map(|artifact| &artifact.preview);
    let source_kind = source_kind(
        preview.map(|p| p.source.as_str()).unwrap_or_default(),
        &lead.highlight.external_reference,
        &lead.highlight.artifact_address,
        &lead.highlight.source_url,
    );
    let url_host = url_host(&lead.highlight.source_url);
    let book_route = book_route_for_highlight(
        &lead.highlight.external_reference,
        &lead.highlight.artifact_address,
    );
    let article_address = article_address_for_highlight(&lead.highlight.artifact_address);
    let article_author_pubkey = article_author_pubkey(
        input.source_article.as_ref(),
        &input.source_article_author_pubkey,
    );
    let article_author_profile = input
        .article_author_profiles
        .iter()
        .find(|candidate| candidate.pubkey == article_author_pubkey)
        .and_then(|candidate| candidate.profile.clone());
    let web_metadata_url = web_metadata_url(source_kind, preview, &lead.highlight.source_url);

    HighlightResourceHeaderProjection {
        source_kind,
        icon_system_name: source_kind_icon_name(source_kind).to_string(),
        title: resource_title(
            source_kind,
            preview,
            input.source_article.as_ref(),
            input.book_preview.as_ref(),
            url_host.as_deref(),
        ),
        author_or_domain: resource_author_or_domain(
            source_kind,
            preview,
            input.book_preview.as_ref(),
            url_host.as_deref(),
            &article_author_pubkey,
            article_author_profile,
        ),
        time_label: resource_time_label(source_kind, preview, input.source_article.as_ref()),
        cover_url: resource_cover_url(
            source_kind,
            preview,
            input.source_article.as_ref(),
            input.book_preview.as_ref(),
        ),
        book_isbn: book_route.map(|route| route.isbn),
        article_address,
        article_author_pubkey,
        web_metadata_url,
    }
}

/// Project the compact resource header on the highlight detail screen. Rust
/// owns the source label, title/author fallback order, and destination policy;
/// native shells only construct platform navigation values and render.
pub fn highlight_detail_resource_projection(
    input: HighlightDetailResourceProjectionInput,
) -> HighlightDetailResourceProjection {
    let item = input.item;
    let preview = item.artifact.as_ref().map(|artifact| &artifact.preview);
    let source_kind = source_kind(
        preview.map(|p| p.source.as_str()).unwrap_or_default(),
        &item.highlight.external_reference,
        &item.highlight.artifact_address,
        &item.highlight.source_url,
    );
    let url_host = url_host(&item.highlight.source_url);
    let article_route = article_route_for_highlight(&item.highlight.artifact_address);
    let book_catalog_id = book_route_for_highlight(
        &item.highlight.external_reference,
        &item.highlight.artifact_address,
    )
    .map(|route| route.catalog_id);
    let web_url = web_reader_url(&item.highlight.source_url);

    HighlightDetailResourceProjection {
        source_kind,
        kind_label: detail_kind_label(source_kind).to_string(),
        icon_system_name: source_kind_icon_name(source_kind).to_string(),
        title: preview
            .and_then(|p| non_empty(&p.title))
            .or(url_host.clone())
            .unwrap_or_else(|| "Untitled".into()),
        author: preview
            .and_then(|p| non_empty(&p.author))
            .or_else(|| preview.and_then(|p| non_empty(&p.domain)))
            .or(url_host)
            .unwrap_or_default(),
        cover_url: preview.and_then(|p| non_empty(&p.image)),
        article_route,
        book_catalog_id,
        web_url,
    }
}

fn source_kind_icon_name(kind: HighlightSourceKind) -> &'static str {
    match kind {
        HighlightSourceKind::Article => "doc.text",
        HighlightSourceKind::Web => "globe",
        HighlightSourceKind::Podcast => "waveform",
        HighlightSourceKind::Book => "book.closed",
        HighlightSourceKind::Video => "play.rectangle",
        HighlightSourceKind::Paper => "doc.richtext",
        HighlightSourceKind::Unknown => "quote.bubble",
    }
}

/// Project highlight quote/note/image content for feed cards. Rust owns text
/// trimming and optional page image presence while the native shell keeps the
/// existing visual treatment.
pub fn highlight_feed_content_projection(
    input: HighlightFeedContentProjectionInput,
) -> HighlightFeedContentProjection {
    let highlight = input.highlight;
    HighlightFeedContentProjection {
        quote_text: highlight.quote.trim().to_string(),
        note_text: non_empty(&highlight.note),
        page_image_url: non_empty_trimmed(&highlight.image_url),
    }
}

/// Project highlight quote/note/image/share-message content for the detail
/// screen. Rust owns the detail-specific blank-note rule and share message.
pub fn highlight_detail_content_projection(
    input: HighlightDetailContentProjectionInput,
) -> HighlightDetailContentProjection {
    let highlight = input.highlight;
    let quote_text = highlight.quote.trim().to_string();
    HighlightDetailContentProjection {
        quote_text: quote_text.clone(),
        note_text: if highlight.note.trim().is_empty() {
            None
        } else {
            Some(highlight.note)
        },
        page_image_url: non_empty_trimmed(&highlight.image_url),
        share_message: quote_text,
    }
}

pub fn highlight_share_url_snapshot(
    event_id_hex: &str,
    author_pubkey_hex: &str,
) -> HighlightShareUrlSnapshot {
    match (|| -> Result<String, CoreError> {
        let id = nostr_sdk::prelude::EventId::from_hex(event_id_hex)
            .map_err(|e| CoreError::InvalidInput(format!("bad event id: {e}")))?;
        let author = nostr_sdk::prelude::PublicKey::from_hex(author_pubkey_hex.trim())
            .map_err(|e| CoreError::InvalidInput(format!("bad author pubkey: {e}")))?;
        let relay = nostr_sdk::prelude::RelayUrl::parse(highlighter_relay())
            .map_err(|e| CoreError::InvalidInput(format!("bad relay: {e}")))?;
        let nevent = nostr_sdk::nips::nip19::Nip19Event::new(id)
            .author(author)
            .kind(nostr_sdk::prelude::Kind::from(KIND_HIGHLIGHT as u16))
            .relays([relay]);
        nostr_sdk::nips::nip19::ToBech32::to_bech32(&nevent)
            .map_err(|e| CoreError::InvalidInput(format!("encode nevent: {e}")))
    })() {
        Ok(nevent) => HighlightShareUrlSnapshot {
            share_url: Some(format!("https://beta.highlighter.com/highlight/{nevent}")),
            ready: true,
            error_message: String::new(),
        },
        Err(error) => HighlightShareUrlSnapshot {
            share_url: None,
            ready: false,
            error_message: error.to_string(),
        },
    }
}

/// Project the article-reader highlight publish action. Rust owns note
/// normalization and the semantic toast copy for success/failure.
pub fn article_highlight_publish_projection(
    input: ArticleHighlightPublishProjectionInput,
) -> ArticleHighlightPublishProjection {
    let submit_note = input.note.trim().to_string();
    let error = input.error.trim();
    let is_success = error.is_empty();
    let toast_message = if is_success {
        if submit_note.is_empty() {
            "Highlighted".to_string()
        } else {
            "Highlighted with note".to_string()
        }
    } else {
        format!("Couldn't save — {error}")
    };

    ArticleHighlightPublishProjection {
        submit_note,
        toast_message,
        is_success,
    }
}

/// Project article-reader selected text into a highlight action payload. Native
/// owns UIKit range extraction; Rust owns normalization and context policy.
pub fn article_reader_selection_projection(
    input: ArticleReaderSelectionProjectionInput,
) -> ArticleReaderSelectionProjection {
    let quote = input.quote.trim().to_string();
    let context = input.context.trim().to_string();
    let context = if context == quote {
        String::new()
    } else {
        context
    };

    ArticleReaderSelectionProjection {
        has_quote: !quote.is_empty(),
        quote,
        context,
    }
}

fn is_isbn_reference(value: &str) -> bool {
    value.trim().to_ascii_lowercase().starts_with("isbn:")
}

/// Return true when `address` is a NIP-23 article address (`30023:<pubkey>:<d-tag>`).
fn is_nip23_article_address(address: &str) -> bool {
    article_reader_route_from_address(address).is_some()
}

/// Parse a NIP-23 article address (`30023:<pubkey>:<d-tag>`) into an
/// `ArticleReaderRoute`. Returns `None` for any other address format.
fn article_reader_route_from_address(address: &str) -> Option<crate::models::ArticleReaderRoute> {
    let rest = address.strip_prefix("30023:")?;
    let colon = rest.find(':')?;
    let pubkey = &rest[..colon];
    let d_tag = &rest[colon + 1..];
    if pubkey.is_empty() || d_tag.is_empty() {
        return None;
    }
    Some(crate::models::ArticleReaderRoute {
        address: address.to_string(),
        pubkey: pubkey.to_string(),
        d_tag: d_tag.to_string(),
    })
}

fn resource_cover_url(
    source_kind: HighlightSourceKind,
    preview: Option<&crate::models::ArtifactPreview>,
    source_article: Option<&crate::models::ArticleRecord>,
    book_preview: Option<&crate::models::ArtifactPreview>,
) -> Option<String> {
    if let Some(image) = preview.and_then(|p| non_empty(&p.image)) {
        return Some(image);
    }
    if source_kind == HighlightSourceKind::Book {
        if let Some(image) = book_preview.and_then(|p| non_empty(&p.image)) {
            return Some(image);
        }
    }
    if source_kind == HighlightSourceKind::Article {
        if let Some(image) = source_article.and_then(|article| non_empty(&article.image)) {
            return Some(image);
        }
    }
    None
}

fn resource_author_or_domain(
    source_kind: HighlightSourceKind,
    preview: Option<&crate::models::ArtifactPreview>,
    book_preview: Option<&crate::models::ArtifactPreview>,
    url_host: Option<&str>,
    article_author_pubkey: &str,
    article_author_profile: Option<ProfileMetadata>,
) -> String {
    match source_kind {
        HighlightSourceKind::Article => {
            let fallback = preview.map(|p| p.author.clone()).unwrap_or_default();
            profile_display_with_label_projection(ProfileDisplayWithLabelProjectionInput {
                pubkey: String::new(),
                profile: if article_author_pubkey.is_empty() {
                    None
                } else {
                    article_author_profile
                },
                label_fallback: fallback,
                pubkey_fallback: ProfileDisplayFallback::Pubkey10,
                empty_fallback: String::new(),
            })
            .display_name
        }
        HighlightSourceKind::Podcast => preview
            .and_then(|p| non_empty(&p.podcast_show_title))
            .or_else(|| preview.and_then(|p| non_empty(&p.author)))
            .unwrap_or_default(),
        HighlightSourceKind::Book => preview
            .map(|p| p.author.clone())
            .or_else(|| book_preview.map(|p| p.author.clone()))
            .unwrap_or_default(),
        HighlightSourceKind::Web => preview
            .and_then(|p| non_empty(&p.domain))
            .or_else(|| url_host.map(str::to_string))
            .unwrap_or_default(),
        HighlightSourceKind::Video | HighlightSourceKind::Paper => preview
            .and_then(|p| non_empty(&p.author))
            .or_else(|| preview.and_then(|p| non_empty(&p.domain)))
            .unwrap_or_default(),
        HighlightSourceKind::Unknown => url_host.map(str::to_string).unwrap_or_default(),
    }
}

fn resource_title(
    source_kind: HighlightSourceKind,
    preview: Option<&crate::models::ArtifactPreview>,
    source_article: Option<&crate::models::ArticleRecord>,
    book_preview: Option<&crate::models::ArtifactPreview>,
    url_host: Option<&str>,
) -> String {
    match source_kind {
        HighlightSourceKind::Article => source_article
            .and_then(|article| non_empty(&article.title))
            .or_else(|| preview.and_then(|p| non_empty(&p.title)))
            .unwrap_or_else(|| "Untitled".into()),
        HighlightSourceKind::Podcast | HighlightSourceKind::Video | HighlightSourceKind::Paper => {
            preview
                .and_then(|p| non_empty(&p.title))
                .unwrap_or_else(|| "Untitled".into())
        }
        HighlightSourceKind::Book => preview
            .and_then(|p| non_empty(&p.title))
            .or_else(|| book_preview.and_then(|p| non_empty(&p.title)))
            .unwrap_or_else(|| "Untitled".into()),
        HighlightSourceKind::Web => preview
            .and_then(|p| non_empty(&p.title))
            .or_else(|| url_host.map(str::to_string))
            .unwrap_or_else(|| "Web page".into()),
        HighlightSourceKind::Unknown => preview
            .and_then(|p| non_empty(&p.title))
            .or_else(|| url_host.map(str::to_string))
            .unwrap_or_else(|| "Highlight".into()),
    }
}

fn resource_time_label(
    source_kind: HighlightSourceKind,
    preview: Option<&crate::models::ArtifactPreview>,
    source_article: Option<&crate::models::ArticleRecord>,
) -> Option<String> {
    match source_kind {
        HighlightSourceKind::Article => source_article
            .and_then(|article| read_minutes(&article.content))
            .map(|minutes| format!("{minutes} min")),
        HighlightSourceKind::Podcast => preview
            .and_then(|p| p.duration_seconds)
            .filter(|seconds| *seconds > 0)
            .map(format_duration),
        _ => None,
    }
}

fn read_minutes(content: &str) -> Option<usize> {
    if content.is_empty() {
        return None;
    }
    let words = content.split_whitespace().count();
    if words <= 60 {
        None
    } else {
        Some(std::cmp::max(1, words / 240))
    }
}

fn format_duration(seconds: i64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

fn article_address_for_highlight(artifact_address: &str) -> Option<String> {
    let trimmed = artifact_address.trim();
    if is_nip23_article_address(trimmed) {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn article_route_for_highlight(
    artifact_address: &str,
) -> Option<crate::models::ArticleReaderRoute> {
    article_reader_route_from_address(artifact_address.trim())
}

fn article_author_pubkey(
    source_article: Option<&crate::models::ArticleRecord>,
    resolved_author_pubkey: &str,
) -> String {
    source_article
        .and_then(|article| non_empty(&article.pubkey))
        .or_else(|| non_empty(resolved_author_pubkey))
        .unwrap_or_default()
}

fn web_metadata_url(
    source_kind: HighlightSourceKind,
    preview: Option<&crate::models::ArtifactPreview>,
    source_url: &str,
) -> Option<String> {
    if source_kind != HighlightSourceKind::Web {
        return None;
    }
    preview
        .and_then(|p| non_empty(&p.url))
        .or_else(|| non_empty_trimmed(source_url))
}

fn url_host(raw_url: &str) -> Option<String> {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        return None;
    }
    Url::parse(trimmed)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
}

fn web_reader_url(raw_url: &str) -> Option<String> {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed = Url::parse(trimmed).ok()?;
    match parsed.scheme().to_ascii_lowercase().as_str() {
        "http" | "https" => Some(trimmed.to_string()),
        _ => None,
    }
}

fn detail_kind_label(source_kind: HighlightSourceKind) -> &'static str {
    match source_kind {
        HighlightSourceKind::Article => "Article",
        HighlightSourceKind::Book => "Book",
        HighlightSourceKind::Podcast => "Podcast",
        HighlightSourceKind::Web => "Web",
        HighlightSourceKind::Video => "Video",
        HighlightSourceKind::Paper => "Paper",
        HighlightSourceKind::Unknown => "Source",
    }
}

fn non_empty(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn non_empty_trimmed(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Port of `publishAndShareHighlight` (`highlights.ts:288-319`).
/// 1. Publishes the canonical kind:9802 highlight on the user's write relays.
/// 2. Publishes a kind:16 repost tagged `h=target_group_id` on the group's relay.
///
/// Returns records in the same order as `drafts`.
pub async fn publish_and_share(
    runtime: &NostrRuntime,
    artifact: ArtifactRecord,
    drafts: Vec<HighlightDraft>,
    target_group_id: &str,
) -> Result<Vec<HighlightRecord>, CoreError> {
    if target_group_id.trim().is_empty() {
        return Err(CoreError::InvalidInput(
            "target_group_id must not be empty".into(),
        ));
    }

    let client = runtime.client();

    // Resolve author pubkey once from the installed signer. We need it for the
    // repost's `p` tag and for the returned `HighlightRecord.pubkey`.
    let signer = client
        .signer()
        .await
        .map_err(|e| CoreError::Signer(format!("no signer installed: {e}")))?;
    let author_pubkey = signer
        .get_public_key()
        .await
        .map_err(|e| CoreError::Signer(format!("get_public_key failed: {e}")))?;
    let author_pubkey_hex = author_pubkey.to_hex();

    let mut records: Vec<HighlightRecord> = Vec::with_capacity(drafts.len());

    for draft in drafts {
        // 1. Build + sign + publish the canonical highlight.
        let builder = build_highlight_event(&draft, &artifact)?;
        let highlight_event = client
            .sign_event_builder(builder)
            .await
            .map_err(|e| CoreError::Signer(format!("sign highlight: {e}")))?;
        client
            .send_event(&highlight_event)
            .await
            .map_err(|e| CoreError::Relay(format!("publish highlight: {e}")))?;

        // 2. Build + sign + publish the kind:16 repost into the target group.
        let repost_builder = build_repost_event(
            highlight_event.id,
            &author_pubkey_hex,
            target_group_id,
            highlighter_relay(),
        )?;
        let repost_event = client
            .sign_event_builder(repost_builder)
            .await
            .map_err(|e| CoreError::Signer(format!("sign repost: {e}")))?;
        client
            .send_event(&repost_event)
            .await
            .map_err(|e| CoreError::Relay(format!("publish repost: {e}")))?;

        // 3. Build the HighlightRecord to return.
        records.push(record_from_event(&highlight_event, &draft, &artifact));
    }

    Ok(records)
}

// #21: bespoke `share_to_community` (kind:16 highlight repost publish) DELETED —
// the kernel `hl.share.highlight_to_room` action is now the sole writer. The
// pure builder `build_repost_event` below is retained as the parity oracle and
// the field-complete reference for the kernel port.

/// Hydrate cached highlights with cached artifact share projections.
/// The returned list preserves the input highlight order.
pub fn hydrate(
    ndb: &Ndb,
    highlights: Vec<HighlightRecord>,
) -> Result<Vec<HydratedHighlight>, CoreError> {
    if highlights.is_empty() {
        return Ok(Vec::new());
    }

    let needed_keys: HashSet<String> = highlights
        .iter()
        .map(highlight_source_key)
        .filter(|key| !key.is_empty())
        .collect();
    if needed_keys.is_empty() {
        return Ok(highlights
            .into_iter()
            .map(|highlight| HydratedHighlight {
                highlight,
                artifact: None,
                shared_by_event_id: None,
                shared_by_pubkey: None,
            })
            .collect());
    }

    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let cap = ((highlights.len() as u32).saturating_mul(8)).clamp(256, 4096) as i32;
    let filter = NdbFilter::new().kinds([11u64]).build();
    let results = ndb
        .query(&txn, &[filter], cap)
        .map_err(|e| CoreError::Cache(format!("query artifact shares for hydration: {e}")))?;

    let mut artifacts_by_key: BTreeMap<String, ArtifactRecord> = BTreeMap::new();
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        let Some(group_id) = first_tag_value(&event, "h") else {
            continue;
        };
        let Some(record) = crate::artifacts::artifact_record_from_event(&event, group_id) else {
            continue;
        };
        for key in artifact_reference_keys(&record) {
            if !needed_keys.contains(&key) {
                continue;
            }
            match artifacts_by_key.get(&key) {
                Some(existing)
                    if existing.created_at.unwrap_or(0) >= record.created_at.unwrap_or(0) => {}
                _ => {
                    artifacts_by_key.insert(key, record.clone());
                }
            }
        }
    }

    Ok(highlights
        .into_iter()
        .map(|highlight| {
            let key = highlight_source_key(&highlight);
            HydratedHighlight {
                artifact: artifacts_by_key.get(&key).cloned(),
                highlight,
                shared_by_event_id: None,
                shared_by_pubkey: None,
            }
        })
        .collect())
}

/// Return a highlight list with `record` present exactly once at the front.
/// Used for optimistic publish projection: native shells render the returned
/// bounded list but do not own highlight identity or dedupe rules.
pub fn insert_unique_front(
    highlights: &[HighlightRecord],
    record: &HighlightRecord,
) -> Vec<HighlightRecord> {
    if highlights
        .iter()
        .any(|highlight| highlight.event_id == record.event_id)
    {
        return highlights.to_vec();
    }

    let mut next = Vec::with_capacity(highlights.len() + 1);
    next.push(record.clone());
    next.extend_from_slice(highlights);
    next
}

fn highlight_source_key(highlight: &HighlightRecord) -> String {
    let key = highlight.source_reference_key.trim();
    if !key.is_empty() {
        return key.to_string();
    }
    if !highlight.artifact_address.trim().is_empty() {
        return format!("a:{}", highlight.artifact_address.trim());
    }
    if !highlight.event_reference.trim().is_empty() {
        return format!("e:{}", highlight.event_reference.trim());
    }
    if !highlight.external_reference.trim().is_empty() {
        return format!("i:{}", highlight.external_reference.trim());
    }
    if !highlight.source_url.trim().is_empty() {
        return format!("r:{}", highlight.source_url.trim());
    }
    String::new()
}

fn artifact_reference_keys(record: &ArtifactRecord) -> Vec<String> {
    let preview = &record.preview;
    let mut keys = Vec::new();
    push_reference_key(
        &mut keys,
        &preview.reference_tag_name,
        &preview.reference_tag_value,
    );
    push_reference_key(
        &mut keys,
        &preview.highlight_tag_name,
        &preview.highlight_tag_value,
    );
    push_reference_key(&mut keys, "r", &preview.url);
    push_reference_key(&mut keys, "r", &preview.audio_url);
    push_reference_key(&mut keys, "e", &record.share_event_id);
    if !preview.catalog_id.trim().is_empty() {
        push_reference_key(&mut keys, "i", &preview.catalog_id);
    }
    if !preview.podcast_item_guid.trim().is_empty() {
        push_reference_key(
            &mut keys,
            "i",
            &format!("podcast:item:guid:{}", preview.podcast_item_guid.trim()),
        );
    }
    keys.sort();
    keys.dedup();
    keys
}

fn push_reference_key(keys: &mut Vec<String>, name: &str, value: &str) {
    let name = name.trim();
    let value = value.trim();
    if !name.is_empty() && !value.is_empty() {
        keys.push(format!("{name}:{value}"));
    }
}

/// Read highlights referencing the given NIP-23 article address
/// (`30023:<pubkey>:<d>`) from nostrdb, newest first. Used by the article
/// reader to overlay existing highlights on the body.
pub fn query_for_article(
    ndb: &Ndb,
    address: &str,
    limit: u32,
) -> Result<Vec<HighlightRecord>, CoreError> {
    let address = address.trim();
    if address.is_empty() {
        return Ok(Vec::new());
    }
    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let ndb_cap = limit.max(32) as i32;
    let filter = NdbFilter::new()
        .kinds([KIND_HIGHLIGHT as u64])
        .tags([address], 'a')
        .build();

    let results = ndb
        .query(&txn, &[filter], ndb_cap)
        .map_err(|e| CoreError::Cache(format!("query highlights for article: {e}")))?;

    let mut records: Vec<HighlightRecord> = Vec::with_capacity(results.len());
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        if let Some(rec) = record_from_cached_event(&event) {
            records.push(rec);
        }
    }
    records.sort_by(|a, b| b.created_at.unwrap_or(0).cmp(&a.created_at.unwrap_or(0)));
    records.truncate(limit as usize);
    Ok(records)
}

/// Publish a solo NIP-84 highlight without any NIP-29 repost. Variant of
/// `publish_and_share` for reader flows that save to the user's vault only —
/// sharing into a community is a later, explicit action.
pub async fn publish(
    runtime: &NostrRuntime,
    draft: HighlightDraft,
    artifact: ArtifactRecord,
) -> Result<HighlightRecord, CoreError> {
    let client = runtime.client();
    let builder = build_highlight_event(&draft, &artifact)?;
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign highlight: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish highlight: {e}")))?;
    Ok(record_from_event(&event, &draft, &artifact))
}

/// Read kind:9802 highlights whose `tag_name` tag holds `tag_value`,
/// newest first. `tag_name` is a single-char tag (e.g. `'a'` for NIP-23
/// addressable refs, `'i'` for NIP-73 external content, `'r'` for URL).
/// Generalizes `query_for_article`, which is now a thin wrapper over this.
pub fn query_for_reference(
    ndb: &Ndb,
    tag_name: char,
    tag_value: &str,
    limit: u32,
) -> Result<Vec<HighlightRecord>, CoreError> {
    let tag_value = tag_value.trim();
    if tag_value.is_empty() {
        return Ok(Vec::new());
    }
    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let ndb_cap = limit.max(32) as i32;
    let filter = NdbFilter::new()
        .kinds([KIND_HIGHLIGHT as u64])
        .tags([tag_value], tag_name)
        .build();

    let results = ndb
        .query(&txn, &[filter], ndb_cap)
        .map_err(|e| CoreError::Cache(format!("query highlights by reference: {e}")))?;

    let mut records: Vec<HighlightRecord> = Vec::with_capacity(results.len());
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        if let Some(rec) = record_from_cached_event(&event) {
            records.push(rec);
        }
    }
    records.sort_by(|a, b| b.created_at.unwrap_or(0).cmp(&a.created_at.unwrap_or(0)));
    records.truncate(limit as usize);
    Ok(records)
}

/// Book detail passages are anchored by the canonical NIP-73 ISBN `i` tag.
/// Accept both raw ISBN catalog ids and already-prefixed `isbn:...` values.
pub fn query_for_book_catalog(
    ndb: &Ndb,
    catalog_id: &str,
    limit: u32,
) -> Result<Vec<HighlightRecord>, CoreError> {
    let Some(reference) = book_highlight_reference(catalog_id) else {
        return Ok(Vec::new());
    };
    query_for_reference(ndb, 'i', &reference, limit)
}

pub fn book_route_for_catalog(catalog_id: &str) -> Option<BookRoute> {
    let catalog_id = book_highlight_reference(catalog_id)?;
    let isbn = catalog_id
        .strip_prefix("isbn:")
        .unwrap_or(&catalog_id)
        .to_string();
    Some(BookRoute { catalog_id, isbn })
}

pub fn book_route_for_highlight(
    external_reference: &str,
    artifact_address: &str,
) -> Option<BookRoute> {
    book_route_for_highlight_reference(external_reference)
        .or_else(|| book_route_for_highlight_reference(artifact_address))
}

fn book_route_for_highlight_reference(reference: &str) -> Option<BookRoute> {
    let trimmed = reference.trim();
    trimmed.strip_prefix("isbn:")?;
    book_route_for_catalog(trimmed)
}

fn book_highlight_reference(catalog_id: &str) -> Option<String> {
    let trimmed = catalog_id.trim();
    let isbn = trimmed.strip_prefix("isbn:").unwrap_or(trimmed).trim();
    if isbn.is_empty() {
        None
    } else {
        Some(format!("isbn:{isbn}"))
    }
}

/// Read kind:9802 highlights for `group_id` from nostrdb, newest first.
/// Scans by kind only and checks `#h` manually, consistent with the pattern
/// used elsewhere to work around nostrdb tag index limitations.
pub fn query_for_group(
    ndb: &Ndb,
    group_id: &str,
    limit: u32,
) -> Result<Vec<HydratedHighlight>, CoreError> {
    let group_id = group_id.trim();
    if group_id.is_empty() {
        return Ok(Vec::new());
    }
    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let cap = (limit.saturating_mul(4)).max(128) as i32;
    let filter = NdbFilter::new().kinds([KIND_HIGHLIGHT as u64]).build();

    let results = ndb
        .query(&txn, &[filter], cap)
        .map_err(|e| CoreError::Cache(format!("query highlights for group: {e}")))?;

    let mut hydrated: Vec<HydratedHighlight> = Vec::new();
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        let Some(h) = first_tag_value(&event, "h") else {
            continue;
        };
        if h != group_id {
            continue;
        }
        if let Some(rec) = record_from_cached_event(&event) {
            hydrated.push(HydratedHighlight {
                highlight: rec,
                artifact: None,
                shared_by_event_id: None,
                shared_by_pubkey: None,
            });
        }
    }

    hydrated.sort_by(|a, b| {
        b.highlight
            .created_at
            .unwrap_or(0)
            .cmp(&a.highlight.created_at.unwrap_or(0))
    });
    hydrated.truncate(limit as usize);
    Ok(hydrated)
}

/// Read highlights authored by `pubkey_hex` from nostrdb, newest first.
/// Used both for the profile page's Highlights tab and for the vault view.
pub fn query_highlights_by_author(
    ndb: &Ndb,
    pubkey_hex: &str,
    limit: u32,
) -> Result<Vec<HighlightRecord>, CoreError> {
    if pubkey_hex.is_empty() {
        return Ok(Vec::new());
    }
    let author = PublicKey::from_hex(pubkey_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;

    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let pk_bytes: [u8; 32] = author.to_bytes();
    let ndb_cap = limit.max(32) as i32;
    let filter = NdbFilter::new()
        .kinds([KIND_HIGHLIGHT as u64])
        .authors([&pk_bytes])
        .build();

    let results = ndb
        .query(&txn, &[filter], ndb_cap)
        .map_err(|e| CoreError::Cache(format!("query highlights: {e}")))?;

    let mut records: Vec<HighlightRecord> = Vec::with_capacity(results.len());
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        if let Some(rec) = record_from_cached_event(&event) {
            records.push(rec);
        }
    }

    records.sort_by(|a, b| b.created_at.unwrap_or(0).cmp(&a.created_at.unwrap_or(0)));
    records.truncate(limit as usize);
    Ok(records)
}

/// Pure: build a `HighlightRecord` from an already-cached kind:9802 event.
/// Separate from `record_from_event` above, which relies on the draft for
/// clip fields known up front.
pub(crate) fn record_from_cached_event(event: &Event) -> Option<HighlightRecord> {
    if event.kind.as_u16() != KIND_HIGHLIGHT {
        return None;
    }
    let artifact_address = first_tag_value(event, "a").unwrap_or("").to_string();
    let event_reference = first_tag_value(event, "e").unwrap_or("").to_string();
    let external_reference = first_tag_value(event, "i").unwrap_or("").to_string();
    let source_url = first_tag_value(event, "r").unwrap_or("").to_string();
    let context = first_tag_value(event, "context").unwrap_or("").to_string();
    let comment = first_tag_value(event, "comment").unwrap_or("").to_string();

    let source_reference_key = if !artifact_address.is_empty() {
        format!("a:{artifact_address}")
    } else if !event_reference.is_empty() {
        format!("e:{event_reference}")
    } else if !external_reference.is_empty() {
        format!("i:{external_reference}")
    } else if !source_url.is_empty() {
        format!("r:{source_url}")
    } else {
        String::new()
    };

    let clip_start_seconds = first_tag_value(event, "start").and_then(|s| s.trim().parse().ok());
    let clip_end_seconds = first_tag_value(event, "end").and_then(|s| s.trim().parse().ok());
    let clip_speaker = first_tag_value(event, "speaker").unwrap_or("").to_string();
    let clip_transcript_segment_ids: Vec<String> = event
        .tags
        .iter()
        .filter_map(|tag| {
            let s = tag.as_slice();
            if s.first().map(String::as_str) == Some("segment") {
                s.get(1).map(|v| v.to_string())
            } else {
                None
            }
        })
        .collect();

    Some(HighlightRecord {
        event_id: event.id.to_hex(),
        pubkey: event.pubkey.to_hex(),
        quote: event.content.clone(),
        context,
        note: comment,
        artifact_address,
        event_reference,
        external_reference,
        source_url,
        source_reference_key,
        clip_start_seconds,
        clip_end_seconds,
        clip_speaker,
        clip_transcript_segment_ids,
        image_url: imeta_image_url(event),
        created_at: Some(event.created_at.as_secs()),
    })
}

fn first_tag_value<'a>(event: &'a Event, name: &str) -> Option<&'a str> {
    for tag in event.tags.iter() {
        let slice = tag.as_slice();
        if slice.first().map(String::as_str) == Some(name) {
            return slice.get(1).map(String::as_str);
        }
    }
    None
}

/// Extract the image URL from a NIP-92 `imeta` tag on a highlight event.
/// Tag shape: `["imeta", "url <url>", "m <mime>", ...]`. Returns the first
/// `url <…>` value found, or empty when no imeta tag carries a url.
pub(crate) fn imeta_image_url(event: &Event) -> String {
    for tag in event.tags.iter() {
        let slice = tag.as_slice();
        if slice.first().map(String::as_str) != Some("imeta") {
            continue;
        }
        for part in slice.iter().skip(1) {
            if let Some(rest) = part.strip_prefix("url ") {
                let url = rest.trim();
                if !url.is_empty() {
                    return url.to_string();
                }
            }
        }
    }
    String::new()
}

// -- Builders (pure: no IO, unit-testable) --

/// Build the draft shape used by article-reader text selection. Rust owns the
/// default clip fields so native shells don't mirror highlight publish policy.
pub fn article_reader_highlight_draft(
    quote: String,
    note: String,
    context: String,
) -> HighlightDraft {
    HighlightDraft {
        quote,
        context,
        note,
        clip_start_seconds: None,
        clip_end_seconds: None,
        clip_speaker: String::new(),
        clip_transcript_segment_ids: Vec::new(),
        image: None,
    }
}

/// Build the kind:9802 highlight `EventBuilder`. Pure — safe to unit test.
/// Matches `publishCanonicalHighlight` (highlights.ts:359-423).
pub(crate) fn build_highlight_event(
    draft: &HighlightDraft,
    artifact: &ArtifactRecord,
) -> Result<EventBuilder, CoreError> {
    let quote = draft.quote.trim();
    let has_clip = draft.clip_start_seconds.is_some() && draft.clip_end_seconds.is_some();
    if quote.is_empty() && !has_clip {
        return Err(CoreError::InvalidInput(
            "highlight must have a quote or a clip".into(),
        ));
    }

    let content = if quote.is_empty() {
        build_clip_fallback_quote(
            draft.clip_start_seconds.unwrap_or(0.0),
            draft.clip_end_seconds.unwrap_or(0.0),
        )
    } else {
        quote.to_string()
    };

    let mut tags: Vec<Tag> = Vec::new();

    // Source reference tag: one of ("a", addr), ("e", id), or ("r", url).
    let ref_name = artifact.preview.highlight_tag_name.trim();
    let ref_value = artifact.preview.highlight_tag_value.trim();
    if ref_name.is_empty() || ref_value.is_empty() {
        return Err(CoreError::InvalidInput(
            "artifact missing highlight reference tag".into(),
        ));
    }
    tags.push(
        Tag::parse(vec![ref_name.to_string(), ref_value.to_string()])
            .map_err(|e| CoreError::Other(format!("build reference tag: {e}")))?,
    );

    // NIP-73 external-entity tag. When the artifact has a canonical catalog
    // id (e.g. an ISBN-sourced book), mirror it onto the highlight so every
    // Nostr client — not just Highlighter — can identify the source. Skipped
    // if the primary reference is already an `i` tag with the same value
    // (would be a duplicate).
    let catalog_id = artifact.preview.catalog_id.trim();
    if !(catalog_id.is_empty() || ref_name == "i" && ref_value == catalog_id) {
        tags.push(
            Tag::parse(vec!["i".to_string(), catalog_id.to_string()])
                .map_err(|e| CoreError::Other(format!("build catalog tag: {e}")))?,
        );
    }

    // Context tag: only if differs from content.
    let context = draft.context.trim();
    if !context.is_empty() && context != content {
        tags.push(
            Tag::parse(vec!["context".to_string(), context.to_string()])
                .map_err(|e| CoreError::Other(format!("build context tag: {e}")))?,
        );
    }

    // Comment tag.
    let note = draft.note.trim();
    if !note.is_empty() {
        tags.push(
            Tag::parse(vec!["comment".to_string(), note.to_string()])
                .map_err(|e| CoreError::Other(format!("build comment tag: {e}")))?,
        );
    }

    // Clip tags. Start/end always appear together (both Some) or not at all.
    // The TS code emits them with `.toFixed(3)` — 3 decimal places, rounded.
    if let (Some(start), Some(end)) = (draft.clip_start_seconds, draft.clip_end_seconds) {
        tags.push(
            Tag::parse(vec!["start".to_string(), format!("{:.3}", start)])
                .map_err(|e| CoreError::Other(format!("build start tag: {e}")))?,
        );
        tags.push(
            Tag::parse(vec!["end".to_string(), format!("{:.3}", end)])
                .map_err(|e| CoreError::Other(format!("build end tag: {e}")))?,
        );

        let speaker = draft.clip_speaker.trim();
        if !speaker.is_empty() {
            tags.push(
                Tag::parse(vec!["speaker".to_string(), speaker.to_string()])
                    .map_err(|e| CoreError::Other(format!("build speaker tag: {e}")))?,
            );
        }

        for segment_id in &draft.clip_transcript_segment_ids {
            let segment_id = segment_id.trim();
            if segment_id.is_empty() {
                continue;
            }
            tags.push(
                Tag::parse(vec!["segment".to_string(), segment_id.to_string()])
                    .map_err(|e| CoreError::Other(format!("build segment tag: {e}")))?,
            );
        }
    }

    // NIP-92 imeta tag — only present when the user attached a photo to the
    // highlight (e.g. the page they OCR'd). The image is uploaded separately
    // via `blossom::upload_blob`; here we only describe it.
    if let Some(image) = &draft.image {
        tags.push(build_imeta_tag(image)?);
    }

    Ok(EventBuilder::new(Kind::Custom(KIND_HIGHLIGHT), content).tags(tags))
}

/// Build a NIP-92 `imeta` tag from a Blossom upload descriptor.
/// Tag shape: `["imeta", "url <url>", "m <mime>", "x <sha>", "size <bytes>", "dim WxH", "alt <text>"]`.
/// `dim` and `alt` are omitted when not meaningful (zero dim, empty alt).
pub(crate) fn build_imeta_tag(image: &BlossomUpload) -> Result<Tag, CoreError> {
    let mut parts: Vec<String> = vec!["imeta".to_string()];
    parts.push(format!("url {}", image.url));
    parts.push(format!("m {}", image.mime));
    parts.push(format!("x {}", image.sha256_hex));
    parts.push(format!("size {}", image.size_bytes));
    if image.width > 0 && image.height > 0 {
        parts.push(format!("dim {}x{}", image.width, image.height));
    }
    let alt = image.alt.trim();
    if !alt.is_empty() {
        parts.push(format!("alt {alt}"));
    }
    Tag::parse(parts).map_err(|e| CoreError::Other(format!("build imeta tag: {e}")))
}

/// Build the kind:16 repost `EventBuilder` that shares a highlight into a
/// NIP-29 community. Pure — safe to unit test.
pub(crate) fn build_repost_event(
    highlight_event_id: EventId,
    highlight_author_pubkey_hex: &str,
    target_group_id: &str,
    relay_hint: &str,
) -> Result<EventBuilder, CoreError> {
    let author_pk = PublicKey::from_hex(highlight_author_pubkey_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid author pubkey: {e}")))?;

    let e_tag = Tag::parse(vec![
        "e".to_string(),
        highlight_event_id.to_hex(),
        relay_hint.to_string(),
    ])
    .map_err(|e| CoreError::Other(format!("build e tag: {e}")))?;

    let k_tag = Tag::parse(vec!["k".to_string(), KIND_HIGHLIGHT.to_string()])
        .map_err(|e| CoreError::Other(format!("build k tag: {e}")))?;

    let p_tag = Tag::public_key(author_pk);

    let h_tag = Tag::parse(vec!["h".to_string(), target_group_id.to_string()])
        .map_err(|e| CoreError::Other(format!("build h tag: {e}")))?;

    Ok(EventBuilder::new(Kind::Custom(KIND_GENERIC_REPOST), "")
        .tags(vec![e_tag, k_tag, p_tag, h_tag]))
}

/// Build a `HighlightRecord` from the signed highlight event + the draft we
/// sent. Mirrors `highlightFromEvent` (highlights.ts:56-82) but uses the
/// draft directly (no re-parsing) for clip fields that are known up front.
fn record_from_event(
    event: &Event,
    draft: &HighlightDraft,
    artifact: &ArtifactRecord,
) -> HighlightRecord {
    let ref_name = artifact.preview.highlight_tag_name.as_str();
    let ref_value = artifact.preview.highlight_tag_value.as_str();

    let (artifact_address, event_reference, external_reference, source_url) = match ref_name {
        "a" => (
            ref_value.to_string(),
            String::new(),
            String::new(),
            String::new(),
        ),
        "e" => (
            String::new(),
            ref_value.to_string(),
            String::new(),
            String::new(),
        ),
        "i" => (
            String::new(),
            String::new(),
            ref_value.to_string(),
            String::new(),
        ),
        "r" => (
            String::new(),
            String::new(),
            String::new(),
            ref_value.to_string(),
        ),
        _ => (String::new(), String::new(), String::new(), String::new()),
    };

    let source_reference_key = if !artifact_address.is_empty() {
        format!("a:{artifact_address}")
    } else if !event_reference.is_empty() {
        format!("e:{event_reference}")
    } else if !external_reference.is_empty() {
        format!("i:{external_reference}")
    } else if !source_url.is_empty() {
        format!("r:{source_url}")
    } else {
        String::new()
    };

    HighlightRecord {
        event_id: event.id.to_hex(),
        pubkey: event.pubkey.to_hex(),
        quote: event.content.clone(),
        context: draft.context.trim().to_string(),
        note: draft.note.trim().to_string(),
        artifact_address,
        event_reference,
        external_reference,
        source_url,
        source_reference_key,
        clip_start_seconds: draft.clip_start_seconds,
        clip_end_seconds: draft.clip_end_seconds,
        clip_speaker: draft.clip_speaker.trim().to_string(),
        clip_transcript_segment_ids: draft
            .clip_transcript_segment_ids
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        image_url: draft
            .image
            .as_ref()
            .map(|img| img.url.clone())
            .unwrap_or_default(),
        created_at: Some(event.created_at.as_secs()),
    }
}

fn build_clip_fallback_quote(start: f64, end: f64) -> String {
    format!("Clip {}-{}", format_clip_time(start), format_clip_time(end))
}

fn format_clip_time(value: f64) -> String {
    let total_seconds = if value.is_finite() && value > 0.0 {
        value.round() as u64
    } else {
        0
    };
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ArtifactPreview, ArtifactRecord, HighlightDraft};
    use crate::test_ndb::process_event_and_wait;

    #[test]
    fn source_kind_uses_preview_source_first() {
        assert_eq!(
            source_kind("Podcast", "", "30023:author:essay", ""),
            HighlightSourceKind::Podcast
        );
        assert_eq!(
            source_kind("mystery", "isbn:9780593716717", "", "https://example.com"),
            HighlightSourceKind::Unknown
        );
    }

    #[test]
    fn source_kind_classifies_reference_fallbacks() {
        assert_eq!(
            source_kind("", "isbn:9780593716717", "", ""),
            HighlightSourceKind::Book
        );
        assert_eq!(
            source_kind("", "", "30023:author:essay", ""),
            HighlightSourceKind::Article
        );
        assert_eq!(
            source_kind("", "", "isbn:9780593716717", ""),
            HighlightSourceKind::Book
        );
        assert_eq!(
            source_kind("", "", "", "https://example.com/read"),
            HighlightSourceKind::Web
        );
        assert_eq!(source_kind("", "", "", ""), HighlightSourceKind::Unknown);
    }

    #[test]
    fn book_route_accepts_raw_or_prefixed_isbn() {
        assert_eq!(
            book_route_for_catalog("9780735211292"),
            Some(BookRoute {
                catalog_id: "isbn:9780735211292".into(),
                isbn: "9780735211292".into()
            })
        );
        assert_eq!(
            book_route_for_catalog(" isbn:9780735211292 "),
            Some(BookRoute {
                catalog_id: "isbn:9780735211292".into(),
                isbn: "9780735211292".into()
            })
        );
        assert_eq!(book_route_for_catalog(" isbn: "), None);
    }

    #[test]
    fn book_route_for_highlight_prefers_external_reference() {
        assert_eq!(
            book_route_for_highlight("isbn:external", "isbn:address"),
            Some(BookRoute {
                catalog_id: "isbn:external".into(),
                isbn: "external".into()
            })
        );
        assert_eq!(
            book_route_for_highlight("", "isbn:address"),
            Some(BookRoute {
                catalog_id: "isbn:address".into(),
                isbn: "address".into()
            })
        );
        assert_eq!(book_route_for_highlight("raw-isbn", ""), None);
        assert_eq!(
            book_route_for_highlight("raw-isbn", "isbn:address"),
            Some(BookRoute {
                catalog_id: "isbn:address".into(),
                isbn: "address".into()
            })
        );
    }

    #[test]
    fn highlight_resource_header_projects_article_metadata() {
        let article_pubkey = "a".repeat(64);
        let article_address = format!("30023:{article_pubkey}:essay");
        let mut artifact = artifact_with_source("article");
        artifact.preview.title = "Preview title".into();
        artifact.preview.author = "Stored author".into();
        artifact.preview.image = "https://example.com/artifact.jpg".into();

        let mut article = article_record(&article_pubkey, "essay");
        article.title = "Article title".into();
        article.image = "https://example.com/article.jpg".into();
        article.content = repeat_words(480);

        let projection =
            highlight_resource_header_projection(HighlightResourceHeaderProjectionInput {
                lead: hydrated_highlight_with_artifact(
                    highlight_for_source_key("event-article", &format!("a:{article_address}"), 1),
                    artifact,
                ),
                source_article: Some(article),
                source_article_author_pubkey: String::new(),
                article_author_profiles: vec![HighlightResourceAuthorProfile {
                    pubkey: article_pubkey.clone(),
                    profile: Some(profile_metadata(
                        &article_pubkey,
                        "author",
                        "Profile Author",
                        "",
                    )),
                }],
                book_preview: None,
            });

        assert_eq!(projection.source_kind, HighlightSourceKind::Article);
        assert_eq!(projection.icon_system_name, "doc.text");
        assert_eq!(projection.title, "Article title");
        assert_eq!(projection.author_or_domain, "Profile Author");
        assert_eq!(
            projection.cover_url,
            Some("https://example.com/artifact.jpg".into())
        );
        assert_eq!(projection.time_label, Some("2 min".into()));
        assert_eq!(projection.article_address, Some(article_address));
        assert_eq!(projection.article_author_pubkey, article_pubkey);
        assert_eq!(projection.web_metadata_url, None);
    }

    #[test]
    fn highlight_resource_header_projects_web_source() {
        let mut artifact = artifact_with_source("web");
        artifact.preview.url = "https://example.com/read".into();
        artifact.preview.domain = "preview.example".into();
        artifact.preview.title = "Preview title".into();

        let projection =
            highlight_resource_header_projection(HighlightResourceHeaderProjectionInput {
                lead: hydrated_highlight_with_artifact(
                    highlight_for_source_key("event-web", "r:https://fallback.example/post", 1),
                    artifact,
                ),
                source_article: None,
                source_article_author_pubkey: String::new(),
                article_author_profiles: Vec::new(),
                book_preview: None,
            });

        assert_eq!(projection.source_kind, HighlightSourceKind::Web);
        assert_eq!(projection.icon_system_name, "globe");
        assert_eq!(projection.title, "Preview title");
        assert_eq!(projection.author_or_domain, "preview.example");
        assert_eq!(projection.cover_url, None);
        assert_eq!(
            projection.web_metadata_url,
            Some("https://example.com/read".into())
        );
        assert_eq!(projection.time_label, None);
    }

    #[test]
    fn highlight_resource_header_formats_podcast_duration() {
        let projection =
            highlight_resource_header_projection(HighlightResourceHeaderProjectionInput {
                lead: hydrated_highlight_with_artifact(
                    highlight_for_source_key("event-podcast", "i:podcast:item:guid:ep-1", 1),
                    artifact_for_podcast("https://example.com/audio.mp3"),
                ),
                source_article: None,
                source_article_author_pubkey: String::new(),
                article_author_profiles: Vec::new(),
                book_preview: None,
            });

        assert_eq!(projection.source_kind, HighlightSourceKind::Podcast);
        assert_eq!(projection.icon_system_name, "waveform");
        assert_eq!(projection.title, "Episode 1");
        assert_eq!(projection.author_or_domain, "Show");
        assert_eq!(projection.time_label, Some("1h 0m".into()));
    }

    #[test]
    fn highlight_resource_header_uses_isbn_preview_without_artifact() {
        let mut book_preview = empty_preview("book");
        book_preview.title = "Book title".into();
        book_preview.author = "Book author".into();
        book_preview.image = "https://example.com/book.jpg".into();

        let projection =
            highlight_resource_header_projection(HighlightResourceHeaderProjectionInput {
                lead: HydratedHighlight {
                    highlight: highlight_for_source_key("event-book", "i:isbn:9780735211292", 1),
                    artifact: None,
                    shared_by_event_id: None,
                    shared_by_pubkey: None,
                },
                source_article: None,
                source_article_author_pubkey: String::new(),
                article_author_profiles: Vec::new(),
                book_preview: Some(book_preview),
            });

        assert_eq!(projection.source_kind, HighlightSourceKind::Book);
        assert_eq!(projection.icon_system_name, "book.closed");
        assert_eq!(projection.title, "Book title");
        assert_eq!(projection.author_or_domain, "Book author");
        assert_eq!(
            projection.cover_url,
            Some("https://example.com/book.jpg".into())
        );
        assert_eq!(projection.book_isbn, Some("9780735211292".into()));
    }

    #[test]
    fn highlight_detail_resource_projects_article_destination_and_header() {
        let article_pubkey = "a".repeat(64);
        let article_address = format!("30023:{article_pubkey}:essay");
        let mut artifact = artifact_with_source("article");
        artifact.preview.title = "Preview essay".into();
        artifact.preview.domain = "example.com".into();
        artifact.preview.image = "https://example.com/cover.jpg".into();

        let projection =
            highlight_detail_resource_projection(HighlightDetailResourceProjectionInput {
                item: hydrated_highlight_with_artifact(
                    highlight_for_source_key(
                        "event-detail-article",
                        &format!("a:{article_address}"),
                        1,
                    ),
                    artifact,
                ),
            });

        assert_eq!(projection.source_kind, HighlightSourceKind::Article);
        assert_eq!(projection.kind_label, "Article");
        assert_eq!(projection.icon_system_name, "doc.text");
        assert_eq!(projection.title, "Preview essay");
        assert_eq!(projection.author, "example.com");
        assert_eq!(
            projection.cover_url,
            Some("https://example.com/cover.jpg".into())
        );
        assert_eq!(
            projection
                .article_route
                .as_ref()
                .map(|route| route.address.as_str()),
            Some(article_address.as_str())
        );
        assert_eq!(projection.book_catalog_id, None);
        assert_eq!(projection.web_url, None);
    }

    #[test]
    fn highlight_detail_resource_projects_book_destination_without_artifact() {
        let projection =
            highlight_detail_resource_projection(HighlightDetailResourceProjectionInput {
                item: HydratedHighlight {
                    highlight: highlight_for_source_key(
                        "event-detail-book",
                        "i:isbn:9780735211292",
                        1,
                    ),
                    artifact: None,
                    shared_by_event_id: None,
                    shared_by_pubkey: None,
                },
            });

        assert_eq!(projection.source_kind, HighlightSourceKind::Book);
        assert_eq!(projection.kind_label, "Book");
        assert_eq!(projection.icon_system_name, "book.closed");
        assert_eq!(projection.title, "Untitled");
        assert_eq!(
            projection.book_catalog_id,
            Some("isbn:9780735211292".into())
        );
        assert_eq!(projection.article_route, None);
    }

    #[test]
    fn highlight_detail_resource_projects_web_destination_and_host_fallbacks() {
        let projection =
            highlight_detail_resource_projection(HighlightDetailResourceProjectionInput {
                item: HydratedHighlight {
                    highlight: highlight_for_source_key(
                        "event-detail-web",
                        "r:https://example.com/read",
                        1,
                    ),
                    artifact: None,
                    shared_by_event_id: None,
                    shared_by_pubkey: None,
                },
            });

        assert_eq!(projection.source_kind, HighlightSourceKind::Web);
        assert_eq!(projection.kind_label, "Web");
        assert_eq!(projection.icon_system_name, "globe");
        assert_eq!(projection.title, "example.com");
        assert_eq!(projection.author, "example.com");
        assert_eq!(projection.web_url, Some("https://example.com/read".into()));
    }

    #[test]
    fn highlight_detail_resource_rejects_non_http_web_destination() {
        let mut highlight =
            highlight_for_source_key("event-detail-web", "r:mailto:a@example.com", 1);
        highlight.source_url = "mailto:a@example.com".into();

        let projection =
            highlight_detail_resource_projection(HighlightDetailResourceProjectionInput {
                item: HydratedHighlight {
                    highlight,
                    artifact: None,
                    shared_by_event_id: None,
                    shared_by_pubkey: None,
                },
            });

        assert_eq!(projection.source_kind, HighlightSourceKind::Web);
        assert_eq!(projection.icon_system_name, "globe");
        assert_eq!(projection.web_url, None);
    }

    fn preview_for_podcast(url: &str) -> ArtifactPreview {
        let item_catalog = format!("podcast:item:guid:{}", "ep-1");
        ArtifactPreview {
            id: "id1".into(),
            url: url.into(),
            title: "Episode 1".into(),
            author: "Alice".into(),
            image: String::new(),
            description: String::new(),
            source: "podcast".into(),
            domain: "example.com".into(),
            catalog_id: item_catalog.clone(),
            catalog_kind: "podcast:item:guid".into(),
            podcast_guid: "guid-1".into(),
            podcast_item_guid: "ep-1".into(),
            podcast_show_title: "Show".into(),
            audio_url: url.into(),
            audio_preview_url: String::new(),
            transcript_url: String::new(),
            feed_url: String::new(),
            published_at: String::new(),
            duration_seconds: Some(3600),
            reference_tag_name: "i".into(),
            reference_tag_value: item_catalog.clone(),
            reference_kind: "podcast:item:guid".into(),
            highlight_tag_name: "i".into(),
            highlight_tag_value: item_catalog.clone(),
            highlight_reference_key: format!("i:{item_catalog}"),
            chapters: Vec::new(),
        }
    }

    fn artifact_for_podcast(url: &str) -> ArtifactRecord {
        ArtifactRecord {
            preview: preview_for_podcast(url),
            group_id: "group-a".into(),
            share_event_id: "share-1".into(),
            pubkey: "f".repeat(64),
            created_at: Some(1_700_000_000),
            note: String::new(),
        }
    }

    fn empty_preview(source: &str) -> ArtifactPreview {
        ArtifactPreview {
            id: "id".into(),
            url: String::new(),
            title: String::new(),
            author: String::new(),
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
        }
    }

    fn artifact_with_source(source: &str) -> ArtifactRecord {
        ArtifactRecord {
            preview: empty_preview(source),
            group_id: "group-a".into(),
            share_event_id: "share-1".into(),
            pubkey: "f".repeat(64),
            created_at: Some(1_700_000_000),
            note: String::new(),
        }
    }

    fn hydrated_highlight_with_artifact(
        highlight: HighlightRecord,
        artifact: ArtifactRecord,
    ) -> HydratedHighlight {
        HydratedHighlight {
            highlight,
            artifact: Some(artifact),
            shared_by_event_id: None,
            shared_by_pubkey: None,
        }
    }

    fn article_record(pubkey: &str, identifier: &str) -> crate::models::ArticleRecord {
        crate::models::ArticleRecord {
            event_id: "article-event".into(),
            address: format!("30023:{pubkey}:{identifier}"),
            pubkey: pubkey.into(),
            identifier: identifier.into(),
            title: String::new(),
            summary: String::new(),
            image: String::new(),
            content: String::new(),
            hashtags: Vec::new(),
            published_at: None,
            created_at: Some(1_700_000_000),
        }
    }

    fn repeat_words(count: usize) -> String {
        std::iter::repeat_n("word", count)
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn draft_with_clip() -> HighlightDraft {
        HighlightDraft {
            quote: "the quote".into(),
            context: String::new(),
            note: String::new(),
            clip_start_seconds: Some(12.5),
            clip_end_seconds: Some(34.5678),
            clip_speaker: String::new(),
            clip_transcript_segment_ids: vec![],
            image: None,
        }
    }

    /// Collect tags into a Vec<Vec<String>> for easy assertion.
    fn tags_as_vec(builder: &EventBuilder) -> Vec<Vec<String>> {
        // EventBuilder doesn't expose its tag list. Sign with a throwaway
        // key to inspect the resulting event.
        let keys = Keys::generate();
        let event = builder
            .clone()
            .sign_with_keys(&keys)
            .expect("sign for inspection");
        event.tags.iter().map(|t| t.as_slice().to_vec()).collect()
    }

    struct ShareEventSpec<'a> {
        group_id: &'a str,
        d_tag: &'a str,
        title: &'a str,
        source: &'a str,
        reference_value: &'a str,
        reference_kind: &'a str,
        url: &'a str,
        created_at: u64,
    }

    fn share_event(keys: &Keys, spec: ShareEventSpec<'_>) -> Event {
        let mut tags = vec![
            Tag::parse(vec!["h".to_string(), spec.group_id.to_string()]).unwrap(),
            Tag::identifier(spec.d_tag),
            Tag::parse(vec!["title".to_string(), spec.title.to_string()]).unwrap(),
            Tag::parse(vec!["source".to_string(), spec.source.to_string()]).unwrap(),
        ];
        if !spec.reference_value.is_empty() {
            tags.push(
                Tag::parse(vec![
                    "i".to_string(),
                    spec.reference_value.to_string(),
                    spec.url.to_string(),
                ])
                .unwrap(),
            );
        }
        if !spec.reference_kind.is_empty() {
            tags.push(Tag::parse(vec!["k".to_string(), spec.reference_kind.to_string()]).unwrap());
        }
        if !spec.url.is_empty() {
            tags.push(Tag::parse(vec!["r".to_string(), spec.url.to_string()]).unwrap());
        }

        EventBuilder::new(Kind::Custom(11), "")
            .tags(tags)
            .custom_created_at(Timestamp::from(spec.created_at))
            .sign_with_keys(keys)
            .expect("sign")
    }

    fn highlight_for_source_key(
        event_id: &str,
        source_key: &str,
        created_at: u64,
    ) -> HighlightRecord {
        let mut highlight = HighlightRecord {
            event_id: event_id.to_string(),
            pubkey: "b".repeat(64),
            quote: "quote".into(),
            context: String::new(),
            note: String::new(),
            artifact_address: String::new(),
            event_reference: String::new(),
            external_reference: String::new(),
            source_url: String::new(),
            source_reference_key: source_key.to_string(),
            clip_start_seconds: None,
            clip_end_seconds: None,
            clip_speaker: String::new(),
            clip_transcript_segment_ids: Vec::new(),
            image_url: String::new(),
            created_at: Some(created_at),
        };
        if let Some((name, value)) = source_key.split_once(':') {
            match name {
                "a" => highlight.artifact_address = value.to_string(),
                "e" => highlight.event_reference = value.to_string(),
                "i" => highlight.external_reference = value.to_string(),
                "r" => highlight.source_url = value.to_string(),
                _ => {}
            }
        }
        highlight
    }

    #[test]
    fn highlight_group_card_projection_dedupes_caps_and_labels_highlighters() {
        let items = vec![
            hydrated_highlight_for_pubkey("event-1", "alicepubkey"),
            hydrated_highlight_for_pubkey("event-2", "bobpubkey"),
            hydrated_highlight_for_pubkey("event-3", "alicepubkey"),
            hydrated_highlight_for_pubkey("event-4", "carolpubkey"),
            hydrated_highlight_for_pubkey("event-5", "danpubkey"),
        ];

        let projection = highlight_group_card_projection(HighlightGroupCardProjectionInput {
            items,
            highlighter_profiles: vec![
                HighlightGroupHighlighterProfile {
                    pubkey: "alicepubkey".into(),
                    profile: Some(profile_metadata(
                        "alicepubkey",
                        "alice",
                        "Alice Doe",
                        "https://example.com/alice.png",
                    )),
                },
                HighlightGroupHighlighterProfile {
                    pubkey: "bobpubkey".into(),
                    profile: Some(profile_metadata("bobpubkey", "bob", "", "")),
                },
            ],
        });

        assert!(projection.show_highlighters_strip);
        assert_eq!(projection.overflow_count, 1);
        let visible_pubkeys: Vec<_> = projection
            .visible_highlighters
            .iter()
            .map(|highlighter| highlighter.pubkey.as_str())
            .collect();
        assert_eq!(visible_pubkeys, ["alicepubkey", "bobpubkey", "carolpubkey"]);
        assert_eq!(
            projection.visible_highlighters[0],
            HighlightGroupHighlighterProjection {
                pubkey: "alicepubkey".into(),
                display_name: "Alice Doe".into(),
                display_initial: "A".into(),
                picture_url: "https://example.com/alice.png".into(),
            }
        );
        assert_eq!(
            projection.highlighters_label_segments,
            vec![
                label_segment("Highlighted by ", false),
                label_segment("Alice Doe", true),
                label_segment(", ", false),
                label_segment("bob", true),
                label_segment(" and ", false),
                label_segment("2 others", true),
            ]
        );
    }

    #[test]
    fn highlight_feed_content_projection_preserves_feed_note_policy() {
        let mut highlight = highlight_for_source_key("event", "r:https://example.com", 1_000);
        highlight.quote = "  Quote\n".into();
        highlight.note = " \n ".into();
        highlight.image_url = " https://example.com/page.jpg\n".into();

        let projection =
            highlight_feed_content_projection(HighlightFeedContentProjectionInput { highlight });

        assert_eq!(projection.quote_text, "Quote");
        assert_eq!(projection.note_text, Some(" \n ".into()));
        assert_eq!(
            projection.page_image_url,
            Some("https://example.com/page.jpg".into())
        );

        let mut highlight = highlight_for_source_key("event", "r:https://example.com", 1_000);
        highlight.image_url = " \n ".into();

        let projection =
            highlight_feed_content_projection(HighlightFeedContentProjectionInput { highlight });

        assert_eq!(projection.note_text, None);
        assert_eq!(projection.page_image_url, None);
    }

    #[test]
    fn highlight_detail_content_projection_hides_blank_notes_and_projects_share_message() {
        let mut highlight = highlight_for_source_key("event", "r:https://example.com", 1_000);
        highlight.quote = "  Quote\n".into();
        highlight.note = " \n ".into();
        highlight.image_url = " https://example.com/page.jpg\n".into();

        let projection =
            highlight_detail_content_projection(HighlightDetailContentProjectionInput {
                highlight,
            });

        assert_eq!(projection.quote_text, "Quote");
        assert_eq!(projection.share_message, "Quote");
        assert_eq!(projection.note_text, None);
        assert_eq!(
            projection.page_image_url,
            Some("https://example.com/page.jpg".into())
        );

        let mut highlight = highlight_for_source_key("event", "r:https://example.com", 1_000);
        highlight.note = " Keep ".into();

        let projection =
            highlight_detail_content_projection(HighlightDetailContentProjectionInput {
                highlight,
            });

        assert_eq!(projection.note_text, Some(" Keep ".into()));
    }

    #[test]
    fn highlight_share_url_snapshot_projects_final_public_url() {
        let snapshot = highlight_share_url_snapshot(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d",
        );

        assert!(snapshot.ready);
        assert!(snapshot.error_message.is_empty());
        let share_url = snapshot.share_url.expect("share url");
        assert!(share_url.starts_with("https://beta.highlighter.com/highlight/nevent1"));
    }

    #[test]
    fn highlight_share_url_snapshot_surfaces_encode_error() {
        let snapshot = highlight_share_url_snapshot(
            "not-hex",
            "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d",
        );

        assert!(!snapshot.ready);
        assert!(snapshot.share_url.is_none());
        assert!(snapshot
            .error_message
            .contains("invalid input: bad event id"));
    }

    #[test]
    fn article_highlight_publish_projection_trims_note_and_projects_success_toast() {
        let blank = article_highlight_publish_projection(ArticleHighlightPublishProjectionInput {
            note: " \n ".into(),
            error: String::new(),
        });
        let noted = article_highlight_publish_projection(ArticleHighlightPublishProjectionInput {
            note: "  keep this  ".into(),
            error: String::new(),
        });

        assert_eq!(blank.submit_note, "");
        assert_eq!(blank.toast_message, "Highlighted");
        assert!(blank.is_success);
        assert_eq!(noted.submit_note, "keep this");
        assert_eq!(noted.toast_message, "Highlighted with note");
        assert!(noted.is_success);
    }

    #[test]
    fn article_highlight_publish_projection_projects_failure_toast() {
        let projection =
            article_highlight_publish_projection(ArticleHighlightPublishProjectionInput {
                note: " note ".into(),
                error: " relay rejected ".into(),
            });

        assert_eq!(projection.submit_note, "note");
        assert_eq!(projection.toast_message, "Couldn't save — relay rejected");
        assert!(!projection.is_success);
    }

    #[test]
    fn article_reader_selection_projection_trims_and_omits_duplicate_context() {
        let duplicate =
            article_reader_selection_projection(ArticleReaderSelectionProjectionInput {
                quote: " selected quote ".into(),
                context: "\nselected quote\n".into(),
            });
        let contextual =
            article_reader_selection_projection(ArticleReaderSelectionProjectionInput {
                quote: " selected ".into(),
                context: "\nparagraph with selected text\n".into(),
            });
        let blank = article_reader_selection_projection(ArticleReaderSelectionProjectionInput {
            quote: " ".into(),
            context: " paragraph ".into(),
        });

        assert_eq!(duplicate.quote, "selected quote");
        assert_eq!(duplicate.context, "");
        assert!(duplicate.has_quote);
        assert_eq!(contextual.quote, "selected");
        assert_eq!(contextual.context, "paragraph with selected text");
        assert!(!blank.has_quote);
    }

    #[test]
    fn article_reader_highlight_draft_sets_text_selection_defaults() {
        let draft =
            article_reader_highlight_draft("quote".into(), "note".into(), "paragraph".into());

        assert_eq!(draft.quote, "quote");
        assert_eq!(draft.note, "note");
        assert_eq!(draft.context, "paragraph");
        assert_eq!(draft.clip_start_seconds, None);
        assert_eq!(draft.clip_end_seconds, None);
        assert!(draft.clip_speaker.is_empty());
        assert!(draft.clip_transcript_segment_ids.is_empty());
        assert!(draft.image.is_none());
    }

    #[test]
    fn highlight_group_card_projection_hides_single_author_groups() {
        let projection = highlight_group_card_projection(HighlightGroupCardProjectionInput {
            items: vec![
                hydrated_highlight_for_pubkey("event-1", "alicepubkey"),
                hydrated_highlight_for_pubkey("event-2", "alicepubkey"),
            ],
            highlighter_profiles: Vec::new(),
        });

        assert!(!projection.show_highlighters_strip);
        assert!(projection.visible_highlighters.is_empty());
        assert_eq!(projection.overflow_count, 0);
        assert!(projection.highlighters_label_segments.is_empty());
    }

    fn hydrated_highlight_for_pubkey(event_id: &str, pubkey: &str) -> HydratedHighlight {
        let mut highlight = highlight_for_source_key(event_id, "a:article", 1);
        highlight.pubkey = pubkey.to_string();
        HydratedHighlight {
            highlight,
            artifact: None,
            shared_by_event_id: None,
            shared_by_pubkey: None,
        }
    }

    fn profile_metadata(
        pubkey: &str,
        name: &str,
        display_name: &str,
        picture: &str,
    ) -> ProfileMetadata {
        ProfileMetadata {
            pubkey: pubkey.into(),
            name: name.into(),
            display_name: display_name.into(),
            about: String::new(),
            picture: picture.into(),
            banner: String::new(),
            nip05: String::new(),
            website: String::new(),
            lud16: String::new(),
            created_at: None,
        }
    }

    fn label_segment(text: &str, emphasized: bool) -> HighlightGroupLabelSegment {
        HighlightGroupLabelSegment {
            text: text.into(),
            emphasized,
        }
    }

    #[test]
    fn insert_unique_front_adds_new_highlight_before_existing_records() {
        let existing = vec![
            highlight_for_source_key("old-1", "a:article", 2),
            highlight_for_source_key("old-2", "a:article", 1),
        ];
        let published = highlight_for_source_key("new", "a:article", 3);

        let out = insert_unique_front(&existing, &published);

        let ids: Vec<_> = out
            .iter()
            .map(|highlight| highlight.event_id.as_str())
            .collect();
        assert_eq!(ids, ["new", "old-1", "old-2"]);
        let original_ids: Vec<_> = existing
            .iter()
            .map(|highlight| highlight.event_id.as_str())
            .collect();
        assert_eq!(original_ids, ["old-1", "old-2"]);
    }

    #[test]
    fn insert_unique_front_preserves_list_when_event_already_exists() {
        let existing = vec![
            highlight_for_source_key("old-1", "a:article", 2),
            highlight_for_source_key("old-2", "a:article", 1),
        ];
        let duplicate = highlight_for_source_key("old-2", "a:article", 3);

        let out = insert_unique_front(&existing, &duplicate);

        let ids: Vec<_> = out
            .iter()
            .map(|highlight| highlight.event_id.as_str())
            .collect();
        assert_eq!(ids, ["old-1", "old-2"]);
    }

    fn artifact_for_isbn(isbn: &str) -> ArtifactRecord {
        let catalog_id = format!("isbn:{isbn}");
        ArtifactRecord {
            preview: ArtifactPreview {
                id: "cbook".into(),
                url: String::new(),
                title: "Some Book".into(),
                author: "An Author".into(),
                image: String::new(),
                description: String::new(),
                source: "book".into(),
                domain: String::new(),
                catalog_id: catalog_id.clone(),
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
                reference_tag_value: catalog_id.clone(),
                reference_kind: "isbn".into(),
                highlight_tag_name: "i".into(),
                highlight_tag_value: catalog_id.clone(),
                highlight_reference_key: format!("i:{catalog_id}"),
                chapters: Vec::new(),
            },
            group_id: "group-a".into(),
            share_event_id: "share-isbn-1".into(),
            pubkey: "a".repeat(64),
            created_at: Some(1_700_000_000),
            note: String::new(),
        }
    }

    #[test]
    fn hydrate_attaches_isbn_artifact_from_cached_share() {
        let (ndb, _tmp) = isolated_ndb();
        let keys = Keys::generate();
        let share = share_event(
            &keys,
            ShareEventSpec {
                group_id: "books",
                d_tag: "book-1",
                title: "The Rust Programming Language",
                source: "book",
                reference_value: "isbn:9781593278281",
                reference_kind: "isbn",
                url: "https://openlibrary.org/isbn/9781593278281",
                created_at: 2_000,
            },
        );
        ingest(&ndb, &share);

        let highlight = highlight_for_source_key("h1", "i:isbn:9781593278281", 1_000);
        let out = hydrate(&ndb, vec![highlight.clone()]).expect("hydrate");

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].highlight.event_id, "h1");
        let artifact = out[0].artifact.as_ref().expect("artifact attached");
        assert_eq!(artifact.preview.title, "The Rust Programming Language");
        assert_eq!(
            artifact.preview.highlight_reference_key,
            "i:isbn:9781593278281"
        );
    }

    #[test]
    fn hydrate_attaches_web_artifact_by_url_reference() {
        let (ndb, _tmp) = isolated_ndb();
        let keys = Keys::generate();
        let share = share_event(
            &keys,
            ShareEventSpec {
                group_id: "articles",
                d_tag: "web-1",
                title: "A Useful Essay",
                source: "article",
                reference_value: "https://example.com/essay",
                reference_kind: "web",
                url: "https://example.com/essay",
                created_at: 2_000,
            },
        );
        ingest(&ndb, &share);

        let highlight = highlight_for_source_key("h-web", "r:https://example.com/essay", 1_000);
        let out = hydrate(&ndb, vec![highlight]).expect("hydrate");

        let artifact = out[0].artifact.as_ref().expect("artifact attached");
        assert_eq!(artifact.preview.title, "A Useful Essay");
        assert_eq!(
            artifact.preview.highlight_reference_key,
            "r:https://example.com/essay"
        );
    }

    #[test]
    fn hydrate_preserves_highlight_when_artifact_missing() {
        let (ndb, _tmp) = isolated_ndb();
        let highlight = highlight_for_source_key("missing", "i:isbn:0000000000000", 1_000);

        let out = hydrate(&ndb, vec![highlight]).expect("hydrate");

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].highlight.event_id, "missing");
        assert!(out[0].artifact.is_none());
        assert!(out[0].shared_by_event_id.is_none());
        assert!(out[0].shared_by_pubkey.is_none());
    }

    #[test]
    fn isbn_highlight_emits_single_i_tag_with_catalog_id() {
        let artifact = artifact_for_isbn("9780735211292");
        let draft = HighlightDraft {
            quote: "one tiny spark".into(),
            context: String::new(),
            note: String::new(),
            clip_start_seconds: None,
            clip_end_seconds: None,
            clip_speaker: String::new(),
            clip_transcript_segment_ids: vec![],
            image: None,
        };
        let builder = build_highlight_event(&draft, &artifact).expect("build");
        let tags = tags_as_vec(&builder);

        let i_tags: Vec<_> = tags
            .iter()
            .filter(|t| t.first().map(String::as_str) == Some("i"))
            .collect();
        assert_eq!(
            i_tags.len(),
            1,
            "exactly one i tag (primary reference already covers NIP-73)"
        );
        assert_eq!(
            i_tags[0],
            &vec!["i".to_string(), "isbn:9780735211292".to_string()]
        );
    }

    #[test]
    fn non_isbn_artifact_with_catalog_id_gets_extra_i_tag() {
        // Simulates a future case where the primary reference is `a` (kind:11
        // address) but the artifact still carries a catalog id like an ISBN —
        // belt-and-suspenders NIP-73 tagging per Nostr convention.
        let mut artifact = artifact_for_isbn("9780735211292");
        artifact.preview.highlight_tag_name = "a".into();
        artifact.preview.highlight_tag_value = "11:abc:def".into();

        let draft = HighlightDraft {
            quote: "q".into(),
            context: String::new(),
            note: String::new(),
            clip_start_seconds: None,
            clip_end_seconds: None,
            clip_speaker: String::new(),
            clip_transcript_segment_ids: vec![],
            image: None,
        };
        let builder = build_highlight_event(&draft, &artifact).expect("build");
        let tags = tags_as_vec(&builder);

        assert!(
            tags.iter().any(|t| t.as_slice() == ["a", "11:abc:def"]),
            "primary `a` reference present"
        );
        assert!(
            tags.iter()
                .any(|t| t.as_slice() == ["i", "isbn:9780735211292"]),
            "NIP-73 `i` catalog tag present alongside"
        );
    }

    #[test]
    fn audio_clip_tags_use_3_decimal_format() {
        let artifact = artifact_for_podcast("https://example.com/ep1");
        let draft = draft_with_clip();
        let builder = build_highlight_event(&draft, &artifact).expect("build highlight event");
        let tags = tags_as_vec(&builder);

        let starts: Vec<_> = tags
            .iter()
            .filter(|t| t.first().map(String::as_str) == Some("start"))
            .collect();
        let ends: Vec<_> = tags
            .iter()
            .filter(|t| t.first().map(String::as_str) == Some("end"))
            .collect();

        assert_eq!(starts.len(), 1, "exactly one start tag");
        assert_eq!(ends.len(), 1, "exactly one end tag");
        assert_eq!(starts[0], &vec!["start".to_string(), "12.500".to_string()]);
        assert_eq!(ends[0], &vec!["end".to_string(), "34.568".to_string()]);
    }

    #[test]
    fn empty_speaker_produces_no_speaker_tag() {
        let artifact = artifact_for_podcast("https://example.com/ep1");
        let mut draft = draft_with_clip();
        draft.clip_speaker = String::new();
        let builder = build_highlight_event(&draft, &artifact).expect("build highlight event");
        let tags = tags_as_vec(&builder);

        assert!(
            !tags
                .iter()
                .any(|t| t.first().map(String::as_str) == Some("speaker")),
            "no speaker tag when speaker is empty, got: {tags:?}"
        );
    }

    #[test]
    fn multiple_segment_ids_produce_multiple_tags() {
        let artifact = artifact_for_podcast("https://example.com/ep1");
        let mut draft = draft_with_clip();
        draft.clip_transcript_segment_ids = vec!["a".into(), "b".into(), "c".into()];
        let builder = build_highlight_event(&draft, &artifact).expect("build highlight event");
        let tags = tags_as_vec(&builder);

        let segments: Vec<_> = tags
            .iter()
            .filter(|t| t.first().map(String::as_str) == Some("segment"))
            .collect();
        assert_eq!(segments.len(), 3, "one tag per segment id");
        assert_eq!(segments[0], &vec!["segment".to_string(), "a".to_string()]);
        assert_eq!(segments[1], &vec!["segment".to_string(), "b".to_string()]);
        assert_eq!(segments[2], &vec!["segment".to_string(), "c".to_string()]);
    }

    #[test]
    fn highlight_for_podcast_uses_nip73_episode_tag() {
        let url = "https://example.com/ep";
        let artifact = artifact_for_podcast(url);
        let draft = HighlightDraft {
            quote: "hello".into(),
            context: String::new(),
            note: String::new(),
            clip_start_seconds: None,
            clip_end_seconds: None,
            clip_speaker: String::new(),
            clip_transcript_segment_ids: vec![],
            image: None,
        };
        let builder = build_highlight_event(&draft, &artifact).expect("build highlight event");
        let tags = tags_as_vec(&builder);

        // NIP-73: podcast highlights use `i podcast:item:guid:<episode-guid>`.
        // The audio URL must not appear as an `r` tag — that's the non-
        // canonical shape we're moving away from.
        let i_tags: Vec<_> = tags
            .iter()
            .filter(|t| t.first().map(String::as_str) == Some("i"))
            .collect();
        assert_eq!(
            i_tags.first().copied(),
            Some(&vec!["i".to_string(), "podcast:item:guid:ep-1".to_string()]),
            "first i-tag must be the canonical episode identifier, got: {tags:?}"
        );
        let has_r_url = tags.iter().any(|t| {
            t.first().map(String::as_str) == Some("r") && t.get(1).map(String::as_str) == Some(url)
        });
        assert!(
            !has_r_url,
            "r:<url> must not appear on a canonical podcast highlight, got: {tags:?}"
        );
    }

    #[test]
    fn imeta_tag_omitted_when_no_image() {
        let artifact = artifact_for_podcast("https://example.com/ep1");
        let draft = draft_with_clip();
        let builder = build_highlight_event(&draft, &artifact).expect("build");
        let tags = tags_as_vec(&builder);
        assert!(
            !tags
                .iter()
                .any(|t| t.first().map(String::as_str) == Some("imeta")),
            "no imeta tag when draft.image is None: {tags:?}"
        );
    }

    #[test]
    fn imeta_tag_present_with_all_fields() {
        let artifact = artifact_for_podcast("https://example.com/ep1");
        let mut draft = draft_with_clip();
        draft.image = Some(BlossomUpload {
            url: "https://blossom.primal.net/abc.jpg".into(),
            sha256_hex: "abc123".into(),
            mime: "image/jpeg".into(),
            size_bytes: 12345,
            width: 1536,
            height: 2048,
            alt: "page text".into(),
        });
        let builder = build_highlight_event(&draft, &artifact).expect("build");
        let tags = tags_as_vec(&builder);

        let imeta = tags
            .iter()
            .find(|t| t.first().map(String::as_str) == Some("imeta"))
            .expect("imeta tag present");
        // Each field is a single space-separated element after "imeta".
        let parts: Vec<&str> = imeta.iter().skip(1).map(String::as_str).collect();
        assert!(parts.contains(&"url https://blossom.primal.net/abc.jpg"));
        assert!(parts.contains(&"m image/jpeg"));
        assert!(parts.contains(&"x abc123"));
        assert!(parts.contains(&"size 12345"));
        assert!(parts.contains(&"dim 1536x2048"));
        assert!(parts.contains(&"alt page text"));
    }

    #[test]
    fn imeta_tag_omits_dim_and_alt_when_unset() {
        let artifact = artifact_for_podcast("https://example.com/ep1");
        let mut draft = draft_with_clip();
        draft.image = Some(BlossomUpload {
            url: "https://blossom.primal.net/abc.jpg".into(),
            sha256_hex: "abc123".into(),
            mime: "image/jpeg".into(),
            size_bytes: 1,
            width: 0,
            height: 0,
            alt: String::new(),
        });
        let builder = build_highlight_event(&draft, &artifact).expect("build");
        let tags = tags_as_vec(&builder);
        let imeta = tags
            .iter()
            .find(|t| t.first().map(String::as_str) == Some("imeta"))
            .expect("imeta tag present");
        let parts: Vec<&str> = imeta.iter().skip(1).map(String::as_str).collect();
        assert!(!parts.iter().any(|p| p.starts_with("dim ")));
        assert!(!parts.iter().any(|p| p.starts_with("alt ")));
    }

    #[test]
    fn comment_tag_emitted_for_note() {
        let artifact = artifact_for_podcast("https://example.com/ep1");
        let draft = HighlightDraft {
            quote: "q".into(),
            context: String::new(),
            note: "a note".into(),
            clip_start_seconds: None,
            clip_end_seconds: None,
            clip_speaker: String::new(),
            clip_transcript_segment_ids: vec![],
            image: None,
        };
        let builder = build_highlight_event(&draft, &artifact).expect("build highlight event");
        let tags = tags_as_vec(&builder);
        assert!(
            tags.iter()
                .any(|t| t.as_slice() == ["comment".to_string(), "a note".to_string()]),
            "comment tag missing: {tags:?}"
        );
    }

    #[test]
    fn context_tag_omitted_when_equal_to_content() {
        let artifact = artifact_for_podcast("https://example.com/ep1");
        let draft = HighlightDraft {
            quote: "same".into(),
            context: "same".into(),
            note: String::new(),
            clip_start_seconds: None,
            clip_end_seconds: None,
            clip_speaker: String::new(),
            clip_transcript_segment_ids: vec![],
            image: None,
        };
        let builder = build_highlight_event(&draft, &artifact).expect("build highlight event");
        let tags = tags_as_vec(&builder);
        assert!(
            !tags
                .iter()
                .any(|t| t.first().map(String::as_str) == Some("context")),
            "context tag should be omitted when equal to content: {tags:?}"
        );
    }

    #[test]
    fn repost_event_has_required_tags() {
        // Use two distinct keys: `reposter` signs the kind:16 event, `author`
        // is the original highlight creator. EventBuilder auto-strips `p` tags
        // matching the signer, so self-references get filtered out — we want
        // to see the `p` tag survive.
        let reposter = Keys::generate();
        let author = Keys::generate();
        let highlight_id = EventId::all_zeros();
        let builder = build_repost_event(
            highlight_id,
            &author.public_key().to_hex(),
            "group-a",
            "wss://relay.highlighter.com",
        )
        .expect("build repost");
        let event = builder.sign_with_keys(&reposter).expect("sign");

        let tags: Vec<Vec<String>> = event.tags.iter().map(|t| t.as_slice().to_vec()).collect();

        assert_eq!(event.kind, Kind::Custom(KIND_GENERIC_REPOST));
        assert_eq!(event.content, "");

        let e = tags
            .iter()
            .find(|t| t.first().map(String::as_str) == Some("e"))
            .expect("e tag");
        assert_eq!(e.len(), 3);
        assert_eq!(e[1], highlight_id.to_hex());
        assert_eq!(e[2], "wss://relay.highlighter.com");

        let k = tags
            .iter()
            .find(|t| t.first().map(String::as_str) == Some("k"))
            .expect("k tag");
        assert_eq!(k[1], "9802");

        let p = tags
            .iter()
            .find(|t| t.first().map(String::as_str) == Some("p"))
            .expect("p tag");
        assert_eq!(p[1], author.public_key().to_hex());

        let h = tags
            .iter()
            .find(|t| t.first().map(String::as_str) == Some("h"))
            .expect("h tag");
        assert_eq!(h[1], "group-a");
    }

    #[test]
    fn query_for_article_returns_only_matching_a_tag() {
        use nostrdb::{Config as NdbConfig, Ndb};
        use tempfile::tempdir;

        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().to_str().unwrap();
        let ndb =
            Ndb::new(db_path, &NdbConfig::new().set_mapsize(64 * 1024 * 1024)).expect("open ndb");

        let keys = Keys::generate();
        let target_address = "30023:aabb:post-1";
        let other_address = "30023:aabb:post-2";

        let matching = EventBuilder::new(Kind::Custom(KIND_HIGHLIGHT), "matching quote")
            .tags(vec![Tag::parse(vec![
                "a".to_string(),
                target_address.to_string(),
            ])
            .unwrap()])
            .sign_with_keys(&keys)
            .expect("sign");
        let other = EventBuilder::new(Kind::Custom(KIND_HIGHLIGHT), "other quote")
            .tags(vec![Tag::parse(vec![
                "a".to_string(),
                other_address.to_string(),
            ])
            .unwrap()])
            .sign_with_keys(&keys)
            .expect("sign");

        for event in [&matching, &other] {
            process_event_and_wait(&ndb, event);
        }

        let hits = query_for_article(&ndb, target_address, 32).expect("query");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].quote, "matching quote");
        assert_eq!(hits[0].artifact_address, target_address);
    }

    #[test]
    fn book_highlight_reference_accepts_raw_or_prefixed_isbn() {
        assert_eq!(
            book_highlight_reference("9780735211292"),
            Some("isbn:9780735211292".to_string())
        );
        assert_eq!(
            book_highlight_reference(" isbn:9780735211292 "),
            Some("isbn:9780735211292".to_string())
        );
        assert_eq!(book_highlight_reference("isbn:  "), None);
        assert_eq!(book_highlight_reference("  "), None);
    }

    #[test]
    fn clip_fallback_quote_formats_hms() {
        assert_eq!(build_clip_fallback_quote(0.0, 65.0), "Clip 0:00-1:05");
        assert_eq!(
            build_clip_fallback_quote(3_600.0, 3_665.0),
            "Clip 1:00:00-1:01:05"
        );
    }

    // -- query_for_group tests ------------------------------------------------

    fn isolated_ndb() -> (Ndb, tempfile::TempDir) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("ndb");
        std::fs::create_dir_all(&path).expect("mkdir");
        let cfg = nostrdb::Config::new().set_mapsize(32 * 1024 * 1024);
        let ndb = Ndb::new(path.to_str().unwrap(), &cfg).expect("open ndb");
        (ndb, tmp)
    }

    fn ingest(ndb: &Ndb, event: &Event) {
        process_event_and_wait(ndb, event);
    }

    fn make_group_highlight(keys: &Keys, group_id: &str, quote: &str) -> Event {
        EventBuilder::new(Kind::Custom(KIND_HIGHLIGHT), quote)
            .tags(vec![
                Tag::parse(vec!["h".to_string(), group_id.to_string()]).unwrap(),
                Tag::parse(vec!["r".to_string(), "https://example.com".to_string()]).unwrap(),
            ])
            .sign_with_keys(keys)
            .expect("sign")
    }

    #[test]
    fn query_for_group_returns_matching_highlights() {
        let (ndb, _tmp) = isolated_ndb();
        let keys = Keys::generate();
        let hl = make_group_highlight(&keys, "alpha", "my insight");
        ingest(&ndb, &hl);

        let records = query_for_group(&ndb, "alpha", 32).expect("query");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].highlight.quote, "my insight");
    }

    #[test]
    fn query_for_group_filters_by_group() {
        let (ndb, _tmp) = isolated_ndb();
        let keys = Keys::generate();
        ingest(&ndb, &make_group_highlight(&keys, "alpha", "alpha hl"));
        ingest(&ndb, &make_group_highlight(&keys, "bravo", "bravo hl"));

        let alpha = query_for_group(&ndb, "alpha", 32).expect("alpha");
        assert_eq!(alpha.len(), 1);
        assert_eq!(alpha[0].highlight.quote, "alpha hl");

        let bravo = query_for_group(&ndb, "bravo", 32).expect("bravo");
        assert_eq!(bravo.len(), 1);
        assert_eq!(bravo[0].highlight.quote, "bravo hl");
    }

    #[test]
    fn query_for_group_excludes_highlights_without_h_tag() {
        let (ndb, _tmp) = isolated_ndb();
        let keys = Keys::generate();
        // Highlight without any h tag — must be excluded from group queries.
        let no_h = EventBuilder::new(Kind::Custom(KIND_HIGHLIGHT), "no group")
            .tags(vec![Tag::parse(vec![
                "r".to_string(),
                "https://example.com".to_string(),
            ])
            .unwrap()])
            .sign_with_keys(&keys)
            .expect("sign");
        ingest(&ndb, &no_h);
        ingest(&ndb, &make_group_highlight(&keys, "alpha", "alpha hl"));

        let records = query_for_group(&ndb, "alpha", 32).expect("query");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].highlight.quote, "alpha hl");
    }

    // Phase 7 parity (gotcha #7): the kernel `decode_highlight_row` must extract
    // the SAME NIP-84/NIP-73 source/clip/image fields as the bespoke
    // `record_from_cached_event`. Build ONE kind:9802 event, parse it BOTH ways
    // from the same tags, assert field-by-field equality (no hardcoded expected
    // values — both impls are exercised against the real fixture).
    #[test]
    fn kernel_highlight_row_matches_bespoke_record_parse() {
        let keys = Keys::generate();
        let context = "The surrounding paragraph.";
        let comment = "my note";
        let artifact = "30023:aaaa:my-article";
        let isbn = "isbn:9780735211292";
        let segment_a = "seg-1";
        let segment_b = "seg-2";
        let image = "https://example.com/scan.jpg";

        let event = EventBuilder::new(Kind::Custom(KIND_HIGHLIGHT), "the quoted text")
            .tags(vec![
                Tag::parse(vec!["a".to_string(), artifact.to_string()]).unwrap(),
                Tag::parse(vec!["i".to_string(), isbn.to_string()]).unwrap(),
                Tag::parse(vec!["context".to_string(), context.to_string()]).unwrap(),
                Tag::parse(vec!["comment".to_string(), comment.to_string()]).unwrap(),
                Tag::parse(vec!["start".to_string(), "12.5".to_string()]).unwrap(),
                Tag::parse(vec!["end".to_string(), "30.0".to_string()]).unwrap(),
                Tag::parse(vec!["speaker".to_string(), "Alice".to_string()]).unwrap(),
                Tag::parse(vec!["segment".to_string(), segment_a.to_string()]).unwrap(),
                Tag::parse(vec!["segment".to_string(), segment_b.to_string()]).unwrap(),
                Tag::parse(vec![
                    "imeta".to_string(),
                    format!("url {image}"),
                    "m image/jpeg".to_string(),
                ])
                .unwrap(),
            ])
            .sign_with_keys(&keys)
            .expect("sign");

        // Bespoke parse.
        let bespoke = record_from_cached_event(&event).expect("bespoke record");

        // Equivalent raw kernel event from the SAME tags.
        let kernel_event = nmp_core::substrate::KernelEvent {
            id: event.id.to_hex(),
            author: event.pubkey.to_hex(),
            kind: KIND_HIGHLIGHT as u32,
            created_at: event.created_at.as_secs(),
            content: event.content.clone(),
            tags: event.tags.iter().map(|t| t.as_slice().to_vec()).collect(),
            relay_provenance: vec![],
        };
        let kernel = crate::kernel::domains::highlight_feed::decode_highlight_row(&kernel_event)
            .expect("kernel row");

        assert_eq!(kernel.content, bespoke.quote, "quote/content");
        assert_eq!(kernel.context, bespoke.context, "context");
        assert_eq!(
            kernel.note.unwrap_or_default(),
            bespoke.note,
            "note/comment"
        );
        assert_eq!(
            kernel.artifact_address, bespoke.artifact_address,
            "artifact_address (a)"
        );
        assert_eq!(
            kernel.event_reference, bespoke.event_reference,
            "event_reference (e)"
        );
        assert_eq!(
            kernel.external_reference, bespoke.external_reference,
            "external_reference (i)"
        );
        assert_eq!(kernel.source_url, bespoke.source_url, "source_url (r)");
        assert_eq!(
            kernel.source_reference_key, bespoke.source_reference_key,
            "source_reference_key"
        );
        assert_eq!(
            kernel.clip_start_seconds, bespoke.clip_start_seconds,
            "clip_start_seconds"
        );
        assert_eq!(
            kernel.clip_end_seconds, bespoke.clip_end_seconds,
            "clip_end_seconds"
        );
        assert_eq!(kernel.clip_speaker, bespoke.clip_speaker, "clip_speaker");
        assert_eq!(
            kernel.clip_transcript_segment_ids, bespoke.clip_transcript_segment_ids,
            "clip_transcript_segment_ids"
        );
        assert_eq!(kernel.image_url, bespoke.image_url, "image_url (imeta)");
    }
}
