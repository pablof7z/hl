//! Open Library ISBN → ArtifactPreview. Free, public-domain, no API key.
//!
//! Mirrors the flow described in `docs/plan.md`:
//! 1. Normalize + validate the ISBN (10 or 13 digit, converted to ISBN-13).
//! 2. `GET https://openlibrary.org/isbn/{isbn}.json` with a 5s timeout.
//! 3. Parse title + authors + cover; resolve author refs best-effort.
//! 4. On any network / parse failure, fall through to a partial preview so
//!    the user can fill the rest in manually.

use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Notify;

use crate::errors::CoreError;
use crate::models::ArtifactPreview;

const OPEN_LIBRARY_TIMEOUT: Duration = Duration::from_secs(5);

/// Rust-owned ISBN preview cache. Durable entries are enriched previews from
/// Open Library; partial previews from transient misses stay in memory only
/// for the current core session so a network blip cannot permanently poison
/// the user's library.
pub struct IsbnPreviewStore {
    path: PathBuf,
    state: Mutex<IsbnCacheState>,
    inflight: Mutex<HashMap<String, Arc<Notify>>>,
}

#[derive(Default)]
struct IsbnCacheState {
    entries: HashMap<String, IsbnCacheEntry>,
}

#[derive(Clone)]
struct IsbnCacheEntry {
    preview: ArtifactPreview,
    durable: bool,
}

#[derive(Default, Serialize, Deserialize)]
struct IsbnCacheFile {
    entries: HashMap<String, ArtifactPreview>,
}

impl IsbnPreviewStore {
    /// Open the ISBN preview store rooted at the core data directory. Missing
    /// or unreadable cache files are treated as empty.
    pub fn open(data_dir: &std::path::Path) -> Self {
        let path = data_dir.join("isbn_previews.json");
        let file = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<IsbnCacheFile>(&bytes).unwrap_or_default(),
            Err(_) => IsbnCacheFile::default(),
        };
        let entries = file
            .entries
            .into_iter()
            .map(|(isbn, preview)| {
                (
                    isbn,
                    IsbnCacheEntry {
                        preview,
                        durable: true,
                    },
                )
            })
            .collect();
        Self {
            path,
            state: Mutex::new(IsbnCacheState { entries }),
            inflight: Mutex::new(HashMap::new()),
        }
    }

    fn get(&self, isbn13: &str) -> Option<ArtifactPreview> {
        let guard = self.state.lock();
        guard.entries.get(isbn13).map(|entry| entry.preview.clone())
    }

    fn put(&self, isbn13: String, preview: ArtifactPreview) {
        let durable = is_durable_preview(&preview);
        let entry = IsbnCacheEntry { preview, durable };
        let snapshot = {
            let mut guard = self.state.lock();
            guard.entries.insert(isbn13, entry);
            if durable {
                let entries = guard
                    .entries
                    .iter()
                    .filter(|(_, entry)| entry.durable)
                    .map(|(isbn, entry)| (isbn.clone(), entry.preview.clone()))
                    .collect();
                serde_json::to_vec(&IsbnCacheFile { entries }).ok()
            } else {
                None
            }
        };
        if let Some(bytes) = snapshot {
            if let Err(e) = std::fs::write(&self.path, &bytes) {
                tracing::warn!(error = %e, path = ?self.path, "persist ISBN preview cache");
            }
        }
    }

    fn acquire(self: &Arc<Self>, isbn13: &str) -> IsbnInflightSlot {
        let mut guard = self.inflight.lock();
        if let Some(notify) = guard.get(isbn13) {
            return IsbnInflightSlot::Follower(IsbnInflightFollower {
                notify: notify.clone(),
            });
        }
        let notify = Arc::new(Notify::new());
        guard.insert(isbn13.to_string(), notify.clone());
        IsbnInflightSlot::Lead(IsbnInflightLead {
            store: Arc::clone(self),
            isbn13: isbn13.to_string(),
            notify,
        })
    }
}

enum IsbnInflightSlot {
    Lead(IsbnInflightLead),
    Follower(IsbnInflightFollower),
}

struct IsbnInflightLead {
    store: Arc<IsbnPreviewStore>,
    isbn13: String,
    notify: Arc<Notify>,
}

impl IsbnInflightLead {
    fn done(self) {
        {
            let mut guard = self.store.inflight.lock();
            guard.remove(&self.isbn13);
        }
        self.notify.notify_waiters();
    }
}

struct IsbnInflightFollower {
    notify: Arc<Notify>,
}

impl IsbnInflightFollower {
    async fn wait(self) {
        self.notify.notified().await;
    }
}

/// Cached entry point used by `HighlighterCore`. Native platforms call the
/// same FFI method and therefore share normalization, fetch, cache, and
/// fallback semantics.
pub async fn lookup_isbn_cached(
    store: Arc<IsbnPreviewStore>,
    isbn: &str,
) -> Result<ArtifactPreview, CoreError> {
    let isbn13 = normalize_isbn(isbn)?;
    if let Some(hit) = store.get(&isbn13) {
        return Ok(hit);
    }

    match store.acquire(&isbn13) {
        IsbnInflightSlot::Lead(lead) => {
            let result = lookup_isbn(&isbn13).await;
            if let Ok(preview) = &result {
                store.put(isbn13, preview.clone());
            }
            lead.done();
            result
        }
        IsbnInflightSlot::Follower(follower) => {
            follower.wait().await;
            store
                .get(&isbn13)
                .ok_or_else(|| CoreError::Other("ISBN lookup completed without preview".into()))
        }
    }
}

/// Normalize, validate, and look up an ISBN via Open Library. On any failure,
/// returns a partial `ArtifactPreview` with only `catalog_id=isbn:{digits}`,
/// `catalog_kind="isbn"`, and `source="book"` set so the caller can fall
/// through to manual entry.
pub async fn lookup_isbn(isbn: &str) -> Result<ArtifactPreview, CoreError> {
    let isbn13 = normalize_isbn(isbn)?;

    // Build the preview on a successful fetch; fall back to the minimal one
    // on any failure (404, timeout, bad JSON, etc.).
    match fetch_open_library(&isbn13).await {
        Ok(preview) => Ok(preview),
        Err(e) => {
            tracing::warn!(isbn = %isbn13, error = %e, "open library lookup failed, returning partial");
            Ok(partial_preview(&isbn13))
        }
    }
}

/// Strip dashes/whitespace, require either a valid 10-digit ISBN or a valid
/// Bookland ISBN-13, and canonicalize to 13 digits.
pub fn normalize_isbn(raw: &str) -> Result<String, CoreError> {
    let digits: String = raw
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect();

    if digits.chars().all(|c| c.is_ascii_digit())
        && digits.len() == 13
        && (digits.starts_with("978") || digits.starts_with("979"))
        && has_valid_isbn13_checksum(&digits)
    {
        return Ok(digits);
    }

    // ISBN-10 may end in 'X' (check digit). It must pass the ISBN-10 checksum
    // before conversion so every platform rejects the same false positives.
    if digits.len() == 10
        && digits[..9].chars().all(|c| c.is_ascii_digit())
        && digits
            .chars()
            .nth(9)
            .map(|c| c.is_ascii_digit() || c == 'X' || c == 'x')
            .unwrap_or(false)
        && has_valid_isbn10_checksum(&digits)
    {
        return Ok(isbn10_to_13(&digits));
    }

    Err(CoreError::InvalidInput(format!(
        "ISBN must be a valid 10-digit ISBN or Bookland ISBN-13, got {:?}",
        raw
    )))
}

fn has_valid_isbn13_checksum(digits: &str) -> bool {
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

fn has_valid_isbn10_checksum(digits: &str) -> bool {
    if digits.len() != 10 {
        return false;
    }
    let mut sum = 0u32;
    for (i, c) in digits.chars().enumerate() {
        let value = if c == 'X' || c == 'x' {
            if i != 9 {
                return false;
            }
            10
        } else if let Some(d) = c.to_digit(10) {
            d
        } else {
            return false;
        };
        sum += value * (10 - i as u32);
    }
    sum.is_multiple_of(11)
}

/// Convert a valid 10-digit ISBN to 13 digits by prepending "978" and
/// recomputing the final ISBN-13 check digit.
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

fn is_durable_preview(preview: &ArtifactPreview) -> bool {
    [
        preview.title.as_str(),
        preview.author.as_str(),
        preview.image.as_str(),
        preview.description.as_str(),
    ]
    .iter()
    .any(|value| !value.trim().is_empty())
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
        // ISBN-10 0-7352-1129-9 -> ISBN-13 with a recomputed check digit.
        let n = normalize_isbn("0735211299").unwrap();
        assert_eq!(n, "9780735211292");
    }

    #[test]
    fn normalize_isbn_rejects_non_bookland_ean13() {
        assert!(normalize_isbn("1234567890128").is_err());
    }

    #[test]
    fn normalize_isbn_rejects_bad_checksums() {
        assert!(normalize_isbn("9780735211293").is_err());
        assert!(normalize_isbn("073521129X").is_err());
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
    fn isbn_preview_store_persists_enriched_previews() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = IsbnPreviewStore::open(tmp.path());
        let preview = build_preview(
            "9780735211292",
            "The Pragmatic Programmer".into(),
            "Andrew Hunt, David Thomas".into(),
            "https://covers.openlibrary.org/b/isbn/9780735211292-L.jpg".into(),
            "A practical software book.".into(),
        );

        store.put("9780735211292".into(), preview.clone());
        let reopened = IsbnPreviewStore::open(tmp.path());
        let cached = reopened
            .get("9780735211292")
            .expect("enriched preview should persist");

        assert_eq!(cached.catalog_id, preview.catalog_id);
        assert_eq!(cached.title, preview.title);
        assert_eq!(cached.author, preview.author);
        assert_eq!(
            cached.highlight_reference_key,
            preview.highlight_reference_key
        );
    }

    #[test]
    fn isbn_preview_store_keeps_partial_previews_session_only() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = IsbnPreviewStore::open(tmp.path());
        let preview = partial_preview("9999999999994");

        store.put("9999999999994".into(), preview);
        assert!(
            store.get("9999999999994").is_some(),
            "partial preview should satisfy same-session callers"
        );

        let reopened = IsbnPreviewStore::open(tmp.path());
        assert!(
            reopened.get("9999999999994").is_none(),
            "partial preview must not become durable cache state"
        );
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
        let preview = lookup_isbn("9780000000002")
            .await
            .expect("partial preview on unknown ISBN, not error");
        assert_eq!(preview.source, "book");
        assert_eq!(preview.catalog_id, "isbn:9780000000002");
        assert_eq!(preview.catalog_kind, "isbn");
        assert_eq!(preview.reference_tag_name, "i");
        assert_eq!(preview.reference_tag_value, "isbn:9780000000002");
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
}
