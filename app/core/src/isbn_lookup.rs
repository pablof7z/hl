//! Open Library ISBN → ArtifactPreview. Free, public-domain, no API key.
//!
//! Mirrors the flow described in `docs/plan.md`:
//! 1. Normalize + validate the ISBN (10 or 13 digit, converted to ISBN-13).
//! 2. `GET https://openlibrary.org/isbn/{isbn}.json` with a 5s timeout.
//! 3. Parse title + authors + cover; resolve author refs best-effort.
//! 4. On any network / parse failure, fall through to a partial preview so
//!    the user can fill the rest in manually.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::errors::CoreError;
use crate::models::{ArtifactPreview, ArtifactRecord};

const OPEN_LIBRARY_TIMEOUT: Duration = Duration::from_secs(5);
const CACHE_FILE_NAME: &str = "isbn-preview-cache-v1.json";

#[derive(Debug, Clone, uniffi::Record)]
pub struct BookPickerQueryProjectionInput {
    pub query: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BookPickerQueryProjection {
    pub search_query: String,
    pub has_query: bool,
    pub normalized_isbn: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct IsbnManualPreviewProjectionInput {
    pub title: String,
    pub author: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct IsbnManualPreviewProjection {
    pub title: String,
    pub author: String,
    pub can_use: bool,
}

/// Rust-owned persistent ISBN preview cache.
///
/// Native callers should not mirror this in `UserDefaults`; they ask Rust for
/// a preview and render the returned state. The cache is intentionally local
/// to the app data directory beside nostrdb so it follows the rest of the
/// mobile core's storage lifecycle.
pub struct IsbnPreviewCache {
    path: PathBuf,
    entries: Mutex<Option<HashMap<String, CachedISBNPreview>>>,
}

impl IsbnPreviewCache {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join(CACHE_FILE_NAME),
            entries: Mutex::new(None),
        }
    }

    pub async fn lookup(&self, isbn: &str) -> Result<ArtifactPreview, CoreError> {
        let isbn13 = normalize_isbn(isbn)?;

        let mut guard = self.entries.lock().await;
        if guard.is_none() {
            *guard = Some(load_cache(&self.path).await);
        }

        let entries = guard.as_mut().expect("cache initialized above");
        if let Some(hit) = entries.get(&isbn13) {
            return Ok(hit.to_preview(&isbn13));
        }

        let preview = lookup_isbn_normalized(&isbn13).await?;
        entries.insert(isbn13.clone(), CachedISBNPreview::from_preview(&preview));
        persist_cache(&self.path, entries).await?;
        Ok(preview)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedISBNPreview {
    id: String,
    url: String,
    title: String,
    author: String,
    image: String,
    description: String,
    domain: String,
    published_at: String,
}

impl CachedISBNPreview {
    fn from_preview(preview: &ArtifactPreview) -> Self {
        Self {
            id: preview.id.clone(),
            url: preview.url.clone(),
            title: preview.title.clone(),
            author: preview.author.clone(),
            image: preview.image.clone(),
            description: preview.description.clone(),
            domain: preview.domain.clone(),
            published_at: preview.published_at.clone(),
        }
    }

    fn to_preview(&self, isbn13: &str) -> ArtifactPreview {
        let catalog_id = format!("isbn:{isbn13}");
        ArtifactPreview {
            id: self.id.clone(),
            url: self.url.clone(),
            title: self.title.clone(),
            author: self.author.clone(),
            image: self.image.clone(),
            description: self.description.clone(),
            source: "book".into(),
            domain: self.domain.clone(),
            catalog_id: catalog_id.clone(),
            catalog_kind: "isbn".into(),
            podcast_guid: String::new(),
            podcast_item_guid: String::new(),
            podcast_show_title: String::new(),
            audio_url: String::new(),
            audio_preview_url: String::new(),
            transcript_url: String::new(),
            feed_url: String::new(),
            published_at: self.published_at.clone(),
            duration_seconds: None,
            reference_tag_name: "i".into(),
            reference_tag_value: catalog_id.clone(),
            reference_kind: "isbn".into(),
            highlight_tag_name: "i".into(),
            highlight_tag_value: catalog_id.clone(),
            highlight_reference_key: format!("i:{catalog_id}"),
            chapters: Vec::new(),
        }
    }
}

/// Normalize, validate, and look up an ISBN via Open Library. On any failure,
/// returns a partial `ArtifactPreview` with only `catalog_id=isbn:{digits}`,
/// `catalog_kind="isbn"`, and `source="book"` set so the caller can fall
/// through to manual entry.
pub async fn lookup_isbn(isbn: &str) -> Result<ArtifactPreview, CoreError> {
    let isbn13 = normalize_isbn(isbn)?;
    lookup_isbn_normalized(&isbn13).await
}

pub fn edited_book_preview(
    isbn: &str,
    base: Option<ArtifactPreview>,
    title: &str,
    author: &str,
) -> Result<ArtifactPreview, CoreError> {
    let isbn13 = normalize_isbn(isbn)?;
    let mut preview = partial_preview(&isbn13);
    if let Some(base) = base {
        if !base.id.trim().is_empty() {
            preview.id = base.id;
        }
        preview.url = base.url;
        preview.image = base.image;
        preview.description = base.description;
        preview.domain = base.domain;
        preview.published_at = base.published_at;
    }
    preview.title = title.trim().to_string();
    preview.author = author.trim().to_string();
    Ok(preview)
}

/// Project the book-picker search field. Rust owns query trimming and ISBN
/// normalization; native shells render and debounce the returned query.
pub fn book_picker_query_projection(
    input: BookPickerQueryProjectionInput,
) -> BookPickerQueryProjection {
    let search_query = input.query.trim().to_string();
    BookPickerQueryProjection {
        has_query: !search_query.is_empty(),
        normalized_isbn: normalize_isbn(&input.query).ok(),
        search_query,
    }
}

/// Project the manual ISBN preview form. Rust owns title/author normalization
/// and whether the "Use" action can proceed.
pub fn manual_preview_projection(
    input: IsbnManualPreviewProjectionInput,
) -> IsbnManualPreviewProjection {
    let title = input.title.trim().to_string();
    IsbnManualPreviewProjection {
        can_use: !title.is_empty(),
        title,
        author: input.author.trim().to_string(),
    }
}

pub fn existing_record_for_isbn(isbn: &str, records: &[ArtifactRecord]) -> Option<ArtifactRecord> {
    let isbn13 = normalize_isbn_reference_input(isbn)?;
    records
        .iter()
        .find(|record| preview_matches_isbn(&record.preview, &isbn13))
        .cloned()
}

async fn lookup_isbn_normalized(isbn13: &str) -> Result<ArtifactPreview, CoreError> {
    // Build the preview on a successful fetch; fall back to the minimal one
    // on any failure (404, timeout, bad JSON, etc.).
    match fetch_open_library(isbn13).await {
        Ok(preview) => Ok(preview),
        Err(e) => {
            tracing::warn!(isbn = %isbn13, error = %e, "open library lookup failed, returning partial");
            Ok(partial_preview(isbn13))
        }
    }
}

async fn load_cache(path: &Path) -> HashMap<String, CachedISBNPreview> {
    match tokio::fs::read(path).await {
        Ok(bytes) => match serde_json::from_slice::<HashMap<String, CachedISBNPreview>>(&bytes) {
            Ok(cache) => cache,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to parse ISBN cache");
                HashMap::new()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "failed to read ISBN cache");
            HashMap::new()
        }
    }
}

async fn persist_cache(
    path: &Path,
    entries: &HashMap<String, CachedISBNPreview>,
) -> Result<(), CoreError> {
    let bytes = serde_json::to_vec(entries)
        .map_err(|e| CoreError::Cache(format!("encode ISBN cache: {e}")))?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| CoreError::Cache(format!("create ISBN cache dir: {e}")))?;
    }
    let tmp = path.with_extension("json.tmp");
    tokio::fs::write(&tmp, bytes)
        .await
        .map_err(|e| CoreError::Cache(format!("write ISBN cache: {e}")))?;
    tokio::fs::rename(&tmp, path)
        .await
        .map_err(|e| CoreError::Cache(format!("commit ISBN cache: {e}")))?;
    Ok(())
}

/// Strip dashes/whitespace, require either a valid Bookland ISBN-13 or a valid
/// ISBN-10, and canonicalize to 13 digits.
pub(crate) fn normalize_isbn(raw: &str) -> Result<String, CoreError> {
    let digits: String = raw
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect();

    if is_valid_bookland_isbn13(&digits) {
        return Ok(digits);
    }

    if is_valid_isbn10(&digits) {
        return Ok(isbn10_to_13(&digits));
    }

    Err(CoreError::InvalidInput(format!(
        "ISBN must be a valid Bookland ISBN-13 or ISBN-10, got {:?}",
        raw
    )))
}

fn is_valid_bookland_isbn13(digits: &str) -> bool {
    digits.len() == 13
        && digits.chars().all(|c| c.is_ascii_digit())
        && (digits.starts_with("978") || digits.starts_with("979"))
        && is_valid_isbn13_checksum(digits)
}

fn is_valid_isbn13_checksum(digits: &str) -> bool {
    if digits.len() != 13 {
        return false;
    }

    let mut sum = 0u32;
    for (i, c) in digits.chars().enumerate() {
        let Some(d) = c.to_digit(10) else {
            return false;
        };
        sum += if i % 2 == 0 { d } else { d * 3 };
    }
    sum.is_multiple_of(10)
}

fn is_valid_isbn10(digits: &str) -> bool {
    if digits.len() != 10 {
        return false;
    }

    let mut sum = 0u32;
    for (i, c) in digits.chars().enumerate() {
        let value = match c {
            'X' | 'x' if i == 9 => 10,
            _ => match c.to_digit(10) {
                Some(d) => d,
                None => return false,
            },
        };
        sum += value * (10 - i as u32);
    }
    sum.is_multiple_of(11)
}

/// Convert a 10-digit ISBN to 13-digit by prepending "978" and recomputing
/// the final check digit per the standard rule.
fn isbn10_to_13(isbn10: &str) -> String {
    let prefix = format!("978{}", &isbn10[..9]);
    let check = compute_isbn13_check_digit(&prefix);
    format!("{prefix}{check}")
}

fn compute_isbn13_check_digit(prefix12: &str) -> char {
    let mut sum = 0u32;
    for (i, c) in prefix12.chars().enumerate() {
        let d = c.to_digit(10).unwrap_or(0);
        sum += if i % 2 == 0 { d } else { d * 3 };
    }
    let check = (10 - (sum % 10)) % 10;
    char::from_digit(check, 10).unwrap_or('0')
}

fn normalize_isbn_reference_input(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let value = strip_ascii_prefix_ci(trimmed, "isbn:").unwrap_or(trimmed);
    normalize_isbn(value).ok()
}

fn preview_matches_isbn(preview: &ArtifactPreview, isbn13: &str) -> bool {
    isbn_reference_matches(&preview.catalog_id, isbn13)
        || isbn_reference_matches(&preview.reference_tag_value, isbn13)
        || isbn_reference_matches(&preview.highlight_tag_value, isbn13)
        || highlight_reference_key_matches(&preview.highlight_reference_key, isbn13)
}

fn isbn_reference_matches(value: &str, isbn13: &str) -> bool {
    normalize_isbn_reference_input(value).as_deref() == Some(isbn13)
}

fn highlight_reference_key_matches(value: &str, isbn13: &str) -> bool {
    strip_ascii_prefix_ci(value.trim(), "i:")
        .is_some_and(|reference| isbn_reference_matches(reference, isbn13))
}

fn strip_ascii_prefix_ci<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    value
        .get(..prefix.len())
        .filter(|head| head.eq_ignore_ascii_case(prefix))
        .map(|_| &value[prefix.len()..])
}

async fn fetch_open_library(isbn13: &str) -> Result<ArtifactPreview, String> {
    let client = reqwest::Client::builder()
        .timeout(OPEN_LIBRARY_TIMEOUT)
        .build()
        .map_err(|e| format!("build http client: {e}"))?;

    let url = format!("https://openlibrary.org/isbn/{isbn13}.json");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("GET {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let body: Value = resp.json().await.map_err(|e| format!("parse json: {e}"))?;

    let title = body
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let description = extract_description(body.get("description"));

    // Prefer the cover ID from the book JSON (higher quality, redirects avoided).
    // Fall back to the ISBN-based cover URL which Open Library always serves
    // for known ISBNs even when the book record omits the `covers` array.
    let image = body
        .get("covers")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .and_then(Value::as_i64)
        .map(|id| format!("https://covers.openlibrary.org/b/id/{id}-L.jpg"))
        .unwrap_or_else(|| format!("https://covers.openlibrary.org/b/isbn/{isbn13}-L.jpg"));

    // Authors: best-effort. Resolve each `/authors/OLxxxA` ref to a name. If
    // the resolution fails (timeout/error), fall back to an empty name for
    // that author — the user can edit post-scan.
    let author_refs: Vec<String> = body
        .get("authors")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.get("key").and_then(Value::as_str).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let mut author_names: Vec<String> = Vec::with_capacity(author_refs.len());
    for key in &author_refs {
        match fetch_author_name(&client, key).await {
            Ok(name) if !name.is_empty() => author_names.push(name),
            Ok(_) => {}
            Err(e) => {
                tracing::debug!(author_key = %key, error = %e, "author lookup failed");
            }
        }
    }
    let author = author_names.join(", ");

    Ok(build_preview(isbn13, title, author, image, description))
}

async fn fetch_author_name(client: &reqwest::Client, key: &str) -> Result<String, String> {
    // `key` looks like "/authors/OL1234567A"; the JSON endpoint is
    // "https://openlibrary.org/authors/OL1234567A.json".
    let trimmed = key.trim_start_matches('/');
    let url = format!("https://openlibrary.org/{trimmed}.json");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("GET {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("parse author json: {e}"))?;
    Ok(body
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string())
}

/// Open Library returns `description` either as a bare string or as
/// `{ "type": "/type/text", "value": "…" }`. Handle both.
fn extract_description(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Object(obj)) => obj
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    }
}

/// The fully-populated preview we return on a successful Open Library hit.
fn build_preview(
    isbn13: &str,
    title: String,
    author: String,
    image: String,
    description: String,
) -> ArtifactPreview {
    let catalog_id = format!("isbn:{isbn13}");
    let highlight_reference_key = format!("i:{catalog_id}");
    let id = format!("c{:x}", fnv1a(&format!("i:{catalog_id}")));

    ArtifactPreview {
        id,
        url: String::new(),
        title,
        author,
        image,
        description,
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
        // Highlights on ISBN-sourced books reference the canonical NIP-73
        // `i` tag — there is no URL for a physical book, so the primary
        // anchor is the ISBN itself. This lets any Nostr client identify
        // the source without relying on Highlighter's kind:11 share.
        highlight_tag_name: "i".into(),
        highlight_tag_value: catalog_id,
        highlight_reference_key,
        chapters: Vec::new(),
    }
}

/// Fallback preview used on network/parse failure or when the API returns 404.
/// Only enough is filled in for the caller to publish a kind:11 after manual
/// title/author entry: the catalog id (so we can dedupe against existing
/// shares) and the reference tags.
fn partial_preview(isbn13: &str) -> ArtifactPreview {
    let catalog_id = format!("isbn:{isbn13}");
    let highlight_reference_key = format!("i:{catalog_id}");
    let id = format!("c{:x}", fnv1a(&format!("i:{catalog_id}")));
    ArtifactPreview {
        id,
        url: String::new(),
        title: String::new(),
        author: String::new(),
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
        // Same reasoning as the fully-populated preview — the ISBN itself
        // is the canonical NIP-73 anchor, regardless of catalog resolution.
        highlight_tag_name: "i".into(),
        highlight_tag_value: catalog_id,
        highlight_reference_key,
        chapters: Vec::new(),
    }
}

/// FNV-1a 32-bit hash. Ported from `fnv1a` in
/// `web/src/lib/ndk/artifacts.ts:1086` so Swift/Rust/TS compute the same
/// artifact id for the same reference key.
fn fnv1a(value: &str) -> u32 {
    let mut hash: u32 = 0x811c9dc5;
    for b in value.bytes() {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_isbn_accepts_13_digit() {
        let n = normalize_isbn("978-0-7352-1129-2").unwrap();
        assert_eq!(n, "9780735211292");
    }

    #[test]
    fn normalize_isbn_converts_10_to_13() {
        let n = normalize_isbn("0735211299").unwrap();
        assert!(n.starts_with("9780735211"));
        assert_eq!(n.len(), 13);
    }

    #[test]
    fn normalize_isbn_accepts_trailing_x_10_digit() {
        let n = normalize_isbn("0-8044-2957-X").unwrap();
        assert_eq!(n, "9780804429573");
    }

    #[test]
    fn normalize_isbn_rejects_garbage() {
        assert!(normalize_isbn("").is_err());
        assert!(normalize_isbn("hello").is_err());
        assert!(normalize_isbn("123").is_err());
        assert!(normalize_isbn("12345678901234567890").is_err());
        assert!(normalize_isbn("abc-def-ghi-jk").is_err());
    }

    #[test]
    fn normalize_isbn_rejects_non_bookland_and_bad_checksums() {
        assert!(normalize_isbn("4006381333931").is_err());
        assert!(normalize_isbn("9780735211290").is_err());
        assert!(normalize_isbn("0735211298").is_err());
    }

    #[test]
    fn fnv1a_matches_webapp_constant() {
        // From `web/src/lib/ndk/artifacts.ts`: empty string → initial state.
        assert_eq!(fnv1a(""), 0x811c9dc5);
        // Known-good vector.
        assert_eq!(fnv1a("a"), 0xe40c292c);
    }

    #[test]
    fn partial_preview_has_only_isbn_fields() {
        let p = partial_preview("9780735211292");
        assert_eq!(p.source, "book");
        assert_eq!(p.catalog_id, "isbn:9780735211292");
        assert_eq!(p.catalog_kind, "isbn");
        assert_eq!(p.reference_tag_name, "i");
        assert_eq!(p.reference_tag_value, "isbn:9780735211292");
        assert!(p.title.is_empty());
        assert!(p.author.is_empty());
        assert!(p.image.is_empty());
        assert!(p.description.is_empty());
        // The NIP-73 `i` tag is what anchors a highlight to the book, so
        // partial previews still carry it — that's the whole point of the
        // fallback path.
        assert_eq!(p.highlight_tag_name, "i");
        assert_eq!(p.highlight_tag_value, "isbn:9780735211292");
        assert!(!p.highlight_reference_key.is_empty());
    }

    #[test]
    fn edited_book_preview_preserves_lookup_media_and_canonical_references() {
        let mut base = partial_preview("9780735211292");
        base.id = "lookup-id".into();
        base.url = "https://example.com/book".into();
        base.image = "https://example.com/cover.jpg".into();
        base.description = "Lookup description".into();
        base.domain = "example.com".into();
        base.published_at = "2026".into();
        base.catalog_id = "wrong".into();
        base.reference_tag_value = "wrong".into();
        base.highlight_reference_key = "wrong".into();

        let edited = edited_book_preview(
            "978-0-7352-1129-2",
            Some(base),
            "  Manual Title  ",
            " Author ",
        )
        .unwrap();
        assert_eq!(edited.id, "lookup-id");
        assert_eq!(edited.title, "Manual Title");
        assert_eq!(edited.author, "Author");
        assert_eq!(edited.image, "https://example.com/cover.jpg");
        assert_eq!(edited.description, "Lookup description");
        assert_eq!(edited.domain, "example.com");
        assert_eq!(edited.published_at, "2026");
        assert_eq!(edited.source, "book");
        assert_eq!(edited.catalog_id, "isbn:9780735211292");
        assert_eq!(edited.catalog_kind, "isbn");
        assert_eq!(edited.reference_tag_name, "i");
        assert_eq!(edited.reference_tag_value, "isbn:9780735211292");
        assert_eq!(edited.reference_kind, "isbn");
        assert_eq!(edited.highlight_tag_name, "i");
        assert_eq!(edited.highlight_tag_value, "isbn:9780735211292");
        assert_eq!(edited.highlight_reference_key, "i:isbn:9780735211292");
    }

    #[test]
    fn edited_book_preview_without_lookup_still_has_stable_id() {
        let edited = edited_book_preview("9780735211292", None, "Manual Title", "").unwrap();
        assert!(edited.id.starts_with('c'));
        assert_eq!(edited.title, "Manual Title");
        assert_eq!(edited.catalog_id, "isbn:9780735211292");
        assert_eq!(edited.highlight_reference_key, "i:isbn:9780735211292");
        assert!(edited.image.is_empty());
    }

    #[test]
    fn book_picker_query_projection_trims_and_detects_isbn() {
        let projection = book_picker_query_projection(BookPickerQueryProjectionInput {
            query: "  0735211299  ".into(),
        });
        let blank = book_picker_query_projection(BookPickerQueryProjectionInput {
            query: " \n\t ".into(),
        });
        let search = book_picker_query_projection(BookPickerQueryProjectionInput {
            query: "  clean code  ".into(),
        });

        assert_eq!(projection.search_query, "0735211299");
        assert!(projection.has_query);
        assert_eq!(projection.normalized_isbn, Some("9780735211292".into()));
        assert_eq!(blank.search_query, "");
        assert!(!blank.has_query);
        assert_eq!(blank.normalized_isbn, None);
        assert_eq!(search.search_query, "clean code");
        assert_eq!(search.normalized_isbn, None);
    }

    #[test]
    fn manual_preview_projection_trims_and_requires_title() {
        let projection = manual_preview_projection(IsbnManualPreviewProjectionInput {
            title: "  Manual Title  ".into(),
            author: "  Author Name  ".into(),
        });
        let blank = manual_preview_projection(IsbnManualPreviewProjectionInput {
            title: " \n\t ".into(),
            author: "Author".into(),
        });

        assert_eq!(projection.title, "Manual Title");
        assert_eq!(projection.author, "Author Name");
        assert!(projection.can_use);
        assert_eq!(blank.title, "");
        assert!(!blank.can_use);
    }

    #[test]
    fn existing_record_for_isbn_matches_canonical_catalog_id() {
        let records = vec![
            record_with_preview(partial_preview("9781593278281")),
            record_with_preview(partial_preview("9780735211292")),
        ];

        let found = existing_record_for_isbn("0735211299", &records).unwrap();

        assert_eq!(found.preview.catalog_id, "isbn:9780735211292");
    }

    #[test]
    fn existing_record_for_isbn_matches_reference_key_when_catalog_id_is_missing() {
        let mut preview = partial_preview("9780735211292");
        preview.catalog_id.clear();
        preview.reference_tag_value.clear();
        preview.highlight_tag_value.clear();

        let found = existing_record_for_isbn(
            "isbn:9780735211292",
            &[record_with_preview(preview.clone())],
        )
        .unwrap();

        assert_eq!(
            found.preview.highlight_reference_key,
            "i:isbn:9780735211292"
        );
    }

    #[test]
    fn existing_record_for_isbn_ignores_invalid_input_and_misses() {
        let records = vec![record_with_preview(partial_preview("9781593278281"))];

        assert!(existing_record_for_isbn("not an isbn", &records).is_none());
        assert!(existing_record_for_isbn("9780735211292", &records).is_none());
    }

    #[tokio::test]
    async fn cache_hit_returns_preview_without_network() {
        let dir = tempfile::tempdir().unwrap();
        let cached = CachedISBNPreview {
            id: "cached-id".into(),
            url: String::new(),
            title: "Cached Book".into(),
            author: "Cached Author".into(),
            image: "https://example.test/cover.jpg".into(),
            description: "Cached description".into(),
            domain: String::new(),
            published_at: "2026-01-01".into(),
        };
        let mut entries = HashMap::new();
        entries.insert("9780735211292".into(), cached);
        persist_cache(&dir.path().join(CACHE_FILE_NAME), &entries)
            .await
            .unwrap();

        let cache = IsbnPreviewCache::new(dir.path());
        let preview = cache.lookup("978-0-7352-1129-2").await.unwrap();

        assert_eq!(preview.id, "cached-id");
        assert_eq!(preview.title, "Cached Book");
        assert_eq!(preview.author, "Cached Author");
        assert_eq!(preview.catalog_id, "isbn:9780735211292");
        assert_eq!(preview.highlight_reference_key, "i:isbn:9780735211292");
    }

    /// End-to-end hit against the real Open Library API. Ignored by default
    /// — CI should not depend on a public service being up.
    #[ignore = "hits live Open Library API"]
    #[tokio::test]
    async fn lookup_isbn_returns_preview_on_known_isbn() {
        let preview = lookup_isbn("9780735211292")
            .await
            .expect("lookup must not error — even on API failure it returns partial");
        assert_eq!(preview.source, "book");
        assert_eq!(preview.catalog_id, "isbn:9780735211292");
        assert_eq!(preview.catalog_kind, "isbn");
        assert!(
            !preview.title.is_empty(),
            "expected title from Open Library, got empty preview"
        );
    }

    /// Hits the real Open Library API with an ISBN it doesn't know. Also
    /// ignored so CI stays network-free. Must return a partial preview — not
    /// an error.
    #[ignore = "hits live Open Library API"]
    #[tokio::test]
    async fn lookup_isbn_returns_partial_on_unknown_isbn() {
        let preview = lookup_isbn("9999999999994")
            .await
            .expect("partial preview on unknown ISBN, not error");
        assert_eq!(preview.source, "book");
        assert_eq!(preview.catalog_id, "isbn:9999999999994");
        assert_eq!(preview.catalog_kind, "isbn");
        assert_eq!(preview.reference_tag_name, "i");
        assert_eq!(preview.reference_tag_value, "isbn:9999999999994");
        assert!(preview.title.is_empty());
        assert!(preview.author.is_empty());
        assert!(preview.image.is_empty());
    }

    /// Runs offline: validates the format check on the way in without
    /// reaching the network.
    #[tokio::test]
    async fn lookup_isbn_rejects_invalid_format() {
        use crate::errors::CoreError;

        for bad in ["", "abc", "123", "12345678901234567890", "hello-world"] {
            match lookup_isbn(bad).await {
                Err(CoreError::InvalidInput(_)) => {}
                other => panic!("expected InvalidInput for {bad:?}, got {other:?}"),
            }
        }
    }

    fn record_with_preview(preview: ArtifactPreview) -> ArtifactRecord {
        ArtifactRecord {
            preview,
            group_id: "books".into(),
            share_event_id: "event".into(),
            pubkey: "pubkey".into(),
            created_at: Some(1),
            note: String::new(),
        }
    }
}
