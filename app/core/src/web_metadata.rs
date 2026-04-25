//! OpenGraph + favicon enrichment for web URL highlights.
//!
//! `build_preview` in `artifacts.rs` only normalizes URLs — it has never
//! fetched the page. For web highlights that means the iOS card module
//! falls back to a globe icon and the URL host. This module adds a
//! best-effort metadata fetcher (OG/Twitter/`<title>` + favicon) with a
//! small JSON-on-disk cache so each URL is fetched at most once per TTL.
//!
//! Caching:
//! - Hits live for 7 days, negative entries (404, network failure) for 1 hour
//!   so dead links don't hammer relays.
//! - The disk file is `<data_dir>/web_metadata.json`.
//! - Concurrent requests for the same URL are coalesced via an in-flight
//!   `Notify` map; the first caller fetches, the rest re-read the cache.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
use url::Url;

use crate::errors::CoreError;

/// Project-default user agent. Matches the rest of the iOS surface so
/// publishers can distinguish Highlighter traffic in their access logs.
const USER_AGENT: &str = "Highlighter/0.1 (+https://highlighter.com)";
/// Hard ceiling on response body size. Avoids parsing absurd pages that
/// somehow report `text/html` for a 50 MiB binary blob.
const MAX_BODY_BYTES: usize = 1 * 1024 * 1024;
/// Per-request timeout. iOS shows the fallback header instantly; the OG
/// data slots in once the fetch lands. Five seconds is generous.
const FETCH_TIMEOUT: Duration = Duration::from_secs(5);
/// Successful entries live this long before re-fetch.
const HIT_TTL: Duration = Duration::from_secs(7 * 24 * 3600);
/// Failures live this long — long enough to throttle, short enough that a
/// publisher fixing their page doesn't have to wait a week to see it work.
const NEGATIVE_TTL: Duration = Duration::from_secs(60 * 60);

/// Public record exposed via UniFFI. Empty strings fill missing fields so
/// Swift call sites don't have to handle `Option` everywhere.
#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct WebMetadata {
    pub url: String,
    pub title: String,
    pub description: String,
    pub image: String,
    pub site_name: String,
    pub author: String,
    pub favicon: String,
    pub fetched_at: u64,
}

impl WebMetadata {
    fn empty(url: String) -> Self {
        Self {
            url,
            title: String::new(),
            description: String::new(),
            image: String::new(),
            site_name: String::new(),
            author: String::new(),
            favicon: String::new(),
            fetched_at: 0,
        }
    }

    fn is_negative(&self) -> bool {
        self.fetched_at == 0
            && self.title.is_empty()
            && self.image.is_empty()
            && self.favicon.is_empty()
    }
}

/// Cache + in-flight coordinator. Holds the JSON-backed map and a per-URL
/// `Notify` so concurrent fetches for the same URL coalesce into one HTTP
/// request.
pub struct WebMetadataStore {
    path: PathBuf,
    /// Cached entry by canonical URL. `last_attempt` is the wall-clock
    /// timestamp of the most recent fetch (success or failure) — used for
    /// TTL checks separately from the metadata's own `fetched_at`.
    state: Mutex<CacheState>,
    inflight: Mutex<HashMap<String, Arc<Notify>>>,
}

#[derive(Default, Serialize, Deserialize)]
struct CacheState {
    entries: HashMap<String, CacheEntry>,
}

#[derive(Clone, Serialize, Deserialize)]
struct CacheEntry {
    metadata: WebMetadata,
    /// Wall-clock seconds of the last attempt (success or failure).
    last_attempt: u64,
}

impl WebMetadataStore {
    /// Open a store rooted at `data_dir`. Loads the JSON file if it
    /// already exists; missing or unreadable file is treated as empty.
    pub fn open(data_dir: &std::path::Path) -> Self {
        let path = data_dir.join("web_metadata.json");
        let state = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<CacheState>(&bytes).unwrap_or_default(),
            Err(_) => CacheState::default(),
        };
        Self {
            path,
            state: Mutex::new(state),
            inflight: Mutex::new(HashMap::new()),
        }
    }

    /// Read a fresh cached entry. Returns `None` if absent or stale per the
    /// applicable TTL (positive or negative).
    pub fn get(&self, url: &str) -> Option<WebMetadata> {
        let now = unix_now();
        let guard = self.state.lock();
        let entry = guard.entries.get(url)?;
        let ttl = if entry.metadata.is_negative() { NEGATIVE_TTL } else { HIT_TTL };
        if now.saturating_sub(entry.last_attempt) > ttl.as_secs() {
            None
        } else {
            Some(entry.metadata.clone())
        }
    }

    /// Insert or replace an entry, persisting to disk. Failures to persist
    /// are logged at warn but don't propagate — the in-memory copy is still
    /// good for the rest of this session.
    pub fn put(&self, metadata: WebMetadata) {
        let url = metadata.url.clone();
        let entry = CacheEntry {
            metadata,
            last_attempt: unix_now(),
        };
        let snapshot = {
            let mut guard = self.state.lock();
            guard.entries.insert(url, entry);
            serde_json::to_vec(&*guard).ok()
        };
        if let Some(bytes) = snapshot {
            if let Err(e) = std::fs::write(&self.path, &bytes) {
                tracing::warn!(error = %e, path = ?self.path, "persist web metadata cache");
            }
        }
    }

    /// Acquire (or join) the inflight slot for `url`. The caller that
    /// receives `InflightLead` must perform the fetch and call `done()`
    /// when finished; followers receive `InflightFollower` and `await`
    /// completion.
    fn acquire(self: &Arc<Self>, url: &str) -> InflightSlot {
        let mut guard = self.inflight.lock();
        if let Some(notify) = guard.get(url) {
            return InflightSlot::Follower(InflightFollower {
                notify: notify.clone(),
            });
        }
        let notify = Arc::new(Notify::new());
        guard.insert(url.to_string(), notify.clone());
        InflightSlot::Lead(InflightLead {
            store: Arc::clone(self),
            url: url.to_string(),
            notify,
        })
    }
}

enum InflightSlot {
    Lead(InflightLead),
    Follower(InflightFollower),
}

struct InflightLead {
    store: Arc<WebMetadataStore>,
    url: String,
    notify: Arc<Notify>,
}

impl InflightLead {
    fn done(self) {
        {
            let mut guard = self.store.inflight.lock();
            guard.remove(&self.url);
        }
        self.notify.notify_waiters();
    }
}

struct InflightFollower {
    notify: Arc<Notify>,
}

impl InflightFollower {
    async fn wait(self) {
        self.notify.notified().await;
    }
}

/// Top-level entry point used by `HighlighterCore`. Returns the cached
/// metadata if fresh; otherwise fetches, caches, returns. Coalesces
/// concurrent calls for the same URL.
pub async fn get_or_fetch(
    store: Arc<WebMetadataStore>,
    url: &str,
) -> Result<WebMetadata, CoreError> {
    let canonical = crate::artifacts::normalize_artifact_url(url)
        .ok_or_else(|| CoreError::InvalidInput("invalid URL".into()))?;

    if let Some(hit) = store.get(&canonical) {
        if hit.is_negative() {
            return Err(CoreError::NotFound);
        }
        return Ok(hit);
    }

    match store.acquire(&canonical) {
        InflightSlot::Lead(lead) => {
            let result = fetch_and_parse(&canonical).await;
            match &result {
                Ok(metadata) => store.put(metadata.clone()),
                Err(_) => store.put(WebMetadata::empty(canonical.clone())),
            }
            lead.done();
            result
        }
        InflightSlot::Follower(follower) => {
            follower.wait().await;
            store
                .get(&canonical)
                .filter(|m| !m.is_negative())
                .ok_or(CoreError::NotFound)
        }
    }
}

async fn fetch_and_parse(url: &str) -> Result<WebMetadata, CoreError> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(FETCH_TIMEOUT)
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| CoreError::Network(format!("build http client: {e}")))?;

    let resp = client
        .get(url)
        .header(reqwest::header::ACCEPT, "text/html,application/xhtml+xml")
        .send()
        .await
        .map_err(|e| CoreError::Network(format!("GET {url}: {e}")))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(CoreError::NotFound);
    }

    if let Some(ct) = resp.headers().get(reqwest::header::CONTENT_TYPE) {
        if let Ok(s) = ct.to_str() {
            let lower = s.to_ascii_lowercase();
            if !lower.contains("html") {
                return Err(CoreError::InvalidInput(format!(
                    "unsupported content-type: {s}"
                )));
            }
        }
    }

    // Capped-length body read so a malicious or misconfigured server can't
    // wedge us with a multi-GB stream. `bytes()` would buffer the whole
    // thing; chunk it instead and bail on overflow.
    let mut buffer: Vec<u8> = Vec::with_capacity(64 * 1024);
    let mut stream = resp;
    loop {
        let chunk = stream
            .chunk()
            .await
            .map_err(|e| CoreError::Network(format!("read body: {e}")))?;
        match chunk {
            Some(bytes) => {
                if buffer.len() + bytes.len() > MAX_BODY_BYTES {
                    let take = MAX_BODY_BYTES.saturating_sub(buffer.len());
                    buffer.extend_from_slice(&bytes[..take]);
                    break;
                }
                buffer.extend_from_slice(&bytes);
            }
            None => break,
        }
    }

    let html = String::from_utf8_lossy(&buffer);
    let final_url = Url::parse(url).map_err(|e| CoreError::InvalidInput(format!("parse url: {e}")))?;
    Ok(parse_metadata(&html, &final_url))
}

/// Parse a metadata block out of an HTML string. Extracted so unit tests
/// can drive the parser with fixture HTML and a synthetic base URL.
pub(crate) fn parse_metadata(html: &str, base: &Url) -> WebMetadata {
    let doc = Html::parse_document(html);

    // `<base href>` overrides the document URL for relative resolution.
    let base = effective_base(&doc, base);

    let mut og_title = String::new();
    let mut og_description = String::new();
    let mut og_image = String::new();
    let mut og_site_name = String::new();
    let mut twitter_title = String::new();
    let mut twitter_description = String::new();
    let mut twitter_image = String::new();
    let mut article_author = String::new();
    let mut author_meta = String::new();
    let mut description_meta = String::new();

    let meta_sel = Selector::parse("meta").expect("meta selector");
    for node in doc.select(&meta_sel) {
        let attrs = node.value();
        let property = attrs.attr("property").map(str::to_ascii_lowercase);
        let name = attrs.attr("name").map(str::to_ascii_lowercase);
        let content = attrs.attr("content").unwrap_or("").trim().to_string();
        if content.is_empty() {
            continue;
        }
        match property.as_deref() {
            Some("og:title") => og_title = content.clone(),
            Some("og:description") => og_description = content.clone(),
            Some("og:image") | Some("og:image:url") | Some("og:image:secure_url") => {
                if og_image.is_empty() {
                    og_image = content.clone();
                }
            }
            Some("og:site_name") => og_site_name = content.clone(),
            Some("article:author") => article_author = content.clone(),
            _ => {}
        }
        match name.as_deref() {
            Some("twitter:title") => twitter_title = content.clone(),
            Some("twitter:description") => twitter_description = content.clone(),
            Some("twitter:image") | Some("twitter:image:src") => {
                if twitter_image.is_empty() {
                    twitter_image = content.clone();
                }
            }
            Some("author") => author_meta = content.clone(),
            Some("description") => description_meta = content.clone(),
            _ => {}
        }
    }

    let title_tag = doc
        .select(&Selector::parse("title").expect("title selector"))
        .next()
        .map(|n| n.text().collect::<String>().trim().to_string())
        .unwrap_or_default();

    let title = pick_first(&[&og_title, &twitter_title, &title_tag]);
    let description = pick_first(&[&og_description, &twitter_description, &description_meta]);
    let site_name = pick_first(&[
        &og_site_name,
        &author_meta,
        &base
            .host_str()
            .map(|h| h.trim_start_matches("www.").to_string())
            .unwrap_or_default(),
    ]);
    let author = pick_first(&[&article_author, &author_meta]);

    let image_raw = pick_first(&[&og_image, &twitter_image]);
    let image = absolutize(&base, &image_raw).unwrap_or_default();
    let favicon = pick_favicon(&doc, &base);

    WebMetadata {
        url: base.to_string(),
        title,
        description,
        image,
        site_name,
        author,
        favicon,
        fetched_at: unix_now(),
    }
}

fn effective_base(doc: &Html, fallback: &Url) -> Url {
    let sel = Selector::parse("base[href]").expect("base selector");
    if let Some(node) = doc.select(&sel).next() {
        if let Some(href) = node.value().attr("href") {
            if let Ok(parsed) = fallback.join(href) {
                return parsed;
            }
        }
    }
    fallback.clone()
}

fn pick_first(values: &[&str]) -> String {
    for v in values {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    String::new()
}

fn absolutize(base: &Url, value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    base.join(trimmed).ok().map(|u| u.to_string())
}

/// Choose the best icon: largest apple-touch-icon by `sizes`, then any
/// `<link rel="icon">`, then `https://<host>/favicon.ico` as the universal
/// fallback every browser already implements.
fn pick_favicon(doc: &Html, base: &Url) -> String {
    let sel = Selector::parse("link[rel][href]").expect("link selector");

    let mut apple_candidates: Vec<(u32, String)> = Vec::new();
    let mut icon_candidates: Vec<String> = Vec::new();

    for node in doc.select(&sel) {
        let attrs = node.value();
        let rel = attrs.attr("rel").unwrap_or("").to_ascii_lowercase();
        let href = attrs.attr("href").unwrap_or("").trim();
        if href.is_empty() {
            continue;
        }
        let absolute = match absolutize(base, href) {
            Some(u) => u,
            None => continue,
        };
        let sizes_score = attrs
            .attr("sizes")
            .and_then(largest_size_dimension)
            .unwrap_or(0);

        if rel.split_ascii_whitespace().any(|tok| {
            tok == "apple-touch-icon" || tok == "apple-touch-icon-precomposed"
        }) {
            apple_candidates.push((sizes_score, absolute));
        } else if rel.split_ascii_whitespace().any(|tok| tok == "icon" || tok == "shortcut") {
            icon_candidates.push(absolute);
        }
    }

    if let Some((_, url)) = apple_candidates
        .into_iter()
        .max_by_key(|(score, _)| *score)
    {
        return url;
    }
    if let Some(url) = icon_candidates.into_iter().next() {
        return url;
    }
    base.join("/favicon.ico")
        .ok()
        .map(|u| u.to_string())
        .unwrap_or_default()
}

/// Parse the largest dimension out of a `sizes` attribute. `"any"` becomes
/// `u32::MAX` so SVG vector icons win over any raster size.
fn largest_size_dimension(value: &str) -> Option<u32> {
    let mut best: Option<u32> = None;
    for token in value.split_ascii_whitespace() {
        if token.eq_ignore_ascii_case("any") {
            return Some(u32::MAX);
        }
        let mut parts = token.split(|c: char| c == 'x' || c == 'X');
        let w = parts.next()?.parse::<u32>().ok()?;
        let h = parts.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(w);
        let dim = w.max(h);
        best = Some(best.map(|b| b.max(dim)).unwrap_or(dim));
    }
    best
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_url() -> Url {
        Url::parse("https://example.com/article/post").unwrap()
    }

    #[test]
    fn parses_open_graph_meta() {
        let html = r#"
            <html><head>
              <meta property="og:title" content="Hello World"/>
              <meta property="og:description" content="A nice page"/>
              <meta property="og:image" content="https://cdn.example.com/cover.jpg"/>
              <meta property="og:site_name" content="Example"/>
              <meta property="article:author" content="Alice"/>
            </head></html>
        "#;
        let m = parse_metadata(html, &base_url());
        assert_eq!(m.title, "Hello World");
        assert_eq!(m.description, "A nice page");
        assert_eq!(m.image, "https://cdn.example.com/cover.jpg");
        assert_eq!(m.site_name, "Example");
        assert_eq!(m.author, "Alice");
    }

    #[test]
    fn falls_back_to_title_tag_when_og_missing() {
        let html = r#"
            <html><head>
              <title>Plain Title</title>
              <meta name="description" content="meta desc"/>
              <meta name="author" content="Bob"/>
            </head></html>
        "#;
        let m = parse_metadata(html, &base_url());
        assert_eq!(m.title, "Plain Title");
        assert_eq!(m.description, "meta desc");
        assert_eq!(m.author, "Bob");
        assert_eq!(m.site_name, "Bob");
    }

    #[test]
    fn falls_back_to_twitter_when_og_missing() {
        let html = r#"
            <html><head>
              <meta name="twitter:title" content="Tweet Title"/>
              <meta name="twitter:image" content="/relative/cover.png"/>
            </head></html>
        "#;
        let m = parse_metadata(html, &base_url());
        assert_eq!(m.title, "Tweet Title");
        assert_eq!(m.image, "https://example.com/relative/cover.png");
    }

    #[test]
    fn site_name_falls_back_to_host() {
        let html = "<html><head></head></html>";
        let m = parse_metadata(html, &base_url());
        assert_eq!(m.site_name, "example.com");
    }

    #[test]
    fn resolves_relative_image_against_base() {
        let html = r#"
            <html><head>
              <meta property="og:image" content="/img/cover.jpg"/>
            </head></html>
        "#;
        let m = parse_metadata(html, &base_url());
        assert_eq!(m.image, "https://example.com/img/cover.jpg");
    }

    #[test]
    fn protocol_relative_image_resolves() {
        let html = r#"
            <html><head>
              <meta property="og:image" content="//cdn.example.com/x.png"/>
            </head></html>
        "#;
        let m = parse_metadata(html, &base_url());
        assert_eq!(m.image, "https://cdn.example.com/x.png");
    }

    #[test]
    fn base_href_overrides_document_url() {
        let html = r#"
            <html><head>
              <base href="https://other.example/"/>
              <meta property="og:image" content="cover.jpg"/>
            </head></html>
        "#;
        let m = parse_metadata(html, &base_url());
        assert_eq!(m.image, "https://other.example/cover.jpg");
    }

    #[test]
    fn picks_largest_apple_touch_icon() {
        let html = r#"
            <html><head>
              <link rel="icon" href="/favicon.ico"/>
              <link rel="apple-touch-icon" sizes="180x180" href="/touch-180.png"/>
              <link rel="apple-touch-icon" sizes="120x120" href="/touch-120.png"/>
            </head></html>
        "#;
        let m = parse_metadata(html, &base_url());
        assert_eq!(m.favicon, "https://example.com/touch-180.png");
    }

    #[test]
    fn falls_back_to_link_icon_then_default() {
        let html = r#"
            <html><head>
              <link rel="icon" href="/static/icon.png"/>
            </head></html>
        "#;
        let m = parse_metadata(html, &base_url());
        assert_eq!(m.favicon, "https://example.com/static/icon.png");

        let html2 = "<html><head></head></html>";
        let m2 = parse_metadata(html2, &base_url());
        assert_eq!(m2.favicon, "https://example.com/favicon.ico");
    }

    #[test]
    fn cache_round_trip() {
        let tmp = tempfile::tempdir().expect("tmp");
        let store = WebMetadataStore::open(tmp.path());
        let mut metadata = WebMetadata::empty("https://example.com/post".into());
        metadata.title = "Post".into();
        metadata.fetched_at = unix_now();
        store.put(metadata.clone());

        let read = store.get("https://example.com/post").expect("hit");
        assert_eq!(read.title, "Post");

        // Reopen — should survive the round-trip through disk.
        drop(store);
        let store2 = WebMetadataStore::open(tmp.path());
        let read2 = store2.get("https://example.com/post").expect("hit2");
        assert_eq!(read2.title, "Post");
    }

    #[test]
    fn negative_entry_distinguished_from_hit() {
        let tmp = tempfile::tempdir().expect("tmp");
        let store = WebMetadataStore::open(tmp.path());
        store.put(WebMetadata::empty("https://example.com/dead".into()));
        // Entry exists in cache but `get` returns it; `is_negative` lets the
        // caller treat it as NotFound without re-fetching.
        let read = store.get("https://example.com/dead").expect("hit");
        assert!(read.is_negative());
    }

    #[test]
    fn largest_size_dimension_handles_any_keyword() {
        assert_eq!(largest_size_dimension("any"), Some(u32::MAX));
        assert_eq!(largest_size_dimension("16x16 32x32 192x192"), Some(192));
        assert_eq!(largest_size_dimension("180x180"), Some(180));
        assert_eq!(largest_size_dimension("garbage"), None);
    }
}
