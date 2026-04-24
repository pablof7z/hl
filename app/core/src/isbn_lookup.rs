//! Open Library ISBN → ArtifactPreview. Free, public-domain, no API key.
//!
//! Mirrors the flow described in `docs/plan.md`:
//! 1. Normalize + validate the ISBN (10 or 13 digit, converted to ISBN-13).
//! 2. `GET https://openlibrary.org/isbn/{isbn}.json` with a 5s timeout.
//! 3. Parse title + authors + cover; resolve author refs best-effort.
//! 4. On any network / parse failure, fall through to a partial preview so
//!    the user can fill the rest in manually.

use std::time::Duration;

use serde_json::Value;

use crate::errors::CoreError;
use crate::models::ArtifactPreview;

const OPEN_LIBRARY_TIMEOUT: Duration = Duration::from_secs(5);

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

/// Strip dashes/whitespace, require either 10 or 13 digits, canonicalize to
/// 13-digit. Anything else → `CoreError::InvalidInput`.
fn normalize_isbn(raw: &str) -> Result<String, CoreError> {
    let digits: String = raw
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect();

    if digits.chars().all(|c| c.is_ascii_digit()) && digits.len() == 13 {
        return Ok(digits);
    }

    // ISBN-10 may end in 'X' (check digit). We accept it as a single trailing
    // 'X' after 9 digits and then convert to ISBN-13.
    if digits.len() == 10
        && digits[..9].chars().all(|c| c.is_ascii_digit())
        && digits
            .chars()
            .nth(9)
            .map(|c| c.is_ascii_digit() || c == 'X' || c == 'x')
            .unwrap_or(false)
    {
        return Ok(isbn10_to_13(&digits));
    }

    Err(CoreError::InvalidInput(format!(
        "ISBN must be 10 or 13 digits, got {:?}",
        raw
    )))
}

/// Convert a 10-digit ISBN to 13-digit by prepending "978" and recomputing
/// the final check digit per the standard rule. We don't validate the
/// incoming check digit — Open Library will reject malformed inputs.
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
    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("parse json: {e}"))?;

    let title = body
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let description = extract_description(body.get("description"));

    let image = body
        .get("covers")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .and_then(Value::as_i64)
        .map(|id| format!("https://covers.openlibrary.org/b/id/{id}-L.jpg"))
        .unwrap_or_default();

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
        // ISBN-10 0-7352-1129-X (fictional check) → 978-0-7352-1129-? with
        // recomputed check digit.
        let n = normalize_isbn("0735211299").unwrap();
        assert!(n.starts_with("9780735211"));
        assert_eq!(n.len(), 13);
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
}
