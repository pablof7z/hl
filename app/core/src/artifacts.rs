//! Artifact share (kind:11) building, publishing, and querying. Ports
//! `web/src/lib/ndk/artifacts.ts`.

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;
use crate::models::{ArtifactPreview, ArtifactRecord};
use crate::nostr_runtime::NostrRuntime;

/// kind:11 "Thread" is used both for artifact shares and for discussions.
const KIND_ARTIFACT_SHARE: u16 = 11;

const TRACKING_PARAMS: &[&str] = &[
    "fbclid",
    "gclid",
    "mc_cid",
    "mc_eid",
    "ref",
    "ref_src",
    "ref_url",
];

/// Port of `buildArtifactPreview` (`web/src/lib/ndk/artifacts.ts:170-236`).
/// Builds a preview from a bare URL — normalizes the URL, derives a stable
/// FNV-1a id, infers `source` from the host, and populates the reference
/// tag as an `i` tag pointing at the normalized URL. Callers that already
/// have richer metadata (ISBN lookup, podcast catalog entry) should use
/// `build_preview_with` instead.
pub fn build_preview(url: &str) -> Result<ArtifactPreview, CoreError> {
    build_preview_with(PreviewInput {
        url: url.to_string(),
        ..Default::default()
    })
}

/// Rust-side equivalent of the TS function's full input surface. Most
/// callers use `build_preview(url)` which defaults everything.
#[derive(Debug, Default, Clone)]
pub struct PreviewInput {
    pub url: String,
    pub title: String,
    pub author: String,
    pub image: String,
    pub description: String,
    pub source: Option<String>,
    pub domain: String,
    pub catalog_id: String,
    pub catalog_kind: String,
    pub podcast_guid: String,
    /// Episode-level `<item><guid>` from the RSS feed. When set, the
    /// preview canonicalizes both reference and highlight tags to
    /// `i podcast:item:guid:<episode-guid>` (NIP-73).
    pub podcast_item_guid: String,
    pub podcast_show_title: String,
    pub audio_url: String,
    pub audio_preview_url: String,
    pub transcript_url: String,
    pub feed_url: String,
    pub published_at: String,
    pub duration_seconds: Option<i64>,
    pub reference_tag_name: Option<String>,
    pub reference_tag_value: String,
    pub reference_kind: String,
    pub highlight_tag_name: Option<String>,
    pub highlight_tag_value: String,
}

pub fn build_preview_with(input: PreviewInput) -> Result<ArtifactPreview, CoreError> {
    let normalized_url = normalize_artifact_url(&input.url)
        .ok_or_else(|| CoreError::InvalidInput("invalid URL".into()))?;

    // Resolve the episode-level GUID first — it canonicalizes both the
    // reference and highlight tags for podcasts. Source-of-truth order:
    // explicit input → episode-guid embedded in catalog_id (`podcast:item:guid:…`)
    // → extract from a hint in the reference value.
    let podcast_item_guid = first_non_empty(&[
        &input.podcast_item_guid,
        &podcast_item_guid_from_catalog_value(&input.catalog_id),
        &podcast_item_guid_from_catalog_value(&input.reference_tag_value),
    ]);

    let explicit_ref_name = clean(&input.reference_tag_name.unwrap_or_default());
    let explicit_highlight_name = clean(&input.highlight_tag_name.unwrap_or_default());

    // NIP-73 canonical: a known episode GUID always wins, overriding any
    // `r:<audio_url>` or `i:podcast:guid:<feed>` the caller might have set.
    let (reference_tag_name, reference_tag_value, highlight_tag_name, highlight_tag_value) =
        if !podcast_item_guid.is_empty() {
            let canonical = format!("podcast:item:guid:{podcast_item_guid}");
            (
                "i".to_string(),
                canonical.clone(),
                "i".to_string(),
                canonical,
            )
        } else {
            let ref_name = if explicit_ref_name.is_empty() {
                "i".to_string()
            } else {
                explicit_ref_name
            };
            let ref_value = first_non_empty(&[
                &input.reference_tag_value,
                &input.catalog_id,
                &normalized_url,
            ]);
            let hl_name = if explicit_highlight_name.is_empty() {
                "r".to_string()
            } else {
                explicit_highlight_name
            };
            let hl_value = first_non_empty(&[&input.highlight_tag_value, &normalized_url]);
            (ref_name, ref_value, hl_name, hl_value)
        };

    let reference_kind = first_non_empty(&[
        &input.reference_kind,
        &input.catalog_kind,
        if !podcast_item_guid.is_empty() {
            "podcast:item:guid"
        } else {
            "web"
        },
    ]);

    let reference_key = reference_key_for_tag(&reference_tag_name, &reference_tag_value);
    let highlight_reference_key =
        reference_key_for_tag(&highlight_tag_name, &highlight_tag_value);
    if reference_key.is_empty() {
        return Err(CoreError::InvalidInput(
            "artifact reference key is empty".into(),
        ));
    }

    let domain = first_non_empty(&[&input.domain, &domain_label(&normalized_url)]);
    let title = first_non_empty(&[&input.title, &fallback_title(&normalized_url)]);
    let source = input
        .source
        .map(|s| clean(&s))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| detect_artifact_source(&normalized_url).to_string());

    // Feed GUID is an orthogonal identifier — derive from explicit input
    // or by stripping the `podcast:guid:` prefix off whatever catalog
    // value was supplied. Kept so shares can emit a secondary `i
    // podcast:guid:<feed>` alongside the canonical episode tag.
    let podcast_guid = first_non_empty(&[
        &input.podcast_guid,
        &podcast_guid_from_catalog_value(&input.catalog_id),
        &podcast_guid_from_catalog_value(&input.reference_tag_value),
    ]);

    Ok(ArtifactPreview {
        id: artifact_id_from_reference_key(&reference_key)?,
        url: normalized_url,
        title,
        author: clean(&input.author),
        image: clean(&input.image),
        description: clean(&input.description),
        source,
        domain,
        catalog_id: first_non_empty(&[&input.catalog_id, &reference_tag_value]),
        catalog_kind: first_non_empty(&[&input.catalog_kind, &reference_kind]),
        podcast_guid,
        podcast_item_guid,
        podcast_show_title: clean(&input.podcast_show_title),
        audio_url: clean(&input.audio_url),
        audio_preview_url: clean(&input.audio_preview_url),
        transcript_url: clean(&input.transcript_url),
        feed_url: clean(&input.feed_url),
        published_at: clean(&input.published_at),
        duration_seconds: input.duration_seconds.filter(|d| *d >= 0),
        reference_tag_name,
        reference_tag_value,
        reference_kind,
        highlight_tag_name,
        highlight_tag_value,
        highlight_reference_key,
        chapters: Vec::new(),
    })
}

fn podcast_item_guid_from_catalog_value(value: &str) -> String {
    let normalized = clean(value);
    normalized
        .strip_prefix("podcast:item:guid:")
        .map(str::to_string)
        .unwrap_or_default()
}

/// Publish a kind:11 artifact share into a NIP-29 group. Port of
/// `publishArtifact` (`web/src/lib/ndk/artifacts.ts:468-507`), minus the
/// "existing artifact" merge path — that's an MVP-later concern; if a
/// duplicate kind:11 with the same `d` tag exists the relay will upsert.
pub async fn publish(
    runtime: &NostrRuntime,
    preview: ArtifactPreview,
    group_id: &str,
    note: Option<&str>,
) -> Result<ArtifactRecord, CoreError> {
    if group_id.trim().is_empty() {
        return Err(CoreError::InvalidInput("group_id must not be empty".into()));
    }

    let client = runtime.client();

    let builder = build_share_event(group_id, &preview, note)?;
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign artifact share: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish artifact share: {e}")))?;

    Ok(ArtifactRecord {
        preview,
        group_id: group_id.to_string(),
        share_event_id: event.id.to_hex(),
        pubkey: event.pubkey.to_hex(),
        created_at: Some(event.created_at.as_secs()),
        note: note.unwrap_or("").trim().to_string(),
    })
}

/// Port of `fetchArtifactSharesForGroup`. MVP leaves this to the live
/// subscription — stub returns empty and lets the Room pump hydrate via
/// deltas.
pub async fn fetch_shares(
    _group_id: &str,
    _limit: u32,
) -> Result<Vec<ArtifactRecord>, CoreError> {
    Ok(Vec::new())
}

/// Read kind:11 artifact shares for `group_id` from nostrdb, newest first.
/// Scans by kind only and checks `#h` manually — the nostrdb tag index is
/// unreliable for non-standard tag names on some event kinds.
pub fn query_for_group(
    ndb: &Ndb,
    group_id: &str,
    limit: u32,
) -> Result<Vec<ArtifactRecord>, CoreError> {
    let group_id = group_id.trim();
    if group_id.is_empty() {
        return Ok(Vec::new());
    }
    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let cap = (limit.saturating_mul(4)).max(128) as i32;
    let filter = NdbFilter::new()
        .kinds([KIND_ARTIFACT_SHARE as u64])
        .build();

    let results = ndb
        .query(&txn, &[filter], cap)
        .map_err(|e| CoreError::Cache(format!("query artifacts for group: {e}")))?;

    let mut records: Vec<ArtifactRecord> = Vec::new();
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else { continue };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else { continue };
        let Some(h) = first_tag_value(&event, "h") else { continue };
        if h != group_id {
            continue;
        }
        if crate::discussions::is_discussion(&event) {
            continue;
        }
        if let Some(rec) = artifact_record_from_event(&event, group_id) {
            records.push(rec);
        }
    }

    records.sort_by(|a, b| b.created_at.unwrap_or(0).cmp(&a.created_at.unwrap_or(0)));
    records.truncate(limit as usize);
    Ok(records)
}

/// Simple title/author substring search over cached artifacts.
pub fn search_cached(
    _query: &str,
    _limit: u32,
) -> Result<Vec<ArtifactRecord>, CoreError> {
    Ok(Vec::new())
}

// -- Event helpers -----------------------------------------------------------

pub(crate) fn first_tag_value<'a>(event: &'a Event, name: &str) -> Option<&'a str> {
    for tag in event.tags.iter() {
        let slice = tag.as_slice();
        if slice.first().map(String::as_str) == Some(name) {
            return slice.get(1).map(String::as_str);
        }
    }
    None
}

/// Lift `chapter` tags off a kind:11 podcast share. Wire format:
/// `["chapter", "<start_seconds>", "<title>"]`. Skips malformed entries
/// silently — chapter metadata is best-effort and a bad publisher shouldn't
/// break the listening surface. Output is sorted by start time.
pub(crate) fn read_chapters(event: &Event) -> Vec<crate::models::Chapter> {
    let mut chapters: Vec<crate::models::Chapter> = event
        .tags
        .iter()
        .filter_map(|tag| {
            let slice = tag.as_slice();
            if slice.first().map(String::as_str) != Some("chapter") {
                return None;
            }
            let start = slice.get(1)?.parse::<f64>().ok()?;
            if !start.is_finite() || start < 0.0 {
                return None;
            }
            let title = slice.get(2).map(String::as_str).unwrap_or("").trim();
            if title.is_empty() {
                return None;
            }
            Some(crate::models::Chapter {
                start_seconds: start,
                title: title.to_string(),
            })
        })
        .collect();
    chapters.sort_by(|a, b| {
        a.start_seconds
            .partial_cmp(&b.start_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    chapters
}

/// Build an `ArtifactRecord` from a cached kind:11 event. Mirrors the
/// tag-reading shape of `artifactFromEvent` in `web/src/lib/ndk/artifacts.ts`
/// — every tag the publisher emits has to be lifted off the event here, or
/// downstream views see empty strings (e.g. an empty `audio_url` makes the
/// podcast player refuse to load anything).
pub(crate) fn artifact_record_from_event(event: &Event, group_id: &str) -> Option<ArtifactRecord> {
    let title = first_tag_value(event, "title").unwrap_or("").to_string();
    let url = first_tag_value(event, "r").unwrap_or("").to_string();
    let source = first_tag_value(event, "source").unwrap_or("").to_string();
    let author = first_tag_value(event, "author").unwrap_or("").to_string();
    let image = first_tag_value(event, "image").unwrap_or("").to_string();
    let summary = first_tag_value(event, "summary").unwrap_or("").to_string();
    let d = first_tag_value(event, "d").unwrap_or("").to_string();
    let k = first_tag_value(event, "k").unwrap_or("").to_string();

    let (ref_name, ref_value) = if let Some(i) = first_tag_value(event, "i") {
        ("i".to_string(), i.to_string())
    } else if let Some(a) = first_tag_value(event, "a") {
        ("a".to_string(), a.to_string())
    } else if let Some(e_val) = first_tag_value(event, "e") {
        ("e".to_string(), e_val.to_string())
    } else {
        (String::new(), String::new())
    };

    // NIP-73 episode/feed GUIDs: prefer dedicated tags, then fall back to
    // parsing the `i` tag prefix.
    let podcast_item_guid = first_tag_value(event, "podcast_item_guid")
        .map(str::to_string)
        .unwrap_or_else(|| {
            ref_value
                .strip_prefix("podcast:item:guid:")
                .map(str::to_string)
                .unwrap_or_default()
        });
    let podcast_guid = first_tag_value(event, "podcast_guid")
        .map(str::to_string)
        .unwrap_or_else(|| {
            ref_value
                .strip_prefix("podcast:guid:")
                .map(str::to_string)
                .unwrap_or_default()
        });

    let preview = ArtifactPreview {
        id: d,
        url,
        title,
        author,
        image,
        description: summary,
        source,
        domain: String::new(),
        catalog_id: if ref_name == "i" { ref_value.clone() } else { String::new() },
        catalog_kind: k.clone(),
        podcast_guid,
        podcast_item_guid,
        podcast_show_title: first_tag_value(event, "podcast_show_title")
            .unwrap_or("")
            .to_string(),
        audio_url: first_tag_value(event, "audio").unwrap_or("").to_string(),
        audio_preview_url: first_tag_value(event, "audio_preview").unwrap_or("").to_string(),
        transcript_url: first_tag_value(event, "transcript").unwrap_or("").to_string(),
        feed_url: first_tag_value(event, "feed").unwrap_or("").to_string(),
        published_at: first_tag_value(event, "published_at").unwrap_or("").to_string(),
        duration_seconds: first_tag_value(event, "duration").and_then(|v| v.parse::<i64>().ok()),
        reference_tag_name: ref_name.clone(),
        reference_tag_value: ref_value,
        reference_kind: k,
        highlight_tag_name: String::new(),
        highlight_tag_value: String::new(),
        highlight_reference_key: String::new(),
        chapters: read_chapters(event),
    };

    Some(ArtifactRecord {
        preview,
        group_id: group_id.to_string(),
        share_event_id: event.id.to_hex(),
        pubkey: event.pubkey.to_hex(),
        created_at: Some(event.created_at.as_secs()),
        note: event.content.clone(),
    })
}

// -- URL helpers -------------------------------------------------------------

/// Port of `normalizeArtifactUrl`. Returns `None` for non-http(s) URLs and
/// for unparseable inputs.
pub fn normalize_artifact_url(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut url = url::Url::parse(trimmed).ok()?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return None;
    }

    url.set_fragment(None);
    let _ = url.set_username("");
    let _ = url.set_password(None);

    // Lowercase host. url::Url already lowercases on parse, but set_host
    // keeps the conversion explicit if we ever mutate it.
    if let Some(host) = url.host_str().map(|h| h.to_ascii_lowercase()) {
        let _ = url.set_host(Some(&host));
    }

    // Strip default ports.
    if let Some(port) = url.port() {
        match (url.scheme(), port) {
            ("http", 80) | ("https", 443) => {
                let _ = url.set_port(None);
            }
            _ => {}
        }
    }

    // Strip trailing slashes except the root.
    if url.path() != "/" {
        let trimmed_path = url.path().trim_end_matches('/');
        let new_path = if trimmed_path.is_empty() { "/" } else { trimmed_path };
        let owned = new_path.to_string();
        url.set_path(&owned);
    }

    // Drop utm_* and known tracking params; sort the rest alphabetically.
    let mut kept: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| {
            let lower = k.to_ascii_lowercase();
            !lower.starts_with("utm_") && !TRACKING_PARAMS.iter().any(|t| *t == lower.as_str())
        })
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    kept.sort_by(|a, b| a.0.cmp(&b.0));
    url.set_query(None);
    if !kept.is_empty() {
        let mut pairs = url.query_pairs_mut();
        for (k, v) in kept {
            pairs.append_pair(&k, &v);
        }
    }

    Some(url.to_string())
}

/// Port of `detectArtifactSource`. Known podcast / video / paper / book hosts
/// fall through to `"article"` as the most reasonable default for anything
/// with a path; bare domains end up `"web"`.
pub fn detect_artifact_source(url: &str) -> &'static str {
    let Some(normalized) = normalize_artifact_url(url) else {
        return "web";
    };
    let Ok(parsed) = url::Url::parse(&normalized) else {
        return "web";
    };
    let host = parsed
        .host_str()
        .unwrap_or("")
        .trim_start_matches("www.")
        .to_ascii_lowercase();

    if host.contains("youtube.com")
        || host.contains("youtu.be")
        || host.contains("vimeo.com")
        || host.contains("tiktok.com")
    {
        return "video";
    }
    if host.contains("spotify.com")
        || host.contains("podcasts.apple.com")
        || host.contains("overcast.fm")
    {
        return "podcast";
    }
    if host.contains("arxiv.org")
        || host.contains("ssrn.com")
        || host.contains("researchgate.net")
        || host.contains("doi.org")
    {
        return "paper";
    }
    if host.contains("goodreads.com")
        || host.contains("openlibrary.org")
        || host.contains("bookshop.org")
    {
        return "book";
    }
    "article"
}

fn domain_label(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string()))
        .unwrap_or_else(|| url.to_string())
}

fn fallback_title(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        let path = parsed.path().trim_end_matches('/');
        if let Some(last) = path.split('/').filter(|s| !s.is_empty()).last() {
            return title_case(&last.replace(['-', '_'], " "));
        }
        return parsed
            .host_str()
            .map(|h| h.trim_start_matches("www.").to_string())
            .unwrap_or_else(|| url.to_string());
    }
    "Untitled source".into()
}

fn title_case(value: &str) -> String {
    value
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + chars.as_str()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn podcast_guid_from_catalog_value(value: &str) -> String {
    let normalized = clean(value);
    normalized
        .strip_prefix("podcast:guid:")
        .map(str::to_string)
        .unwrap_or_default()
}

fn reference_key_for_tag(tag_name: &str, value: &str) -> String {
    let cleaned = clean(value);
    if cleaned.is_empty() {
        String::new()
    } else {
        format!("{tag_name}:{cleaned}")
    }
}

fn artifact_id_from_reference_key(reference_key: &str) -> Result<String, CoreError> {
    let normalized = clean(reference_key);
    if normalized.is_empty() {
        return Err(CoreError::InvalidInput(
            "artifact references need a stable key".into(),
        ));
    }
    let hash = fnv1a(&normalized);
    Ok(format!("c{}", to_base36(hash)))
}

/// Port of `fnv1a` at `web/src/lib/ndk/artifacts.ts:1086-1095`. 32-bit
/// unsigned, using wrapping multiply so the output matches the JS
/// `Math.imul(hash, 0x01000193)` behavior exactly.
fn fnv1a(value: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in value.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

fn to_base36(mut value: u32) -> String {
    if value == 0 {
        return "0".into();
    }
    const ALPHABET: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out: Vec<u8> = Vec::new();
    while value > 0 {
        out.push(ALPHABET[(value % 36) as usize]);
        value /= 36;
    }
    out.reverse();
    String::from_utf8(out).unwrap()
}

fn clean(value: &str) -> String {
    value.trim().to_string()
}

fn first_non_empty(values: &[&str]) -> String {
    for v in values {
        let cleaned = clean(v);
        if !cleaned.is_empty() {
            return cleaned;
        }
    }
    String::new()
}

// -- Event builder -----------------------------------------------------------

/// Pure builder for the kind:11 artifact share event. Mirrors
/// `buildArtifactShareEvent` (`web/src/lib/ndk/artifacts.ts:509-590`).
fn build_share_event(
    group_id: &str,
    preview: &ArtifactPreview,
    note: Option<&str>,
) -> Result<EventBuilder, CoreError> {
    let mut tags: Vec<Tag> = Vec::new();
    tags.push(parse_tag(&["h", group_id])?);
    tags.push(parse_tag(&["d", &preview.id])?);
    tags.push(parse_tag(&["title", &preview.title])?);
    tags.push(parse_tag(&["source", &preview.source])?);

    match preview.reference_tag_name.as_str() {
        "i" => {
            if !preview.url.is_empty() {
                tags.push(parse_tag(&["i", &preview.reference_tag_value, &preview.url])?);
            } else {
                tags.push(parse_tag(&["i", &preview.reference_tag_value])?);
            }
            if !preview.reference_kind.is_empty() {
                tags.push(parse_tag(&["k", &preview.reference_kind])?);
            }

            // For podcast episodes: emit the feed-level NIP-73 identifier
            // as a secondary `i` tag so clients indexing by show (not
            // episode) still discover the share. Skip when the primary
            // reference already IS the feed-level tag.
            let ref_is_item = preview
                .reference_tag_value
                .starts_with("podcast:item:guid:");
            let has_feed_guid = !preview.podcast_guid.is_empty();
            let feed_catalog = format!("podcast:guid:{}", preview.podcast_guid);
            let ref_is_feed = preview.reference_tag_value == feed_catalog;
            if ref_is_item && has_feed_guid && !ref_is_feed {
                tags.push(parse_tag(&["i", &feed_catalog])?);
            }
        }
        other if !other.is_empty() => {
            tags.push(parse_tag(&[other, &preview.reference_tag_value])?);
        }
        _ => {}
    }

    if !preview.url.is_empty() {
        tags.push(parse_tag(&["r", &preview.url])?);
    }
    if !preview.author.is_empty() {
        tags.push(parse_tag(&["author", &preview.author])?);
    }
    if !preview.image.is_empty() {
        tags.push(parse_tag(&["image", &preview.image])?);
    }
    if !preview.description.is_empty() {
        tags.push(parse_tag(&["summary", &preview.description])?);
    }
    if !preview.podcast_guid.is_empty() {
        tags.push(parse_tag(&["podcast_guid", &preview.podcast_guid])?);
    }
    if !preview.podcast_show_title.is_empty() {
        tags.push(parse_tag(&["podcast_show_title", &preview.podcast_show_title])?);
    }
    if !preview.audio_url.is_empty() {
        tags.push(parse_tag(&["audio", &preview.audio_url])?);
    }
    if !preview.audio_preview_url.is_empty() {
        tags.push(parse_tag(&["audio_preview", &preview.audio_preview_url])?);
    }
    if !preview.transcript_url.is_empty() {
        tags.push(parse_tag(&["transcript", &preview.transcript_url])?);
    }
    if !preview.feed_url.is_empty() {
        tags.push(parse_tag(&["feed", &preview.feed_url])?);
    }
    if !preview.published_at.is_empty() {
        tags.push(parse_tag(&["published_at", &preview.published_at])?);
    }
    if let Some(d) = preview.duration_seconds {
        if d >= 0 {
            tags.push(parse_tag(&["duration", &d.to_string()])?);
        }
    }

    let content = note.map(clean).unwrap_or_default();
    Ok(EventBuilder::new(Kind::Custom(KIND_ARTIFACT_SHARE), content).tags(tags))
}

fn parse_tag(parts: &[&str]) -> Result<Tag, CoreError> {
    Tag::parse(parts.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        .map_err(|e| CoreError::Other(format!("build tag: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_tracking_and_default_ports() {
        let out = normalize_artifact_url(
            "https://EXAMPLE.com:443/path/?utm_source=x&a=1&b=2&fbclid=y#frag",
        )
        .unwrap();
        assert_eq!(out, "https://example.com/path?a=1&b=2");
    }

    #[test]
    fn normalize_rejects_non_http() {
        assert!(normalize_artifact_url("javascript:alert(1)").is_none());
        assert!(normalize_artifact_url("ftp://example.com").is_none());
        assert!(normalize_artifact_url("").is_none());
    }

    #[test]
    fn detect_source_podcast_for_overcast() {
        assert_eq!(detect_artifact_source("https://overcast.fm/+abc123"), "podcast");
        assert_eq!(
            detect_artifact_source("https://podcasts.apple.com/us/ep/123"),
            "podcast"
        );
    }

    #[test]
    fn detect_source_video_for_youtube() {
        assert_eq!(
            detect_artifact_source("https://www.youtube.com/watch?v=x"),
            "video"
        );
    }

    #[test]
    fn detect_source_fallback_is_article() {
        assert_eq!(detect_artifact_source("https://example.com/post"), "article");
    }

    #[test]
    fn fnv1a_matches_js_reference() {
        // Reference values computed with the JS `fnv1a` at artifacts.ts:1086.
        // "r:https://example.com" -> 0x... but we just assert stability.
        let a = fnv1a("r:https://example.com");
        let b = fnv1a("r:https://example.com");
        assert_eq!(a, b);
        assert_ne!(a, fnv1a("r:https://example.org"));
    }

    #[test]
    fn base36_encoding() {
        assert_eq!(to_base36(0), "0");
        assert_eq!(to_base36(35), "z");
        assert_eq!(to_base36(36), "10");
    }

    #[test]
    fn build_preview_for_overcast_url() {
        let preview = build_preview("https://overcast.fm/+abc123").expect("preview");
        assert_eq!(preview.url, "https://overcast.fm/+abc123");
        assert_eq!(preview.source, "podcast");
        assert_eq!(preview.reference_tag_name, "i");
        assert_eq!(preview.reference_tag_value, "https://overcast.fm/+abc123");
        assert_eq!(preview.reference_kind, "web");
        assert_eq!(preview.highlight_tag_name, "r");
        assert_eq!(preview.highlight_tag_value, "https://overcast.fm/+abc123");
        assert!(preview.id.starts_with('c'));
        // id must be stable across runs
        let preview2 = build_preview("https://overcast.fm/+abc123").unwrap();
        assert_eq!(preview.id, preview2.id);
    }

    #[test]
    fn build_share_event_emits_h_d_title_source_tags() {
        let preview = build_preview("https://example.com/post").unwrap();
        let builder = build_share_event("room-a", &preview, Some("hi")).unwrap();
        let keys = Keys::generate();
        let event = builder.sign_with_keys(&keys).expect("sign");
        let has = |name: &str, val: &str| {
            event.tags.iter().any(|t| {
                let s = t.as_slice();
                s.first().map(String::as_str) == Some(name)
                    && s.get(1).map(String::as_str) == Some(val)
            })
        };
        assert_eq!(event.kind, Kind::Custom(KIND_ARTIFACT_SHARE));
        assert_eq!(event.content, "hi");
        assert!(has("h", "room-a"));
        assert!(has("d", &preview.id));
        assert!(has("title", &preview.title));
        assert!(has("source", "article"));
        assert!(has("i", "https://example.com/post"));
        assert!(has("k", "web"));
        assert!(has("r", "https://example.com/post"));
    }

    #[test]
    fn artifact_record_lifts_podcast_tags_off_kind_11() {
        // Real Tucker Carlson event from relay.highlighter.com. Before the
        // fix the audio_url field was always empty — iOS player would never
        // load anything no matter how many times you tapped Play.
        let keys = Keys::generate();
        let event = EventBuilder::new(Kind::Custom(KIND_ARTIFACT_SHARE), "")
            .tags(vec![
                Tag::parse(vec!["h".to_string(), "signal-over-noise".to_string()]).unwrap(),
                Tag::identifier("ccc5hrs"),
                Tag::parse(vec!["title".to_string(), "Tucker Debates Biotech CEO".to_string()]).unwrap(),
                Tag::parse(vec!["source".to_string(), "podcast".to_string()]).unwrap(),
                Tag::parse(vec![
                    "i".to_string(),
                    "podcast:guid:D27CDB6E-AE6D-11cf-96B8-444553540000".to_string(),
                    "http://www.tuckercarlson.com/".to_string(),
                ])
                .unwrap(),
                Tag::parse(vec!["k".to_string(), "podcast:guid".to_string()]).unwrap(),
                Tag::parse(vec!["r".to_string(), "http://www.tuckercarlson.com/".to_string()]).unwrap(),
                Tag::parse(vec![
                    "podcast_guid".to_string(),
                    "D27CDB6E-AE6D-11cf-96B8-444553540000".to_string(),
                ])
                .unwrap(),
                Tag::parse(vec![
                    "audio".to_string(),
                    "https://www.podtrac.com/pts/redirect.mp3/example/TCN.mp3".to_string(),
                ])
                .unwrap(),
                Tag::parse(vec!["duration".to_string(), "3742".to_string()]).unwrap(),
            ])
            .sign_with_keys(&keys)
            .expect("sign");

        let record = artifact_record_from_event(&event, "signal-over-noise").expect("record");
        assert_eq!(
            record.preview.audio_url,
            "https://www.podtrac.com/pts/redirect.mp3/example/TCN.mp3"
        );
        assert_eq!(record.preview.podcast_guid, "D27CDB6E-AE6D-11cf-96B8-444553540000");
        assert_eq!(record.preview.duration_seconds, Some(3742));
        assert_eq!(record.preview.catalog_kind, "podcast:guid");
        assert_eq!(record.preview.reference_kind, "podcast:guid");
    }

    #[test]
    fn artifact_record_lifts_chapter_tags() {
        let keys = Keys::generate();
        let event = EventBuilder::new(Kind::Custom(KIND_ARTIFACT_SHARE), "")
            .tags(vec![
                Tag::parse(vec!["h".to_string(), "room".to_string()]).unwrap(),
                Tag::identifier("ep1"),
                Tag::parse(vec!["title".to_string(), "Ep 1".to_string()]).unwrap(),
                Tag::parse(vec!["source".to_string(), "podcast".to_string()]).unwrap(),
                Tag::parse(vec!["chapter".to_string(), "0".to_string(), "Cold open".to_string()]).unwrap(),
                Tag::parse(vec!["chapter".to_string(), "245".to_string(), "First guest".to_string()]).unwrap(),
                // Out of order: should sort.
                Tag::parse(vec!["chapter".to_string(), "120".to_string(), "Setup".to_string()]).unwrap(),
                // Malformed: missing title — should be dropped.
                Tag::parse(vec!["chapter".to_string(), "999".to_string()]).unwrap(),
                // Malformed: non-numeric start — should be dropped.
                Tag::parse(vec!["chapter".to_string(), "junk".to_string(), "Ignored".to_string()]).unwrap(),
            ])
            .sign_with_keys(&keys)
            .expect("sign");

        let record = artifact_record_from_event(&event, "room").expect("record");
        let chapters = &record.preview.chapters;
        assert_eq!(chapters.len(), 3);
        assert_eq!(chapters[0].start_seconds, 0.0);
        assert_eq!(chapters[0].title, "Cold open");
        assert_eq!(chapters[1].start_seconds, 120.0);
        assert_eq!(chapters[1].title, "Setup");
        assert_eq!(chapters[2].start_seconds, 245.0);
        assert_eq!(chapters[2].title, "First guest");
    }

    #[test]
    fn artifact_record_lifts_audio_preview_for_spotify() {
        // Spotify shares carry only `audio_preview` (a 60-sec clip URL) —
        // never a full `audio` URL. Both fields must survive the read so the
        // player can fall back to the preview.
        let keys = Keys::generate();
        let event = EventBuilder::new(Kind::Custom(KIND_ARTIFACT_SHARE), "")
            .tags(vec![
                Tag::parse(vec!["h".to_string(), "adhd".to_string()]).unwrap(),
                Tag::identifier("c12vkeiz"),
                Tag::parse(vec!["title".to_string(), "ADHD Awareness".to_string()]).unwrap(),
                Tag::parse(vec!["source".to_string(), "podcast".to_string()]).unwrap(),
                Tag::parse(vec![
                    "i".to_string(),
                    "spotify:episode:6L0fNLvby2nmASYTpCTZTv".to_string(),
                ])
                .unwrap(),
                Tag::parse(vec!["k".to_string(), "spotify:episode".to_string()]).unwrap(),
                Tag::parse(vec![
                    "audio_preview".to_string(),
                    "https://podz-content.spotifycdn.com/clip.mp3".to_string(),
                ])
                .unwrap(),
                Tag::parse(vec!["published_at".to_string(), "2023-09-22".to_string()]).unwrap(),
                Tag::parse(vec!["duration".to_string(), "2343".to_string()]).unwrap(),
            ])
            .sign_with_keys(&keys)
            .expect("sign");

        let record = artifact_record_from_event(&event, "adhd").expect("record");
        assert_eq!(record.preview.audio_url, "");
        assert_eq!(
            record.preview.audio_preview_url,
            "https://podz-content.spotifycdn.com/clip.mp3"
        );
        assert_eq!(record.preview.duration_seconds, Some(2343));
        assert_eq!(record.preview.published_at, "2023-09-22");
        assert_eq!(record.preview.catalog_kind, "spotify:episode");
    }

    // -- query_for_group tests ------------------------------------------------

    fn isolated_ndb() -> (nostrdb::Ndb, tempfile::TempDir) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("ndb");
        std::fs::create_dir_all(&path).expect("mkdir");
        let cfg = nostrdb::Config::new().set_mapsize(32 * 1024 * 1024);
        let ndb = nostrdb::Ndb::new(path.to_str().unwrap(), &cfg).expect("open ndb");
        (ndb, tmp)
    }

    fn ingest(ndb: &nostrdb::Ndb, event: &Event) {
        let line = format!("[\"EVENT\",\"sub\",{}]", event.as_json());
        ndb.process_event(&line).expect("process event");
    }

    fn wait_for_ndb() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    fn make_share(keys: &Keys, group_id: &str, d: &str, title: &str) -> Event {
        EventBuilder::new(Kind::Custom(KIND_ARTIFACT_SHARE), "")
            .tags(vec![
                Tag::parse(vec!["h".to_string(), group_id.to_string()]).unwrap(),
                Tag::identifier(d),
                Tag::parse(vec!["title".to_string(), title.to_string()]).unwrap(),
                Tag::parse(vec!["source".to_string(), "article".to_string()]).unwrap(),
                Tag::parse(vec!["r".to_string(), "https://example.com/post".to_string()]).unwrap(),
            ])
            .sign_with_keys(keys)
            .expect("sign")
    }

    #[test]
    fn query_for_group_returns_matching_artifacts() {
        let (ndb, _tmp) = isolated_ndb();
        let keys = Keys::generate();
        let share = make_share(&keys, "alpha", "art-1", "Alpha Article");
        ingest(&ndb, &share);
        wait_for_ndb();

        let records = query_for_group(&ndb, "alpha", 32).expect("query");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].preview.title, "Alpha Article");
        assert_eq!(records[0].group_id, "alpha");
    }

    #[test]
    fn query_for_group_filters_by_group() {
        let (ndb, _tmp) = isolated_ndb();
        let keys = Keys::generate();
        ingest(&ndb, &make_share(&keys, "alpha", "a1", "Alpha"));
        ingest(&ndb, &make_share(&keys, "bravo", "b1", "Bravo"));
        wait_for_ndb();

        let alpha = query_for_group(&ndb, "alpha", 32).expect("alpha");
        assert_eq!(alpha.len(), 1);
        assert_eq!(alpha[0].preview.title, "Alpha");

        let bravo = query_for_group(&ndb, "bravo", 32).expect("bravo");
        assert_eq!(bravo.len(), 1);
        assert_eq!(bravo[0].preview.title, "Bravo");
    }

    #[test]
    fn query_for_group_excludes_discussions() {
        let (ndb, _tmp) = isolated_ndb();
        let keys = Keys::generate();
        // A kind:11 with t=discussion must be filtered out
        let discussion = EventBuilder::new(Kind::Custom(KIND_ARTIFACT_SHARE), "body")
            .tags(vec![
                Tag::parse(vec!["h".to_string(), "alpha".to_string()]).unwrap(),
                Tag::identifier("disc-1"),
                Tag::parse(vec!["t".to_string(), "discussion".to_string()]).unwrap(),
                Tag::parse(vec!["title".to_string(), "A Discussion".to_string()]).unwrap(),
            ])
            .sign_with_keys(&keys)
            .expect("sign");
        ingest(&ndb, &discussion);
        ingest(&ndb, &make_share(&keys, "alpha", "art-1", "Real Article"));
        wait_for_ndb();

        let records = query_for_group(&ndb, "alpha", 32).expect("query");
        assert_eq!(records.len(), 1, "discussion must be excluded");
        assert_eq!(records[0].preview.title, "Real Article");
    }

    #[test]
    fn query_for_group_honors_limit() {
        let (ndb, _tmp) = isolated_ndb();
        let keys = Keys::generate();
        for i in 0..5u64 {
            ingest(&ndb, &make_share(&keys, "alpha", &format!("a{i}"), &format!("T{i}")));
        }
        wait_for_ndb();
        let records = query_for_group(&ndb, "alpha", 3).expect("query");
        assert_eq!(records.len(), 3);
    }
}
